//! Checked consumption of the tools-only recorder. Serialization happens after
//! the completed act call; no policy, belief or value evaluator is rerun here.

use agent::SeatView;
use engine::GameAction;
use serde_json::{json, Value};
use std::time::Instant;

use super::AssessmentRecorder;

/// Evidence belongs to exactly the act call validated by `consume`. Policy
/// timing excludes capture cloning; serialization is measured independently.
#[derive(Debug)]
pub struct CapturedEvidence {
    pub policy_nanos: u128,
    pub assessment: Option<Value>,
    pub serialization_nanos: u128,
    /// A retained assessment can differ from a fallback/prepared act output.
    /// False is disclosed, not silently represented as the choice's ranking.
    pub assessment_matches_chosen_action: Option<bool>,
}

impl CapturedEvidence {
    pub fn attach(self, row: &mut Value, measured_act_nanos: u128) {
        row["ms"] = json!(self.policy_nanos as f64 / 1_000_000.0);
        row["capture_ms"] = json!(measured_act_nanos.saturating_sub(self.policy_nanos) as f64 / 1_000_000.0);
        row["assessment_serialization_ms"] = json!(self.serialization_nanos as f64 / 1_000_000.0);
        row["assessment_recorded"] = json!(true);
        row["value_assessment_present"] = json!(self
            .assessment
            .as_ref()
            .is_some_and(|a| a["value_assessment_present"] == true));
        row["sequence_present"] = json!(self
            .assessment
            .as_ref()
            .is_some_and(|a| a["extensions"]["sequence"].is_object()));
        row["assessment_matches_chosen_action"] = json!(self.assessment_matches_chosen_action);
        row["assessment"] = self.assessment.unwrap_or(Value::Null);
    }
}

impl AssessmentRecorder {
    /// Check at each independent comparison/game boundary and at its end. Plain
    /// historical agents do not enqueue anything; absence is not an error.
    pub fn ensure_empty(&self) -> Result<(), String> {
        if self.0.borrow().is_empty() {
            Ok(())
        } else {
            Err("unconsumed assessment at recording boundary".into())
        }
    }

    /// Peek and validate before removal. On mismatch the offending record stays
    /// queued and the caller must fail the run rather than attach it elsewhere.
    /// The only context accepted is the caller's honest acting SeatView.
    pub fn consume(
        &self,
        view: &SeatView<'_>,
        action: &GameAction,
        raising: bool,
    ) -> Result<Option<CapturedEvidence>, String> {
        let selected_matches;
        {
            let queued = self.0.borrow();
            let Some(captured) = queued.front() else {
                return Ok(None);
            };
            if queued.len() != 1 {
                return Err("multiple queued assessments for one decision".into());
            }
            if captured.seat != view.seat
                || captured.event_count != view.events.len()
                || captured.action != *action
                || action.player.0 != view.seat
            {
                return Err("assessment identity mismatch: seat, event count or action".into());
            }
            if let Some(a) = &captured.assessment {
                if a.context.observer != view.seat
                    || a.context.knowledge != *view.knowledge
                    || a.context.events.as_slice() != view.events
                    || a.context.kinds != view.kinds
                    || a.context.series != view.series
                {
                    return Err("assessment context mismatch: honest view, history or metadata".into());
                }
                let selected = a
                    .actions
                    .get(a.values.selected)
                    .ok_or("assessment selected index is invalid")?;
                selected_matches = Some(selected == action);
            } else {
                selected_matches = None;
            }
            if let Some(sequence) = &captured.sequence {
                if let Some(error) = super::sequence_authorization_error(sequence) {
                    return Err(format!("sequence authorization mismatch: {error}"));
                }
            }
            if captured.sequence.as_ref().is_some_and(|s| !s.information.matches(view)) {
                return Err("sequence context mismatch: honest view, history or metadata".into());
            }
            if let (Some(a), Some(s)) = (&captured.assessment, &captured.sequence) {
                if a.context.belief_scope != s.scope {
                    return Err("assessment/sequence information scope mismatch".into());
                }
            }
        }
        let captured = self
            .0
            .borrow_mut()
            .pop_front()
            .ok_or("assessment vanished while consuming")?;
        let started = Instant::now();
        let assessment = super::captured_json(&captured, raising);
        let serialization_nanos = if assessment.is_some() {
            started.elapsed().as_nanos()
        } else {
            0
        };
        Ok(Some(CapturedEvidence {
            policy_nanos: captured.policy_nanos,
            assessment,
            serialization_nanos,
            assessment_matches_chosen_action: selected_matches,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent::{SeatKind, Table};
    use eval::suite::DealSuite;
    use heuristic::{sequence_concepts::Information, BeliefScope, PlanMode, SequenceTrace};

    fn table() -> Table {
        let mut table = Table::new(&DealSuite::named("seed7", 1).unwrap().hands(0));
        table.kinds = [SeatKind::Heuristic; 4];
        table
    }

    fn enqueue(recorder: &AssessmentRecorder, view: &SeatView<'_>, action: GameAction) {
        recorder.0.borrow_mut().push_back(super::super::CapturedDecision {
            seat: view.seat,
            event_count: view.events.len(),
            action,
            policy_nanos: 17,
            assessment: None,
            sequence: Some(SequenceTrace {
                authorization: None,
                mode: PlanMode::Deferred,
                information: Information::of(view),
                scope: BeliefScope::CurrentInformation,
                program: None,
                choices: vec![],
                terminals: Default::default(),
                work: Default::default(),
                failure: None,
                deferred_limit: None,
                total_candidates: None,
            }),
        });
    }

    #[test]
    fn consumes_sequence_only_once_with_separate_timing_and_identity() {
        let mut table = table();
        let first = table.to_act().unwrap().1[0];
        table.apply(first);
        let (seat, legal) = table.to_act().unwrap();
        let view = table.view(seat);
        let recorder = AssessmentRecorder::default();
        assert!(recorder.consume(&view, &legal[0], false).unwrap().is_none());
        enqueue(&recorder, &view, legal[0]);
        let evidence = recorder.consume(&view, &legal[0], false).unwrap().unwrap();
        assert_eq!(evidence.policy_nanos, 17);
        let mut row = json!({});
        evidence.attach(&mut row, 50);
        assert_eq!(row["assessment_recorded"], true);
        assert_eq!(row["value_assessment_present"], false);
        assert_eq!(row["sequence_present"], true);
        assert_eq!(row["capture_ms"], 33.0 / 1_000_000.0);
        assert_eq!(row["assessment"]["extensions"]["sequence"]["identity"]["seat"], seat);
        assert!(recorder.consume(&view, &legal[0], false).unwrap().is_none());
        recorder.ensure_empty().unwrap();
    }

    #[test]
    fn rejects_identity_history_and_same_length_metadata_changes_without_consuming() {
        let mut table = table();
        let first = table.to_act().unwrap().1[0];
        table.apply(first);
        let (seat, legal) = table.to_act().unwrap();
        let view = table.view(seat);
        let recorder = AssessmentRecorder::default();
        enqueue(&recorder, &view, legal[0]);
        assert!(recorder.consume(&view, &legal[1], false).is_err());
        let mut events = view.events.to_vec();
        events[0] = observation::SeatEvent::Undone;
        let fork = SeatView {
            events: &events,
            ..view
        };
        assert!(recorder
            .consume(&fork, &legal[0], false)
            .unwrap_err()
            .contains("context"));
        let mut changed = view;
        changed.series.team_totals[0] += 1;
        assert!(recorder.consume(&changed, &legal[0], false).is_err());
        changed = view;
        changed.kinds[1] = SeatKind::Human;
        assert!(recorder.consume(&changed, &legal[0], false).is_err());
        changed = SeatView {
            knowledge: table.view((seat + 1) % 4).knowledge,
            ..view
        };
        assert!(recorder.consume(&changed, &legal[0], false).is_err());
        assert!(recorder.ensure_empty().is_err());
        assert!(recorder.consume(&view, &legal[0], false).unwrap().is_some());
    }

    #[test]
    fn capture_serializes_only_the_acting_observers_redacted_passes() {
        use engine::ActionType;
        let mut table = table();
        let mut outsider = None;
        for _ in 0..128 {
            let (seat, legal) = table.to_act().unwrap();
            if table
                .view(seat)
                .events
                .iter()
                .filter(|e| matches!(e, observation::SeatEvent::Pass { cards: None, .. }))
                .count()
                == 2
            {
                outsider = Some((seat, legal[0]));
                break;
            }
            let action = legal
                .iter()
                .find(|a| a.action_type == ActionType::NewBid(140))
                .copied()
                .or_else(|| legal.iter().find(|a| a.action_type == ActionType::StopBidding).copied())
                .unwrap_or(legal[0]);
            table.apply(action);
        }
        let (seat, action) = outsider.expect("outsider acts after the private exchange");
        let view = table.view(seat);
        let recorder = AssessmentRecorder::default();
        enqueue(&recorder, &view, action);
        let evidence = recorder.consume(&view, &action, false).unwrap().unwrap();
        let assessment = evidence.assessment.unwrap();
        let events = &assessment["extensions"]["sequence"]["information_sets"][0]["value"]["events"];
        let passes: Vec<_> = events
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["kind"] == "pass")
            .collect();
        assert_eq!(passes.len(), 2);
        assert!(passes.iter().all(|e| e["cards"].is_null()));
    }

    #[test]
    fn value_context_is_checked_and_a_retained_ranking_is_not_claimed_as_the_action() {
        use heuristic::value_concepts::{CohortMethod, Provenance, ValueStore};
        let table = table();
        let (seat, legal) = table.to_act().unwrap();
        let view = table.view(seat);
        // Numerical fixture for the real evaluator, not an executed policy forecast.
        let cohort = [(
            engine::GameOutcome {
                playing_team: None,
                game_value: 120,
                team_points: [70, 70],
                team_tricks: [4, 5],
            },
            1.0,
        )];
        let candidates = [cohort.as_slice(), cohort.as_slice()];
        let values = ValueStore::new(
            &candidates,
            usize::from(seat % 2),
            Provenance {
                belief_model: "synthetic-recording-test",
                response_model: "supplied-terminal-fixture",
                cohort: CohortMethod::Enumerated,
            },
        )
        .unwrap()
        .assess()
        .unwrap();
        assert_eq!(values.selected, 0);
        let assessment = heuristic::PlanningAssessment {
            actions: vec![legal[0], legal[1]],
            values,
            context: heuristic::PlanningContext {
                observer: seat,
                belief_scope: BeliefScope::CurrentInformation,
                knowledge: *view.knowledge,
                events: view.events.to_vec(),
                kinds: view.kinds,
                series: view.series,
                response_params: Default::default(),
                seed: 0,
                possible_allocations: 1,
                requested_worlds: 1,
                proposal_draws: 1,
            },
        };
        let recorder = AssessmentRecorder::default();
        recorder.0.borrow_mut().push_back(super::super::CapturedDecision {
            seat,
            event_count: view.events.len(),
            action: legal[1],
            policy_nanos: 7,
            assessment: Some(assessment),
            sequence: None,
        });
        let mut changed = view;
        changed.series.team_totals[0] += 1;
        assert!(recorder
            .consume(&changed, &legal[1], false)
            .unwrap_err()
            .contains("context"));
        let evidence = recorder.consume(&view, &legal[1], false).unwrap().unwrap();
        assert_eq!(evidence.assessment_matches_chosen_action, Some(false));
        assert_eq!(evidence.assessment.unwrap()["value_assessment_present"], true);
        recorder.ensure_empty().unwrap();
    }
}

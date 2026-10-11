//! Private semantic intent, separate from disposable evaluation caches.

use agent::SeatView;
use engine::GameAction;

use crate::concepts::sequence::{Offer, Receipt, SequenceError};
use crate::Params;

use super::sequence::{Context, PreparedChoice, Terminal, Work};
use super::PlanningContext;

#[derive(Clone, Debug, PartialEq)]
pub struct RetainedPlan {
    pub receipt: Receipt,
    pub prepared: std::sync::Arc<Vec<PreparedChoice>>,
    pub parameters: Params,
    pub origin: PlanningContext,
    pub context: Context,
    pub review_pending: bool,
    pub raw_events: Vec<observation::SeatEvent>,
}

impl RetainedPlan {
    /// New evidence is accepted before any current-value or legality lookup.
    /// A fork/undo requests recovery; its old assessments never certify a move.
    pub fn synchronize(
        &mut self,
        raw: &SeatView<'_>,
        canonical: &SeatView<'_>,
        parameters: &Params,
    ) -> Result<(), SequenceError> {
        if &self.parameters != parameters {
            return Err(SequenceError::WrongContext);
        }
        if !raw.events.starts_with(&self.raw_events)
            || raw.events[self.raw_events.len()..]
                .iter()
                .any(|event| *event == observation::SeatEvent::Undone)
        {
            return Err(SequenceError::HistoryChanged);
        }
        self.receipt.synchronize(canonical)?;
        self.raw_events = raw.events.to_vec();
        Ok(())
    }

    /// Retrying a proposal on identical accepted evidence spends no resource
    /// and does not rerun the planning search. Changed context cannot hit here.
    pub fn pending(&self, view: &SeatView<'_>, legal: &[GameAction]) -> Option<Offer> {
        let offer = self.receipt.pending.as_ref()?;
        (self.receipt.accepted.matches(view) && legal.contains(&offer.action)).then(|| offer.clone())
    }
}

/// Runtime evidence for a formation, accepted continuation, or deferred review.
/// It contains only observer projections of conditional branches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanMode {
    Formation,
    Repair,
    Prepared,
    Retry,
    Deferred,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SequenceTrace {
    pub mode: PlanMode,
    pub information: crate::concepts::sequence::Information,
    pub scope: super::BeliefScope,
    pub program: Option<crate::concepts::sequence::Suffix>,
    pub choices: Vec<PreparedChoice>,
    pub terminals: std::sync::Arc<Vec<Terminal>>,
    pub work: Work,
    pub failure: Option<SequenceError>,
}

pub(super) fn view<'a>(ctx: &'a crate::Ctx<'_>) -> SeatView<'a> {
    SeatView {
        seat: ctx.me,
        knowledge: ctx.k,
        events: ctx.mind.events(),
        kinds: ctx.mind.kinds(),
        series: ctx.series,
    }
}

/// Retry and exact predicted-observation reuse do no continuation search. A
/// changed catalogue or unrepresented observation falls through to repair.
pub(crate) fn prepared(ctx: &crate::Ctx<'_>, legal: &[GameAction]) -> Option<crate::Decision> {
    if ctx.params.plan_worlds == 0 {
        return None;
    }
    let view = view(ctx);
    let mut slot = ctx.intent.borrow_mut();
    let plan = slot.as_mut()?;
    if plan.review_pending {
        return None;
    }
    let (mode, offer) = if let Some(offer) = plan.pending(&view, legal) {
        (PlanMode::Retry, offer)
    } else {
        let membership = super::sequence::Membership::authored(&view, legal).ok()?;
        let retained_suffix = plan.receipt.program.suffix(plan.receipt.stage).ok()?;
        let mut required = membership.programs.clone();
        if !required.contains(&retained_suffix) {
            required.push(retained_suffix.clone());
        }
        // A generated catalogue records availability, not comparison coverage.
        // Only alternatives actually ranked on this same conditional support
        // can certify warm selection against today's authored routes.
        let certificate_index = plan.prepared.iter().position(|choice| {
            choice.information.matches(&view)
                && choice.semantic_suffix == retained_suffix
                && choice.context == plan.context
                && covers(choice, &required)
        })?;
        let certificate = &plan.prepared[certificate_index];
        // Use the very record whose coverage was checked, with its support.
        // An earlier narrow record must not be authorized by a later one.
        let mut certified_path = vec![certificate.clone()];
        certified_path.extend(
            plan.prepared
                .iter()
                .enumerate()
                .filter(|(index, choice)| {
                    *index != certificate_index
                        && choice.information.matches(&view)
                        && choice.semantic_suffix != retained_suffix
                        && choice.context == certificate.context
                        && (choice.assessment.is_none() || choice.support == certificate.support)
                })
                .map(|(_, choice)| choice.clone()),
        );
        let offer = super::sequence::prepared_lookup(
            &plan.receipt,
            &view,
            legal,
            ctx.mind,
            &plan.context,
            &membership,
            &certified_path,
        )
        .ok()?;
        (PlanMode::Prepared, offer)
    };
    plan.receipt.propose(offer.clone(), &view, legal).ok()?;
    let scope = super::BeliefScope::PreparedConditional {
        origin: crate::concepts::sequence::Information {
            seat: plan.origin.observer,
            knowledge: plan.origin.knowledge,
            events: plan.origin.events.clone(),
            kinds: plan.origin.kinds,
            series: plan.origin.series,
        },
    };
    ctx.concepts.borrow_mut().sequence = Some(SequenceTrace {
        mode,
        information: crate::concepts::sequence::Information::of(&view),
        scope,
        program: plan.receipt.program.suffix(plan.receipt.stage).ok(),
        choices: plan
            .prepared
            .iter()
            .filter(|choice| choice.information.matches(&view))
            .cloned()
            .collect(),
        terminals: std::sync::Arc::new(Vec::new()),
        work: Work::default(),
        failure: None,
    });
    Some(crate::decision(
        &offer.action,
        crate::RuleId::HP1,
        vec![
            crate::fact("sequence_prepared", u8::from(mode == PlanMode::Prepared)),
            crate::fact("sequence_retry", u8::from(mode == PlanMode::Retry)),
            crate::fact("sequence_conditioned_original_model", 1),
        ],
    ))
}

/// Forced game actions still consume accepted plan steps. No probability or
/// ranking is invented for the sole legal move.
pub(crate) fn forced(ctx: &crate::Ctx<'_>, legal: &[GameAction]) {
    use crate::concepts::sequence::{inspect, Frontier, Information};
    let view = view(ctx);
    let mut slot = ctx.intent.borrow_mut();
    let Some(plan) = slot.as_mut() else {
        return;
    };
    let offered = if let Some(offer) = plan.pending(&view, legal) {
        Some(offer)
    } else {
        inspect(&plan.receipt.program, plan.receipt.stage, &view, legal, ctx.mind)
            .ok()
            .and_then(|inspection| match inspection.frontier {
                Frontier::Actions(offers) if offers.len() == 1 => offers.into_iter().next(),
                _ => None,
            })
    };
    if let Some(offer) = offered {
        if plan.receipt.propose(offer, &view, legal).is_ok() {
            return;
        }
    }
    // A forced action outside the retained stage changes the execution path.
    // The next decision must recover; keeping the old value would hide that gap.
    let program = plan.receipt.program.suffix(plan.receipt.stage).ok();
    plan.review_pending = true;
    plan.prepared = std::sync::Arc::new(Vec::new());
    plan.receipt.pending = None;
    ctx.concepts.borrow_mut().sequence = Some(SequenceTrace {
        mode: PlanMode::Deferred,
        information: Information::of(&view),
        scope: super::BeliefScope::CurrentInformation,
        program,
        choices: Vec::new(),
        terminals: std::sync::Arc::new(Vec::new()),
        work: Work::default(),
        failure: Some(SequenceError::WrongContext),
    });
}

/// Establish comparison-domain inclusion from executed alternatives, never
/// from the availability catalogue that happened to be generated beside them.
fn covers(choice: &PreparedChoice, required: &[crate::concepts::sequence::Suffix]) -> bool {
    use super::sequence::Selection;
    use crate::concepts::sequence::{Actions, Node, Program, Stage};
    let Some(assessment) = &choice.assessment else {
        return false;
    };
    if !assessment.cohort_complete || assessment.candidates.len() != choice.alternatives.len() {
        return false;
    }
    let actual: Vec<_> = choice
        .alternatives
        .iter()
        .filter_map(|selection| match selection {
            Selection::Stage(suffix) => Some(suffix.clone()),
            Selection::Offer { action, next } => {
                let mut nodes = next.nodes.clone();
                let start = Stage(nodes.len());
                nodes.push(Node::Action {
                    actions: Actions::Exact(action.action_type),
                    next: next.start,
                });
                Program {
                    name: "evaluated-action-suffix",
                    version: 1,
                    start,
                    nodes,
                }
                .suffix(start)
                .ok()
            }
        })
        .collect();
    required
        .iter()
        .all(|suffix| *suffix == choice.semantic_suffix || actual.contains(suffix))
}

#[cfg(test)]
mod tests {
    use super::super::sequence::{Membership, Selection};
    use super::*;
    use crate::concepts::sequence::{Actions, Information, Node, Program, Stage};
    use crate::concepts::value::{CohortMethod, Provenance, ValueStore};
    #[test]
    fn an_available_better_route_must_be_compared_not_merely_listed() {
        let hand = engine::CardSet(0x1ff);
        let mind = beliefs::Mind::new(
            0,
            hand,
            [agent::SeatKind::Unknown; 4],
            beliefs::BeliefParams::base_only(),
        );
        let view = SeatView {
            seat: 0,
            knowledge: mind.knowledge(),
            events: &[],
            kinds: mind.kinds(),
            series: agent::SeriesScore::default(),
        };
        let tail = Program {
            name: "tail",
            version: 1,
            start: Stage(0),
            nodes: vec![Node::Tail],
        }
        .suffix(Stage(0))
        .unwrap();
        let action = engine::GameAction {
            player: engine::PlaceAtTable(0),
            action_type: engine::ActionType::CardPlayed(engine::Card::from_index(8)),
        };
        let other = Program {
            name: "other",
            version: 1,
            start: Stage(1),
            nodes: vec![
                Node::Tail,
                Node::Action {
                    actions: Actions::Exact(action.action_type),
                    next: Stage(0),
                },
            ],
        }
        .suffix(Stage(1))
        .unwrap();
        let provenance = Provenance {
            belief_model: "coverage-fixture",
            response_model: "coverage-fixture",
            cohort: CohortMethod::Enumerated,
        };
        let old = [(
            engine::GameOutcome {
                playing_team: None,
                game_value: 120,
                team_points: [80, 50],
                team_tricks: [4, 5],
            },
            1.0,
        )];
        let better = [(
            engine::GameOutcome {
                playing_team: None,
                game_value: 120,
                team_points: [100, 30],
                team_tricks: [5, 4],
            },
            1.0,
        )];
        let mut choice = PreparedChoice {
            information: Information::of(&view),
            semantic_suffix: tail.clone(),
            context: Context {
                params: crate::Params::default(),
                seed: 0,
                posterior_revision: 1,
                provenance,
            },
            membership: Membership {
                legal: vec![action],
                programs: vec![tail.clone(), other.clone()],
            },
            alternatives: vec![Selection::Stage(tail.clone())],
            selected: 0,
            assessment: Some(
                ValueStore::new(&[old.as_slice()], 0, provenance)
                    .unwrap()
                    .assess()
                    .unwrap(),
            ),
            consumed: Vec::new(),
            support: vec![(0, 1.0)],
        };
        let required = [tail.clone(), other];
        assert!(
            !covers(&choice, &required),
            "the catalogue alone does not compare the alternative"
        );
        choice.alternatives.push(Selection::Offer { action, next: tail });
        choice.assessment = Some(
            ValueStore::new(&[old.as_slice(), better.as_slice()], 0, provenance)
                .unwrap()
                .assess()
                .unwrap(),
        );
        choice.selected = choice.assessment.as_ref().unwrap().selected;
        assert!(covers(&choice, &required));
        assert_eq!(
            choice.selected, 1,
            "the newly compared route improves engine score difference"
        );
    }
}

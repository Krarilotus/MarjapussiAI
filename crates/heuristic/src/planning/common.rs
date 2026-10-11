//! Common comparison, completed-response dispatch and receipt publication.
//! Phase modules own nomination and scheduling; neither duplicates this path.

use std::{sync::Arc, time::Instant};

use super::retained::{view, PlanMode, RetainedPlan, SequenceTrace};
use super::sequence::{self, Candidate, Context, Deferred, DeferredLimit, Limits, Work};
use super::{BeliefScope, Forecast, PlanningAssessment};
use crate::concepts::sequence::{Information, SequenceError};
use crate::{fact, Ctx, Decision, Fact, RuleId};

/// Scheduling and semantic intent differ: a first midgame formation can use
/// the repair allowance without claiming it repaired an existing receipt.
#[derive(Clone, Copy)]
pub(super) enum Allowance {
    Formation,
    Repair,
}

#[derive(Clone, Copy)]
pub(super) enum Mode {
    Preplay { replacement_rule: RuleId },
    Formation,
    Repair,
}

pub(super) struct Evaluated {
    pub comparison: sequence::Comparison,
    pub context: Context,
}

pub(super) struct Failure {
    reason: SequenceError,
    work: Work,
    limit: Option<DeferredLimit>,
    candidates: Option<usize>,
}

impl Failure {
    pub fn before_evaluation(reason: SequenceError, candidates: Option<usize>) -> Self {
        Self {
            reason,
            work: Work::default(),
            limit: None,
            candidates,
        }
    }
}
impl From<Deferred> for Failure {
    fn from(value: Deferred) -> Self {
        Self {
            reason: value.reason,
            work: value.work,
            limit: value.limit,
            candidates: Some(value.total_candidates),
        }
    }
}

/// A fallback is never a confirmed plan step. Keep semantic intent for later
/// repair, while invalidating its predictions and unpublished pending action.
pub(super) fn defer(ctx: &Ctx<'_>, fallback: &mut Decision, mode: Mode, failure: Failure) {
    ctx.concepts.borrow_mut().sequence = Some(SequenceTrace {
        mode: PlanMode::Deferred,
        information: Information::of(&view(ctx)),
        scope: BeliefScope::CurrentInformation,
        program: None,
        choices: Vec::new(),
        terminals: Arc::new(Vec::new()),
        work: failure.work,
        failure: Some(failure.reason),
        deferred_limit: failure.limit,
        total_candidates: failure.candidates,
    });
    if let Some(plan) = ctx.intent.borrow_mut().as_mut() {
        plan.review_pending = true;
        plan.prepared = Arc::new(Vec::new());
        plan.receipt.pending = None;
        plan.pending_legal.clear();
    }
    if matches!(mode, Mode::Preplay { .. }) {
        fallback.why.extend([
            fact("forecast_complete", 0),
            fact("forecast_candidates", failure.candidates.unwrap_or(0) as u32),
        ]);
    }
    fallback.why.extend([
        fact("sequence_deferred", 1),
        fact("sequence_transitions", failure.work.transitions as u32),
        fact(
            "sequence_completed_candidates",
            failure.work.completed_candidates as u32,
        ),
    ]);
}

impl Forecast {
    /// Evaluate the supplied programs on this unchanged cohort. This is the
    /// sole production adapter to the sequence evaluator and strict response
    /// primitive, including its canonical solver effort on success or failure.
    pub(super) fn evaluate_programs(
        &self,
        ctx: &Ctx<'_>,
        candidates: &[Candidate],
        allowance: Allowance,
    ) -> Result<Evaluated, Deferred> {
        let _timing = beliefs::profile::scope("sequence.evaluate");
        let context = Context {
            params: self.context.response_params.clone(),
            seed: self.context.seed,
            posterior_revision: ctx.mind.events().len() as u64,
            provenance: self.provenance,
        };
        let limits = Limits {
            transitions: match allowance {
                Allowance::Formation => ctx.params.plan_formation_transitions as usize,
                Allowance::Repair => ctx.params.plan_repair_transitions as usize,
            },
            memo_bytes: ctx.params.plan_memo_mib as usize * 1024 * 1024,
            deadline: Some(self.deadline),
            // Comparisons are completed value reductions, not a bound on the
            // preceding expansion work. The finite authored graph is bounded
            // by represented transitions and the caller's wall allowance.
            comparisons: usize::MAX,
            ..Limits::default()
        };
        let mut players = self.players.borrow_mut();
        let mut response = |actor: &agent::SeatView<'_>, legal: &[engine::GameAction], budget: sequence::LeafBudget| {
            if budget.remaining_transitions == 0 || budget.deadline.is_some_and(|end| Instant::now() >= end) {
                return Err(sequence::LeafFailure::unit(SequenceError::Budget));
            }
            // Hypothetical actors use their filtered view and private reader.
            let completed = players[actor.seat as usize]
                .completed_response_with_effort_before(actor, legal, budget.deadline, budget.remaining_solver_nodes)
                .map_err(|failure| sequence::LeafFailure {
                    reason: SequenceError::Budget,
                    canonical_solver_nodes: failure.canonical_solver_nodes,
                })?;
            if budget.deadline.is_some_and(|end| Instant::now() >= end) {
                return Err(sequence::LeafFailure {
                    reason: SequenceError::Budget,
                    canonical_solver_nodes: completed.canonical_solver_nodes,
                });
            }
            Ok(sequence::LeafResponse {
                action: completed.decision.action,
                canonical_solver_nodes: completed.canonical_solver_nodes,
            })
        };
        let comparison = sequence::evaluate_with_cache(
            candidates,
            &self.tables,
            ctx.me,
            self.provenance,
            &context,
            limits,
            &mut ctx.completed.borrow_mut(),
            &mut response,
        )?;
        Ok(Evaluated { comparison, context })
    }

    /// Install exactly the completed assessment's chosen receipt and evidence.
    /// Phase-specific labels do not alter the action, ranking or program.
    pub(super) fn install(&self, ctx: &Ctx<'_>, evaluated: Evaluated, mode: Mode, mut baseline: Decision) -> Decision {
        let Evaluated {
            comparison: compared,
            context,
        } = evaluated;
        let selected = &compared.executions[compared.values.selected];
        let action = selected.first.action;
        match mode {
            Mode::Preplay { replacement_rule } => {
                if baseline.action != action {
                    baseline.action = action;
                    baseline.rule = replacement_rule;
                    baseline.why.clear();
                }
            }
            Mode::Formation | Mode::Repair => {
                baseline.action = action;
                baseline.rule = RuleId::HP1;
                baseline.why.clear();
            }
        }
        baseline.why.extend(self.facts(&compared, mode));
        let initial = view(ctx);
        let trace = SequenceTrace {
            mode: if matches!(mode, Mode::Repair) {
                PlanMode::Repair
            } else {
                PlanMode::Formation
            },
            information: Information::of(&initial),
            scope: BeliefScope::CurrentInformation,
            program: compared.receipt.program.suffix(compared.receipt.stage).ok(),
            choices: compared
                .prepared
                .iter()
                .filter(|choice| choice.information.matches(&initial))
                .cloned()
                .collect(),
            terminals: Arc::new(selected.terminals.clone()),
            work: compared.work,
            failure: None,
            deferred_limit: None,
            total_candidates: Some(compared.executions.len()),
        };
        let actions = compared
            .executions
            .iter()
            .map(|execution| execution.first.action)
            .collect();
        *ctx.intent.borrow_mut() = Some(RetainedPlan {
            receipt: compared.receipt,
            pending_legal: crate::legal_actions::eligible(
                self.tables[0]
                    .0
                    .to_act()
                    .expect("completed comparison has a root action")
                    .1,
            ),
            prepared: Arc::new(compared.prepared),
            parameters: ctx.params.clone(),
            origin: self.context.clone(),
            context,
            review_pending: false,
            raw_events: ctx.raw_events.to_vec(),
        });
        let mut reads = ctx.concepts.borrow_mut();
        reads.assessment = Some(PlanningAssessment {
            actions,
            values: compared.values,
            context: self.context.clone(),
        });
        reads.sequence = Some(trace);
        baseline
    }

    fn facts(&self, compared: &sequence::Comparison, mode: Mode) -> Vec<Fact> {
        let estimate = &compared.values.candidates[compared.values.selected];
        let mut facts = match mode {
            Mode::Preplay { .. } => vec![
                fact("forecast_complete", 1),
                fact("forecast_candidates", compared.executions.len() as u32),
                fact("forecast_worlds", self.tables.len() as u32),
                fact("forecast_score", estimate.difference),
                fact("forecast_our_score", estimate.our_score),
                fact("forecast_opponent_score", estimate.opponent_score),
                fact(
                    "forecast_gain",
                    estimate.difference - compared.values.candidates[0].difference,
                ),
                fact("forecast_model_estimate", 1),
                fact("sequence_complete", 1),
                fact("sequence_preplay", 1),
            ],
            Mode::Formation | Mode::Repair => vec![
                fact("sequence_complete", 1),
                fact("sequence_repaired", u8::from(matches!(mode, Mode::Repair))),
                fact("sequence_candidates", compared.executions.len() as u32),
            ],
        };
        facts.extend([
            fact("sequence_transitions", compared.work.transitions as u32),
            fact(
                "sequence_represented_transitions",
                compared.work.represented_transitions as u32,
            ),
            fact("sequence_response_hits", compared.work.response_hits as u32),
            fact("sequence_suffix_hits", compared.work.suffix_hits as u32),
            fact("sequence_difference", estimate.difference),
        ]);
        facts
    }
}

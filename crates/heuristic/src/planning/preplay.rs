//! Auction/return interventions followed by the common authored continuation.
//! Nomination belongs to the caller. This bridge neither resamples after an
//! intervention nor selects a separate best future in each hidden deal.

use std::{sync::Arc, time::Instant};

use engine::GameAction;

use super::retained::{view, PlanMode, RetainedPlan, SequenceTrace};
use super::sequence::{self, Candidate, Context, Deferred, Limits, Work};
use super::{BeliefScope, Forecast, PlanningAssessment};
use crate::concepts::sequence::{Actions, Guard, Information, Node, Program, SequenceError, Stage, MAX_BOUNDARY_DEPTH};
use crate::{fact, Ctx, Decision, RuleId};

/// Finite coverage, not a claim that every legal auction ends within this many
/// root decisions. Nonroot transitions also consume the evaluator's allowance.
const PHASE_RESPONSES: usize = 16;

/// Earlier-phase responses have a declared deterministic model, not a ranked
/// play-route domain. Reuse only its actually recorded offer at the identical
/// permitted input; an unrepresented response still requests ordinary repair.
pub(super) fn prepared_response(
    plan: &RetainedPlan,
    current: &agent::SeatView<'_>,
    legal: &[GameAction],
    mind: &beliefs::Mind,
) -> Option<crate::concepts::sequence::Offer> {
    use super::sequence::Selection;
    use crate::concepts::sequence::{inspect, Frontier, Offer};

    if plan.review_pending || plan.receipt.pending.is_some() || !plan.receipt.accepted.matches(current) {
        return None;
    }
    let Frontier::Response { next } = inspect(&plan.receipt.program, plan.receipt.stage, current, legal, mind)
        .ok()?
        .frontier
    else {
        return None;
    };
    let suffix = plan.receipt.program.suffix(plan.receipt.stage).ok()?;
    let next_suffix = plan.receipt.program.suffix(next).ok()?;
    let membership = sequence::Membership::from_candidates(legal, &[]).ok()?;
    let certificate = plan.prepared.iter().find(|choice| {
        choice.information.matches(current)
            && choice.semantic_suffix == suffix
            && choice.context == plan.context
            && choice.membership.legal == membership.legal
            && choice.assessment.is_none()
            && choice.alternatives.len() == 1
            && choice.selected == 0
            && !choice.support.is_empty()
    })?;
    let Selection::Offer {
        action,
        next: recorded_next,
    } = &certificate.alternatives[0]
    else {
        return None;
    };
    (recorded_next == &next_suffix && action.player.0 == current.seat && legal.contains(action))
        .then_some(Offer { action: *action, next })
}

/// An exact intervention followed by a visible phase gate. The same authored
/// catalogue binds after exchange, including optional raising. Root responses
/// before that boundary use the declared completed base model; they never
/// masquerade as an authored play formation.
fn intervention(action: GameAction, responses: usize) -> Candidate {
    let mut nodes = vec![Node::Tail, Node::Deferred];
    let formation = Stage(nodes.len());
    nodes.push(Node::Reconsider {
        // Initial binding is not a spent play-reconsideration opportunity.
        remaining: MAX_BOUNDARY_DEPTH,
        retained: Stage(0),
    });
    let mut exhausted = Stage(1);
    for step in 0..=responses {
        let unavailable = if step == 0 {
            exhausted
        } else {
            let response = Stage(nodes.len());
            nodes.push(Node::Response { next: exhausted });
            response
        };
        exhausted = Stage(nodes.len());
        nodes.push(Node::Guard {
            predicate: Guard::PlayReady,
            yes: formation,
            no: unavailable,
            unknown: unavailable,
        });
    }
    let start = Stage(nodes.len());
    nodes.push(Node::Action {
        actions: Actions::Exact(action.action_type),
        next: exhausted,
    });
    Candidate::new(Program {
        name: "preplay-intervention-then-formation",
        version: 1,
        start,
        nodes,
    })
}

/// Evaluate precisely the caller's ordered nominations. All required candidates
/// and worlds complete, or the existing choice survives with a deferred trace.
/// The selected receipt carries the bid/allocation into later accepted history.
pub(crate) fn choose(
    ctx: &Ctx<'_>,
    actions: &[GameAction],
    worlds: usize,
    deadline: Instant,
    replacement_rule: RuleId,
    mut baseline: Decision,
) -> Decision {
    let deadline = ctx.deadline.map_or(deadline, |outer| outer.min(deadline));
    let initial = view(ctx);
    let defer = |baseline: &mut Decision, failure: Deferred| {
        ctx.concepts.borrow_mut().sequence = Some(SequenceTrace {
            mode: PlanMode::Deferred,
            information: Information::of(&initial),
            scope: BeliefScope::CurrentInformation,
            program: None,
            choices: Vec::new(),
            terminals: Arc::new(Vec::new()),
            work: failure.work,
            failure: Some(failure.reason),
            deferred_limit: failure.limit,
            total_candidates: Some(actions.len()),
        });
        if let Some(plan) = ctx.intent.borrow_mut().as_mut() {
            plan.review_pending = true;
            plan.prepared = Arc::new(Vec::new());
            plan.receipt.pending = None;
        }
        baseline.why.extend([
            fact("forecast_complete", 0),
            fact("forecast_candidates", actions.len() as u32),
            fact("sequence_deferred", 1),
            fact("sequence_transitions", failure.work.transitions as u32),
            fact(
                "sequence_completed_candidates",
                failure.work.completed_candidates as u32,
            ),
        ]);
    };
    let Some(forecast) = Forecast::new(ctx, worlds, deadline) else {
        defer(
            &mut baseline,
            Deferred {
                reason: SequenceError::InvalidCohort,
                work: Work::default(),
                total_candidates: actions.len(),
                limit: None,
            },
        );
        return baseline;
    };
    let candidates: Vec<_> = actions
        .iter()
        .map(|&action| intervention(action, PHASE_RESPONSES))
        .collect();
    let mut provenance = forecast.provenance;
    provenance.response_model = "seat-policy-completed-primitives-v2";
    let context = Context {
        params: forecast.context.response_params.clone(),
        seed: forecast.context.seed,
        posterior_revision: ctx.mind.events().len() as u64,
        provenance,
    };
    let limits = Limits {
        transitions: ctx.params.plan_formation_transitions as usize,
        memo_bytes: ctx.params.plan_memo_mib as usize * 1024 * 1024,
        deadline: Some(deadline),
        ..Limits::default()
    };
    let mut players = forecast.players.borrow_mut();
    let mut response = |actor: &agent::SeatView<'_>, legal: &[GameAction], allowance: sequence::LeafBudget| {
        if allowance.remaining_transitions == 0 || allowance.deadline.is_some_and(|end| Instant::now() >= end) {
            return Err(SequenceError::Budget);
        }
        let decision = players[actor.seat as usize]
            .completed_response_before(actor, legal, allowance.deadline)
            .ok_or(SequenceError::Budget)?;
        if allowance.deadline.is_some_and(|end| Instant::now() >= end) {
            return Err(SequenceError::Budget);
        }
        Ok(decision.action)
    };
    let compared = match sequence::evaluate_with_cache(
        &candidates,
        &forecast.tables,
        ctx.me,
        provenance,
        &context,
        limits,
        &mut ctx.completed.borrow_mut(),
        &mut response,
    ) {
        Ok(compared) => compared,
        Err(failure) => {
            defer(&mut baseline, failure);
            return baseline;
        }
    };
    let selected = compared.values.selected;
    let execution = &compared.executions[selected];
    let estimate = &compared.values.candidates[selected];
    if baseline.action != execution.first.action {
        baseline.action = execution.first.action;
        baseline.rule = replacement_rule;
        baseline.why.clear();
    }
    baseline.why.extend([
        fact("forecast_complete", 1),
        fact("forecast_candidates", actions.len() as u32),
        fact("forecast_worlds", forecast.tables.len() as u32),
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
        fact("sequence_transitions", compared.work.transitions as u32),
        fact(
            "sequence_represented_transitions",
            compared.work.represented_transitions as u32,
        ),
        fact("sequence_response_hits", compared.work.response_hits as u32),
        fact("sequence_suffix_hits", compared.work.suffix_hits as u32),
        fact("sequence_difference", estimate.difference),
    ]);
    let trace = SequenceTrace {
        mode: PlanMode::Formation,
        information: Information::of(&initial),
        scope: BeliefScope::CurrentInformation,
        program: compared.receipt.program.suffix(compared.receipt.stage).ok(),
        choices: compared
            .prepared
            .iter()
            .filter(|choice| choice.information.matches(&initial))
            .cloned()
            .collect(),
        terminals: Arc::new(execution.terminals.clone()),
        work: compared.work,
        failure: None,
        deferred_limit: None,
        total_candidates: Some(actions.len()),
    };
    *ctx.intent.borrow_mut() = Some(RetainedPlan {
        receipt: compared.receipt,
        prepared: Arc::new(compared.prepared),
        parameters: ctx.params.clone(),
        origin: forecast.context.clone(),
        context,
        review_pending: false,
        raw_events: ctx.raw_events.to_vec(),
    });
    let mut reads = ctx.concepts.borrow_mut();
    reads.assessment = Some(PlanningAssessment {
        actions: actions.to_vec(),
        values: compared.values,
        context: forecast.context.clone(),
    });
    reads.sequence = Some(trace);
    baseline
}

#[cfg(test)]
#[path = "../../tests/internal/preplay_bridge.rs"]
mod tests;

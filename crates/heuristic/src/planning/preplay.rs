//! Auction/return interventions followed by the common authored continuation.
//! Nomination belongs to the caller. This bridge neither resamples after an
//! intervention nor selects a separate best future in each hidden deal.

use std::time::Instant;

use engine::GameAction;

use super::common::{self, Allowance, Failure, Mode};
use super::retained::RetainedPlan;
use super::sequence::{self, Candidate};
use super::Forecast;
#[cfg(test)]
use crate::concepts::sequence::SequenceError;
use crate::concepts::sequence::{Actions, Guard, Node, Program, Stage, MAX_BOUNDARY_DEPTH};
use crate::{Ctx, Decision, RuleId};

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
    let certificate = plan.authorized_records()?.iter().find(|choice| {
        choice.information.matches(current)
            && choice.semantic_suffix == suffix
            && choice.context == plan.context
            && choice.scope == sequence::ChoiceScope::Response
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
    let mode = Mode::Preplay { replacement_rule };
    let forecast = match Forecast::new(ctx, worlds, deadline) {
        Ok(forecast) => forecast,
        Err(error) => {
            common::defer(
                ctx,
                &mut baseline,
                mode,
                Failure::before_evaluation(error.reason(), Some(actions.len())),
            );
            return baseline;
        }
    };
    let candidates: Vec<_> = actions
        .iter()
        .map(|&action| intervention(action, PHASE_RESPONSES))
        .collect();
    match forecast.evaluate_programs(ctx, &candidates, Allowance::Formation) {
        Ok(evaluated) => forecast.install(ctx, evaluated, mode, baseline),
        Err(failure) => {
            common::defer(ctx, &mut baseline, mode, failure.into());
            baseline
        }
    }
}

#[cfg(test)]
#[path = "../../tests/internal/preplay_bridge.rs"]
mod tests;

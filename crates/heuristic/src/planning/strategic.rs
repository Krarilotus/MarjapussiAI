//! Phase nomination and scheduling for authored play continuations.
use super::common::{self, Allowance, Failure, Mode};
use super::retained::view;
use super::sequence::Candidate;
use super::Forecast;
use crate::concepts::sequence::{authored_programs_with_reconsideration, SequenceError, MAX_RECONSIDERATIONS};
use crate::{Ctx, Decision};
use engine::GameAction;
use std::time::{Duration, Instant};

/// Preserve the existing play catalogue, retained alternative and budget lane.
pub(super) fn choose(ctx: &Ctx<'_>, legal: &[GameAction], mut fallback: Decision) -> Decision {
    let retained = ctx
        .intent
        .borrow()
        .as_ref()
        .map(|plan| Candidate::suffix(&plan.receipt));
    let repairing = retained.is_some();
    let full_formation = !repairing && ctx.k.played().is_empty() && !ctx.formation_attempted.get();
    if full_formation {
        ctx.formation_attempted.set(true);
    }
    let budget = if full_formation {
        super::BUDGET
    } else {
        Duration::from_millis(50)
    };
    let own_deadline = Instant::now() + budget;
    let deadline = ctx.deadline.map_or(own_deadline, |outer| outer.min(own_deadline));
    let mode = if repairing { Mode::Repair } else { Mode::Formation };
    let requested_worlds = if ctx.params.plan_worlds > 0 {
        ctx.params.plan_worlds as usize
    } else {
        // A return-only planner still owns the continuation it selected. Its
        // repair preserves that declared cohort size instead of abandoning it.
        ctx.intent
            .borrow()
            .as_ref()
            .map_or(0, |plan| plan.origin.requested_worlds)
    };
    let Some(forecast) = Forecast::new(ctx, requested_worlds, deadline) else {
        common::defer(
            ctx,
            &mut fallback,
            mode,
            Failure::before_evaluation(SequenceError::InvalidCohort, None),
        );
        return fallback;
    };
    let initial = view(ctx);
    let programs = match authored_programs_with_reconsideration(&initial, legal, MAX_RECONSIDERATIONS) {
        Ok(programs) => programs,
        Err(error) => {
            common::defer(ctx, &mut fallback, mode, Failure::before_evaluation(error, None));
            return fallback;
        }
    };
    let mut candidates: Vec<_> = programs.into_iter().map(Candidate::new).collect();
    if let Some(retained) = retained {
        let suffix = retained.program.suffix(retained.stage).ok();
        if !candidates.iter().any(|c| c.program.suffix(c.stage).ok() == suffix) {
            candidates.push(retained);
        }
    }
    let allowance = if full_formation {
        Allowance::Formation
    } else {
        Allowance::Repair
    };
    match forecast.evaluate_programs(ctx, &candidates, allowance) {
        Ok(evaluated) => forecast.install(ctx, evaluated, mode, fallback),
        Err(failure) => {
            common::defer(ctx, &mut fallback, mode, failure.into());
            fallback
        }
    }
}

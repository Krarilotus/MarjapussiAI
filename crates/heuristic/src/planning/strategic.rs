//! Policy integration for the authored continuation definitions.
use super::retained::{view, PlanMode, RetainedPlan, SequenceTrace};
use super::sequence::{self, Candidate, Context, Limits, Work};
use super::{BeliefScope, Forecast, PlanningAssessment};
use crate::concepts::sequence::{authored_programs, Information, SequenceError};
use crate::{fact, Ctx, Decision, RuleId};
use engine::GameAction;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn evidence(ctx: &Ctx<'_>, mode: PlanMode, work: Work, failure: SequenceError) {
    ctx.concepts.borrow_mut().sequence = Some(SequenceTrace {
        mode,
        information: Information::of(&view(ctx)),
        scope: BeliefScope::CurrentInformation,
        program: None,
        choices: Vec::new(),
        terminals: Arc::new(Vec::new()),
        work,
        failure: Some(failure),
    });
}

/// This path owns selection in its declared finite family. An unfinished
/// comparison is reported and leaves the supplied outside-scope fallback intact.
pub(super) fn choose(ctx: &Ctx<'_>, legal: &[GameAction], mut fallback: Decision) -> Decision {
    let retained = ctx
        .intent
        .borrow()
        .as_ref()
        .map(|plan| Candidate::suffix(&plan.receipt));
    let repairing = retained.is_some();
    let full_formation = !repairing && ctx.k.played().is_empty();
    let budget = if full_formation {
        super::BUDGET
    } else {
        Duration::from_millis(50)
    };
    let own_deadline = Instant::now() + budget;
    let deadline = ctx.deadline.map_or(own_deadline, |outer| outer.min(own_deadline));
    let defer = |fallback: &mut Decision, work: Work, error: SequenceError| {
        evidence(ctx, PlanMode::Deferred, work, error);
        if let Some(plan) = ctx.intent.borrow_mut().as_mut() {
            // Keep semantic intent available for later repair, but invalidate
            // its old predictions. The fallback is not a confirmed plan step.
            plan.review_pending = true;
            plan.prepared = Arc::new(Vec::new());
            plan.receipt.pending = None;
        }
        fallback.why.extend([
            fact("sequence_deferred", 1),
            fact("sequence_transitions", work.transitions as u32),
            fact("sequence_completed_candidates", work.completed_candidates as u32),
        ]);
    };
    let Some(forecast) = Forecast::new(ctx, ctx.params.plan_worlds as usize, deadline) else {
        defer(&mut fallback, Work::default(), SequenceError::InvalidCohort);
        return fallback;
    };
    let initial = view(ctx);
    let mut candidates: Vec<_> = authored_programs(&initial, legal)
        .into_iter()
        .map(Candidate::new)
        .collect();
    if let Some(retained) = retained {
        let suffix = retained.program.suffix(retained.stage).ok();
        if !candidates.iter().any(|c| c.program.suffix(c.stage).ok() == suffix) {
            candidates.push(retained);
        }
    }
    let context = Context {
        params: forecast.context.response_params.clone(),
        seed: forecast.context.seed,
        posterior_revision: ctx.mind.events().len() as u64,
        provenance: forecast.provenance,
    };
    let limits = Limits {
        deadline: Some(deadline),
        ..Limits::default()
    };
    let mut players = forecast.players.borrow_mut();
    let mut leaf = |actor: &agent::SeatView<'_>, actions: &[GameAction], allowance: sequence::LeafBudget| {
        if allowance.remaining_transitions == 0 || allowance.deadline.is_some_and(|until| Instant::now() >= until) {
            return Err(SequenceError::Budget);
        }
        // Fresh private hypothetical actors never receive a real player's plan.
        // Their bounded search shares the caller's remaining soft deadline.
        let action = players[actor.seat as usize]
            .decide_before(actor, actions, allowance.deadline)
            .action;
        if allowance.deadline.is_some_and(|until| Instant::now() >= until) {
            return Err(SequenceError::Budget);
        }
        Ok(action)
    };
    let compared = match sequence::evaluate(
        &candidates,
        &forecast.tables,
        ctx.me,
        forecast.provenance,
        &context,
        limits,
        &mut leaf,
    ) {
        Ok(value) => value,
        Err(deferred) => {
            defer(&mut fallback, deferred.work, deferred.reason);
            return fallback;
        }
    };
    let selected = &compared.executions[compared.values.selected];
    let action = selected.first.action;
    let program = selected.candidate.program.suffix(selected.candidate.stage).ok();
    let terminals = Arc::new(selected.terminals.clone());
    let mode = if repairing {
        PlanMode::Repair
    } else {
        PlanMode::Formation
    };
    let choices = compared
        .prepared
        .iter()
        .filter(|choice| choice.information.matches(&initial))
        .cloned()
        .collect();
    let trace = SequenceTrace {
        mode,
        information: Information::of(&initial),
        scope: BeliefScope::CurrentInformation,
        program,
        choices,
        terminals,
        work: compared.work,
        failure: None,
    };
    let actions = compared
        .executions
        .iter()
        .map(|execution| execution.first.action)
        .collect();
    let value = &compared.values.candidates[compared.values.selected];
    let decision = crate::decision(
        &action,
        RuleId::HP1,
        vec![
            fact("sequence_complete", 1),
            fact("sequence_repaired", u8::from(repairing)),
            fact("sequence_candidates", compared.executions.len() as u32),
            fact("sequence_transitions", compared.work.transitions as u32),
            fact(
                "sequence_represented_transitions",
                compared.work.represented_transitions as u32,
            ),
            fact("sequence_response_hits", compared.work.response_hits as u32),
            fact("sequence_suffix_hits", compared.work.suffix_hits as u32),
            fact("sequence_difference", value.difference),
        ],
    );
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
        actions,
        values: compared.values,
        context: forecast.context.clone(),
    });
    reads.sequence = Some(trace);
    decision
}

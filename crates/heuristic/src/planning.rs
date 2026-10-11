//! One root-conditioned cohort for authored continuation comparisons.
//! Shared evaluation and receipt ownership live in `common`; phase modules
//! nominate their existing domains without adding separate scoring models.

use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

mod common;
mod history;
mod preparation;
pub(crate) mod preplay;
pub(crate) use preparation::PreparationError;
pub mod retained;
pub mod sequence;
mod strategic;

use agent::{SeatView, Table};
use engine::{ActionType, GameAction};

use crate::concepts::value::CohortMethod;
use crate::concepts::value::{Assessment, Provenance};
use crate::{Ctx, Decision, HeuristicAgent, Rng, RuleId};

pub(crate) const BUDGET: Duration = Duration::from_millis(2_000);

/// A prepared future comparison conditions the original cohort and response
/// model. It is not silently relabeled as a newly constructed Mind posterior.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BeliefScope {
    CurrentInformation,
    PreparedConditional {
        origin: crate::concepts::sequence::Information,
    },
}

/// Inputs of the original forecast. These are observer-visible context and
/// model settings, never another actual player's private Mind or intentions.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanningContext {
    pub observer: u8,
    pub belief_scope: BeliefScope,
    pub knowledge: knowledge::Knowledge,
    pub events: Vec<observation::SeatEvent>,
    pub kinds: [agent::SeatKind; 4],
    pub series: agent::SeriesScore,
    pub response_params: crate::Params,
    pub seed: u64,
    pub possible_allocations: u64,
    pub requested_worlds: usize,
    pub proposal_draws: usize,
}

/// The same stored values select the move and supply its explanation.
/// Candidate scope and the completed response approximation remain explicit.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanningAssessment {
    pub actions: Vec<GameAction>,
    pub values: Assessment,
    pub context: PlanningContext,
}

/// One cohort for every alternative. The retained tables are hypotheses, never
/// observations passed to a policy. The proposal pool keeps correlated bidding
/// evidence represented before spending the continuation evaluation budget.
pub(crate) struct Forecast {
    tables: Vec<(Table, f64)>,
    // Actor views are resynchronized on every call. Reuse the bounded solver
    // allocations across hypotheses. Strict responses own canonical solver resets.
    players: RefCell<[HeuristicAgent; 4]>,
    deadline: Instant,
    provenance: Provenance,
    context: PlanningContext,
}

impl Forecast {
    pub fn new(ctx: &Ctx, n: usize, deadline: Instant) -> Result<Self, PreparationError> {
        Self::new_checked(ctx, n, deadline, &mut || Instant::now() < deadline)
    }

    // Preserve an operational interruption separately from inconsistent support.
    // Caller-owned Failure mapping must retain this cause, not label all errors
    // InvalidCohort. No incomplete pool is published by this checked boundary.
    fn new_checked(
        ctx: &Ctx,
        n: usize,
        deadline: Instant,
        keep_going: &mut impl FnMut() -> bool,
    ) -> Result<Self, PreparationError> {
        use preparation::{check, proposal_pool, replay_worlds, select_worlds};
        let _timing = beliefs::profile::scope("forecast.build");
        check(keep_going)?;
        if n == 0 {
            return Err(PreparationError::InvalidCohort);
        }
        let mut rng = Rng::new(ctx.seed);
        let (pool, allocation_count, proposal_draws) = {
            let model = ctx.mind.deal_model();
            check(keep_going)?;
            let (space, weights) = (model.space(), model.weights());
            proposal_pool(space, weights, &mut rng, n, keep_going)?
        };
        let selected = select_worlds(pool, allocation_count, proposal_draws, n, &mut rng, keep_going)?;
        let view = SeatView {
            seat: ctx.me,
            knowledge: ctx.k,
            events: ctx.mind.events(),
            series: ctx.series,
            kinds: ctx.mind.kinds(),
        };
        let (tables, cohort, belief_model) = if history::has_hidden_passes(&view) {
            let _timing = beliefs::profile::scope("forecast.cohort.history");
            let model = ctx.mind.deal_model();
            // Completion work bound, not a change to current draws/world count.
            // Root may tune this cap separately; exhaustion defers every slot.
            let completed = history::complete_slots(
                &view,
                &selected.slots,
                model.weights(),
                ctx.seed,
                history::HistoryLimits {
                    max_proposals_per_slot: 4096,
                },
                keep_going,
            )
            .map_err(|error| match error {
                history::HistoryError::Interrupted
                | history::HistoryError::ProposalLimit { .. }
                | history::HistoryError::Replay(agent::HistoryReplayError::Interrupted) => PreparationError::Deferred,
                _ => PreparationError::InvalidCohort,
            })?;
            let (current_draws, current_proposal_ess, current_resampled) = match selected.cohort {
                CohortMethod::Enumerated => (0, None, false),
                CohortMethod::ImportanceSampled { draws, proposal_ess } => (draws, Some(proposal_ess), false),
                CohortMethod::Resampled { draws, proposal_ess } => (draws, Some(proposal_ess), true),
                CohortMethod::HistorySampled { .. } => return Err(PreparationError::InvalidCohort),
            };
            let cohort = CohortMethod::HistorySampled {
                current_draws,
                current_proposal_ess,
                current_resampled,
                model: completed.model,
                proposals: completed.proposals,
                slots: selected.slots.len(),
            };
            (
                completed.tables,
                cohort,
                "joint-deal-model-v1+legal-exchange-history-feature-bid-v1",
            )
        } else {
            (
                replay_worlds(&view, selected.worlds, keep_going)?,
                selected.cohort,
                "joint-deal-model-v1",
            )
        };
        let mut params = ctx.params.clone();
        params.return_worlds = 0;
        params.plan_worlds = 0;
        params.endgame_tricks = params.endgame_tricks.min(2);
        params.endgame_worlds = params.endgame_worlds.min(8);
        let context = PlanningContext {
            observer: ctx.me,
            belief_scope: BeliefScope::CurrentInformation,
            knowledge: *ctx.k,
            events: ctx.mind.events().to_vec(),
            kinds: ctx.mind.kinds(),
            series: ctx.series,
            response_params: params.clone(),
            seed: ctx.seed,
            possible_allocations: allocation_count,
            requested_worlds: n,
            proposal_draws,
        };
        check(keep_going)?;
        let forecast = Self {
            tables,
            players: RefCell::new(std::array::from_fn(|_| HeuristicAgent::new(params.clone()))),
            deadline,
            provenance: Provenance {
                belief_model,
                response_model: "seat-policy-completed-primitives-v2",
                cohort,
            },
            context,
        };
        check(keep_going)?;
        Ok(forecast)
    }
}

/// Auction entry retains its convention policy. Optional raising and card play
/// enter the authored sequence comparison, whose selected receipt owns later play.
pub(crate) fn choose(ctx: &Ctx, legal: &[GameAction], baseline: Decision) -> Decision {
    if ctx.params.plan_worlds == 0 && ctx.intent.borrow().is_none() {
        return baseline;
    }
    let auction = legal.iter().any(|a| a.action_type == ActionType::StopBidding);
    let raising = !auction && legal.iter().any(|a| matches!(a.action_type, ActionType::NewBid(_)));
    if auction && baseline.action.action_type != ActionType::StopBidding {
        return baseline;
    }
    let retained = ctx.intent.borrow().is_some();
    if !auction
        && !raising
        && !retained
        && (baseline.rule == RuleId::HE1
            || !ctx.k.trick().is_empty()
            || ctx.k.hand_len(ctx.me) as u32 <= ctx.params.endgame_tricks)
    {
        return baseline;
    }
    if !auction {
        return strategic::choose(ctx, legal, baseline);
    }
    let mut options = vec![baseline.action];
    if let Some(action) = legal
        .iter()
        .filter(|a| matches!(a.action_type, ActionType::NewBid(_)))
        .min_by_key(|a| match a.action_type {
            ActionType::NewBid(value) => value,
            _ => unreachable!(),
        })
    {
        options.push(*action);
    }
    preplay::choose(
        ctx,
        &options,
        ctx.params.plan_worlds as usize,
        Instant::now() + BUDGET,
        RuleId::HP1,
        baseline,
    )
}

#[cfg(test)]
#[path = "../tests/internal/forecast_reuse.rs"]
mod reuse_tests;

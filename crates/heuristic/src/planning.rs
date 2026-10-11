//! One root-conditioned cohort for authored continuation comparisons.
//! Shared evaluation and receipt ownership live in `common`; phase modules
//! nominate their existing domains without adding separate scoring models.

use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

mod common;
pub(crate) mod preplay;
pub mod retained;
pub mod sequence;
mod strategic;

use agent::{SeatView, Table};
use engine::{ActionType, GameAction};
use knowledge::World;

use crate::concepts::value::{Assessment, CohortMethod, Provenance};
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
    pub fn new(ctx: &Ctx, n: usize, deadline: Instant) -> Option<Self> {
        let _timing = beliefs::profile::scope("forecast.build");
        if n == 0
            || Instant::now() >= deadline
            || ctx
                .mind
                .events()
                .iter()
                .any(|e| matches!(e, observation::SeatEvent::Pass { cards: None, .. }))
        {
            return None;
        }
        let mut rng = Rng::new(ctx.seed);
        let (mut pool, allocation_count, proposal_draws) = {
            let model = ctx.mind.deal_model();
            let (space, weights) = (model.space(), model.weights());
            let count = space.count();
            if count <= n as u64 {
                use knowledge::Belief;
                let mut pool = Vec::new();
                space.for_each_deal(|hands| {
                    pool.push(World {
                        hands: *hands,
                        weight: weights.weight(hands),
                    })
                });
                (pool, count, 0)
            } else {
                (space.sample_worlds(&mut rng, n * 256, weights), count, n * 256)
            }
        };
        if pool.iter().any(|w| !w.weight.is_finite() || w.weight < 0.0) {
            return None;
        }
        pool.retain(|w| w.weight > 0.0);
        let mass: f64 = pool.iter().map(|w| w.weight).sum();
        if !mass.is_finite() || mass <= 0.0 {
            return None;
        }
        let ess = mass * mass / pool.iter().map(|w| w.weight * w.weight).sum::<f64>();
        let cohort = if allocation_count <= n as u64 {
            CohortMethod::Enumerated
        } else if pool.len() <= n {
            CohortMethod::ImportanceSampled {
                draws: proposal_draws,
                proposal_ess: ess,
            }
        } else {
            CohortMethod::Resampled {
                draws: proposal_draws,
                proposal_ess: ess,
            }
        };
        let worlds = if pool.len() <= n {
            for w in &mut pool {
                w.weight /= mass;
            }
            pool
        } else {
            let step = mass / n as f64;
            let start = (rng.next() >> 11) as f64 / (1u64 << 53) as f64 * step;
            let mut worlds: Vec<World> = Vec::new();
            let (mut index, mut sum) = (0, pool[0].weight);
            for i in 0..n {
                let target = start + i as f64 * step;
                while sum <= target && index + 1 < pool.len() {
                    index += 1;
                    sum += pool[index].weight;
                }
                let hands = pool[index].hands;
                if let Some(w) = worlds.iter_mut().find(|w| w.hands == hands) {
                    w.weight += 1.0 / n as f64;
                } else {
                    worlds.push(World {
                        hands,
                        weight: 1.0 / n as f64,
                    });
                }
            }
            worlds
        };
        let view = SeatView {
            seat: ctx.me,
            knowledge: ctx.k,
            events: ctx.mind.events(),
            series: ctx.series,
            kinds: ctx.mind.kinds(),
        };
        let tables: Option<Vec<_>> = worlds
            .into_iter()
            .map(|w| Some((Table::from_view(&view, w.hands)?, w.weight)))
            .collect();
        let mut params = ctx.params.clone();
        params.return_worlds = 0;
        params.plan_worlds = 0;
        params.endgame_tricks = params.endgame_tricks.min(2);
        params.endgame_worlds = params.endgame_worlds.min(8);
        let tables = tables?;
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
        Some(Self {
            tables,
            players: RefCell::new(std::array::from_fn(|_| HeuristicAgent::new(params.clone()))),
            deadline,
            provenance: Provenance {
                belief_model: "joint-deal-model-v1",
                response_model: "seat-policy-completed-primitives-v2",
                cohort,
            },
            context,
        })
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

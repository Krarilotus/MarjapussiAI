//! The exchange by exact search: candidate passes judged over the believed worlds by the
//! double-dummy solver, as the endgame judges card play (perfect-information Monte Carlo).
//! Per candidate and world, the hands after the exchange are played from the declarer's first
//! lead by both teams' best play with all hands open; the value is the contract's outcome for
//! the declaring team (+value if it reaches the game value, -value if not). The passing store's
//! choice stands unless another candidate is clearly better (`WorldValues::clearly_better`).
//!
//! Open hands overstate both teams' play alike; the comparison between candidates on the same
//! worlds is what decides, not the level of the estimate.

use engine::{partner, CardSet, FastState, HAND_SIZE};
use knowledge::World;
use solver::Solver;

use crate::outlook::WorldValues;
use crate::Ctx;

/// Transposition table of the exchange search: 2^18 entries (10 MB) for whole-game solves.
const TT_BITS: u32 = 18;

/// Whether the declaring team, on lead with `declarer`, reaches the game value in `hands`
/// against best defence: the contract's outcome for that team, +value or -value.
fn outcome(solver: &mut Solver, hands: [CardSet; 4], declarer: u8, value: i32) -> Option<i32> {
    if hands.iter().any(|h| h.len() != HAND_SIZE as u32) {
        return None;
    }
    let state = FastState::new(hands, declarer);
    Some(if solver.reaches(&state, declarer % 2, value) {
        value
    } else {
        -value
    })
}

/// The declaring team's final points after the exchange, per believed world (its weight and the
/// points under both teams' best play with open hands), from the current position.
pub(crate) fn team_points(ctx: &Ctx) -> Option<Vec<(f64, i32)>> {
    let _timing = beliefs::profile::scope("exchange.team_points");
    let us = ctx.me % 2;
    let public = ctx.k.public_state(ctx.me, engine::PlayPhase::Cards);
    let mut solver = Solver::with_tt_bits(TT_BITS);
    let points: Vec<(f64, i32)> = ctx.with_worlds(|worlds: &[World]| {
        worlds_of(ctx, worlds)
            .iter()
            .filter_map(|w| {
                let state = FastState::from_public(public, w.hands).ok()?;
                Some((w.weight, state.points(us) + solver.value(&state, us)))
            })
            .collect()
    });
    (!points.is_empty()).then_some(points)
}

/// The believed worlds the exchange searches: a prefix of the decision's pool (a sample).
fn worlds_of<'a>(ctx: &Ctx, worlds: &'a [World]) -> &'a [World] {
    &worlds[..worlds.len().min(ctx.params.exchange_worlds as usize)]
}

/// The declarer's return from its thirteen cards, per candidate.
pub(crate) fn return_values(ctx: &Ctx, candidates: &[CardSet]) -> Option<WorldValues<CardSet>> {
    let _timing = beliefs::profile::scope("exchange.return");
    let (me, mate, value) = (ctx.me, partner(ctx.me), ctx.k.value());
    let mut solver = Solver::with_tt_bits(TT_BITS);
    ctx.with_worlds(|worlds: &[World]| {
        WorldValues::over(ctx, worlds_of(ctx, worlds), candidates.to_vec(), |w, &cards| {
            if cards.len() != 4 || !cards.is_subset(w.hands[me as usize]) {
                return None;
            }
            let mut hands = w.hands;
            hands[me as usize] -= cards;
            hands[mate as usize] |= cards;
            outcome(&mut solver, hands, me, value)
        })
    })
}

/// The pass forth from my nine cards to the declaring partner, per candidate: in each world the
/// declarer returns by the passing store (`passing::level0_back`, reading me from the auction).
pub(crate) fn forth_values(ctx: &Ctx, candidates: &[CardSet]) -> Option<WorldValues<CardSet>> {
    let _timing = beliefs::profile::scope("exchange.forth");
    let (me, declarer, value) = (ctx.me, partner(ctx.me), ctx.k.value());
    let reads_me = ctx.mind.class_likelihood(me, declarer);
    let conf = ctx.params.partner_half_conf;
    let mut solver = Solver::with_tt_bits(TT_BITS);
    ctx.with_worlds(|worlds: &[World]| {
        WorldValues::over(ctx, worlds_of(ctx, worlds), candidates.to_vec(), |w, &cards| {
            if !cards.is_subset(w.hands[me as usize]) {
                return None;
            }
            let hands = crate::passing::level0_back(w.hands, declarer, cards, conf, reads_me.as_deref());
            outcome(&mut solver, hands, declarer, value)
        })
    })
}

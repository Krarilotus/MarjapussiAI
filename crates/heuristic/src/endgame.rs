//! The endgame (design D-c / F10): with at most `endgame_tricks` tricks left, the
//! play action is chosen by the bounded solver over the deals I can believe in, not by
//! the valuation's closed forms.
//!
//! - **Worlds:** the deals consistent with my facts (`knowledge::DealSpace`): all of them when
//!   there are at most `endgame_worlds`, otherwise that many drawn uniformly.
//! - **Weights:** the same joint dealt-hand bidding likelihood as the feature
//!   model, with its hidden-exchange kernel and optional card signal factors,
//!   times the partner's raise as the route worlds read it (`routes::RaiseEvidence`).
//!   With facts alone every consistent deal counts the same.
//! - **Objective:** the contract (HC-9: declarers make it, then keep the defenders' points
//!   down; defenders take points, avoiding schwarz), or the own points in an all-pass game.
//! - **Ties:** cards the solver values exactly alike are told apart by the valuation (e.g. a
//!   standing ace that wins either now or later is kept, as the future-trick value says).

use engine::{CardSet, GameAction, Move, PlayPhase, QuestionType};
use solver::{evaluate_moves, Effort, Limits, Objective, Racing, Solver};
use std::time::{Duration, Instant};

use crate::rules::RuleId;
use crate::valuation::{Choice, Valuation};
use crate::{decision, fact, Ctx, Decision, Rng};

/// Transposition table of the endgame solver: 2^16 entries (2.6 MB) suffice for 4 tricks.
const TT_BITS: u32 = 16;
/// The endgame search's own wall-clock share of a play decision under deadlines (ms): the
/// logical limit `Params::endgame_nodes` normally ends it first; the rest of the play limit
/// stays for the rules' fallback.
const SEARCH_MS: u64 = 65;

pub(crate) fn decide(ctx: &Ctx, legal: &[GameAction], val: &Valuation) -> Option<Decision> {
    let _timing = beliefs::profile::scope("endgame");
    let k = ctx.k;
    let tricks_left = k.hand_len(ctx.me) as u32;
    if tricks_left == 0 || tricks_left > ctx.params.endgame_tricks {
        return None;
    }
    // Under deadlines, the search keeps the rest of the play limit for the valuation's fallback.
    let deadline = ctx
        .deadline
        .map(|outer| outer.min(Instant::now() + Duration::from_millis(SEARCH_MS)));
    if deadline.is_some_and(|d| Instant::now() >= d) {
        ctx.deadline_hit.set(true);
        return None;
    }
    let table = k.public_state(ctx.me, PlayPhase::from_actions(legal));
    let objective = match k.declarer() {
        Some(d) => Objective::Contract {
            playing_team: d % 2,
            game_value: k.value(),
        },
        None => Objective::Points,
    };
    // A called suit cannot become trump again; known answers without activation add no
    // information. Such questions are dominated by a card, so they are not searched; this root
    // policy filter does not change the engine's legal question list.
    let mate = engine::partner(ctx.me);
    let keep = |m: Move| match m {
        Move::Question(QuestionType::Yours) => k.pairs(mate).possible != 0,
        Move::Question(QuestionType::YourHalf(suit)) => {
            !k.is_called(suit)
                && match k.known_half(mate, suit) {
                    Some(false) => false,
                    Some(true) => !(k.certain(ctx.me) & CardSet::halves(suit)).is_empty(),
                    None => true,
                }
        }
        _ => true,
    };
    let mut slot = ctx.solver.borrow_mut();
    let solver = slot.get_or_insert_with(|| Solver::with_tt_bits(TT_BITS));
    let mut rng = Rng::new(ctx.seed);
    let min_worlds = ctx.params.endgame_min_worlds.min(ctx.params.endgame_worlds) as usize;
    let evaluated = {
        let model = ctx.mind.deal_model();
        let raise = crate::routes::RaiseEvidence::new(ctx);
        let belief = crate::routes::WithRaise {
            base: model.weights(),
            raise: &raise,
        };
        let nodes = ctx.params.endgame_nodes;
        let limits = Limits {
            deadline,
            nodes: (nodes > 0).then_some(nodes),
        };
        let effort = Effort {
            worlds: ctx.params.endgame_worlds as usize,
            limits,
            racing: (ctx.params.endgame_race_z > 0.0).then(|| Racing {
                batch: ctx.params.endgame_race_batch as usize,
                z: ctx.params.endgame_race_z as f64,
            }),
            // A search that cannot reach the minimum stops early and leaves the time to the rules.
            required: min_worlds,
        };
        evaluate_moves(solver, k, &table, objective, &belief, &mut rng, effort, keep).ok()
    };
    // A complete evaluation decides even with fewer consistent deals than the minimum.
    let d = evaluated.filter(|d| d.complete || d.worlds >= min_worlds)?;
    // An incomplete evaluation stopped at the clock only if the deadline has passed; otherwise
    // the node limit (`Params::endgame_nodes`) ended it, deterministically.
    if !d.complete && deadline.is_some_and(|d| Instant::now() >= d) {
        ctx.deadline_hit.set(true);
    }
    let unavailable = || None;
    let Some(mut best) = d.best() else { return unavailable() };
    let means = |m: Move| d.moves.iter().find(|v| v.mv == m).map(|v| v.means.clone());
    let Some(top) = means(best) else { return unavailable() };
    // A question or announcement that gains nothing over a card spends rights and tells the
    // table: on a tie, play the card (the owner: cash first, ask when it is needed).
    let card_ties = d.moves.iter().any(|v| matches!(v.mv, Move::Card(_)) && v.means == top);
    if matches!(best, Move::Card(_)) || card_ties {
        let choice = Choice::new(ctx);
        let leading = k.trick().is_empty();
        let tied = d
            .moves
            .iter()
            .filter_map(|v| match v.mv {
                Move::Card(c) if v.means == top => Some(c),
                _ => None,
            })
            .map(|c| (c, choice.value_of_play(ctx, val, c, leading)))
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)));
        if let Some((c, _)) = tied {
            best = Move::Card(c);
        }
    }
    let Some(action) = legal.iter().find(|a| Move::from_action(&a.action_type) == Some(best)) else {
        return unavailable();
    };
    let mean = top[0];
    Some(decision(
        action,
        RuleId::HE1,
        vec![
            fact("worlds", d.worlds as u32),
            fact("all_deals", d.all_deals as u8),
            fact("complete", d.complete as u8),
            fact("ess", d.ess),
            fact("value", mean),
        ],
    ))
}

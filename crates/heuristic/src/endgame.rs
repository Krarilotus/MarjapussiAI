//! The endgame (design D-c / F10): with at most `endgame_tricks` tricks left, the
//! play action is chosen by the bounded solver over the deals I can believe in, not by
//! the valuation's closed forms.
//!
//! - **Worlds:** the deals consistent with my facts (`knowledge::DealSpace`): all of them when
//!   there are at most `endgame_worlds`, otherwise that many drawn uniformly.
//! - **Weights:** the same joint dealt-hand bidding likelihood as the feature
//!   model, with its hidden-exchange kernel and optional card signal factors.
//!   With facts alone every consistent deal counts the same.
//! - **Objective:** the contract (HC-9: declarers make it, then keep the defenders' points
//!   down; defenders take points, avoiding schwarz), or the own points in an all-pass game.
//! - **Ties:** cards the solver values exactly alike are told apart by the valuation (e.g. a
//!   standing ace that wins either now or later is kept, as the future-trick value says).

use engine::{CardSet, GameAction, Move, PlayPhase, QuestionType};
use solver::{evaluate_limited, Limits, Objective, Solver};
use std::time::{Duration, Instant};

use crate::rules::RuleId;
use crate::valuation::{Choice, Valuation};
use crate::{decision, fact, Ctx, Decision, Rng};

/// Transposition table of the endgame solver: 2^16 entries (2.6 MB) suffice for 4 tricks.
const TT_BITS: u32 = 16;

pub(crate) fn decide(ctx: &Ctx, legal: &[GameAction], val: &Valuation) -> Option<Decision> {
    let _timing = beliefs::profile::scope("endgame");
    let own_deadline = Instant::now() + Duration::from_millis(45);
    let deadline = ctx.deadline.map_or(own_deadline, |outer| outer.min(own_deadline));
    if Instant::now() >= deadline {
        return None;
    }
    let k = ctx.k;
    let tricks_left = k.hand_len(ctx.me) as u32;
    if tricks_left == 0 || tricks_left > ctx.params.endgame_tricks {
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
    let mut slot = ctx.solver.borrow_mut();
    let solver = slot.get_or_insert_with(|| Solver::with_tt_bits(TT_BITS));
    let mut rng = Rng::new(ctx.seed);
    let mut d = {
        let model = ctx.mind.deal_model();
        evaluate_limited(
            solver,
            k,
            &table,
            objective,
            ctx.params.endgame_worlds as usize,
            model.weights(),
            &mut rng,
            Limits {
                deadline: Some(deadline),
                nodes: None,
            },
        )
    }
    .ok()?;
    // A called suit cannot become trump again; known answers without activation add no information.
    // This root policy filter does not change the engine's legal question list.
    let mate = engine::partner(ctx.me);
    d.moves.retain(|v| match v.mv {
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
    });
    let mut best = d.best()?;
    let means = |m: Move| d.moves.iter().find(|v| v.mv == m).map(|v| v.means.clone());
    let top = means(best)?;
    if matches!(best, Move::Card(_)) {
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
    let action = legal.iter().find(|a| Move::from_action(&a.action_type) == Some(best))?;
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

//! Return candidates followed through by seat-local policies, not exposed-deal minimax.
//! The engine owns every transition and score; incomplete comparisons keep C42's choice.

use std::time::{Duration, Instant};

use agent::{SeatView, Table};
use engine::{ActionType, CardSet, GameAction, Suit, SUITS};
use knowledge::Hands;

use super::{back_features, Partner, BACK_FEATURES, PASSING_WEIGHTS};
use crate::planning::{rollout, Outcome};
use crate::{fact, Ctx, Decision, Rng};

const BUDGET: Duration = Duration::from_millis(2_000);

/// Represent suit length, pair ownership and each legal returned-Ace entry.
/// C42 orders alternatives within each allocation; continuation play values them.
fn candidates(ctx: &Ctx, mut ranked: Vec<(GameAction, CardSet, f64)>, best: GameAction) -> Vec<GameAction> {
    let hand = ctx.k.certain(ctx.me);
    // Stable ordering preserves the engine's order on equal C42 scores.
    ranked.sort_by(|a, b| b.2.total_cmp(&a.2));
    let mut result = vec![best];
    for (a, _, _) in ranked.iter().take(4) {
        if !result.contains(a) {
            result.push(*a);
        }
    }
    let opening = engine::play_levels(&[], None, true);
    for suit in SUITS {
        let held = hand & CardSet::suit(suit);
        if held.is_empty() {
            continue;
        }
        // A void, a short entry suit and a long developing suit are distinct
        // continuations, even when their current standing-card counts agree.
        for kept in 0..=held.len() {
            add_candidate(&mut result, &ranked, |given| (held - given).len() == kept);
        }
        let halves = held & CardSet::HALVES;
        for kept in 0..=halves.len() {
            add_candidate(&mut result, &ranked, |given| (halves - given).len() == kept);
        }
        add_candidate(&mut result, &ranked, |given| {
            !(given & held & CardSet::ACES).is_empty()
                && !(opening.allowed(hand - given) & CardSet::suit(suit)).is_empty()
        });
    }
    result
}

fn add_candidate(
    result: &mut Vec<GameAction>,
    ranked: &[(GameAction, CardSet, f64)],
    matches: impl Fn(CardSet) -> bool,
) {
    if let Some((action, _, _)) = ranked.iter().find(|(_, given, _)| matches(*given)) {
        if !result.contains(action) {
            result.push(*action);
        }
    }
}

/// Undo visible transfers in a sampled current deal, then replay the real auction.
/// Never substitute hypothetical cards into the deciding player's observations.
fn before_return(ctx: &Ctx, hands: Hands) -> Option<Table> {
    Table::from_view(
        &SeatView {
            seat: ctx.me,
            knowledge: ctx.k,
            events: ctx.mind.events(),
            series: ctx.series,
            kinds: ctx.mind.kinds(),
        },
        hands,
    )
}

pub(super) fn choose(
    ctx: &Ctx,
    options: Vec<(GameAction, CardSet, f64)>,
    mate: &Partner,
    trump: Option<Suit>,
    baseline: Decision,
) -> Decision {
    choose_until(ctx, options, mate, trump, baseline, Instant::now() + BUDGET)
}

fn choose_until(
    ctx: &Ctx,
    options: Vec<(GameAction, CardSet, f64)>,
    mate: &Partner,
    trump: Option<Suit>,
    mut baseline: Decision,
    deadline: Instant,
) -> Decision {
    if ctx.params.return_worlds == 0 {
        return baseline;
    }
    let options = candidates(ctx, options, baseline.action);
    if options.len() == 1 {
        return baseline;
    }
    let weights = ctx.mind.deal_weights();
    let worlds = ctx
        .k
        .deals()
        .sample_worlds(&mut Rng::new(ctx.seed), ctx.params.return_worlds as usize, &weights);
    let mut params = ctx.params.clone();
    // Bounded lower-cost continuation: still the same policy and seat-local
    // inference; the small solver begins at two cards with at most eight deals.
    params.return_worlds = 0;
    params.endgame_tricks = params.endgame_tricks.min(2);
    params.endgame_worlds = params.endgame_worlds.min(8);
    let mut outcomes: Vec<Outcome> = options.iter().map(|_| Outcome::default()).collect();
    let (mut mass, mut squared, mut completed) = (0.0, 0.0, 0);
    for world in &worlds {
        if !world.weight.is_finite() || world.weight <= 0.0 {
            continue;
        }
        let Some(table) = before_return(ctx, world.hands) else {
            break;
        };
        let batch: Option<Vec<_>> = options
            .iter()
            .map(|&a| rollout(table.clone(), a, &params, deadline))
            .collect();
        let Some(batch) = batch else {
            break;
        };
        for (outcome, result) in outcomes.iter_mut().zip(&batch) {
            outcome.add(result, world.weight);
        }
        mass += world.weight;
        squared += world.weight * world.weight;
        completed += 1;
    }
    let complete =
        completed == worlds.iter().filter(|w| w.weight.is_finite() && w.weight > 0.0).count() as u32 && mass > 0.0;
    let progress = [
        fact("forecast_worlds", completed),
        fact("forecast_candidates", options.len() as u32),
        fact("forecast_complete", complete as u8),
    ];
    if !complete {
        baseline.why.extend(progress);
        return baseline;
    }
    let best = (0..options.len())
        .max_by(|&a, &b| outcomes[a].score.total_cmp(&outcomes[b].score).then(b.cmp(&a)))
        .unwrap_or(0);
    if best != 0 {
        baseline.action = options[best];
        let ActionType::Pass(cards) = baseline.action.action_type else {
            unreachable!()
        };
        let features = back_features(ctx.k.certain(ctx.me), mate, trump, CardSet::from_cards(&cards));
        baseline.why = super::explain(&PASSING_WEIGHTS.back, &BACK_FEATURES, features);
    }
    baseline.why.extend(progress);
    let chosen = &outcomes[best];
    let ActionType::Pass(cards) = baseline.action.action_type else {
        unreachable!()
    };
    let returned = CardSet::from_cards(&cards);
    let kept = ctx.k.certain(ctx.me) - returned;
    baseline.why.extend([
        fact("forecast_kept_wins", chosen.wins(kept) / mass),
        fact("forecast_kept_developed_wins", chosen.developed_wins(kept) / mass),
        fact("forecast_returned_wins", chosen.wins(returned) / mass),
        fact(
            "forecast_returned_developed_wins",
            chosen.developed_wins(returned) / mass,
        ),
        fact("forecast_target", ctx.k.value()),
        fact("forecast_ess", mass * mass / squared),
        fact("forecast_score", chosen.score / mass),
        fact("forecast_made", chosen.made / mass),
        fact("forecast_points", chosen.points / mass),
        fact("forecast_shortfall", chosen.shortfall / mass),
        fact("forecast_tricks", chosen.tricks / mass),
        fact("forecast_pair_points", chosen.pairs / mass),
        fact("forecast_last", chosen.last / mass),
        fact("forecast_opening", chosen.opening),
        fact("forecast_gain", (chosen.score - outcomes[0].score) / mass),
    ]);
    baseline
}

#[cfg(test)]
#[path = "../../tests/internal/return_forecast.rs"]
mod tests;

//! Return candidates followed through by seat-local policies, not exposed-deal minimax.
//! The engine owns every transition and score; incomplete comparisons keep C42's choice.

use std::time::Instant;

#[cfg(test)]
use agent::Table;
use beliefs::{Concept, ConceptBudget, ConceptQuery, Mind, PerspectiveBudget};
use engine::{ActionType, CardSet, GameAction, Suit, SUITS};
use observation::SeatEvent;

use super::{back_features, Partner, BACK_FEATURES, PASSING_WEIGHTS};
use crate::planning::{Forecast, BUDGET};
use crate::{fact, Ctx, Decision};

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
    add_connected_allocations(ctx, &mut result, &ranked);
    add_known_connection(ctx, &mut result, &ranked);
    result
}

/// Cover combined pair/control allocations missed by independent suit buckets.
/// At most three allocations per suit, selected by the existing C42 ordering.
/// These shapes nominate continuations; standing for the team is not a promise
/// about the next leader, pair survival or what partner will choose to do.
fn add_connected_allocations(ctx: &Ctx, result: &mut Vec<GameAction>, ranked: &[(GameAction, CardSet, f64)]) {
    let hand = ctx.k.certain(ctx.me);
    let mate = engine::partner(ctx.me);
    let theirs = ctx.k.certain(mate);
    let winners = (ctx.k.standing(ctx.me) | ctx.k.standing(mate)) - CardSet::HALVES;
    let opening = engine::play_levels(&[], None, true);
    for suit in SUITS.into_iter().filter(|&s| !ctx.k.is_called(s)) {
        let halves = CardSet::halves(suit);
        if !halves.is_subset(hand | theirs) {
            continue;
        }
        add_candidate(result, ranked, |given| {
            let kept = hand - given;
            halves.is_subset(kept) && !(kept & winners).is_empty()
        });
        add_candidate(result, ranked, |given| {
            let kept = hand - given;
            let partner_winners = (theirs | given) & winners;
            halves.is_subset(theirs | given)
                && partner_winners.iter().any(|winner| {
                    !(opening.allowed(kept) & (CardSet::suit(engine::suit_of(winner)) - CardSet::HALVES)).is_empty()
                })
        });
        add_candidate(result, ranked, |given| {
            let kept = hand - given;
            (kept & halves).len() == 1
                && ((theirs | given) & halves).len() == 1
                && !(kept & (winners - CardSet::suit(suit))).is_empty()
        });
    }
}

/// Preserve a connected announcement resource and winner that partner knows
/// about. Independent length/half buckets need not represent their conjunction.
/// This only nominates one missing allocation; completed engine outcomes decide
/// whether it is useful. Knowledge receives no additional score bonus.
fn add_known_connection(ctx: &Ctx, result: &mut Vec<GameAction>, ranked: &[(GameAction, CardSet, f64)]) {
    let mate = engine::partner(ctx.me);
    let budget = PerspectiveBudget {
        outer_nodes: 0,
        inner: ConceptBudget { max_nodes: 0 },
        max_hypotheses: 0,
    };
    let own = ctx.k.certain(ctx.me);
    let winners = ctx.k.standing(ctx.me) - CardSet::HALVES;
    let mut missing = Vec::new();
    for suit in SUITS
        .into_iter()
        .filter(|&s| !ctx.k.is_called(s) && CardSet::halves(s).is_subset(own))
    {
        for card in winners.iter() {
            let query = ConceptQuery {
                event: vec![
                    Concept::Pair { seat: ctx.me, suit }.into(),
                    Concept::Card { seat: ctx.me, card }.into(),
                ],
                given: vec![],
            };
            if ctx
                .mind
                .perspective_concept(mate, &query, budget)
                .ok()
                .and_then(|p| p.known_by_viewer)
                != Some(true)
            {
                continue;
            }
            let preserved = |given| {
                let mut after = *ctx.k;
                after
                    .apply(&SeatEvent::Pass {
                        from: ctx.me,
                        to: mate,
                        cards: Some(given),
                    })
                    .is_ok()
                    && CardSet::halves(suit).is_subset(after.certain(ctx.me))
                    && after.standing(ctx.me).contains(card)
            };
            if !ranked
                .iter()
                .any(|(a, given, _)| result.contains(a) && preserved(*given))
            {
                missing.push((query, suit, card));
            }
        }
    }
    if missing.is_empty() {
        return;
    }
    for (action, given, _) in ranked.iter().filter(|(a, _, _)| !result.contains(a)) {
        let event = SeatEvent::Pass {
            from: ctx.me,
            to: mate,
            cards: Some(*given),
        };
        let mut after = *ctx.k;
        if after.apply(&event).is_err() {
            continue;
        }
        let relevant: Vec<_> = missing
            .iter()
            .filter(|(_, suit, card)| {
                CardSet::halves(*suit).is_subset(after.certain(ctx.me)) && after.standing(ctx.me).contains(*card)
            })
            .collect();
        if relevant.is_empty() {
            continue;
        }
        let mut model = Mind::new(
            ctx.me,
            ctx.k.dealt_certain(ctx.me),
            ctx.mind.kinds(),
            ctx.mind.params().clone(),
        );
        if ctx
            .mind
            .events()
            .iter()
            .chain(std::iter::once(&event))
            .any(|e| model.apply(e).is_err())
        {
            continue;
        }
        if relevant.iter().any(|(query, _, _)| {
            model
                .perspective_concept(mate, query, budget)
                .ok()
                .and_then(|p| p.known_by_viewer)
                == Some(true)
        }) {
            result.push(*action);
            return;
        }
    }
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
    let incomplete = |mut baseline: Decision| {
        baseline.why.extend([
            fact("forecast_candidates", options.len() as u32),
            fact("forecast_complete", 0),
        ]);
        baseline
    };
    let Some(forecast) = Forecast::new(ctx, ctx.params.return_worlds as usize, deadline) else {
        return incomplete(baseline);
    };
    let Some(compared) = forecast.compare(&options) else {
        baseline.why.extend(forecast.work_facts());
        return incomplete(baseline);
    };
    let best = compared.best();
    if best != 0 {
        baseline.action = options[best];
        let ActionType::Pass(cards) = baseline.action.action_type else {
            unreachable!()
        };
        let features = back_features(ctx.k.certain(ctx.me), mate, trump, CardSet::from_cards(&cards));
        baseline.why = super::explain(&PASSING_WEIGHTS.back, &BACK_FEATURES, features);
    }
    baseline.why.extend(forecast.facts(&compared, best));
    let chosen = &compared.means[best];
    let ActionType::Pass(cards) = baseline.action.action_type else {
        unreachable!()
    };
    let returned = CardSet::from_cards(&cards);
    let kept = ctx.k.certain(ctx.me) - returned;
    baseline.why.extend([
        fact("forecast_kept_wins", chosen.wins(kept)),
        fact("forecast_kept_developed_wins", chosen.developed_wins(kept)),
        fact("forecast_returned_wins", chosen.wins(returned)),
        fact("forecast_returned_developed_wins", chosen.developed_wins(returned)),
        fact("forecast_target", ctx.k.value()),
        fact("forecast_opening", chosen.opening),
    ]);
    baseline
}

#[cfg(test)]
#[path = "../../tests/internal/return_forecast.rs"]
mod tests;

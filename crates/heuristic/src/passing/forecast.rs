//! Return candidates followed through by seat-local policies, not exposed-deal minimax.
//! The engine owns every transition and score; incomplete comparisons keep C42's choice.

use std::time::{Duration, Instant};

use agent::Table;
use engine::{ActionType, CardSet, Clock, GameAction, GameFinishedInfo, PlaceAtTable, Suit, SUITS};
use knowledge::Hands;
use observation::SeatEvent;

use super::{back_features, Partner, BACK_FEATURES, PASSING_WEIGHTS};
use crate::{fact, Ctx, Decision, HeuristicAgent, Params, Rng};

const BUDGET: Duration = Duration::from_millis(2_000);

#[derive(Default)]
struct Outcome {
    score: f64,
    points: f64,
    made: f64,
    shortfall: f64,
    tricks: f64,
    pairs: f64,
    last: f64,
    opening: u8,
}

impl Outcome {
    fn add(&mut self, other: &Self, weight: f64) {
        self.score += weight * other.score;
        self.points += weight * other.points;
        self.made += weight * other.made;
        self.shortfall += weight * other.shortfall;
        self.tricks += weight * other.tricks;
        self.pairs += weight * other.pairs;
        self.last += weight * other.last;
        self.opening = other.opening;
    }
}

/// C42's leading choices, each held half/pair allocation, and a legal opening
/// toward a returned ace. Pure card-play routes remain represented without pairs.
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
    for suit in SUITS {
        let halves = hand & CardSet::halves(suit);
        if halves.is_empty() {
            continue;
        }
        for kept in 0..=halves.len() {
            if let Some((a, _, _)) = ranked.iter().find(|(_, given, _)| (halves - *given).len() == kept) {
                if !result.contains(a) {
                    result.push(*a);
                }
            }
        }
    }
    let opening = engine::play_levels(&[], None, true);
    if let Some((a, _, _)) = ranked.iter().find(|(_, given, _)| {
        let allowed = opening.allowed(hand - *given);
        (*given & CardSet::ACES)
            .iter()
            .any(|ace| !(allowed & CardSet::suit(engine::suit_of(ace))).is_empty())
    }) {
        if !result.contains(a) {
            result.push(*a);
        }
    }
    result
}

/// Undo visible transfers in a sampled current deal, then replay the real auction.
/// Never substitute hypothetical cards into the deciding player's observations.
fn before_return(ctx: &Ctx, mut hands: Hands) -> Option<Table> {
    for event in ctx.mind.events().iter().rev() {
        if let SeatEvent::Pass { from, to, cards } = event {
            let cards = (*cards)?;
            if !cards.is_subset(hands[*to as usize]) {
                return None;
            }
            hands[*to as usize] -= cards;
            hands[*from as usize] |= cards;
        }
    }
    if hands.iter().any(|h| h.len() != engine::HAND_SIZE as u32) {
        return None;
    }
    let mut table = Table::new(&hands.map(|h| h.cards()));
    table.game.clock = Clock::Off;
    table.kinds = ctx.mind.kinds();
    for event in ctx.mind.events() {
        let (seat, action_type) = match *event {
            SeatEvent::Bid { seat, value } => (seat, ActionType::NewBid(value)),
            SeatEvent::StopBidding { seat } => (seat, ActionType::StopBidding),
            SeatEvent::Pass {
                from,
                cards: Some(cards),
                ..
            } => (from, ActionType::Pass(cards.cards().try_into().ok()?)),
            _ => return None,
        };
        let requested = GameAction {
            player: PlaceAtTable(seat),
            action_type,
        };
        let action = *table.to_act()?.1.iter().find(|a| {
            a.player == requested.player
                && match (a.action_type, requested.action_type) {
                    (ActionType::Pass(a), ActionType::Pass(b)) => CardSet::from_cards(&a) == CardSet::from_cards(&b),
                    (a, b) => a == b,
                }
        })?;
        table.apply(action);
    }
    // A sampled reconstruction must reproduce the entire acting seat's view.
    let view = table.view(ctx.me);
    if view.events != ctx.mind.events() || view.knowledge != ctx.k {
        return None;
    }
    Some(table)
}

fn rollout(mut table: Table, action: GameAction, params: &Params, deadline: Instant) -> Option<Outcome> {
    let team = action.player.0 as usize % 2;
    table.apply(action);
    let mut players: [HeuristicAgent; 4] = std::array::from_fn(|_| HeuristicAgent::new(params.clone()));
    let mut opening = None;
    // Negative questions are suppressed by the existing policy. The cap is a
    // defensive bound, not a terminal heuristic: unfinished games do not count.
    for _ in 0..128 {
        let Some((seat, legal)) = table.to_act() else {
            break;
        };
        if Instant::now() >= deadline {
            return None;
        }
        let decision = players[seat as usize].decide(&table.view(seat), legal);
        if !legal.contains(&decision.action) {
            return None;
        }
        if let ActionType::CardPlayed(c) = decision.action.action_type {
            opening.get_or_insert(c.index());
        }
        table.apply(decision.action);
    }
    if !table.game.ended() {
        return None;
    }
    let info = GameFinishedInfo::from(table.game);
    let scores = info.scores();
    let outcome = info.outcome();
    let card_points: i32 = info
        .tricks
        .iter()
        .filter(|t| t.winner.0 as usize % 2 == team)
        .map(|t| t.points.0)
        .sum();
    Some(Outcome {
        score: (scores[team] - scores[1 - team]) as f64,
        points: info.team_points[team] as f64,
        made: f64::from(info.team_points[team] >= info.game_value.0),
        shortfall: (info.game_value.0 - info.team_points[team]).max(0) as f64,
        tricks: outcome.team_tricks[team] as f64,
        pairs: (info.team_points[team] - card_points) as f64,
        last: f64::from(info.tricks.last()?.winner.0 as usize % 2 == team),
        opening: opening?,
    })
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
    baseline.why.extend([
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

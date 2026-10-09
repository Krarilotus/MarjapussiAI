//! Conditional whole-game outcomes under the existing seat-local policy.
//! Each forecast follows one coherent legal continuation per weighted deal.
//! Its realized entries, promotions, announcements and last trick are not
//! independent bonuses: the engine's final score still selects the plan.

use std::time::Instant;

use agent::Table;
use engine::{ActionType, CardSet, GameAction, GameFinishedInfo};

use crate::{HeuristicAgent, Params};

pub(crate) struct Outcome {
    pub score: f64,
    pub points: f64,
    pub made: f64,
    pub shortfall: f64,
    pub tricks: f64,
    pub pairs: f64,
    pub last: f64,
    pub opening: u8,
    winners: [f64; 36],
    developed: [f64; 36],
}

impl Default for Outcome {
    fn default() -> Self {
        Self {
            score: 0.0,
            points: 0.0,
            made: 0.0,
            shortfall: 0.0,
            tricks: 0.0,
            pairs: 0.0,
            last: 0.0,
            opening: 0,
            winners: [0.0; 36],
            developed: [0.0; 36],
        }
    }
}

impl Outcome {
    /// Expected tricks won by these allocated cards, before normalizing sample mass.
    pub fn wins(&self, cards: CardSet) -> f64 {
        cards.iter().map(|c| self.winners[c as usize]).sum()
    }

    /// Wins after all initially external higher ranks fell in earlier tricks.
    /// This is realized promotion in a policy continuation, not a guarantee that
    /// a blocked card will win or that its suit is safe from a later trump change.
    pub fn developed_wins(&self, cards: CardSet) -> f64 {
        cards.iter().map(|c| self.developed[c as usize]).sum()
    }

    pub(crate) fn add(&mut self, other: &Self, weight: f64) {
        self.score += weight * other.score;
        self.points += weight * other.points;
        self.made += weight * other.made;
        self.shortfall += weight * other.shortfall;
        self.tricks += weight * other.tricks;
        self.pairs += weight * other.pairs;
        self.last += weight * other.last;
        self.opening = other.opening;
        for (mine, theirs) in self.winners.iter_mut().zip(other.winners) {
            *mine += weight * theirs;
        }
        for (mine, theirs) in self.developed.iter_mut().zip(other.developed) {
            *mine += weight * theirs;
        }
    }
}

/// Finish a candidate return using only each simulated actor's own view.
/// Callers disable recursive return search and supply the comparison deadline.
pub(crate) fn rollout(mut table: Table, action: GameAction, params: &Params, deadline: Instant) -> Option<Outcome> {
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
    let initial = info.after_passing.as_ref()?;
    let mut played = CardSet::EMPTY;
    let mut winners = [0.0; 36];
    let mut developed = [0.0; 36];
    for trick in &info.tricks {
        if trick.winner.0 as usize % 2 == team {
            let i = (trick.winner.0 + 4 - trick.leader.0) as usize % 4;
            let c = trick.cards[i].index();
            let blockers = crate::hand::higher(c) - CardSet::from_cards(&initial[trick.winner.0 as usize]);
            winners[c as usize] = 1.0;
            developed[c as usize] = f64::from(!blockers.is_empty() && blockers.is_subset(played));
        }
        played |= CardSet::from_cards(&trick.cards);
    }
    Some(Outcome {
        score: (scores[team] - scores[1 - team]) as f64,
        points: info.team_points[team] as f64,
        made: f64::from(info.team_points[team] >= info.game_value.0),
        shortfall: (info.game_value.0 - info.team_points[team]).max(0) as f64,
        tricks: outcome.team_tricks[team] as f64,
        pairs: (info.team_points[team] - card_points) as f64,
        last: f64::from(info.tricks.last()?.winner.0 as usize % 2 == team),
        opening: opening?,
        winners,
        developed,
    })
}

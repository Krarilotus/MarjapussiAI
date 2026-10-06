//! Scoring, single owner: the score of one game and the standings of a series.
//!
//! Sources: the rules page (marjapussi.de/rules, "Spielbewertung") and the site's series table
//! (`marjapussi-online`, `SeriesTable.vue`).
//! - The playing party (S) scores `+value` if its points (cards + announced pairs) reach the
//!   game value, else `-value`.
//! - The other party (N) scores its points rounded to a multiple of 5 (137 → 135, 138 → 140).
//! - Schwarz: if N took no trick and S made its contract, N scores
//!   `-SCHWARZ_FACTOR_FIFTHS / 5 × value` (= −2 × value) instead.
//! - No contract (all passed): each team scores its rounded points (as on the site).
//! - Series: by a fixed number of games (tournament round) or until a team reaches a target.
//!   After a tournament round: ±1/5 of the point difference to the other team, and +300
//!   (other team −300) for the team that first reached 500 (`SeriesSettings`).

use crate::game::gameevent::{GameCallback, GameEvent};
use crate::game::gamestate::FinishedTrick;
use crate::game::points::PAIR_POINTS;

/// Running points per team (team 0 = seats 0 and 2) at any state of a game.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TeamPoints {
    /// Card points of the tricks won, including the last-trick bonus.
    pub cards: [i32; 2],
    /// Announced pairs: every `NewTrump` callback, credited to the team of the player whose
    /// action caused it (announcement or answer).
    pub pairs: [i32; 2],
    pub tricks: [u8; 2],
}

impl TeamPoints {
    /// Card points plus pair points.
    pub fn total(&self) -> [i32; 2] {
        [self.cards[0] + self.pairs[0], self.cards[1] + self.pairs[1]]
    }
}

/// Running team points from a game's finished tricks and its events (`game.state.all_tricks`,
/// `game.all_events`); the one place that adds them up.
pub fn team_points(tricks: &[FinishedTrick], events: &[GameEvent]) -> TeamPoints {
    let mut p = TeamPoints::default();
    for t in tricks {
        let team = (t.winner.0 % 2) as usize;
        p.cards[team] += t.points.0;
        p.tricks[team] += 1;
    }
    for e in events {
        if let Some(GameCallback::NewTrump(suit)) = e.callback {
            p.pairs[(e.last_action.player.0 % 2) as usize] += PAIR_POINTS[suit as usize];
        }
    }
    p
}

/// Schwarz penalty in fifths of the game value (10/5 = 2×); the one owner of this factor.
pub const SCHWARZ_FACTOR_FIFTHS: i32 = 10;

/// Points rounded to a multiple of 5, halves up (137 → 135, 138 → 140).
pub fn round5(points: i32) -> i32 {
    (points + 2).div_euclid(5) * 5
}

/// What scoring needs from a finished game (team 0 = seats 0 and 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameOutcome {
    /// The team that played the contract; `None` if all passed.
    pub playing_team: Option<u8>,
    pub game_value: i32,
    /// Card points (with the last-trick bonus) plus announced pairs, per team.
    pub team_points: [i32; 2],
    pub team_tricks: [u8; 2],
}

/// Scores of one game per team, with the schwarz factor in fifths of the game value.
pub fn game_scores(o: &GameOutcome, schwarz_factor_fifths: i32) -> [i32; 2] {
    let Some(s) = o.playing_team.map(|t| (t % 2) as usize) else {
        return o.team_points.map(round5);
    };
    let n = 1 - s;
    let made = o.team_points[s] >= o.game_value;
    let mut scores = [0; 2];
    scores[s] = if made { o.game_value } else { -o.game_value };
    scores[n] = if made && o.team_tricks[n] == 0 {
        -schwarz_factor_fifths * o.game_value / 5
    } else {
        round5(o.team_points[n])
    };
    scores
}

/// How long a series runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesLength {
    /// A tournament round of this many games; the end bonuses apply.
    Games(u32),
    /// Until a team's total reaches this many points ("bis 500"); no end bonuses.
    UntilPoints(i32),
}

/// Series rules; defaults are the site's (`marjapussi-online` series settings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeriesSettings {
    pub length: SeriesLength,
    pub schwarzfactor_fifths: i32,
    pub shuffle_players: bool,
    /// The first team to reach this total gets the bonus.
    pub bonus_at: i32,
    /// Bonus for that team; the other team loses as much.
    pub bonus_value: i32,
    /// The leading team gets `difference / diff_divisor`, the other team loses as much.
    pub diff_divisor: i32,
}

impl Default for SeriesSettings {
    fn default() -> Self {
        SeriesSettings {
            length: SeriesLength::Games(4),
            schwarzfactor_fifths: SCHWARZ_FACTOR_FIFTHS,
            shuffle_players: true,
            bonus_at: 500,
            bonus_value: 300,
            diff_divisor: 5,
        }
    }
}

/// Running totals of a series.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeriesTally {
    pub settings: SeriesSettings,
    totals: [i32; 2],
    games: u32,
    bonus_team: Option<u8>,
}

/// The final standings of a series, by part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeriesResult {
    /// Sum of the game scores.
    pub games: [i32; 2],
    /// ± difference / divisor (tournament rounds only).
    pub difference: [i32; 2],
    /// ± bonus for the first team to reach `bonus_at` (tournament rounds only).
    pub bonus: [i32; 2],
    pub total: [i32; 2],
}

impl SeriesTally {
    pub fn new(settings: SeriesSettings) -> SeriesTally {
        SeriesTally {
            settings,
            totals: [0; 2],
            games: 0,
            bonus_team: None,
        }
    }

    /// Adds one game's outcome.
    pub fn add(&mut self, outcome: &GameOutcome) {
        self.add_scores(game_scores(outcome, self.settings.schwarzfactor_fifths));
    }

    /// Adds one game's scores (as from `game_scores`).
    pub fn add_scores(&mut self, scores: [i32; 2]) {
        self.totals[0] += scores[0];
        self.totals[1] += scores[1];
        self.games += 1;
        if self.bonus_team.is_none() {
            let reached = self.totals.map(|t| t >= self.settings.bonus_at);
            self.bonus_team = match reached {
                [false, false] => None,
                [true, false] => Some(0),
                [false, true] => Some(1),
                // Both in the same game: the higher total; on a tie the site gives it to team 1.
                [true, true] => Some(if self.totals[0] > self.totals[1] { 0 } else { 1 }),
            };
        }
    }

    /// Sums of the game scores so far.
    pub fn totals(&self) -> [i32; 2] {
        self.totals
    }

    pub fn games_played(&self) -> u32 {
        self.games
    }

    /// Games left in a tournament round; `None` when playing to a target.
    pub fn games_left(&self) -> Option<u32> {
        match self.settings.length {
            SeriesLength::Games(n) => Some(n.saturating_sub(self.games)),
            SeriesLength::UntilPoints(_) => None,
        }
    }

    /// The team that first reached `bonus_at`.
    pub fn bonus_team(&self) -> Option<u8> {
        self.bonus_team
    }

    pub fn is_over(&self) -> bool {
        match self.settings.length {
            SeriesLength::Games(n) => self.games >= n,
            SeriesLength::UntilPoints(p) => self.totals.iter().any(|&t| t >= p),
        }
    }

    /// The standings with the end-of-round bonuses (tournament rounds; none when playing to a
    /// target).
    pub fn result(&self) -> SeriesResult {
        let games = self.totals;
        let (mut difference, mut bonus) = ([0; 2], [0; 2]);
        if let SeriesLength::Games(_) = self.settings.length {
            let d = (games[0] - games[1]).abs() / self.settings.diff_divisor;
            if games[0] != games[1] {
                let lead = if games[0] > games[1] { 0 } else { 1 };
                difference[lead] = d;
                difference[1 - lead] = -d;
            }
            if let Some(b) = self.bonus_team {
                bonus[b as usize] = self.settings.bonus_value;
                bonus[1 - b as usize] = -self.settings.bonus_value;
            }
        }
        let total = [0, 1].map(|t| games[t] + difference[t] + bonus[t]);
        SeriesResult {
            games,
            difference,
            bonus,
            total,
        }
    }
}

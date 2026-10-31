//! The series objective (HS-1, [D]): what a game's outcome is worth in the round it is played
//! in. A tournament round (the site's four games, `engine::SeriesSettings`, the rules' owner)
//! adds ±1/5 of the point difference and ±300 for the first team to reach 500. Decisions that
//! weigh a game's outcomes read their worth here: the expected final series margin (ours −
//! theirs) after the game, up to a constant shared by every outcome of it.
//!
//! The last game is scored exactly by `SeriesTally`. Before it, the rest of the round is a race:
//! each remaining game's scores are drawn from the measured self-play distribution
//! (`GAME_SCORES`), symmetric between the teams, so later games add nothing to the expected
//! margin except through the bonus, whose odds a dynamic program over the totals gives
//! (`BonusOdds`). Outside a tournament round the worth is the game's score difference.

use agent::SeriesScore;
use engine::{SeriesLength, SeriesSettings, SeriesTally};
use std::sync::OnceLock;

/// One game's engine scores (team A, team B), rounded to 20 points, and how often they occurred:
/// `base` against itself, 2,500 deals in 625 series each played twice (deal seed 1), both team
/// orders counted. Regenerate with `scripts/marja_bench/series_table.py RUN_DIR` from such a run.
#[rustfmt::skip]
const GAME_SCORES: [(i16, i16, u16); 213] = [
    (-280, 140, 460), (140, -280, 460), (40, 140, 444), (140, 40, 444), (20, 140, 404), (140, 20, 404),
    (-140, 80, 324), (80, -140, 324), (-140, 60, 208), (60, -140, 208), (-140, 40, 198), (40, -140, 198),
    (60, 140, 182), (140, 60, 182), (40, 160, 178), (160, 40, 178), (-140, 160, 174), (160, -140, 174),
    (-140, 120, 160), (120, -140, 160), (20, 160, 152), (160, 20, 152), (-140, 100, 146), (100, -140, 146),
    (80, 140, 138), (140, 80, 138), (140, 140, 132), (80, 160, 108), (160, 80, 108), (-140, 180, 90),
    (120, 140, 90), (140, 120, 90), (180, -140, 90), (-140, 200, 88), (200, -140, 88), (-140, 140, 80),
    (140, -140, 80), (140, 160, 78), (160, 140, 78), (60, 160, 72), (160, 60, 72), (100, 140, 64), (140, 100, 64),
    (160, 160, 56), (-300, 160, 54), (160, -300, 54), (-320, 160, 52), (160, -320, 52), (-140, 20, 48),
    (20, -140, 48), (120, 160, 44), (160, 120, 44), (-140, 220, 42), (220, -140, 42), (0, 140, 40), (140, 0, 40),
    (100, 160, 36), (160, 100, 36), (20, 180, 34), (180, 20, 34), (140, 180, 32), (180, 140, 32), (-160, 160, 28),
    (160, -160, 28), (-140, 240, 26), (0, 160, 26), (40, 120, 26), (120, 40, 26), (160, 0, 26), (240, -140, 26),
    (-360, 180, 24), (-140, 280, 24), (-120, 100, 24), (100, -120, 24), (180, -360, 24), (280, -140, 24),
    (40, 180, 22), (180, 40, 22), (-260, 120, 20), (-160, 80, 20), (-140, 0, 20), (-120, 40, 20), (0, -140, 20),
    (20, 120, 20), (40, -120, 20), (80, -160, 20), (120, -260, 20), (120, 20, 20), (160, 180, 20), (180, 160, 20),
    (-120, 80, 18), (60, 120, 18), (80, -120, 18), (120, 60, 18), (-160, 200, 16), (200, -160, 16),
    (-400, 200, 14), (-120, 120, 14), (120, -120, 14), (200, -400, 14), (-160, 120, 12), (-160, 140, 12),
    (-120, 60, 12), (60, -120, 12), (60, 180, 12), (80, 180, 12), (120, -160, 12), (120, 120, 12), (140, -160, 12),
    (180, 60, 12), (180, 80, 12), (-120, 200, 10), (200, -120, 10), (-480, 240, 8), (-440, 220, 8), (-340, 160, 8),
    (-160, 100, 8), (-120, 20, 8), (20, -120, 8), (100, -160, 8), (160, -340, 8), (220, -440, 8), (240, -480, 8),
    (-280, 180, 6), (-280, 200, 6), (-280, 240, 6), (-180, 80, 6), (-160, 40, 6), (-160, 220, 6), (-120, 160, 6),
    (0, 120, 6), (20, 200, 6), (40, -160, 6), (80, -180, 6), (80, 120, 6), (120, 0, 6), (120, 80, 6),
    (140, 200, 6), (160, -120, 6), (160, 200, 6), (180, -280, 6), (200, -280, 6), (200, 20, 6), (200, 140, 6),
    (200, 160, 6), (220, -160, 6), (240, -280, 6), (-380, 200, 4), (-320, 140, 4), (-280, 220, 4), (-260, 140, 4),
    (-160, 180, 4), (-160, 240, 4), (-140, 260, 4), (-140, 300, 4), (-140, 320, 4), (-120, 180, 4), (0, 180, 4),
    (40, 200, 4), (80, 200, 4), (100, 120, 4), (100, 200, 4), (120, 100, 4), (140, -320, 4), (140, -260, 4),
    (140, 220, 4), (180, -160, 4), (180, -120, 4), (180, 0, 4), (200, -380, 4), (200, 40, 4), (200, 80, 4),
    (200, 100, 4), (220, -280, 4), (220, 140, 4), (240, -160, 4), (260, -140, 4), (300, -140, 4), (320, -140, 4),
    (-640, 320, 2), (-600, 300, 2), (-360, 220, 2), (-320, 180, 2), (-320, 280, 2), (-300, 140, 2), (-240, 80, 2),
    (-180, 140, 2), (-180, 160, 2), (-180, 180, 2), (-160, 60, 2), (-120, 0, 2), (-120, 140, 2), (0, -120, 2),
    (20, 220, 2), (60, -160, 2), (60, 200, 2), (80, -240, 2), (120, 180, 2), (140, -300, 2), (140, -180, 2),
    (140, -120, 2), (160, -180, 2), (160, 220, 2), (180, -320, 2), (180, -180, 2), (180, 120, 2), (200, 60, 2),
    (220, -360, 2), (220, 20, 2), (220, 160, 2), (280, -320, 2), (300, -600, 2), (320, -640, 2),
];

/// The bonus race's grid of totals: from `GRID_MIN` in `GRID_STEP` steps up to the bonus total.
const GRID_STEP: i32 = 20;
const GRID_MIN: i32 = -1200;
/// The most games left after the current one in a four-game round.
const MAX_LATER: usize = 3;

/// P(each physical team is the first to reach the bonus total) with `later` games still to play
/// from totals on the grid, both below the bonus total and the bonus undecided.
struct BonusOdds {
    side: usize,
    odds: Vec<[f64; 2]>,
}

/// Builds the bonus race's table now (about 60 ms once per process), so no timed decision pays
/// for it.
pub(crate) fn prepare() {
    BonusOdds::get();
}

impl BonusOdds {
    fn get() -> &'static BonusOdds {
        static ODDS: OnceLock<BonusOdds> = OnceLock::new();
        ODDS.get_or_init(BonusOdds::solve)
    }

    fn solve() -> BonusOdds {
        let settings = SeriesSettings::default();
        let side = ((settings.bonus_at - GRID_MIN) / GRID_STEP) as usize;
        let total: f64 = GAME_SCORES.iter().map(|&(_, _, n)| n as f64).sum();
        let mut odds = vec![[0.0; 2]; (MAX_LATER + 1) * side * side];
        for later in 1..=MAX_LATER {
            for i in 0..side {
                for j in 0..side {
                    let totals = [GRID_MIN + GRID_STEP * i as i32, GRID_MIN + GRID_STEP * j as i32];
                    let mut p = [0.0; 2];
                    for &(a, b, n) in &GAME_SCORES {
                        let w = n as f64 / total;
                        let mut tally = SeriesTally::resume(settings, totals, 0, None);
                        tally.add_scores([a as i32, b as i32]);
                        let after = match tally.bonus_team() {
                            Some(t) => [(t == 0) as u8 as f64, (t == 1) as u8 as f64],
                            None => {
                                let cell = |x: i32| (((x - GRID_MIN) / GRID_STEP).max(0) as usize).min(side - 1);
                                let t = tally.totals();
                                odds[((later - 1) * side + cell(t[0])) * side + cell(t[1])]
                            }
                        };
                        p[0] += w * after[0];
                        p[1] += w * after[1];
                    }
                    odds[(later * side + i) * side + j] = p;
                }
            }
        }
        BonusOdds { side, odds }
    }

    /// The odds from `totals` (physical teams), bilinear between the grid's totals.
    fn at(&self, later: usize, totals: [i32; 2]) -> [f64; 2] {
        let later = later.min(MAX_LATER);
        let pos = |x: i32| ((x - GRID_MIN).max(0) as f64 / GRID_STEP as f64).min((self.side - 1) as f64);
        let (x, y) = (pos(totals[0]), pos(totals[1]));
        let (i, j) = (x as usize, y as usize);
        let (fx, fy) = (x - i as f64, y - j as f64);
        let cell = |i: usize, j: usize| {
            self.odds[(later * self.side + i.min(self.side - 1)) * self.side + j.min(self.side - 1)]
        };
        let mut out = [0.0; 2];
        for t in 0..2 {
            out[t] = (1.0 - fx) * (1.0 - fy) * cell(i, j)[t]
                + fx * (1.0 - fy) * cell(i + 1, j)[t]
                + (1.0 - fx) * fy * cell(i, j + 1)[t]
                + fx * fy * cell(i + 1, j + 1)[t];
        }
        out
    }
}

/// My team's stakes in the current game's outcome, from the public series score.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SeriesStakes {
    /// My team (seat % 2).
    us: u8,
    /// The round's standing before this game; `None` outside a tournament round.
    round: Option<SeriesScore>,
}

impl SeriesStakes {
    pub fn new(series: SeriesScore, me: u8) -> Self {
        SeriesStakes {
            us: me % 2,
            round: series.games_left.filter(|&n| n > 0).map(|_| series),
        }
    }

    /// `worth` of the game ending with `ours` for my team and `theirs` for the other team.
    pub fn worth_of(&self, ours: i32, theirs: i32) -> f64 {
        let mut scores = [ours, theirs];
        if self.us == 1 {
            scores.swap(0, 1);
        }
        self.worth(scores)
    }

    /// The expected final series margin (ours − theirs) when this game ends with `scores`
    /// (engine scores by physical team), up to a constant shared by all its outcomes.
    pub fn worth(&self, scores: [i32; 2]) -> f64 {
        let (us, them) = (self.us as usize, 1 - self.us as usize);
        let Some(round) = self.round else {
            return (scores[us] - scores[them]) as f64;
        };
        let left = round.games_left.unwrap_or(1);
        let settings = SeriesSettings {
            length: SeriesLength::Games(left),
            ..SeriesSettings::default()
        };
        let mut tally = SeriesTally::resume(settings, round.team_totals, 0, round.bonus_team);
        tally.add_scores(scores);
        if tally.is_over() {
            let r = tally.result();
            return (r.total[us] - r.total[them]) as f64;
        }
        // Later games are symmetric: the totals stand, scored by the round's rules at its end,
        // with the bonus as the race decides it.
        let t = tally.totals();
        let end = |bonus: Option<u8>| {
            let r = SeriesTally::resume(settings, t, 0, bonus).result();
            (r.total[us] - r.total[them]) as f64
        };
        match tally.bonus_team() {
            Some(b) => end(Some(b)),
            None => {
                let odds = BonusOdds::get().at(left as usize - 1, t);
                odds[0] * end(Some(0)) + odds[1] * end(Some(1)) + (1.0 - odds[0] - odds[1]) * end(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(totals: [i32; 2], games_left: u32) -> SeriesStakes {
        SeriesStakes::new(
            SeriesScore {
                team_totals: totals,
                games_left: Some(games_left),
                bonus_team: None,
            },
            0,
        )
    }

    #[test]
    fn outside_a_round_the_worth_is_the_game_difference() {
        let s = SeriesStakes::new(SeriesScore::default(), 1);
        assert_eq!(s.worth([140, 35]), -105.0);
    }

    #[test]
    fn the_last_game_is_scored_by_the_series_rules() {
        // 380 + 140 reaches 500 first: +300 and the fifth of the difference, against them.
        let s = round([380, 300], 1);
        let made = s.worth([140, 40]);
        let failed = s.worth([-140, 40]);
        assert_eq!(made, (520 + 300 + 36) as f64 - (340 - 300 - 36) as f64);
        assert_eq!(failed, (240 - 20) as f64 - (340 + 20) as f64);
        assert!(made - failed > 2.0 * 140.0 * 1.4 + 500.0);
    }

    #[test]
    fn the_bonus_race_favours_the_team_nearer_the_total() {
        let odds = BonusOdds::get();
        let near = odds.at(2, [440, 0]);
        let far = odds.at(2, [0, 0]);
        assert!(near[0] > far[0] && near[0] > near[1]);
        assert!((far[0] - far[1]).abs() < 0.05, "symmetric scores: {far:?}");
        assert!(odds.at(0, [480, 480]) == [0.0, 0.0]);
        // Earlier, a bonus already decided is worth its full value.
        let decided = SeriesStakes::new(
            SeriesScore {
                team_totals: [300, 520],
                games_left: Some(3),
                bonus_team: Some(1),
            },
            0,
        );
        assert_eq!(decided.worth([0, 0]) - decided.worth([20, 0]), -20.0 * 1.4);
    }
}

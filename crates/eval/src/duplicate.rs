//! Duplicate deals: two contestants A and B play each deal from both sides, so the luck of the
//! cards cancels and only the play differs.
//!
//! A *deal pair* is one deal played twice: A on seats 0/2 (B on 1/3), then A on seats 1/3 with
//! the same hands per seat. Its score is A's score minus B's score, summed over the two games
//! (engine scoring). With `Rotation::TeamsAndSeats` the deal is also played with the hands
//! turned by two seats (the other partner opens the auction); the deal's score is then the mean
//! of its two pairs, so the unit stays "points per deal pair".

use agent::Agent;
use engine::Card;
use engine::GameFinishedInfo;
use engine::PAIR_POINTS;
use engine::{ActionType, GameCallback};
use observation::Seat;

use crate::stats::{conf_seq, mean_ci, rate};
use crate::suite::DealSuite;
use crate::{play_game, Logged};

/// One side of a match: a name for the report and a factory for its agent at a seat (a fresh
/// agent per game).
pub struct Contestant<'a> {
    pub name: String,
    pub make: Box<dyn Fn(Seat) -> Box<dyn Agent> + 'a>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    /// Each deal twice: A on team 0, then on team 1.
    Teams,
    /// Also with the hands turned by two seats: four games per deal.
    TeamsAndSeats,
}

/// Stop early once the confidence sequence decides (checked every `every` deals from
/// `min_deals` on).
#[derive(Clone, Copy, Debug)]
pub struct Sequential {
    pub alpha: f64,
    /// Stop as "no meaningful difference" once the whole interval lies within ±margin.
    pub margin: f64,
    pub min_deals: u64,
    pub every: u64,
}

impl Default for Sequential {
    fn default() -> Self {
        Sequential {
            alpha: 0.05,
            margin: 5.0,
            min_deals: 100,
            every: 50,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub suite: DealSuite,
    pub rotation: Rotation,
    pub sequential: Option<Sequential>,
}

/// One finished game of a match, for hooks (logs, per-rule counts).
pub struct Played<'g> {
    pub deal: u64,
    /// The team (0 = seats 0/2) A played.
    pub a_team: u8,
    /// Hands turned by two seats (`Rotation::TeamsAndSeats`).
    pub turned: bool,
    pub info: &'g GameFinishedInfo,
    pub log: &'g [Logged],
}

/// Per-contestant totals over all games.
#[derive(Clone, Debug, Default)]
pub struct Side {
    pub contracts: u64,
    pub made: u64,
    pub value_sum: i64,
    pub questions: u64,
    pub melds: u64,
    pub pair_points: i64,
    pub score_sum: i64,
    /// Intention detectors (`detectors.rs`): A/10 thrown onto the partner's trick while
    /// standing; halves of a needed pair led while a question was legal.
    pub schmiered_standing: u64,
    pub needed_half_played: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    AllDeals,
    ABetter,
    BBetter,
    WithinMargin,
}

#[derive(Clone, Debug)]
pub struct MatchResult {
    pub a: String,
    pub b: String,
    pub suite: DealSuite,
    pub rotation: Rotation,
    pub sequential: Option<Sequential>,
    /// Score difference A − B per deal (pair).
    pub diffs: Vec<f64>,
    pub games: u64,
    pub pass_games: u64,
    pub schwarz: u64,
    /// [A, B].
    pub sides: [Side; 2],
    pub stop: Stop,
}

/// Hands for the seats, turned by `turn` seats (seat s gets hand s + turn).
fn turned(hands: &[Vec<Card>; 4], turn: usize) -> [Vec<Card>; 4] {
    std::array::from_fn(|s| hands[(s + turn) % 4].clone())
}

/// Plays the match. `hook` sees every finished game.
pub fn run(a: &Contestant, b: &Contestant, cfg: &Config, mut hook: impl FnMut(&Played)) -> MatchResult {
    let mut r = MatchResult {
        a: a.name.clone(),
        b: b.name.clone(),
        suite: cfg.suite.clone(),
        rotation: cfg.rotation,
        sequential: cfg.sequential,
        diffs: Vec::with_capacity(cfg.suite.deals as usize),
        games: 0,
        pass_games: 0,
        schwarz: 0,
        sides: Default::default(),
        stop: Stop::AllDeals,
    };
    let turns: &[usize] = match cfg.rotation {
        Rotation::Teams => &[0],
        Rotation::TeamsAndSeats => &[0, 2],
    };
    for d in 0..cfg.suite.deals {
        let base = cfg.suite.hands(d);
        let mut diff = 0.0;
        for &turn in turns {
            let hands = turned(&base, turn);
            for a_team in 0..2u8 {
                let mut players: Vec<Box<dyn Agent>> = (0..4u8)
                    .map(|s| if s % 2 == a_team { (a.make)(s) } else { (b.make)(s) })
                    .collect();
                let (info, log) = play_game(&hands, &mut players);
                diff += r.add(&info, a_team) as f64;
                for l in &log {
                    let side = &mut r.sides[(l.seat % 2 != a_team) as usize];
                    side.schmiered_standing += l.flags.schmiered_standing as u64;
                    side.needed_half_played += l.flags.needed_half_played as u64;
                }
                hook(&Played {
                    deal: d,
                    a_team,
                    turned: turn != 0,
                    info: &info,
                    log: &log,
                });
            }
        }
        r.diffs.push(diff / turns.len() as f64);
        if let Some(stop) = cfg.sequential.and_then(|s| decide(&r.diffs, &s)) {
            r.stop = stop;
            break;
        }
    }
    r
}

/// The sequential decision after the deals so far, if any.
fn decide(diffs: &[f64], s: &Sequential) -> Option<Stop> {
    let n = diffs.len() as u64;
    if n < s.min_deals || !n.is_multiple_of(s.every) {
        return None;
    }
    let (mean, half) = conf_seq(diffs, s.alpha, s.min_deals as f64 * 5.0);
    if mean - half > 0.0 {
        Some(Stop::ABetter)
    } else if mean + half < 0.0 {
        Some(Stop::BBetter)
    } else if mean - half > -s.margin && mean + half < s.margin {
        Some(Stop::WithinMargin)
    } else {
        None
    }
}

impl MatchResult {
    /// Adds one game; returns A's score minus B's score.
    fn add(&mut self, info: &GameFinishedInfo, a_team: u8) -> i32 {
        let scores = info.scores();
        let side_of = |team: u8| (team != a_team) as usize;
        self.games += 1;
        self.schwarz += info.schwarz_game as u64;
        for team in 0..2u8 {
            self.sides[side_of(team)].score_sum += scores[team as usize] as i64;
        }
        match &info.playing_party {
            None => self.pass_games += 1,
            Some(p) => {
                let side = &mut self.sides[side_of(p.0 % 2)];
                side.contracts += 1;
                side.made += info.won.unwrap_or(false) as u64;
                side.value_sum += info.game_value.0 as i64;
            }
        }
        for e in &info.all_events {
            let team = e.last_action.player.0 % 2;
            if matches!(e.last_action.action_type, ActionType::Question(_)) {
                self.sides[side_of(team)].questions += 1;
            }
            if let Some(GameCallback::NewTrump(s)) = e.callback {
                self.sides[side_of(team)].melds += 1;
                self.sides[side_of(team)].pair_points += PAIR_POINTS[s as usize] as i64;
            }
        }
        scores[a_team as usize] - scores[1 - a_team as usize]
    }

    /// Mean difference per deal pair and its 95 % normal half-width.
    pub fn mean_ci(&self) -> (f64, f64) {
        mean_ci(&self.diffs)
    }

    /// One plain sentence: who is stronger, by how much, how sure.
    pub fn verdict(&self) -> String {
        let (mean, ci) = self.mean_ci();
        let n = self.diffs.len();
        let how = if mean - ci > 0.0 {
            format!("{} is stronger", self.a)
        } else if mean + ci < 0.0 {
            format!("{} is stronger", self.b)
        } else {
            "no clear difference".into()
        };
        format!(
            "{} scores {mean:+.1} points per deal pair against {} (95 % CI ± {ci:.1}, {n} deals): {how}.",
            self.a, self.b
        )
    }

    /// The plain-language report (markdown): verdict first, then the details.
    pub fn report(&self) -> String {
        let n = self.diffs.len() as u64;
        let games_per_deal = self.games / n.max(1);
        let mut out = format!(
            "{}\n\n- deals: suite `{}` (seed {}), {n} of {} played, {games_per_deal} games each ({:?}); scores by the engine.\n",
            self.verdict(),
            self.suite.name,
            self.suite.seed,
            self.suite.deals,
            self.rotation
        );
        if let Some(s) = &self.sequential {
            let (mean, half) = conf_seq(&self.diffs, s.alpha, s.min_deals as f64 * 5.0);
            let why = match self.stop {
                Stop::AllDeals => "ran all deals without a decision".to_string(),
                Stop::ABetter => format!("stopped early: {} is stronger", self.a),
                Stop::BBetter => format!("stopped early: {} is stronger", self.b),
                Stop::WithinMargin => format!("stopped early: the difference is within ±{}", s.margin),
            };
            out += &format!(
                "- sequential test (confidence sequence, α = {}): [{:+.1}, {:+.1}]; {why}.\n",
                s.alpha,
                mean - half,
                mean + half
            );
        }
        let g = self.games.max(1) as f64;
        for (side, name) in self.sides.iter().zip([&self.a, &self.b]) {
            out += &format!(
                "- {name}: declared {} games ({:.1} %), made {}, mean value {:.1}; {:.2} questions, {:.2} melds and {:.1} pair points per game; mean score {:+.1} per game; per 1,000 games {:.1} standing A/10 thrown onto the partner's trick, {:.1} needed halves led while a question was legal.\n",
                side.contracts,
                100.0 * side.contracts as f64 / g,
                rate(side.made, side.contracts),
                side.value_sum as f64 / side.contracts.max(1) as f64,
                side.questions as f64 / g,
                side.melds as f64 / g,
                side.pair_points as f64 / g,
                side.score_sum as f64 / g,
                1000.0 * side.schmiered_standing as f64 / g,
                1000.0 * side.needed_half_played as f64 / g
            );
        }
        out += &format!(
            "- all passed: {}; schwarz: {}.\n",
            rate(self.pass_games, self.games),
            rate(self.schwarz, self.games)
        );
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RandomAgent, Rng};

    fn random(name: &str, salt: u64) -> Contestant<'static> {
        Contestant {
            name: name.into(),
            make: Box::new(move |s| Box::new(RandomAgent(Rng::new(salt + s as u64)))),
        }
    }

    #[test]
    fn identical_deterministic_contestants_tie_on_every_deal() {
        let cfg = Config {
            suite: DealSuite::named("dev", 20).unwrap(),
            rotation: Rotation::TeamsAndSeats,
            sequential: None,
        };
        // The same agent factory on both sides: each deal pair is exactly symmetric.
        let r = run(&random("A", 7), &random("B", 7), &cfg, |_| {});
        assert_eq!(r.games, 80);
        assert!(r.diffs.iter().all(|&d| d == 0.0), "{:?}", r.diffs);
        assert!(r.report().starts_with("A scores +0.0 points per deal pair"));
    }

    #[test]
    fn the_sequential_test_stops_at_a_check_point() {
        let s = Sequential::default();
        let wins: Vec<f64> = (0..100).map(|i| 100.0 + (i % 7) as f64).collect();
        assert_eq!(decide(&wins[..99], &s), None);
        assert_eq!(decide(&wins, &s), Some(Stop::ABetter));
        let ties: Vec<f64> = (0..400).map(|i| ((i % 3) as f64 - 1.0) * 0.5).collect();
        assert_eq!(decide(&ties, &s), Some(Stop::WithinMargin));
    }
}

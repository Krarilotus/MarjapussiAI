//! The fair mistake meter (S7, ported to the maintained harness; C28 acceptance), with the C15
//! intention detectors, for any agent.
//!
//! At every card or announcement decision with at most `tricks` tricks left and a choice, the
//! move is compared with the best move over the worlds the seat could believe in:
//! `solver::evaluate` on the seat's own `Knowledge` and the public card-play state, objective
//! the team's final points. The worlds are the deals consistent with the seat's facts (all, or
//! `worlds` uniform draws), each weighted by a judge's `beliefs::Mind` of that seat (built from
//! the seat's own view with default belief parameters, the same judge for every agent):
//! Π over the hidden seats' cards of P_believed / P_facts. No true hand goes in. The loss is
//! the best mean minus the mean of the move, in points.
//!
//! The double-dummy loss (the same decision solved on the true deal) is an upper-bound
//! diagnostic only, reported apart.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use agent::{Agent, Decision};
use beliefs::{BeliefParams, Mind};
use engine::{ActionType, FastState, Move};
use knowledge::Hands;
use observation::Seat;
use solver::{evaluate, Objective, Solver};

use crate::suite::DealSuite;
use crate::{play_game_with, Logged, Rng, Table};

#[derive(Clone, Copy, Debug)]
pub struct MeterConfig {
    /// Decisions with at most this many tricks left are measured.
    pub tricks: u8,
    /// Worlds per decision (every consistent deal if there are fewer).
    pub worlds: usize,
}

impl Default for MeterConfig {
    fn default() -> Self {
        MeterConfig { tricks: 5, worlds: 100 }
    }
}

/// Loss totals of one group of decisions.
#[derive(Clone, Copy, Debug, Default)]
struct Sum {
    n: u64,
    fair: f64,
    dd: f64,
}

impl Sum {
    fn add(&mut self, fair: f64, dd: f64) {
        self.n += 1;
        self.fair += fair;
        self.dd += dd;
    }
}

/// One measured position, for the costliest list.
#[derive(Clone, Debug)]
struct Position {
    game: u64,
    step: usize,
    seat: Seat,
    label: &'static str,
    fair: f64,
    worlds: usize,
}

/// The detector counts of the C15 harness (`detectors.rs`), per game set.
#[derive(Clone, Copy, Debug, Default)]
struct Detected {
    card_plays: u64,
    voluntary_pair_half: u64,
    blocked_pairs: u64,
    schmiered_standing: u64,
    needed_half_played: u64,
}

pub struct Meter {
    cfg: MeterConfig,
    solver: Solver,
    rng: Rng,
    games: u64,
    all: Sum,
    by_rule: BTreeMap<&'static str, Sum>,
    by_tricks: BTreeMap<u8, Sum>,
    worst: Vec<Position>,
    detected: Detected,
}

/// `eval::Rng` for the solver's world draws.
struct Draws<'a>(&'a mut Rng);

impl rand::RngCore for Draws<'_> {
    fn next_u32(&mut self) -> u32 {
        (self.0.next_u64() >> 32) as u32
    }

    fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }

    fn fill_bytes(&mut self, dst: &mut [u8]) {
        for chunk in dst.chunks_mut(8) {
            chunk.copy_from_slice(&self.0.next_u64().to_le_bytes()[..chunk.len()]);
        }
    }
}

fn move_of(d: &Decision) -> Option<Move> {
    match d.action.action_type {
        ActionType::CardPlayed(c) => Some(Move::Card(c.index())),
        ActionType::AnnounceTrump(s) => Some(Move::Announce(s)),
        _ => None,
    }
}

impl Meter {
    pub fn new(cfg: MeterConfig) -> Meter {
        Meter {
            cfg,
            solver: Solver::with_tt_bits(18),
            rng: Rng::new(0x5EED_0FA1),
            games: 0,
            all: Sum::default(),
            by_rule: BTreeMap::new(),
            by_tricks: BTreeMap::new(),
            worst: vec![],
            detected: Detected::default(),
        }
    }

    /// Measures one decision, seen before it is applied.
    pub fn observe(&mut self, game: u64, step: usize, table: &Table, seat: Seat, d: &Decision) {
        let Some(mv) = move_of(d) else { return };
        let Some(state) = FastState::from_game(&table.game) else {
            return;
        };
        let left = table.hand(seat).len() as u8;
        if left > self.cfg.tricks || state.moves().count() < 2 {
            return;
        }
        let view = table.view(seat);
        let k = view.knowledge;
        let mut mind = Mind::new(seat, k.dealt_certain(seat), view.kinds, BeliefParams::default());
        for e in view.events {
            if mind.apply(e).is_err() {
                return;
            }
        }
        let facts = k.deals().marginals();
        let ratio: [[f64; 36]; 4] = std::array::from_fn(|y| {
            std::array::from_fn(|c| {
                let base = facts[y][c];
                if y as u8 == seat || base <= 0.0 {
                    1.0
                } else {
                    mind.p_holds(y as u8, c as u8) as f64 / base
                }
            })
        });
        let belief = |hands: &Hands| -> f64 {
            (0..4)
                .filter(|&y| y != seat as usize)
                .map(|y| hands[y].iter().map(|c| ratio[y][c as usize]).product::<f64>())
                .product()
        };
        let public = state.public();
        let Ok(dec) = evaluate(
            &mut self.solver,
            k,
            &public,
            Objective::Points,
            self.cfg.worlds,
            &belief,
            &mut Draws(&mut self.rng),
        ) else {
            return;
        };
        let mean = |m: Move| dec.moves.iter().find(|v| v.mv == m).map(|v| v.means[0]);
        let Some(chosen) = mean(mv) else { return };
        let best = dec.moves.iter().map(|v| v.means[0]).fold(f64::MIN, f64::max);
        let fair = best - chosen;
        // Double dummy on the true deal (upper bound).
        let dd_values = self.solver.move_values(&state, seat % 2);
        let dd_best = dd_values.iter().map(|v| v.1).max().unwrap_or(0);
        let dd = dd_values
            .iter()
            .find(|v| v.0 == mv)
            .map_or(0.0, |v| (dd_best - v.1) as f64);
        let label = d.explanation.label;
        self.all.add(fair, dd);
        self.by_rule.entry(label).or_default().add(fair, dd);
        self.by_tricks.entry(left).or_default().add(fair, dd);
        self.worst.push(Position {
            game,
            step,
            seat,
            label,
            fair,
            worlds: dec.worlds,
        });
        self.worst.sort_by(|a, b| b.fair.total_cmp(&a.fair));
        self.worst.truncate(20);
    }

    /// Adds a finished game's decisions to the detector counts.
    pub fn add_game(&mut self, log: &[Logged]) {
        self.games += 1;
        for l in log {
            let d = &mut self.detected;
            d.card_plays += matches!(l.decision.action.action_type, ActionType::CardPlayed(_)) as u64;
            d.voluntary_pair_half += l.flags.voluntary_pair_half as u64;
            d.blocked_pairs += l.flags.blocked_pairs.count_ones() as u64;
            d.schmiered_standing += l.flags.schmiered_standing as u64;
            d.needed_half_played += l.flags.needed_half_played as u64;
        }
    }

    /// The report in Markdown.
    pub fn report(&self) -> String {
        let g = self.games.max(1) as f64;
        let per_k = |x: f64| 1000.0 * x / g;
        let mut out = String::new();
        let a = &self.all;
        let n = a.n.max(1) as f64;
        let _ = writeln!(
            out,
            "## Fair mistake meter ({} games, ≤ {} tricks left, ≤ {} worlds)\n",
            self.games, self.cfg.tricks, self.cfg.worlds
        );
        let _ = writeln!(
            out,
            "- {} decisions: **{:.2} points lost per decision** (fair), {:.0} per 1,000 games; double dummy {:.2} (upper bound)",
            a.n,
            a.fair / n,
            per_k(a.fair),
            a.dd / n
        );
        let _ = writeln!(
            out,
            "\n| rule | decisions | fair loss / decision | fair loss / 1,000 games | dd / decision |"
        );
        let _ = writeln!(out, "|---|---|---|---|---|");
        let mut rules: Vec<_> = self.by_rule.iter().collect();
        rules.sort_by(|x, y| y.1.fair.total_cmp(&x.1.fair));
        for (rule, s) in rules {
            let m = s.n.max(1) as f64;
            let _ = writeln!(
                out,
                "| {rule} | {} | {:.2} | {:.0} | {:.2} |",
                s.n,
                s.fair / m,
                per_k(s.fair),
                s.dd / m
            );
        }
        let _ = writeln!(out, "\n| tricks left | decisions | fair loss / decision |");
        let _ = writeln!(out, "|---|---|---|");
        for (t, s) in &self.by_tricks {
            let _ = writeln!(out, "| {t} | {} | {:.2} |", s.n, s.fair / s.n.max(1) as f64);
        }
        let d = &self.detected;
        let _ = writeln!(out, "\n## Intention detectors (C15), per 1,000 games\n");
        let _ = writeln!(
            out,
            "- standing A/10 thrown onto the partner's trick: {:.1}",
            per_k(d.schmiered_standing as f64)
        );
        let _ = writeln!(
            out,
            "- halves of a needed pair led while a question was legal: {:.1}",
            per_k(d.needed_half_played as f64)
        );
        let _ = writeln!(
            out,
            "- K/O of an own uncalled pair played voluntarily: {:.1} ({} in {} card plays)",
            per_k(d.voluntary_pair_half as f64),
            d.voluntary_pair_half,
            d.card_plays
        );
        let _ = writeln!(
            out,
            "- own pairs not announceable at a lead (right spent): {:.1}",
            per_k(d.blocked_pairs as f64)
        );
        let _ = writeln!(out, "\n## The 20 costliest positions (fair)\n");
        for p in &self.worst {
            let _ = writeln!(
                out,
                "- game {} step {} seat {}: {} lost {:.1} ({} worlds)",
                p.game, p.step, p.seat, p.label, p.fair, p.worlds
            );
        }
        out
    }
}

/// Self-play of `make`'s agents on `games` deals of `suite`, measured.
pub fn audit(suite: &DealSuite, games: u64, make: &dyn Fn(Seat) -> Box<dyn Agent>, cfg: MeterConfig) -> Meter {
    let mut meter = Meter::new(cfg);
    for g in 0..games {
        let mut players: Vec<Box<dyn Agent>> = (0..4).map(make).collect();
        let mut step = 0;
        let (_, log) = play_game_with(&suite.hands(g), &mut players, |table, seat, d| {
            meter.observe(g, step, table, seat, d);
            step += 1;
        });
        meter.add_game(&log);
    }
    meter
}

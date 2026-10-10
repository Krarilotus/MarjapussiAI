//! Calibration and cost of the `beliefs` crate (S9 step 2).
//!
//! Two data sets, each split into a fit part and an independent test part:
//! - **self-play (the main set for agent seats):** 4×HP1 on fresh deal suites, `seed4101` to fit
//!   and `seed4102` to test (never used for any other fitting or tuning). HP1 seats are read
//!   through the current convention reader with the agent-kind trust (D-f).
//! - **human games (a check):** the 1,577 human games, fitted on the even games, tested on the
//!   odd games.
//!
//! Fits per seat kind, few parameters, shrunk towards "no information": the exchange kernel
//! (4 values, F7), the trust (1 value, D-f), the per-card factors (16 values, F4); one
//! temperature per feature family (F8) is fitted on self-play and kept only if it helps on both.
//! Every decision (bid, pass, card) of every seat's view is scored: cards, seat features now and
//! as dealt, against the facts-only reference, with the read-out bands; plus µs per decision of
//! the seat to act (~225 card reads and the seat features, the epoch's computation included).
//!
//! `beliefs_study --games PATH [--deals N] [--threads 2] [--max-games M] [--sets both|selfplay|human]`

use std::collections::HashMap;
use std::fmt::Write as _;
use std::time::Instant;

use agent::{Agent, SeatKind};
use beliefs::factors::{for_each_key, Factors, Key};
use beliefs::{BeliefParams, Calibration, FeatureCards, Kernel, Mind, PerKind, SeatFeatures, UNIFORM_STAY};
use engine::{partner, CardSet, GameAction, SUITS};
use eval::{suite::DealSuite, Table};
use heuristic::{HeuristicAgent, Params};
use knowledge::{Knowledge, Passed};
use observation::Seat;
use serde_json::Value;
use tools::legacy;

const DECISIONS: [&str; 8] = [
    "first bid",
    "later bid",
    "raise",
    "pass forth",
    "pass back",
    "lead",
    "follow",
    "trump or lead",
];
const CARD_PHASES: [&str; 3] = ["lead", "follow", "trump or lead"];

/// Shrinkage (pseudo-observations at "no information"): factors towards 1, the kernel towards
/// uniform passing.
const FACTOR_PRIOR: f64 = 100.0;
const KERNEL_PRIOR: f64 = 50.0;
const TRUST_GRID: [f32; 5] = [0.6, 0.75, 0.9, 0.95, 0.99];
const TEMPERATURES: [f64; 9] = [0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.35, 1.5, 1.75];

fn timing_phase(phase: &str) -> usize {
    match phase {
        "first bid" | "later bid" => 0,
        "pass forth" | "pass back" => 1,
        _ => 2,
    }
}

const FEATURE_FAMILIES: [&str; 6] = [
    "has an ace",
    "≥ 1 half",
    "≥ 2 halves",
    "≥ 3 halves",
    "half of suit s",
    "pair in suit s",
];
const CARD_FAMILIES: [&str; 2] = ["holds another card", "holds an A/K/O card"];
/// Feature family (report order) → `beliefs::FAMILIES` index.
const FAMILY_SLOT: [usize; 6] = [2, 3, 4, 5, 6, 7];

fn feature_family(i: usize) -> usize {
    match i {
        0..=3 => i,
        4..=7 => 4,
        _ => 5,
    }
}

fn values(f: SeatFeatures) -> [f32; 12] {
    let mut v = [0.0; 12];
    v[0] = f.ace;
    v[1..4].copy_from_slice(&f.halves_at_least);
    v[4..8].copy_from_slice(&f.half);
    v[8..12].copy_from_slice(&f.pair);
    v
}

/// The features of a hand, in `values` order (each grows with the hand).
fn truth(hand: CardSet) -> [bool; 12] {
    let halves = SUITS.map(|s| (hand & CardSet::halves(s)).len());
    let total: u32 = halves.iter().sum();
    let mut v = [false; 12];
    v[0] = !(hand & CardSet::ACES).is_empty();
    for k in 0..3 {
        v[1 + k] = total > k as u32;
    }
    for s in 0..4 {
        v[4 + s] = halves[s] >= 1;
        v[8 + s] = halves[s] >= 2;
    }
    v
}

/// Undecided by `sure ⊆ hand ⊆ may`.
fn undecided(sure: CardSet, may: CardSet) -> [bool; 12] {
    let (yes, maybe) = (truth(sure), truth(may));
    std::array::from_fn(|i| !yes[i] && maybe[i])
}

// ------------------------------------------------------------------ the data sets

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Set {
    SelfPlay,
    Human,
}

impl Set {
    /// The seat kind this set fits.
    fn kind(self) -> SeatKind {
        match self {
            Set::SelfPlay => SeatKind::Heuristic,
            Set::Human => SeatKind::Human,
        }
    }
}

/// One game of a set.
#[derive(Clone, Copy)]
enum Game<'a> {
    Human(&'a Value),
    SelfPlay(u64, u64),
}

/// Calls `visit(table, seat to act, phase)` at every decision of the game; `false` if a human
/// record does not replay.
fn play(game: Game, mut visit: impl FnMut(&Table, Seat, &'static str)) -> bool {
    match game {
        Game::Human(rec) => legacy::replay(rec, |step| {
            if step.claimed_by.is_none() {
                visit(step.table, step.seat, step.phase);
            }
        })
        .is_ok(),
        Game::SelfPlay(seed, i) => {
            let suite = DealSuite {
                name: format!("seed{seed}"),
                seed,
                deals: i + 1,
            };
            let mut table = Table::new(&suite.hands(i));
            table.kinds = [SeatKind::Heuristic; 4];
            let mut players: Vec<HeuristicAgent> = (0..4).map(|_| HeuristicAgent::new(Params::default())).collect();
            while let Some((seat, legal)) = table.to_act() {
                let legal: Vec<GameAction> = legal.to_vec();
                let phase = legacy::phase_of(&table, seat, &legal);
                visit(&table, seat, phase);
                let d = players[seat as usize].act(&table.view(seat), &legal);
                table.apply(d.action);
            }
            true
        }
    }
}

// ------------------------------------------------------------------ fitted parameters

#[derive(Clone)]
struct Fit {
    params: BeliefParams,
}

impl Fit {
    fn new() -> Fit {
        Fit {
            params: BeliefParams {
                factors: PerKind {
                    human: Factors::NONE,
                    agent: Factors::NONE,
                },
                kernel: PerKind {
                    human: Kernel::UNIFORM,
                    agent: Kernel::UNIFORM,
                },
                calibration: Calibration::NONE,
                ..BeliefParams::default()
            },
        }
    }
}

fn set_kind<T: Copy>(x: &mut PerKind<T>, kind: SeatKind, v: T) {
    match kind {
        SeatKind::Heuristic => x.agent = v,
        _ => x.human = v,
    }
}

/// The test variants: 0 is the facts-only reference.
fn variants(fit: &BeliefParams, temps: Calibration) -> Vec<(&'static str, BeliefParams)> {
    let no_factors = BeliefParams {
        factors: PerKind {
            human: Factors::NONE,
            agent: Factors::NONE,
        },
        ..fit.clone()
    };
    let pinned = BeliefParams {
        feature_cards: FeatureCards::Pinned,
        ..fit.clone()
    };
    let uniform = BeliefParams {
        kernel: PerKind {
            human: Kernel::UNIFORM,
            agent: Kernel::UNIFORM,
        },
        ..fit.clone()
    };
    let calibrated = BeliefParams {
        calibration: temps,
        ..fit.clone()
    };
    vec![
        (
            "facts only (n-over-k base; feature DP without signals)",
            BeliefParams::base_only(),
        ),
        ("bids (exact feature DP), no card factors", no_factors),
        ("full: bids + partner's pass + early leads", fit.clone()),
        ("full, A/K/O pinned at the DP marginals (F6)", pinned),
        ("full, uniform exchange kernel", uniform),
        ("full + per-family temperatures", calibrated),
    ]
}

const FULL: usize = 2;

// ------------------------------------------------------------------ accumulators

#[derive(Clone, Default)]
struct Cell {
    n: f64,
    se: f64,
    /// Reliability bins: Σp, Σy, n.
    bins: [[f64; 3]; 10],
    no: [f64; 2],
    yes: [f64; 2],
}

impl Cell {
    fn add(&mut self, p: f64, y: bool) {
        let y = y as u8 as f64;
        self.n += 1.0;
        self.se += (p - y) * (p - y);
        let b = ((p * 10.0) as usize).min(9);
        self.bins[b][0] += p;
        self.bins[b][1] += y;
        self.bins[b][2] += 1.0;
        if p <= 0.2 {
            self.no[0] += 1.0;
            self.no[1] += 1.0 - y;
        }
        if p >= 0.8 {
            self.yes[0] += 1.0;
            self.yes[1] += y;
        }
    }

    fn merge(&mut self, o: &Cell) {
        self.n += o.n;
        self.se += o.se;
        for (a, b) in self.bins.iter_mut().zip(&o.bins) {
            for i in 0..3 {
                a[i] += b[i];
            }
        }
        for i in 0..2 {
            self.no[i] += o.no[i];
            self.yes[i] += o.yes[i];
        }
    }

    fn ece(&self) -> f64 {
        self.bins.iter().map(|b| (b[0] - b[1]).abs()).sum::<f64>() / self.n.max(1.0)
    }
}

/// (kind 0 cards / 1 features now / 2 dealt features, family, hidden exchange 0/1).
type Group = (usize, usize, usize);

#[derive(Default)]
struct Acc {
    cells: HashMap<(Group, usize), Cell>,
    /// Per group: per game, (instances, squared error per variant), for clustered intervals.
    games: HashMap<Group, Vec<(f64, Vec<f64>)>>,
    game: HashMap<Group, (f64, Vec<f64>)>,
    /// Kernel fit [role][holder] (dealt to the holder, total).
    kernel: [[[f64; 2]; 2]; 2],
    /// Factor fit per key (Σ truth, Σ prediction).
    keys: HashMap<Key, (f64, f64)>,
    /// Feature Brier sum and count per family (trust and HB-9 fits).
    feature_se: [[f64; 2]; 6],
    /// Temperature fit: per feature family, (p, truth).
    calib: Vec<Vec<(f32, bool)>>,
    /// µs per decision of the seat to act, per timing phase.
    us: [Vec<f64>; 3],
    views: u64,
    games_played: u64,
    /// Decisions slower than `SLOW_US`: (µs, leaves, groups, hidden exchange, phase).
    slow: Vec<(f64, usize, usize, bool, &'static str)>,
}

const SLOW_US: f64 = 2000.0;

impl Acc {
    fn add(&mut self, g: Group, v: usize, nv: usize, p: f64, y: bool) {
        self.cells.entry((g, v)).or_default().add(p, y);
        let e = self.game.entry(g).or_insert_with(|| (0.0, vec![0.0; nv]));
        if v == 0 {
            e.0 += 1.0;
        }
        let y = y as u8 as f64;
        e.1[v] += (p - y) * (p - y);
    }

    fn end_game(&mut self) {
        for (g, x) in self.game.drain() {
            self.games.entry(g).or_default().push(x);
        }
        self.games_played += 1;
    }

    fn merge(&mut self, o: Acc) {
        for (k, c) in o.cells {
            self.cells.entry(k).or_default().merge(&c);
        }
        for (g, v) in o.games {
            self.games.entry(g).or_default().extend(v);
        }
        for r in 0..2 {
            for h in 0..2 {
                for i in 0..2 {
                    self.kernel[r][h][i] += o.kernel[r][h][i];
                }
            }
        }
        for (k, (y, p)) in o.keys {
            let e = self.keys.entry(k).or_default();
            e.0 += y;
            e.1 += p;
        }
        for f in 0..6 {
            for i in 0..2 {
                self.feature_se[f][i] += o.feature_se[f][i];
            }
        }
        self.calib.resize(FEATURE_FAMILIES.len(), vec![]);
        for (a, b) in self.calib.iter_mut().zip(o.calib) {
            a.extend(b);
        }
        for i in 0..3 {
            self.us[i].extend(&o.us[i]);
        }
        self.views += o.views;
        self.games_played += o.games_played;
        self.slow.extend(o.slow);
    }

    /// Variant v minus the reference, Brier, with a game-clustered 95 % interval.
    fn delta(&self, g: Group, v: usize, w: usize) -> Option<(f64, f64)> {
        let games = self.games.get(&g)?;
        let n: f64 = games.iter().map(|x| x.0).sum();
        if n == 0.0 {
            return None;
        }
        let d: f64 = games.iter().map(|x| x.1[v] - x.1[w]).sum::<f64>() / n;
        let var: f64 = games.iter().map(|x| (x.1[v] - x.1[w] - d * x.0).powi(2)).sum();
        Some((d, 1.96 * var.sqrt() / n))
    }
}

// ------------------------------------------------------------------ the passes

#[derive(Clone, Copy, PartialEq)]
enum Pass {
    Kernel,
    Trust,
    Factors,
    Calibration,
    Test,
}

fn hidden_exchange(k: &Knowledge) -> bool {
    (0..4).any(|y| k.received(y) == Passed::Hidden)
}

fn run_game(game: Game, pass: Pass, vars: &[BeliefParams], acc: &mut Acc) -> bool {
    let nv = vars.len();
    let mut minds: Vec<Vec<Option<Mind>>> = (0..4).map(|_| (0..nv).map(|_| None).collect()).collect();
    let mut fed = [0usize; 4];
    let mut kernel_done = false;
    let ok = play(game, |table, to_act, phase| {
        let g = &table.game;
        if pass == Pass::Kernel {
            // Once per game, at the first card decision after an exchange: who was dealt each
            // feature card the declaring team holds after the exchange.
            let k = table.view(0).knowledge;
            if kernel_done || !CARD_PHASES.contains(&phase) {
                return;
            }
            kernel_done = true;
            let Some(m) = k.first_passer() else { return };
            for (holder, y) in [(0, m), (1, partner(m))] {
                let held = table.hand(y) | k.played_by(y);
                let dealt = CardSet::from_cards(&g.info.player_start_cards[y as usize]);
                for c in (held & (CardSet::ACES | CardSet::HALVES)).iter() {
                    let role = CardSet::HALVES.contains(c) as usize;
                    acc.kernel[role][holder][0] += dealt.contains(c) as u8 as f64;
                    acc.kernel[role][holder][1] += 1.0;
                }
            }
            return;
        }
        if !DECISIONS.contains(&phase) {
            return;
        }
        for seat in 0..4u8 {
            let view = table.view(seat);
            let new = &view.events[fed[seat as usize]..];
            for (v, params) in vars.iter().enumerate() {
                let m = minds[seat as usize][v].get_or_insert_with(|| {
                    let hand = CardSet::from_cards(&g.info.player_start_cards[seat as usize]);
                    Mind::new(seat, hand, table.kinds, params.clone())
                });
                for e in new {
                    m.apply(e).expect("a legal game");
                }
            }
            fed[seat as usize] = view.events.len();
        }
        // Agent A's cost: the seat to act reads ~225 card probabilities and the seat features.
        if pass == Pass::Test {
            let m = minds[to_act as usize][FULL].as_ref().unwrap();
            let t0 = Instant::now();
            let mut sink = 0.0f32;
            for i in 0..225u32 {
                sink += m.p_holds((i % 4) as u8, (i % 36) as u8);
            }
            for y in 0..4 {
                sink += m.features(y).ace;
            }
            std::hint::black_box(sink);
            let us = t0.elapsed().as_secs_f64() * 1e6;
            acc.us[timing_phase(phase)].push(us);
            if us > SLOW_US {
                let k = m.knowledge();
                let space = knowledge::DealSpace::with_features(k);
                acc.slow.push((
                    us,
                    space.leaves().count(),
                    space.group_cards().len(),
                    hidden_exchange(k),
                    phase,
                ));
            }
        }
        let hands: [CardSet; 4] = std::array::from_fn(|y| table.hand(y as u8));
        let dealt: [CardSet; 4] = std::array::from_fn(|y| CardSet::from_cards(&g.info.player_start_cards[y]));
        for seat in 0..4u8 {
            let k = *table.view(seat).knowledge;
            let space = k.deals();
            if space.count() == 0 {
                continue;
            }
            acc.views += 1;
            let exact = space.exact_sets();
            let hx = hidden_exchange(&k) as usize;
            let ms: Vec<&Mind> = minds[seat as usize].iter().map(|m| m.as_ref().unwrap()).collect();
            for y in (0..4u8).filter(|&y| y != seat) {
                let open = undecided(exact.certain[y as usize], exact.may_hold(y));
                let t = truth(hands[y as usize]);
                match pass {
                    Pass::Trust => {
                        for (i, p) in values(ms[0].features(y)).into_iter().enumerate() {
                            if open[i] {
                                let f = feature_family(i);
                                acc.feature_se[f][0] += (p as f64 - t[i] as u8 as f64).powi(2);
                                acc.feature_se[f][1] += 1.0;
                            }
                        }
                    }
                    Pass::Calibration => {
                        acc.calib.resize(FEATURE_FAMILIES.len(), vec![]);
                        for (i, p) in values(ms[0].features(y)).into_iter().enumerate() {
                            if open[i] {
                                acc.calib[feature_family(i)].push((p, t[i]));
                            }
                        }
                    }
                    Pass::Test => {
                        for c in exact.possible[y as usize].iter() {
                            let fam = (CardSet::ACES | CardSet::HALVES).contains(c) as usize;
                            let held = hands[y as usize].contains(c);
                            for (v, m) in ms.iter().enumerate() {
                                acc.add((0, fam, hx), v, nv, m.p_holds(y, c) as f64, held);
                            }
                        }
                        let open_dealt = undecided(k.dealt_certain(y), k.dealt_possible(y));
                        let td = truth(dealt[y as usize]);
                        for (v, m) in ms.iter().enumerate() {
                            let (now, then) = (values(m.features(y)), values(m.dealt_features(y)));
                            for i in 0..12 {
                                if open[i] {
                                    acc.add((1, feature_family(i), hx), v, nv, now[i] as f64, t[i]);
                                }
                                if open_dealt[i] {
                                    acc.add((2, feature_family(i), hx), v, nv, then[i] as f64, td[i]);
                                }
                            }
                        }
                    }
                    Pass::Factors | Pass::Kernel => {}
                }
            }
            if pass == Pass::Factors {
                for_each_key(&k, |y, c, key| {
                    if exact.possible[y as usize].contains(c) {
                        let e = acc.keys.entry(key).or_default();
                        e.0 += hands[y as usize].contains(c) as u8 as f64;
                        e.1 += ms[0].p_holds(y, c) as f64;
                    }
                });
            }
        }
    });
    acc.end_game();
    ok
}

fn parallel(games: &[Game], threads: usize, pass: Pass, vars: &[BeliefParams]) -> Acc {
    let parts: Vec<Acc> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..threads)
            .map(|t| {
                s.spawn(move || {
                    let mut acc = Acc::default();
                    for game in games.iter().skip(t).step_by(threads) {
                        run_game(*game, pass, vars, &mut acc);
                    }
                    acc
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("worker")).collect()
    });
    let mut acc = Acc::default();
    for a in parts {
        acc.merge(a);
    }
    acc
}

// ------------------------------------------------------------------ fits

fn fit_kernel(acc: &Acc) -> Kernel {
    let u = UNIFORM_STAY as f64;
    Kernel {
        stay: std::array::from_fn(|r| {
            std::array::from_fn(|h| {
                let [stayed, total] = acc.kernel[r][h];
                ((stayed + KERNEL_PRIOR * u) / (total + KERNEL_PRIOR)) as f32
            })
        }),
    }
}

fn fit_factors(acc: &Acc, floor: f32) -> Factors {
    let ratio = |key: Key| {
        let (y, p) = acc.keys.get(&key).copied().unwrap_or_default();
        (((y + FACTOR_PRIOR) / (p + FACTOR_PRIOR)) as f32).max(floor)
    };
    Factors {
        pass: std::array::from_fn(|class| std::array::from_fn(|in_pass| ratio(Key::Pass { class, in_pass }))),
        lead: std::array::from_fn(|class| ratio(Key::Lead { class })),
    }
}

/// Per feature family, the temperature with the lowest Brier on the fit games.
fn fit_temperatures(acc: &Acc) -> [f64; 6] {
    std::array::from_fn(|f| {
        let data = acc.calib.get(f).map_or(&[][..], |d| &d[..]);
        let brier = |t: f64| {
            data.iter()
                .map(|&(p, y)| (beliefs::scale(p as f64, t) - y as u8 as f64).powi(2))
                .sum::<f64>()
        };
        TEMPERATURES
            .into_iter()
            .min_by(|&a, &b| brier(a).total_cmp(&brier(b)))
            .unwrap()
    })
}

/// Fits the parts of one seat kind on its fit set, into `fit`; returns a log.
fn fit_set(set: Set, games: &[Game], threads: usize, fit: &mut Fit, temps: Option<&mut [f64; 6]>) -> String {
    let kind = set.kind();
    let wall = Instant::now();
    let mut log = String::new();
    let acc = parallel(games, threads, Pass::Kernel, &[]);
    let kernel = fit_kernel(&acc);
    set_kind(&mut fit.params.kernel, kind, kernel);
    let _ = writeln!(
        log,
        "- exchange kernel P(dealt to the holder after the exchange) [ace, half][first passer, second passer]: {:.3?} (counts {:?}, prior {KERNEL_PRIOR} at {UNIFORM_STAY:.3})",
        kernel.stay, acc.kernel
    );
    // One-parameter grids, each chosen by the seat-feature Brier (all families; per family shown).
    let mut grid = |name: &str, values: &[f32], set: &dyn Fn(&mut BeliefParams, f32), fit: &mut Fit| {
        let mut best = (f64::MAX, values[0]);
        let _ = writeln!(
            log,
            "- {name} (seat-feature Brier, no card factors; total, then per family {FEATURE_FAMILIES:?}):"
        );
        for &x in values {
            let mut p = fit.params.clone();
            set(&mut p, x);
            let acc = parallel(games, threads, Pass::Trust, &[p]);
            let total: f64 = acc.feature_se.iter().map(|f| f[0]).sum::<f64>()
                / acc.feature_se.iter().map(|f| f[1]).sum::<f64>().max(1.0);
            let per: Vec<String> = acc
                .feature_se
                .iter()
                .map(|f| format!("{:.5}", f[0] / f[1].max(1.0)))
                .collect();
            let _ = writeln!(log, "  - {x}: {total:.5} ({})", per.join(", "));
            if total < best.0 {
                best = (total, x);
            }
        }
        set(&mut fit.params, best.1);
        let _ = writeln!(log, "  - chosen {}", best.1);
    };
    grid(
        "trust θ",
        &TRUST_GRID,
        &|p, x| match kind {
            SeatKind::Heuristic => p.conventions.trust_agent = x,
            _ => {
                p.conventions.trust_partner = x;
                p.conventions.trust_opponent = x;
            }
        },
        fit,
    );

    let acc = parallel(games, threads, Pass::Factors, std::slice::from_ref(&fit.params));
    let factors = fit_factors(&acc, fit.params.conventions.convention_floor);
    set_kind(&mut fit.params.factors, kind, factors);
    let _ = writeln!(log, "- per-card factors (Σ truth / Σ prediction without them, prior {FACTOR_PRIOR} at 1):\n\n| key | factor | Σ truth | Σ prediction |\n|---|---|---|---|");
    let mut keys: Vec<_> = acc.keys.iter().collect();
    keys.sort_by_key(|(k, _)| format!("{k:?}"));
    for (k, (y, p)) in keys {
        let _ = writeln!(log, "| {k:?} | {:.3} | {y:.0} | {p:.0} |", factors.of(*k));
    }
    if let Some(temps) = temps {
        let acc = parallel(games, threads, Pass::Calibration, std::slice::from_ref(&fit.params));
        *temps = fit_temperatures(&acc);
        let _ = writeln!(
            log,
            "\n- temperatures per feature family: {:?}",
            FEATURE_FAMILIES.iter().zip(temps.iter()).collect::<Vec<_>>()
        );
    }
    let _ = writeln!(log, "\n({set:?} fit: {:.0} s)\n", wall.elapsed().as_secs_f64());
    log
}

// ------------------------------------------------------------------ report

fn pct(c: &[f64; 2]) -> String {
    if c[0] == 0.0 {
        "–".into()
    } else {
        format!("{:.1} %", 100.0 * c[1] / c[0])
    }
}

fn table(o: &mut String, acc: &Acc, names: &[&str], kind: usize, families: &[&str]) {
    let _ = writeln!(
        o,
        "| family | exchange | method | instances | Brier | Δ vs facts only | Δ vs full | ECE | believed no right (share) | believed yes right (share) |\n|---|---|---|---|---|---|---|---|---|---|"
    );
    for (f, fam) in families.iter().enumerate() {
        for hx in 0..2 {
            for (v, name) in names.iter().enumerate() {
                let Some(c) = acc.cells.get(&((kind, f, hx), v)) else {
                    continue;
                };
                if c.n == 0.0 {
                    continue;
                }
                let delta = |w: usize| {
                    if v == w {
                        "–".into()
                    } else {
                        acc.delta((kind, f, hx), v, w)
                            .map_or("–".into(), |(d, ci)| format!("{d:+.4} ± {ci:.4}"))
                    }
                };
                let _ = writeln!(
                    o,
                    "| {fam} | {} | {name} | {:.0} | {:.4} | {} | {} | {:.4} | {} ({:.1} %) | {} ({:.1} %) |",
                    ["none", "hidden"][hx],
                    c.n,
                    c.se / c.n,
                    delta(0),
                    delta(FULL),
                    c.ece(),
                    pct(&c.no),
                    100.0 * c.no[0] / c.n,
                    pct(&c.yes),
                    100.0 * c.yes[0] / c.n
                );
            }
        }
    }
    o.push('\n');
}

fn quantile(v: &mut [f64], q: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() - 1) as f64 * q) as usize]
}

fn report(o: &mut String, title: &str, acc: &mut Acc, names: &[&str]) {
    let _ = writeln!(
        o,
        "## {title}: test ({} games, {} views)\n",
        acc.games_played, acc.views
    );
    let _ = writeln!(o, "### Cards (open, undecided by the exact facts)\n");
    table(o, acc, names, 0, &CARD_FAMILIES);
    let _ = writeln!(o, "### Seat features now (undecided by the exact facts)\n");
    table(o, acc, names, 1, &FEATURE_FAMILIES);
    let _ = writeln!(o, "### Seat features of the dealt hand (undecided by K5)\n");
    table(o, acc, names, 2, &FEATURE_FAMILIES);
    let _ = writeln!(o, "### µs per decision of the seat to act (225 card reads + 4 feature reads, the epoch's computation included)\n\n| phase | decisions | mean | p50 | p99 | max |\n|---|---|---|---|---|---|");
    for (i, name) in ["bidding", "passing", "play"].iter().enumerate() {
        let v = &mut acc.us[i];
        let n = v.len();
        let mean = v.iter().sum::<f64>() / n.max(1) as f64;
        let (p50, p99, max) = (quantile(v, 0.5), quantile(v, 0.99), quantile(v, 1.0));
        let _ = writeln!(o, "| {name} | {n} | {mean:.1} | {p50:.1} | {p99:.1} | {max:.1} |");
    }
    acc.slow.sort_by(|a, b| b.0.total_cmp(&a.0));
    let _ = writeln!(
        o,
        "\nSlowest decisions (µs, feature-DP leaves, tied groups, hidden exchange, phase): {:?}\n",
        &acc.slow[..acc.slow.len().min(15)]
    );
}

fn main() {
    let cli = tools::cli::Cli::parse(&["--games", "--deals", "--threads", "--max-games", "--sets"], &[]);
    let path = cli.str("--games").expect("--games PATH");
    let deals: u64 = cli.get("--deals", 1000);
    let threads: usize = cli.get("--threads", 2);
    let max: usize = cli.get("--max-games", usize::MAX);
    let sets = cli.str("--sets").unwrap_or("both");
    let (do_self, do_human) = (sets != "human", sets != "selfplay");
    let wall = Instant::now();
    let (records, _) = legacy::read(path);
    let human: Vec<(usize, Game)> = records.iter().take(max).map(Game::Human).enumerate().collect();
    let human_fit: Vec<Game> = human.iter().filter(|(i, _)| i % 2 == 0).map(|x| x.1).collect();
    let human_test: Vec<Game> = human.iter().filter(|(i, _)| i % 2 == 1).map(|x| x.1).collect();
    let self_fit: Vec<Game> = (0..deals).map(|i| Game::SelfPlay(4101, i)).collect();
    let self_test: Vec<Game> = (0..deals).map(|i| Game::SelfPlay(4102, i)).collect();

    let mut fit = Fit::new();
    let mut temps = [1.0f64; 6];
    let mut o = String::new();
    let _ = writeln!(o, "# beliefs_study: S9 step 2\n\nSelf-play: 4×HP1, fit on suite `seed4101`, test on `seed4102` ({deals} deals each; current convention reader with agent-kind trust). Human games: fit on the even, test on the odd games (a check).\n\n## Fits\n\n### Self-play (agent seats)\n");
    if do_self {
        o += &fit_set(Set::SelfPlay, &self_fit, threads, &mut fit, Some(&mut temps));
    }
    if do_human {
        let _ = writeln!(o, "### Human games (human seats)\n");
        o += &fit_set(Set::Human, &human_fit, threads, &mut fit, None);
    }
    eprint!("{o}");
    let mut calibration = Calibration::NONE;
    for (f, t) in temps.iter().enumerate() {
        calibration.temperature[FAMILY_SLOT[f]] = *t as f32;
    }
    let vars = variants(&fit.params, calibration);
    let names: Vec<&str> = vars.iter().map(|v| v.0).collect();
    let params: Vec<BeliefParams> = vars.into_iter().map(|v| v.1).collect();
    if do_self {
        let mut acc = parallel(&self_test, threads, Pass::Test, &params);
        report(&mut o, "Self-play (seed4102)", &mut acc, &names);
    }
    if do_human {
        let mut acc = parallel(&human_test, threads, Pass::Test, &params);
        report(&mut o, "Human games (odd)", &mut acc, &names);
    }
    let _ = writeln!(
        o,
        "Fitted parameters: {:#?}\n\nWall {:.0} s.",
        fit.params,
        wall.elapsed().as_secs_f64()
    );
    print!("{o}");
}

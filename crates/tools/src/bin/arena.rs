//! Arena (spec §9.2, C14).
//!
//! Default: 4×HP1 self-play on `--games N` fixed-seed deals, compared side by side with the
//! human games (`--human PATH`).
//! `arena [--games N] [--seed S] [--params FILE] [--human PATH] [--records FILE [--records-beliefs]]`
//! `--records` writes one game record per line (`docs/design/game-records.md`).
//!
//! A/B: duplicate deals against another opponent (`eval::duplicate`); each deal is played twice,
//! HP1 on seats 0/2 and then on seats 1/3 (`--seats`: also with the hands turned by two seats).
//! `arena --opponent random|hp1|hp1-random-play [--deals N] [--suite dev|check|holdout|seed<N>] [--seed S]
//!        [--params FILE] [--opp-params FILE] [--seats] [--sequential]`
//! `--seed S` is the suite `seed<S>`; the default suite is `dev` (seed 1).
//! `--beliefs full|base` (`--opp-beliefs` for the opponent): which signals HP1's `Mind` reads
//! (`beliefs::Signals`: `full` = bids, the partner's pass and early leads; `base` = facts only).

use std::collections::BTreeMap;

use agent::Agent;
use beliefs::Signals;
use engine::GameFinishedInfo;
use eval::duplicate::{run, Config, Contestant, Rotation, Sequential};
use eval::gamestats::{compare, GameStats};
use eval::suite::DealSuite;
use heuristic::{HeuristicAgent, Params};
use std::io::Write as _;

use eval::{play_game_with, Composite, RandomAgent, Rng};
use tools::cli::Cli;
use tools::record::{beliefs_json, game_record, params_hash, today, Meta, Note};

struct Args {
    games: u64,
    human: Option<String>,
    records: Option<String>,
    records_beliefs: bool,
    suite: DealSuite,
    seats: bool,
    sequential: bool,
    opponent: String,
    params: Params,
    opp_params: Params,
}

fn args() -> Args {
    let cli = Cli::parse(
        &[
            "--games",
            "--human",
            "--records",
            "--deals",
            "--suite",
            "--seed",
            "--opponent",
            "--params",
            "--opp-params",
            "--beliefs",
            "--opp-beliefs",
        ],
        &["--records-beliefs", "--seats", "--sequential"],
    );
    let deals = cli.get("--deals", 500);
    let suite = match (cli.str("--suite"), cli.str("--seed")) {
        (Some(name), _) => DealSuite::named(name, deals),
        (None, Some(seed)) => DealSuite::named(&format!("seed{seed}"), deals),
        (None, None) => DealSuite::named("dev", deals),
    }
    .unwrap_or_else(|e| panic!("{e}"));
    let with_signals = |mut p: Params, flag: &str| {
        p.beliefs.signals = match cli.str(flag).unwrap_or("full") {
            "full" => Signals::Full,
            "base" => Signals::BaseOnly,
            other => panic!("{flag} {other}: expected full or base"),
        };
        p
    };
    Args {
        games: cli.get("--games", 5_000),
        human: cli.str("--human").map(String::from),
        records: cli.str("--records").map(String::from),
        records_beliefs: cli.flag("--records-beliefs"),
        suite,
        seats: cli.flag("--seats"),
        sequential: cli.flag("--sequential"),
        opponent: cli.str("--opponent").unwrap_or_default().to_string(),
        params: with_signals(cli.params("--params"), "--beliefs"),
        opp_params: with_signals(cli.params("--opp-params"), "--opp-beliefs"),
    }
}

fn main() {
    let a = args();
    if a.opponent.is_empty() {
        selfplay(&a);
    } else {
        duplicate(&a);
    }
}

/// 4×HP1 on distinct deals, compared with the human games.
fn selfplay(a: &Args) {
    let mut hp = GameStats::with_barriers(&beliefs::conventions::BARRIERS);
    let (mut nanos, mut voluntary, mut plays): (Vec<u128>, u64, u64) = (vec![], 0, 0);
    // P10a: pairs held at a lead without the right to announce them, and those never called.
    let (mut blocked, mut lost) = (0u64, 0u64);
    // C22, C24: owner intentions checked on the true hands.
    let (mut schmiered, mut needed_half) = (0u64, 0u64);
    let mut writer = a
        .records
        .as_ref()
        .map(|p| std::io::BufWriter::new(std::fs::File::create(p).unwrap_or_else(|e| panic!("--records {p}: {e}"))));
    let wall = std::time::Instant::now();
    for g in 0..a.games {
        let mut observers: Vec<HeuristicAgent> = (0..4).map(|_| HeuristicAgent::new(a.params.clone())).collect();
        let hands = a.suite.hands(g);
        let mut players: Vec<Box<dyn Agent>> = (0..4)
            .map(|_| Box::new(HeuristicAgent::new(a.params.clone())) as Box<dyn Agent>)
            .collect();
        let mut notes = vec![];
        let (info, log) = play_game_with(&hands, &mut players, |table, seat, d| {
            if writer.is_some() {
                let beliefs = a
                    .records_beliefs
                    .then(|| beliefs_json(observers[seat as usize].mind(&table.view(seat))));
                notes.push(Note {
                    claimed_by: None,
                    decision: Some(d.clone()),
                    hp1: None,
                    beliefs,
                });
            }
        });
        if let Some(w) = writer.as_mut() {
            let meta = Meta {
                source: "hp1",
                id: format!("hp1-s{}-g{g}", a.suite.seed),
                seed: Some(a.suite.deal_seed(g)),
                params_hash: Some(params_hash(&a.params)),
                date: today(),
                players: ["HP1", "HP1", "HP1", "HP1"].map(String::from),
            };
            writeln!(w, "{}", game_record(&meta, &info, &notes)).expect("write --records");
        }
        hp.add(&info);
        for l in &log {
            if l.decision.explanation.label != "HX-1" {
                nanos.push(l.nanos);
            }
            plays += matches!(l.decision.action.action_type, engine::ActionType::CardPlayed(_)) as u64;
            voluntary += l.flags.voluntary_pair_half as u64;
            schmiered += l.flags.schmiered_standing as u64;
            needed_half += l.flags.needed_half_played as u64;
        }
        let blocked_suits = log.iter().fold(0u8, |m, l| m | l.flags.blocked_pairs);
        let called = info.all_events.iter().fold(0u8, |m, e| match e.callback {
            Some(engine::GameCallback::NewTrump(s)) => m | 1 << s as u8,
            _ => m,
        });
        blocked += blocked_suits.count_ones() as u64;
        lost += (blocked_suits & !called).count_ones() as u64;
    }
    let secs = wall.elapsed().as_secs_f64();
    let mut human = GameStats::with_barriers(&beliefs::conventions::BARRIERS);
    let mut replayed = 0;
    if let Some(path) = &a.human {
        let (records, _) = tools::legacy::read(path);
        for rec in &records {
            if let Ok(r) = tools::legacy::replay(rec, |_| {}) {
                human.add(&GameFinishedInfo::from(r.table.game));
                replayed += 1;
            }
        }
    }
    nanos.sort_unstable();
    let mean = nanos.iter().sum::<u128>() as f64 / nanos.len().max(1) as f64 / 1000.0;
    println!(
        "# Arena: 4×HP1 self-play, {} games (suite {}, seed {})",
        a.games, a.suite.name, a.suite.seed
    );
    println!();
    println!(
        "{} games in {secs:.1} s; HP1 mean {mean:.1} µs per real decision; HC-1 voluntary pair-half plays: {voluntary} of {plays} card plays.",
        a.games
    );
    println!(
        "P10a: {blocked} pairs were held at a lead whose `Own` right was spent; {lost} of them were never called (a pair lost through spent rights)."
    );
    println!(
        "C22: {schmiered} standing A/10 thrown onto the partner's trick; C24: {needed_half} halves of a needed pair led while a question was legal ({} games).",
        a.games
    );
    println!();
    if replayed > 0 {
        println!("{}", compare(&hp, &human, "4×HP1 self-play", "human games"));
    } else {
        println!(
            "{}",
            compare(&hp, &GameStats::default(), "4×HP1 self-play", "(no human data)")
        );
    }
}

/// The opponent of `--opponent` (a fresh agent per seat and game).
fn opponent(a: &Args) -> Contestant<'_> {
    // A fresh random stream per agent, deterministic over the run.
    let counter = std::cell::Cell::new(0u64);
    let random = move |s: u8| {
        counter.set(counter.get() + 1);
        RandomAgent(Rng::new(a.suite.seed ^ counter.get() << 8 ^ s as u64))
    };
    Contestant {
        name: match a.opponent.as_str() {
            "hp1" => "HP1'".into(),
            other => other.to_string(),
        },
        make: match a.opponent.as_str() {
            "hp1" => Box::new(|_| Box::new(HeuristicAgent::new(a.opp_params.clone()))),
            // Judge S10: HP1's auction and exchange, random card play.
            "hp1-random-play" => Box::new(move |s| {
                Box::new(Composite {
                    auction: Box::new(HeuristicAgent::new(a.opp_params.clone())),
                    play: Box::new(random(s)),
                })
            }),
            "random" => Box::new(move |s| Box::new(random(s))),
            other => panic!("unknown opponent {other} (hp1, hp1-random-play, random)"),
        },
    }
}

/// HP1's side of a duplicate run: contract values, decision times per rule, HC-1 cases.
#[derive(Default)]
struct Hp1Log {
    bids: BTreeMap<i32, u64>,
    nanos: Vec<u128>,
    voluntary: BTreeMap<String, u64>,
    rules: BTreeMap<&'static str, (u64, u128, u128)>,
}

impl Hp1Log {
    fn add(&mut self, g: &eval::duplicate::Played) {
        if g.info.playing_party.is_some_and(|p| p.0 % 2 == g.a_team) {
            *self.bids.entry(g.info.game_value.0).or_default() += 1;
        }
        for l in g.log.iter().filter(|l| l.seat % 2 == g.a_team) {
            self.nanos.push(l.nanos);
            let r = self.rules.entry(l.decision.explanation.label).or_default();
            r.0 += 1;
            r.1 += l.nanos;
            r.2 = r.2.max(l.nanos);
            if l.flags.voluntary_pair_half {
                let why: Vec<String> = l
                    .decision
                    .explanation
                    .facts
                    .iter()
                    .map(|f| format!("{}={:.1}", f.key, f.value))
                    .collect();
                *self
                    .voluntary
                    .entry(format!("{} [{}]", l.decision.explanation.label, why.join(" ")))
                    .or_default() += 1;
            }
        }
    }

    fn print(mut self) {
        let mean = self.nanos.iter().sum::<u128>() as f64 / self.nanos.len().max(1) as f64 / 1000.0;
        self.nanos.sort_unstable();
        let at = |q: usize| {
            *self
                .nanos
                .get((self.nanos.len() * q / 100).min(self.nanos.len().saturating_sub(1)))
                .unwrap_or(&0) as f64
                / 1000.0
        };
        println!(
            "- HP1 decisions: {} ; mean {mean:.1} µs, p99 {:.1} µs, max {:.1} µs per decision (PC shared with other projects)",
            self.nanos.len(),
            at(99),
            at(100)
        );
        println!(
            "
HP1 contract values (game value when HP1's team declared):"
        );
        for (v, n) in &self.bids {
            println!("  {v}: {n}");
        }
        let n_voluntary: u64 = self.voluntary.values().sum();
        println!(
            "
HC-1: voluntary pair-half plays before announcement: {n_voluntary}"
        );
        for (why, n) in self.voluntary.iter().rev().take(20) {
            println!("  {n} × {why}");
        }
        println!(
            "
Rules fired by HP1 (count, mean µs, max µs):"
        );
        for (r, (n, total, max)) in &self.rules {
            println!(
                "  {r}: {n}, {:.1}, {:.1}",
                *total as f64 / *n as f64 / 1000.0,
                *max as f64 / 1000.0
            );
        }
    }
}

/// Duplicate deals against another opponent (`eval::duplicate`), plus HP1's decision log.
fn duplicate(a: &Args) {
    let hp1 = Contestant {
        name: "HP1".into(),
        make: Box::new(|_| Box::new(HeuristicAgent::new(a.params.clone()))),
    };
    let cfg = Config {
        suite: a.suite.clone(),
        rotation: if a.seats {
            Rotation::TeamsAndSeats
        } else {
            Rotation::Teams
        },
        sequential: a.sequential.then(Sequential::default),
    };
    let mut log = Hp1Log::default();
    let wall = std::time::Instant::now();
    let result = run(&hp1, &opponent(a), &cfg, |g| log.add(g));
    println!(
        "# Arena: HP1 vs {} on duplicate deals
",
        result.b
    );
    print!("{}", result.report());
    println!("- wall time {:.1} s", wall.elapsed().as_secs_f64());
    log.print();
}

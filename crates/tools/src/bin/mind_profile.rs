//! HP1 + `Mind` cost per decision (S9 step 2c): 4×HP1 self-play; µs per real decision (more
//! than one legal action) by phase and by rule, with a hash of every action and of every read of
//! the acting seat's `Mind` (card rows, suit counts, seat features now and dealt), so an
//! optimisation can show identical decisions and identical reads.
//!
//! `mind_profile [--deals N] [--seed S]`

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::time::Instant;

use agent::{Agent, SeatKind};
use beliefs::{Mind, SeatFeatures};
use engine::SUITS;
use eval::{suite::DealSuite, Table};
use heuristic::{HeuristicAgent, Params};
use tools::legacy;

fn phase(p: &str) -> &'static str {
    match p {
        "first bid" | "later bid" => "bidding",
        "pass forth" | "pass back" => "passing",
        "raise" => "raise",
        "answer" => "answer",
        _ => "play",
    }
}

fn q(v: &mut [u64], f: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_unstable();
    v[((v.len() - 1) as f64 * f) as usize] as f64 / 1000.0
}

fn features(f: SeatFeatures, h: &mut impl Hasher) {
    for x in [f.ace].iter().chain(&f.halves_at_least).chain(&f.half).chain(&f.pair) {
        x.to_bits().hash(h);
    }
}

/// Every read the valuation can make of `m`, bit for bit.
fn reads(m: &Mind, h: &mut impl Hasher) {
    for y in 0..4 {
        for p in m.holds_row(y) {
            p.to_bits().hash(h);
        }
        for s in SUITS {
            for p in m.count(y, s).p {
                p.to_bits().hash(h);
            }
        }
        features(m.features(y), h);
        features(m.dealt_features(y), h);
    }
}

fn main() {
    let cli: Vec<String> = std::env::args().skip(1).collect();
    let get = |name: &str| cli.iter().position(|a| a == name).and_then(|i| cli.get(i + 1)).cloned();
    let deals: u64 = get("--deals").map_or(300, |x| x.parse().unwrap());
    let seed: u64 = get("--seed").map_or(4107, |x| x.parse().unwrap());
    let suite = DealSuite {
        name: format!("seed{seed}"),
        seed,
        deals,
    };
    let mut groups: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    let (mut decisions, mut read_hash) = (
        std::collections::hash_map::DefaultHasher::new(),
        std::collections::hash_map::DefaultHasher::new(),
    );
    let wall = Instant::now();
    for i in 0..deals {
        let mut table = Table::new(&suite.hands(i));
        table.kinds = [SeatKind::Heuristic; 4];
        let mut players: Vec<HeuristicAgent> = (0..4).map(|_| HeuristicAgent::new(Params::default())).collect();
        while let Some((seat, legal)) = table.to_act() {
            let legal = legal.to_vec();
            let ph = phase(legacy::phase_of(&table, seat, &legal));
            let view = table.view(seat);
            let t0 = Instant::now();
            let d = players[seat as usize].act(&view, &legal);
            let ns = t0.elapsed().as_nanos() as u64;
            format!("{:?}", d.action).hash(&mut decisions);
            if legal.len() > 1 {
                reads(players[seat as usize].mind(&view), &mut read_hash);
                for key in [
                    "all".to_string(),
                    ph.to_string(),
                    format!("rule {}", d.explanation.label),
                ] {
                    groups.entry(key).or_default().push(ns);
                }
            }
            table.apply(d.action);
        }
    }
    println!(
        "# HP1 + Mind per decision: {deals} self-play deals (suite seed{seed}), {:.0} s\n\ndecision hash {:016x}, read hash {:016x}\n",
        wall.elapsed().as_secs_f64(),
        decisions.finish(),
        read_hash.finish()
    );
    println!("| group | decisions | mean µs | p50 | p99 | max |\n|---|---|---|---|---|---|");
    for (name, v) in groups.iter_mut() {
        let mean = v.iter().sum::<u64>() as f64 / v.len().max(1) as f64 / 1000.0;
        let n = v.len();
        let (p50, p99, max) = (q(v, 0.5), q(v, 0.99), q(v, 1.0));
        println!("| {name} | {n} | {mean:.1} | {p50:.1} | {p99:.0} | {max:.0} |");
    }
}

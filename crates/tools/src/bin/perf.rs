//! Speed per phase and the decision guard for optimisations: plays fixed-seed 4×HP1 games,
//! times every decision, and writes every decision (seat, action, rule, why) to a file whose
//! hash must not change when the code only gets faster.
//!
//! `perf [--games N] [--out FILE] [--repeat R]`

use std::collections::BTreeMap;
use std::fmt::Write as _;

use eval::{deal, Table};
use heuristic::HeuristicAgent;
use tools::cli::Cli;
use tools::legacy::phase_of;

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x100_0000_01b3)
    })
}

fn main() {
    let cli = Cli::parse(&["--games", "--out", "--repeat", "--params", "--profile-out"], &[]);
    let (games, repeat) = (cli.get("--games", 2_000u64), cli.get("--repeat", 3usize));
    let out = cli.str("--out").unwrap_or_default().to_string();
    let params = cli.params("--params");
    // Per phase, the best (lowest) mean over the repeats, and p99/max of that run.
    let mut best: BTreeMap<&'static str, (f64, f64, f64, usize)> = BTreeMap::new();
    #[cfg(feature = "profile")]
    beliefs::profile::reset();
    let mut overall_best = f64::MAX;
    let mut log = String::new();
    for r in 0..repeat {
        let mut times: BTreeMap<&'static str, Vec<u64>> = BTreeMap::new();
        log.clear();
        for seed in 0..games {
            let mut table = Table::new(&deal(seed));
            let mut players: Vec<HeuristicAgent> = (0..4).map(|_| HeuristicAgent::new(params.clone())).collect();
            while let Some((seat, legal)) = table.to_act() {
                let phase = if legal.len() == 1 {
                    "forced"
                } else {
                    phase_of(&table, seat, legal)
                };
                let view = table.view(seat);
                let t0 = std::time::Instant::now();
                let d = players[seat as usize].decide(&view, legal);
                let ns = t0.elapsed().as_nanos() as u64;
                times.entry(phase).or_default().push(ns);
                if r == 0 {
                    let why: Vec<String> = d.why.iter().map(|f| format!("{}={:?}", f.key, f.value)).collect();
                    let _ = writeln!(
                        log,
                        "{seed} {seat} {:?} {} [{}]",
                        d.action.action_type,
                        d.rule.code(),
                        why.join(" ")
                    );
                }
                table.apply(d.action);
            }
        }
        let (mut all_sum, mut all_n) = (0u64, 0usize);
        for (phase, mut ts) in times {
            ts.sort_unstable();
            let n = ts.len();
            let sum: u64 = ts.iter().sum();
            if phase != "forced" {
                all_sum += sum;
                all_n += n;
            }
            let mean = sum as f64 / n as f64 / 1000.0;
            let p99 = ts[(n * 99 / 100).min(n - 1)] as f64 / 1000.0;
            let max = ts[n - 1] as f64 / 1000.0;
            let e = best.entry(phase).or_insert((f64::MAX, 0.0, 0.0, n));
            if mean < e.0 {
                *e = (mean, p99, max, n);
            }
        }
        overall_best = overall_best.min(all_sum as f64 / all_n as f64 / 1000.0);
        if r == 0 {
            let hash = fnv(log.as_bytes());
            let actions_only: String = log
                .lines()
                .map(|l| l.split(" [").next().unwrap_or(""))
                .collect::<Vec<_>>()
                .join("\n");
            let bare: String = log
                .lines()
                .map(|l| {
                    l.split(" [")
                        .next()
                        .unwrap_or("")
                        .rsplit_once(' ')
                        .map_or(l, |(a, _)| a)
                })
                .collect::<Vec<_>>()
                .join(
                    "
",
                );
            println!(
                "decisions: {} lines, hash (action+rule+why) {hash:016x}, hash (action+rule) {:016x}, hash (action) {:016x}",
                log.lines().count(),
                fnv(actions_only.as_bytes()),
                fnv(bare.as_bytes())
            );
            if !out.is_empty() {
                std::fs::write(&out, &log).expect("write --out");
            }
        }
    }
    println!("{games} games of 4×HP1, best of {repeat} runs (PC shared with other projects)");
    println!("| phase | decisions | mean µs | p99 µs | max µs |");
    println!("|---|---|---|---|---|");
    for (phase, (mean, p99, max, n)) in &best {
        println!("| {phase} | {n} | {mean:.1} | {p99:.1} | {max:.1} |");
    }
    println!("mean over all non-forced decisions: {overall_best:.1} µs");
    #[cfg(feature = "profile")]
    if let Some(path) = cli.str("--profile-out") {
        let data: serde_json::Map<String, serde_json::Value> = beliefs::profile::snapshot().into_iter().map(|(name, t)| {
            (name.into(), serde_json::json!({"calls":t.calls,"inclusive_ms":t.inclusive_ns as f64 / 1e6,"exclusive_ms":t.exclusive_ns as f64 / 1e6}))
        }).collect();
        std::fs::write(path, serde_json::to_vec_pretty(&data).unwrap()).expect("write --profile-out");
    }

}

//! The fair mistake meter (S7) with the intention detectors, on 4×HP1 self-play
//! (`eval::meter`): `audit [--games N] [--suite dev|check|holdout] [--params FILE]
//! [--tricks T] [--worlds W]`.

use eval::meter::{audit, MeterConfig};
use eval::suite::DealSuite;
use heuristic::HeuristicAgent;
use tools::cli::Cli;

fn main() {
    let cli = Cli::parse(&["--games", "--suite", "--params", "--tricks", "--worlds"], &[]);
    let games = cli.get("--games", 2_000u64);
    let suite = DealSuite::named(cli.str("--suite").unwrap_or("dev"), games).unwrap_or_else(|e| panic!("{e}"));
    let params = cli.params("--params");
    let cfg = MeterConfig {
        tricks: cli.get("--tricks", 5u8),
        worlds: cli.get("--worlds", 100usize),
    };
    let wall = std::time::Instant::now();
    let meter = audit(&suite, games, &|_| Box::new(HeuristicAgent::new(params.clone())), cfg);
    println!(
        "# Audit of HP1 (params {}), suite `{}`\n",
        tools::record::params_hash(&params),
        suite.name
    );
    print!("{}", meter.report());
    println!("\n- wall time {:.0} s", wall.elapsed().as_secs_f64());
}

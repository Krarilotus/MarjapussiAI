//! Solve time T(k) by tricks left, and the time per decision of world-sampled evaluation.
//! Run: `cargo test --release -p marjapussi-solver --test speed -- --ignored --nocapture --test-threads 1`

#[path = "support/positions.rs"]
mod positions;

use std::time::{Duration, Instant};

use engine::{CardSet, CARDS};
use engine::{FastState, Move, TRICKS};
use knowledge::Uniform;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use solver::{evaluate, evaluate_limited, Limits, Objective, Solver, SolverError};

/// A trick start with `left` tricks to go, after random play (announcements taken half the time).
fn position(rng: &mut StdRng, left: u8) -> FastState {
    let mut deck: Vec<u8> = (0..CARDS as u8).collect();
    deck.shuffle(rng);
    let hands = std::array::from_fn(|x| {
        deck[x * 9..x * 9 + 9]
            .iter()
            .fold(CardSet::EMPTY, |a, &c| a | CardSet::single(c))
    });
    let mut s = FastState::new(hands, rng.random_range(0..4));
    while s.tricks_done() < TRICKS - left || !s.trick().is_empty() {
        let moves: Vec<Move> = s.moves().collect();
        let m = match moves.iter().find(|m| matches!(m, Move::Announce(_))) {
            Some(&a) if rng.random_bool(0.5) => a,
            _ => moves[rng.random_range(0..moves.len())],
        };
        s.apply(m);
    }
    s
}

fn ms(ns: u128) -> f64 {
    ns as f64 / 1e6
}

#[test]
#[ignore]
fn solve_time_by_tricks_left() {
    let mut rng = StdRng::seed_from_u64(9);
    let mut solver = Solver::new();
    eprintln!("| k | positions | mean ms | median ms | max ms | mean nodes | max nodes |");
    eprintln!("|---|---|---|---|---|---|---|");
    for (k, count) in [
        (1, 5000),
        (2, 5000),
        (3, 3000),
        (4, 2000),
        (5, 1000),
        (6, 300),
        (7, 100),
        (8, 60),
        (9, 40),
    ] {
        let positions: Vec<FastState> = (0..count).map(|_| position(&mut rng, k)).collect();
        let mut times = vec![];
        let mut nodes = vec![];
        for s in &positions {
            solver.reset_nodes();
            let start = Instant::now();
            std::hint::black_box(solver.value(s, 0));
            times.push(start.elapsed().as_nanos());
            nodes.push(solver.nodes());
        }
        times.sort_unstable();
        let n = positions.len() as f64;
        eprintln!(
            "| {k} | {count} | {:.3} | {:.3} | {:.2} | {:.0} | {} |",
            ms(times.iter().sum::<u128>()) / n,
            ms(times[times.len() / 2]),
            ms(*times.last().unwrap()),
            nodes.iter().sum::<u64>() as f64 / n,
            nodes.iter().max().unwrap(),
        );
    }
}

#[test]
#[ignore]
fn decision_time_by_worlds() {
    let mut solver = Solver::new();
    eprintln!("FastState: {} bytes", std::mem::size_of::<FastState>());
    eprintln!("| k | decisions | mean moves | worlds | points: mean ms | points: max ms | reach: mean ms | contract: mean ms | contract: max ms |");
    eprintln!("|---|---|---|---|---|---|---|---|---|");
    for k in [4usize, 5, 6, 7, 8, 9] {
        // Decisions of random games where the seat to move holds k cards.
        let decisions = positions::decisions(k, 20);
        let moves = decisions.iter().map(|d| d.2).sum::<usize>() as f64 / decisions.len() as f64;
        // Larger hands exercise interruption only; no unbounded full-game run.
        for worlds in [50, 100, 200].into_iter().filter(|_| k <= 6) {
            let mut rng = StdRng::seed_from_u64(worlds as u64);
            let mut times = vec![];
            for (know, table, _, _) in &decisions {
                let start = Instant::now();
                evaluate(&mut solver, know, table, Objective::Points, worlds, &Uniform, &mut rng).unwrap();
                times.push(start.elapsed().as_nanos());
            }
            let start = Instant::now();
            for (know, table, _, points) in &decisions {
                let reach = Objective::Reach {
                    team: know.me() % 2,
                    target: points + 30,
                };
                evaluate(&mut solver, know, table, reach, worlds, &Uniform, &mut rng).unwrap();
            }
            let reach_ns = start.elapsed().as_nanos();
            // HC-9 with an open contract (30 points to go), declared by either team in turn.
            let mut contract_times = vec![];
            for (i, (know, table, _, _)) in decisions.iter().enumerate() {
                let s = (i % 2) as u8;
                let points = (table.card_points[i % 2] + table.pair_points[i % 2]) as i32;
                let contract = Objective::Contract {
                    playing_team: s,
                    game_value: points + 30,
                };
                let start = Instant::now();
                evaluate(&mut solver, know, table, contract, worlds, &Uniform, &mut rng).unwrap();
                contract_times.push(start.elapsed().as_nanos());
            }
            let n = decisions.len() as f64;
            eprintln!(
                "| {k} | {} | {moves:.1} | {worlds} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} |",
                decisions.len(),
                ms(times.iter().sum::<u128>()) / n,
                ms(*times.iter().max().unwrap()),
                ms(reach_ns) / n,
                ms(contract_times.iter().sum::<u128>()) / n,
                ms(*contract_times.iter().max().unwrap()),
            );
        }
        // Same decisions, agent-sized cache and 64 worlds, including sampling in the
        // 45 ms budget. Repeat with the same worlds to measure reusable search bounds.
        let mut bounded = Solver::with_tt_bits(16);
        let mut times = [vec![], vec![]];
        let mut counts = [[0usize; 3]; 2]; // complete, partial, no completed world
        let mut completed_worlds = [0usize; 2];
        for (i, (know, table, _, _)) in decisions.iter().enumerate() {
            bounded.clear();
            let team = i % 2;
            let objective = Objective::Contract {
                playing_team: team as u8,
                game_value: (table.card_points[team] + table.pair_points[team]) as i32 + 30,
            };
            for pass in 0..2 {
                let mut rng = StdRng::seed_from_u64(64);
                let start = Instant::now();
                match evaluate_limited(
                    &mut bounded,
                    know,
                    table,
                    objective,
                    64,
                    &Uniform,
                    &mut rng,
                    Limits {
                        deadline: Some(start + Duration::from_millis(45)),
                        nodes: None,
                    },
                ) {
                    Ok(d) => {
                        counts[pass][usize::from(!d.complete)] += 1;
                        completed_worlds[pass] += d.worlds;
                    }
                    Err(SolverError::Interrupted) => counts[pass][2] += 1,
                    Err(e) => panic!("bounded evaluation failed: {e}"),
                }
                times[pass].push(start.elapsed().as_nanos());
            }
        }
        for pass in 0..2 {
            times[pass].sort_unstable();
            let n = times[pass].len();
            eprintln!(
                "bounded k={k}, {}: mean {:.2} ms, p95 {:.2}, max {:.2}; complete/partial/none {:?}, mean worlds {:.1}",
                if pass == 0 { "cold" } else { "warm" },
                ms(times[pass].iter().sum::<u128>()) / n as f64,
                ms(times[pass][(n * 95).div_ceil(100) - 1]),
                ms(times[pass][n - 1]),
                counts[pass],
                completed_worlds[pass] as f64 / n as f64,
            );
        }
    }
}

/// Full decisions, no deadline truncation: same 20 fixtures as decision_time_by_worlds.
#[test]
#[ignore]
fn decision_time_contract_64() {
    let mut solver = Solver::with_tt_bits(16);
    eprintln!("| k | decisions | worlds | mean ms | max ms | mean nodes | max nodes | ns/node |");
    eprintln!("|---|---|---|---|---|---|---|---|");
    for k in [5, 6, 7, 8] {
        let decisions = positions::decisions(k, 20);
        let mut times = vec![];
        let mut nodes = vec![];
        for (i, (know, table, _, _)) in decisions.iter().enumerate() {
            solver.clear();
            solver.reset_nodes();
            let team = i % 2;
            let objective = Objective::Contract {
                playing_team: team as u8,
                game_value: (table.card_points[team] + table.pair_points[team]) as i32 + 30,
            };
            let start = Instant::now();
            let d = evaluate_limited(
                &mut solver,
                know,
                table,
                objective,
                64,
                &Uniform,
                &mut StdRng::seed_from_u64(64),
                Limits::default(),
            )
            .unwrap();
            times.push(start.elapsed().as_nanos());
            assert!(d.complete);
            nodes.push(solver.nodes());
        }
        let (ns, total) = (times.iter().sum::<u128>(), nodes.iter().sum::<u64>());
        eprintln!(
            "| {k} | 20 | 64 | {:.3} | {:.3} | {} | {} | {:.1} |",
            ms(ns) / 20.0,
            ms(*times.iter().max().unwrap()),
            total / 20,
            nodes.iter().max().unwrap(),
            ns as f64 / total as f64
        );
    }
}

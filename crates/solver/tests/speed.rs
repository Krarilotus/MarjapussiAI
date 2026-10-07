//! Solve time T(k) by tricks left, and the time per decision of world-sampled evaluation.
//! Run: `cargo test --release -p marjapussi-solver --test speed -- --ignored --nocapture --test-threads 1`

use std::time::Instant;

use engine::get_all_cards;
use engine::ActionType;
use engine::Game;
use engine::GamePhase;
use engine::{CardSet, CARDS};
use engine::{FastState, Move, TRICKS};
use knowledge::{Tracker, Uniform};
use observation::{seat_events, start_hand};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use solver::{evaluate, Objective, Solver};

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
    eprintln!("| k | decisions | mean moves | worlds | points: mean ms | points: max ms | reach: mean ms | contract: mean ms | contract: max ms |");
    eprintln!("|---|---|---|---|---|---|---|---|---|");
    for k in [4usize, 5, 6] {
        // Decisions of random games where the seat to move holds k cards.
        let mut decisions = vec![];
        let mut seed = 0u64;
        while decisions.len() < 20 {
            seed += 1;
            let mut rng = StdRng::seed_from_u64(seed);
            let mut deck = get_all_cards();
            deck.shuffle(&mut rng);
            let hands = [0, 9, 18, 27].map(|i| deck[i..i + 9].to_vec());
            let mut game = Game::new("speed".into(), ["S0", "S1", "S2", "S3"].map(String::from), Some(hands));
            let stop = rng.random_range(0..4);
            while !game.ended() {
                let s = &game.state;
                let me = s.player_at_turn.0;
                let in_play = matches!(s.phase, GamePhase::Trick | GamePhase::StartTrick);
                if in_play && s.players[me as usize].cards.len() == k && s.current_trick.len() % 4 == stop {
                    let mut tracker = Tracker::new(me, start_hand(&game, me));
                    for e in seat_events(&game, me) {
                        tracker.apply(&e).unwrap();
                    }
                    let fast = FastState::from_game(&game).unwrap();
                    decisions.push((
                        *tracker.knowledge(),
                        fast.public(),
                        fast.moves().count(),
                        fast.points(me % 2),
                    ));
                    break;
                }
                let legal: Vec<_> = game
                    .legal_actions()
                    .iter()
                    .filter(|a| !matches!(a.action_type, ActionType::UndoRequest))
                    .cloned()
                    .collect();
                game.apply_action_mut(legal[rng.random_range(0..legal.len())]);
            }
        }
        let moves = decisions.iter().map(|d| d.2).sum::<usize>() as f64 / decisions.len() as f64;
        for worlds in [50, 100, 200] {
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
    }
}

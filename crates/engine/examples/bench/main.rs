//! Engine benchmarks (release build, single thread, best of `reps`):
//! `cargo run --release -p marjapussi-engine --example bench -- [games] [reps]`
//! Method and recorded numbers: docs/design/performance.md.

#[macro_use]
mod game;

use std::hint::black_box;

use game::{best_ns, Rng};
use marjapussi_engine::CardSet;
use marjapussi_engine::FastState;

mod game_b {
    use crate::game::{best_ns, Rng};
    game_bench!(marjapussi_engine);
}

fn random_deal(rng: &mut Rng) -> [CardSet; 4] {
    let mut deck: Vec<u8> = (0..36).collect();
    for i in (1..36).rev() {
        deck.swap(i, rng.below(i + 1));
    }
    [0, 1, 2, 3].map(|s| {
        deck[s * 9..s * 9 + 9]
            .iter()
            .fold(CardSet::EMPTY, |h, &c| h | CardSet::single(c))
    })
}

/// The `k`-th card of `set` (ascending index).
#[inline]
fn nth(set: CardSet, k: usize) -> u8 {
    let mut bits = set.0;
    for _ in 0..k {
        bits &= bits - 1;
    }
    bits.trailing_zeros() as u8
}

/// A random game of card play over all legal moves (announcements included); returns moves.
fn random_playout(mut s: FastState, rng: &mut Rng) -> u32 {
    let mut moves = 0;
    while !s.is_over() {
        let mv = s.moves();
        let k = rng.below(mv.len());
        s.apply(mv.clone().nth(k).unwrap());
        moves += 1;
    }
    moves
}

/// A random game of card play over the cards only (as `game_b::card_playout`).
fn card_playout(mut s: FastState, rng: &mut Rng) -> u8 {
    while !s.is_over() {
        let legal = s.legal_cards();
        s.play(nth(legal, rng.below(legal.len() as usize)));
    }
    s.tricks_done()
}

/// Plays (nodes) in the card-play tree to `depth` plies, copy-make as a solver would.
fn perft(s: &FastState, depth: u32) -> u64 {
    if depth == 0 || s.is_over() {
        return 0;
    }
    let mut n = 0;
    for c in s.legal_cards().iter() {
        let mut next = *s;
        next.play(c);
        n += 1 + perft(&next, depth - 1);
    }
    n
}

/// Positions at a trick start with `left` tricks to play, from random playouts.
fn endgames(n: usize, left: u8, rng: &mut Rng) -> Vec<FastState> {
    (0..n)
        .map(|_| {
            let mut s = FastState::new(random_deal(rng), rng.below(4) as u8);
            while s.tricks_done() < 9 - left {
                let legal = s.legal_cards();
                s.play(nth(legal, rng.below(legal.len() as usize)));
            }
            s
        })
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let games: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(300);
    let reps: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);
    let mut rng = Rng(0x5eed);

    println!("## Game (this build), {games} random games, best of {reps}");
    let scripts = game_b::scripts(games, 12345);
    let r = game_b::run(&scripts, reps);
    let labels = [
        "ns per action, apply_action_mut",
        "ns per action, apply_action",
        "ns per full random game (mut)",
        "ns per legal_actions(), all phases",
        "ns per legal_actions(), card play",
        "ns per random card playout, 8 tricks left",
    ];
    for (l, v) in labels.iter().zip(r) {
        println!("| Game | {l} | {v:.0} |");
    }

    println!("## fast");
    let positions = game_b::playout_positions(&scripts);
    let fast_positions: Vec<FastState> = positions.iter().map(|g| FastState::from_game(g).unwrap()).collect();
    let ns = best_ns(reps, fast_positions.len() as u64, || {
        let mut rng = Rng(99);
        for s in &fast_positions {
            black_box(card_playout(*s, &mut rng));
        }
    });
    println!("| fast | ns per random card playout, 8 tricks left (same positions) | {ns:.0} |");

    let deals: Vec<FastState> = (0..10_000)
        .map(|i| FastState::new(random_deal(&mut rng), (i % 4) as u8))
        .collect();
    let mut moves = 0u64;
    let ns_playout = best_ns(reps, deals.len() as u64, || {
        let mut rng = Rng(7);
        moves = 0;
        for s in &deals {
            moves += black_box(random_playout(*s, &mut rng)) as u64;
        }
    });
    let ns_move = ns_playout * deals.len() as f64 / moves as f64;
    println!("| fast | ns per full 9-trick random playout (announcements included) | {ns_playout:.0} |");
    println!(
        "| fast | full playouts per second, one thread | {:.0} |",
        1e9 / ns_playout
    );
    println!("| fast | ns per move (generation + apply + random pick) | {ns_move:.1} |");
    println!("| fast | moves per second, one thread | {:.0} |", 1e9 / ns_move);

    let states: Vec<FastState> = endgames(10_000, 5, &mut rng);
    let ns_legal = best_ns(reps, states.len() as u64 * 4, || {
        for s in &states {
            let mut s = *s;
            for _ in 0..4 {
                let l = black_box(s.legal_cards());
                s.play(l.iter().next().unwrap());
            }
        }
    });
    println!("| fast | ns per legal_cards() + play (one trick from trick starts) | {ns_legal:.1} |");

    for left in [3u8, 4] {
        let ends = endgames(if left == 3 { 2_000 } else { 200 }, left, &mut rng);
        let mut nodes = 0;
        let ns = best_ns(reps, 1, || {
            nodes = ends.iter().map(|s| perft(s, 4 * left as u32)).sum::<u64>();
        });
        println!(
            "| fast | full tree over {left} tricks left ({} positions, {nodes} plays): ns per play | {:.1} |",
            ends.len(),
            ns / nodes as f64
        );
    }
}

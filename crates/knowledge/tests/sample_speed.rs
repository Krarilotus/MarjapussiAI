//! Speed of the deal sampler by number of unknown cards (states from random games).
//! Run: `cargo test --release -p marjapussi-knowledge --test sample_speed -- --ignored --nocapture`

use std::hint::black_box;
use std::time::Instant;

use engine::get_all_cards;
use engine::hands_from_deck;
use engine::ActionType;
use engine::CardSet;
use engine::Game;
use knowledge::{DealSpace, Knowledge, Tracker};
use observation::{seat_events, start_hand};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

fn recorded_states(games: u64) -> Vec<Knowledge> {
    let mut states = vec![];
    for seed in 0..games {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut deck = get_all_cards();
        deck.shuffle(&mut rng);
        let hands = hands_from_deck(&deck);
        let mut game = Game::new("speed".into(), ["S0", "S1", "S2", "S3"].map(String::from), Some(hands));
        while !game.ended() {
            let legal: Vec<_> = game
                .legal_actions()
                .iter()
                .filter(|a| !matches!(a.action_type, ActionType::UndoRequest))
                .cloned()
                .collect();
            game.apply_action_mut(legal[rng.random_range(0..legal.len())]);
        }
        for seat in 0..4 {
            let mut tracker = Tracker::new(seat, start_hand(&game, seat));
            for event in seat_events(&game, seat) {
                tracker.apply(&event).unwrap();
                states.push(*tracker.knowledge());
            }
        }
    }
    states
}

fn unknown(k: &Knowledge) -> u32 {
    let known = (0..4).fold(k.played(), |a, x| a | k.certain(x));
    (!known).len()
}

fn tied(k: &Knowledge) -> bool {
    let free = !(0..4).fold(k.played(), |a, x| a | k.certain(x));
    k.constraints()
        .iter()
        .any(|c| c.seat != k.me() && !(c.mask & k.possible(c.seat) & free).is_empty())
}

fn ns_per(states: &[Knowledge], reps: u32, mut f: impl FnMut(&Knowledge)) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..3 {
        let start = Instant::now();
        for k in states {
            for _ in 0..reps {
                f(k);
            }
        }
        best = best.min(start.elapsed().as_nanos() as f64 / (states.len() as f64 * reps as f64));
    }
    best
}

#[test]
#[ignore]
fn sampler_speed_table() {
    let states = recorded_states(500);
    let mut rng = StdRng::seed_from_u64(1);
    eprintln!("| unknown cards | states | constrained | deals (median) | build µs | sample µs | marginals µs | worst build µs (min of 3) |");
    eprintln!("|---|---|---|---|---|---|---|---|");
    for (lo, hi) in [(0, 0), (1, 6), (7, 12), (13, 18), (19, 24), (25, 27)] {
        let bucket: Vec<Knowledge> = states
            .iter()
            .filter(|k| (lo..=hi).contains(&unknown(k)))
            .copied()
            .collect();
        if bucket.is_empty() {
            continue;
        }
        let constrained = bucket.iter().filter(|k| tied(k)).count();
        let mut deals: Vec<u64> = bucket.iter().map(|k| DealSpace::new(k).count()).collect();
        deals.sort_unstable();
        let build = ns_per(&bucket, 1, |k| {
            black_box(DealSpace::new(black_box(k)));
        });
        let spaces: Vec<DealSpace> = bucket.iter().map(DealSpace::new).collect();
        let mut sample_best = f64::MAX;
        for _ in 0..3 {
            let start = Instant::now();
            let mut sink = CardSet::EMPTY;
            for s in &spaces {
                for _ in 0..20 {
                    sink |= s.sample(&mut rng).unwrap()[0];
                }
            }
            black_box(sink);
            sample_best = sample_best.min(start.elapsed().as_nanos() as f64 / (spaces.len() as f64 * 20.0));
        }
        let mut marg_best = f64::MAX;
        for _ in 0..3 {
            let start = Instant::now();
            for s in &spaces {
                black_box(s.marginals());
            }
            marg_best = marg_best.min(start.elapsed().as_nanos() as f64 / spaces.len() as f64);
        }
        let worst = bucket
            .iter()
            .map(|k| {
                (0..3)
                    .map(|_| {
                        let start = Instant::now();
                        black_box(DealSpace::new(black_box(k)));
                        start.elapsed().as_nanos()
                    })
                    .min()
                    .unwrap()
            })
            .max()
            .unwrap();
        eprintln!(
            "| {lo}–{hi} | {} | {constrained} | {} | {:.2} | {:.2} | {:.2} | {:.1} |",
            bucket.len(),
            deals[deals.len() / 2],
            build / 1000.0,
            sample_best / 1000.0,
            marg_best / 1000.0,
            worst as f64 / 1000.0
        );
    }
}

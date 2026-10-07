//! Baseline speed of the knowledge layer alone (engine time excluded).
//! Run: `cargo test --release -p marjapussi-knowledge --test speed -- --ignored --nocapture`

use std::time::Instant;

use engine::get_all_cards;
use engine::hands_from_deck;
use engine::ActionType;
use engine::CardSet;
use engine::Game;
use knowledge::Tracker;
use observation::{seat_events, start_hand, SeatEvent};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

fn recorded_streams(games: u64) -> Vec<(u8, CardSet, Vec<SeatEvent>)> {
    let mut streams = vec![];
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
            streams.push((seat, start_hand(&game, seat), seat_events(&game, seat)));
        }
    }
    streams
}

#[test]
#[ignore]
fn knowledge_events_per_second() {
    let streams = recorded_streams(2_000);
    let events: usize = streams.iter().map(|(_, _, e)| e.len()).sum();
    let mut best = f64::MAX;
    for _ in 0..5 {
        let start = Instant::now();
        let mut checksum = 0u32;
        for (seat, hand, stream) in &streams {
            let mut tracker = Tracker::new(*seat, *hand);
            for event in stream {
                tracker.apply(event).unwrap();
            }
            checksum += tracker.knowledge().certain((seat + 1) % 4).len();
        }
        let ns = start.elapsed().as_nanos() as f64 / events as f64;
        best = best.min(ns);
        std::hint::black_box(checksum);
    }
    eprintln!(
        "{events} seat-events; best of 5: {best:.0} ns per event, {:.1} µs per seat per game",
        best * events as f64 / streams.len() as f64 / 1000.0
    );
}

/// The known-form queries K1–K5 (C27): ns per query, read after every event of every seat.
#[test]
#[ignore]
fn known_form_queries() {
    let streams = recorded_streams(500);
    let mut sums = [0u128; 5];
    let mut n = 0u128;
    let mut sink = 0u32;
    for (seat, hand, stream) in &streams {
        let mut tracker = Tracker::new(*seat, *hand);
        for event in stream {
            tracker.apply(event).unwrap();
            let k = tracker.knowledge();
            let y = (seat + 1) % 4;
            let t = Instant::now();
            sink += k.standing(*seat).len();
            sums[0] += t.elapsed().as_nanos();
            let t = Instant::now();
            sink += k.team_pairs(*seat).possible() as u32;
            sums[1] += t.elapsed().as_nanos();
            let t = Instant::now();
            sink += k.count_bounds(y, engine::Suit::Red).1 as u32;
            sums[2] += t.elapsed().as_nanos();
            let t = Instant::now();
            sink += k.dealt_certain(y).len();
            sums[3] += t.elapsed().as_nanos();
            let t = Instant::now();
            sink += k.dealt_possible(y).len();
            sums[4] += t.elapsed().as_nanos();
            n += 1;
        }
    }
    std::hint::black_box(sink);
    let names = [
        "standing(me)",
        "team_pairs(me)",
        "count_bounds",
        "dealt_certain",
        "dealt_possible",
    ];
    for (name, total) in names.iter().zip(sums) {
        eprintln!(
            "{name}: {:.0} ns per query (incl. ~20 ns timer overhead)",
            total as f64 / n as f64
        );
    }
}

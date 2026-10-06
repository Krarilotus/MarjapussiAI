//! Baseline speed of the knowledge layer alone (engine time excluded).
//! Run: `cargo test --release -p marjapussi-knowledge --test speed -- --ignored --nocapture`

use std::time::Instant;

use engine::bits::CardSet;
use engine::game::cards::get_all_cards;
use engine::game::gameevent::ActionType;
use engine::game::Game;
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
        let hands = [
            deck[0..9].to_vec(),
            deck[9..18].to_vec(),
            deck[18..27].to_vec(),
            deck[27..36].to_vec(),
        ];
        let mut game = Game::new("speed".into(), ["S0", "S1", "S2", "S3"].map(String::from), Some(hands));
        while !game.ended() {
            let legal: Vec<_> = game
                .legal_actions
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

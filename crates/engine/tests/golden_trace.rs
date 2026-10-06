//! Guard: the engine's observable behaviour is unchanged by speed work.
//!
//! Seeded random games (bidding, passing, questions, answers, announcements, undo requests,
//! accepts and declines) are played through both `apply_action` and `apply_action_mut`. The
//! full state, the legal actions and every event are folded into one digest at every step,
//! plus the `GameFinishedInfo` of each finished game. The expected digest was recorded on
//! `main` at 68c98f0, before any optimisation; any change in behaviour changes it.
//! A mismatch is located with the side-by-side harness described in docs/design/performance.md.

mod common;

use common::{event_line, finished_line, new_game, pick_action, state_line, Digest};
use marjapussi_engine::GameFinishedInfo;
use rand::rngs::StdRng;
use rand::SeedableRng;

const GAMES: u64 = 2_000;
/// Recorded on `main` at 68c98f0.
const EXPECTED: u64 = 0x3544_cf5a_fa28_8fc8;

#[test]
fn random_game_traces_match_main() {
    let mut digest = Digest::new();
    let (mut steps, mut ended) = (0u64, 0u64);
    for seed in 0..GAMES {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut game = new_game(&mut rng);
        digest.add(&state_line(&game));
        for step in 0..600 {
            if game.ended() {
                break;
            }
            let action = pick_action(&game, &mut rng);
            if step % 2 == 0 {
                game.apply_action_mut(action);
            } else {
                game = game.apply_action(action).expect("legal action");
            }
            digest.add(&state_line(&game));
            digest.add(&event_line(game.all_events.last().unwrap()));
            steps += 1;
        }
        if game.ended() {
            ended += 1;
            digest.add(&finished_line(&GameFinishedInfo::from(game)));
        }
    }
    eprintln!(
        "{GAMES} games, {ended} ended, {steps} actions, digest {:#018x}",
        digest.0
    );
    assert_eq!(
        digest.0, EXPECTED,
        "engine behaviour changed (digest {:#018x})",
        digest.0
    );
}

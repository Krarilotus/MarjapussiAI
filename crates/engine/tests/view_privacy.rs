//! Guard (debt D4): `GameInfoPlayer`, the per-seat view, does not depend on other seats' hidden
//! cards.
//! - Swap test: two deals give seat `s` the same hand and the other 27 cards differently. Played
//!   with the same actions through bidding up to the first pass, every view of seat `s` is equal.
//! - In random games, a view lists only its own seat's actions and only its own hand.

mod common;

use common::{new_game, pick_action};
use marjapussi_engine::game::cards::{get_all_cards, Card};
use marjapussi_engine::game::gameevent::ActionType;
use marjapussi_engine::game::gameinfo::GameInfoPlayer;
use marjapussi_engine::game::player::PlaceAtTable;
use marjapussi_engine::game::{Clock, Game};
use std::sync::Arc;

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

fn game_with(hands: [Vec<Card>; 4]) -> Game {
    Game::new("view".into(), ["S0", "S1", "S2", "S3"].map(String::from), Some(hands)).with_clock(Clock::Off)
}

fn view(game: &Game, seat: u8) -> String {
    format!("{:?}", GameInfoPlayer::from_game(game.clone(), PlaceAtTable(seat)))
}

#[test]
fn views_do_not_depend_on_hidden_hands() {
    let mut compared = 0;
    for seed in 0..500u64 {
        let mut rng = StdRng::seed_from_u64(seed);
        let seat = rng.random_range(0..4u8);
        let mut deck = get_all_cards();
        deck.shuffle(&mut rng);
        let own: Vec<Card> = deck[..9].to_vec();
        let deal = |rest: &[Card]| {
            let mut others = rest.chunks(9).map(|c| c.to_vec());
            [0, 1, 2, 3].map(|s| {
                if s == seat as usize {
                    own.clone()
                } else {
                    others.next().unwrap()
                }
            })
        };
        let mut rest = deck[9..].to_vec();
        let mut a = game_with(deal(&rest));
        rest.shuffle(&mut rng);
        let mut b = game_with(deal(&rest));
        loop {
            // Wall-clock times may differ between the two games by a second.
            let times = (a.info.create_time.clone(), a.info.start_time.clone());
            let info = Arc::make_mut(&mut b.info);
            (info.create_time, info.start_time) = times;
            assert_eq!(view(&a, seat), view(&b, seat), "seed {seed}, seat {seat}");
            compared += 1;
            let legal = a.legal_actions();
            // Bids, stops and starts do not depend on the hands; stop at the first pass.
            if legal.iter().any(|x| matches!(x.action_type, ActionType::Pass(_))) || a.ended() {
                break;
            }
            let normal: Vec<_> = legal
                .iter()
                .filter(|x| x.action_type != ActionType::UndoRequest)
                .collect();
            let action = *normal[rng.random_range(0..normal.len())];
            a.apply_action_mut(action);
            b.apply_action_mut(action);
        }
    }
    assert!(compared > 5_000, "{compared}");
}

#[test]
fn views_show_only_own_hand_and_actions() {
    for seed in 0..300u64 {
        let mut rng = StdRng::seed_from_u64(50_000 + seed);
        let mut game = new_game(&mut rng);
        while !game.ended() {
            for seat in 0..4u8 {
                let v = GameInfoPlayer::from_game(game.clone(), PlaceAtTable(seat));
                assert!(v.legal_actions.iter().all(|a| a.player.0 == seat), "seed {seed}");
                if let Some(own) = &v.own_cards {
                    assert_eq!(own, &game.state.players[seat as usize].cards.to_vec());
                }
            }
            let a = pick_action(&game, &mut rng);
            game.apply_action_mut(a);
        }
    }
}

//! Guard: `engine::fast` agrees with `Game` at every card-play state of random games.
//!
//! Random games with bidding, passing, questions, answers, announcements and undo run through
//! `Game`. At every card-play state (phases Raising, Trick, StartTrick, Ended):
//! - the fast state converted from the game has the same legal cards and announcements;
//! - a fast state stepped alongside (the same card or announcement applied) equals the one
//!   converted afresh: same hands, trick, leader (= trick winner), trump, called suits,
//!   tricks and points per team;
//! - `FastParts` round-trips through `from_parts`;
//! - at the end, points and tricks equal `GameFinishedInfo`.

mod common;

use common::new_game;
use marjapussi_engine::bits::CardSet;
use marjapussi_engine::fast::{self, FastState, Move};
use marjapussi_engine::game::gameevent::{ActionType, GameAction};
use marjapussi_engine::game::gameinfo::GameFinishedInfo;
use marjapussi_engine::game::Game;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const GAMES: u64 = 5_000;

/// Random action; an announcement 90 % of the time when possible, undo 4 %.
fn pick(game: &Game, rng: &mut StdRng) -> GameAction {
    let legal = &game.legal_actions;
    let of = |f: fn(&ActionType) -> bool| legal.iter().filter(|a| f(&a.action_type)).collect::<Vec<_>>();
    let announce = of(|t| matches!(t, ActionType::AnnounceTrump(_)));
    if !announce.is_empty() && rng.random_bool(0.9) {
        return announce[rng.random_range(0..announce.len())].clone();
    }
    let undo = of(|t| matches!(t, ActionType::UndoRequest));
    if !undo.is_empty() && rng.random_bool(0.04) {
        return undo[0].clone();
    }
    let normal = of(|t| !matches!(t, ActionType::UndoRequest));
    normal[rng.random_range(0..normal.len())].clone()
}

#[test]
fn fast_matches_game_at_every_card_play_state() {
    let (mut states, mut cards, mut announces, mut tricks, mut ended) = (0u64, 0u64, 0u64, 0u64, 0u64);
    for seed in 0..GAMES {
        let mut rng = StdRng::seed_from_u64(1_000_000 + seed);
        let mut game = new_game(&mut rng);
        let mut stepped: Option<FastState> = None;
        while !game.ended() {
            if let Some(fresh) = FastState::from_game(&game) {
                states += 1;
                if let Some(s) = stepped {
                    assert_eq!(s, fresh, "seed {seed}: stepped fast state differs from Game");
                }
                assert_eq!(
                    FastState::from_parts(fresh.parts()),
                    Ok(fresh),
                    "seed {seed}: parts round trip"
                );
                let seat = game.state.player_at_turn.0;
                assert_eq!(fresh.to_move(), seat, "seed {seed}: seat to move");
                let game_cards = CardSet::from_cards(game.legal_actions.iter().filter_map(|a| match &a.action_type {
                    ActionType::CardPlayed(c) => Some(c),
                    _ => None,
                }));
                assert_eq!(fresh.legal_cards(), game_cards, "seed {seed}: legal cards");
                let game_announce = game.legal_actions.iter().fold(0u8, |b, a| match a.action_type {
                    ActionType::AnnounceTrump(s) => b | 1 << s as u8,
                    _ => b,
                });
                assert_eq!(fresh.announcements(), game_announce, "seed {seed}: announcements");
                let moves: Vec<Move> = fresh.moves().collect();
                assert_eq!(moves.len(), fresh.moves().len());
                assert_eq!(moves.len() as u32, game_cards.len() + game_announce.count_ones());
                for m in moves {
                    assert!(
                        game.legal_actions.contains(&fast::to_action(m, seat)),
                        "seed {seed}: {m:?} not legal in Game"
                    );
                }
                stepped = Some(fresh);
            } else {
                stepped = None;
            }
            let action = pick(&game, &mut rng);
            let tricks_before = game.state.all_tricks.len();
            game.apply_action_mut(action.clone());
            match (fast::from_action(&action.action_type), stepped.as_mut()) {
                (Some(m), Some(s)) => {
                    s.apply(m);
                    match m {
                        Move::Card(_) => cards += 1,
                        Move::Announce(_) => announces += 1,
                    }
                    if game.state.all_tricks.len() > tricks_before {
                        tricks += 1;
                        let t = game.state.all_tricks.last().unwrap();
                        assert_eq!(s.leader(), t.winner.0, "seed {seed}: trick winner");
                        assert_eq!(s.tricks_done() as usize, game.state.all_tricks.len());
                    }
                }
                _ => stepped = None,
            }
        }
        ended += 1;
        let fresh = FastState::from_game(&game).unwrap();
        if let Some(s) = stepped {
            assert_eq!(s, fresh, "seed {seed}: final state");
        }
        assert!(fresh.is_over() && fresh.tricks_done() == 9);
        let info = GameFinishedInfo::from(game);
        let card_points: [i32; 2] = [0, 1].map(|team| {
            info.tricks
                .iter()
                .filter(|t| t.winner.0 % 2 == team)
                .map(|t| t.points.0)
                .sum()
        });
        for team in 0..2u8 {
            assert_eq!(fresh.card_points(team), card_points[team as usize], "seed {seed}");
            assert_eq!(
                fresh.tricks_won(team) as usize,
                info.tricks.iter().filter(|t| t.winner.0 % 2 == team).count()
            );
            if !info.no_one_played {
                // GameFinishedInfo counts announced pairs only when a contract was played.
                assert_eq!(
                    fresh.points(team),
                    info.team_points[team as usize],
                    "seed {seed}: team points"
                );
            }
        }
        assert_eq!(fresh.card_points(0) + fresh.card_points(1), 140);
    }
    eprintln!("{GAMES} games ({ended} ended): {states} card-play states, {cards} cards, {announces} announcements, {tricks} tricks checked");
    assert!(announces >= 1_000, "too few announcements covered: {announces}");
}

#[test]
fn from_parts_rejects_impossible_states() {
    let mut rng = StdRng::seed_from_u64(5);
    let game = new_game(&mut rng);
    let hands = [0, 1, 2, 3].map(|i| CardSet::from_cards(&game.state.players[i].cards));
    let good = FastState::new(hands, 0);
    assert!(FastState::from_parts(good.parts()).is_ok());
    let mut p = good.parts();
    p.hands[1] = p.hands[0];
    assert!(FastState::from_parts(p).is_err(), "shared cards");
    let mut p = good.parts();
    p.tricks_done = 1;
    assert!(FastState::from_parts(p).is_err(), "hand sizes vs tricks done");
    let mut p = good.parts();
    p.trick_len = 1;
    p.trick[0] = p.hands[0].iter().next().unwrap();
    assert!(FastState::from_parts(p).is_err(), "card both in trick and hand");
}

//! Guard: `Game::is_legal` (checked without building the list) equals membership in
//! `Game::legal_actions()` at every state of random games, for every action in the list and for
//! a superset of candidates: all cards, bids, passes (legal and malformed), questions, answers,
//! announcements, starts and undo actions, by every seat.

mod common;

use common::{new_game, pick_action};
use marjapussi_engine::Game;
use marjapussi_engine::PlaceAtTable;
use marjapussi_engine::{get_all_cards, Card, Suit};
use marjapussi_engine::{ActionType, AnswerType, GameAction, QuestionType};
use rand::rngs::StdRng;
use rand::seq::IndexedRandom;
use rand::{Rng, SeedableRng};

const GAMES: u64 = 500;
const SUITS: [Suit; 4] = [Suit::Green, Suit::Acorns, Suit::Bells, Suit::Red];

fn candidates(game: &Game, rng: &mut StdRng) -> Vec<GameAction> {
    let mut types = vec![
        ActionType::Start,
        ActionType::StopBidding,
        ActionType::UndoRequest,
        ActionType::UndoAccept,
        ActionType::UndoDecline,
        ActionType::Question(QuestionType::Yours),
        ActionType::Answer(AnswerType::NoPair),
    ];
    for s in SUITS {
        types.push(ActionType::AnnounceTrump(s));
        types.push(ActionType::Question(QuestionType::YourHalf(s)));
        types.push(ActionType::Answer(AnswerType::YesPair(s)));
        types.push(ActionType::Answer(AnswerType::YesHalf(s)));
        types.push(ActionType::Answer(AnswerType::NoHalf(s)));
    }
    for v in [0, 5, 100, 112, 123, 421, 425, 430]
        .into_iter()
        .chain((110..=420).step_by(5))
    {
        types.push(ActionType::NewBid(v));
    }
    for c in get_all_cards() {
        types.push(ActionType::CardPlayed(c));
    }
    for seat in 0..4 {
        let hand = &game.state.players[seat].cards;
        for _ in 0..3 {
            if hand.len() < 4 {
                break;
            }
            let mut pick: Vec<Card> = hand.choose_multiple(rng, 4).copied().collect();
            pick.sort_by(|a, b| b.cmp(a));
            let sorted: [Card; 4] = pick.clone().try_into().unwrap();
            types.push(ActionType::Pass(sorted));
            types.push(ActionType::Pass([sorted[1], sorted[0], sorted[2], sorted[3]]));
            types.push(ActionType::Pass([sorted[0], sorted[0], sorted[2], sorted[3]]));
            let other = &game.state.players[(seat + 1) % 4].cards;
            if let Some(&foreign) = other.first() {
                let mut mixed = [sorted[0], sorted[1], sorted[2], foreign];
                mixed.sort_by(|a, b| b.cmp(a));
                types.push(ActionType::Pass(mixed));
            }
        }
    }
    let mut out = vec![];
    for t in types {
        for seat in 0..5 {
            out.push(GameAction {
                action_type: t,
                player: PlaceAtTable(seat),
            });
        }
    }
    out
}

#[test]
fn is_legal_equals_list_membership() {
    let (mut states, mut checked, mut legal_seen) = (0u64, 0u64, 0u64);
    for seed in 0..GAMES {
        let mut rng = StdRng::seed_from_u64(3_000_000 + seed);
        let mut game = new_game(&mut rng);
        for _ in 0..600 {
            let legal = game.legal_actions();
            for a in &legal {
                assert!(
                    game.is_legal(a),
                    "seed {seed}: listed action rejected: {a:?} in {:?}",
                    game.state.phase
                );
            }
            for a in candidates(&game, &mut rng) {
                let expected = legal.contains(&a);
                assert_eq!(
                    game.is_legal(&a),
                    expected,
                    "seed {seed}: {a:?} in {:?}",
                    game.state.phase
                );
                assert_eq!(game.apply_action(a).is_ok(), expected);
                checked += 1;
                legal_seen += expected as u64;
            }
            states += 1;
            if game.ended() {
                break;
            }
            let a = if rng.random_bool(0.5) {
                pick_action(&game, &mut rng)
            } else {
                legal[rng.random_range(0..legal.len())]
            };
            game.apply_action_mut(a);
        }
    }
    eprintln!("{GAMES} games, {states} states, {checked} candidate actions ({legal_seen} legal) checked");
}

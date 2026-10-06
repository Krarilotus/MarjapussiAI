//! Guard test (I11, ARCHITECTURE invariant 1): swapping two cards a seat cannot see
//! leaves that seat's observation and knowledge unchanged.
//!
//! Random games are played to a random point. For seat `s`, two cards `a` (held by `x`)
//! and `b` (held by `y`) are swapped in the deal, where `a`, `b` were never played or
//! passed and `s` considers `a` possible for `y` and `b` possible for `x`. The same actions
//! are replayed on the swapped deal. Where every action stays legal and every public event
//! (action and callback) is unchanged, `s` must see the same events and know the same.

use engine::CardSet;
use engine::Game;
use engine::{get_all_cards, hands_from_deck, Card};
use engine::{ActionType, GameAction};
use knowledge::Tracker;
use observation::{seat_events, start_hand, Seat};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

const REQUIRED_SWAPS: u32 = 1_000;

fn game_from(hands: &[Vec<Card>; 4]) -> Game {
    Game::new(
        "swap".into(),
        ["S0", "S1", "S2", "S3"].map(String::from),
        Some(hands.clone()),
    )
}

fn replay(hands: &[Vec<Card>; 4], actions: &[GameAction]) -> Option<Game> {
    let mut game = game_from(hands);
    for action in actions {
        game = game.apply_action(*action).ok()?;
    }
    Some(game)
}

fn knowledge_of(game: &Game, seat: Seat) -> knowledge::Knowledge {
    let mut tracker = Tracker::new(seat, start_hand(game, seat));
    for event in seat_events(game, seat) {
        tracker.apply(&event).unwrap();
    }
    *tracker.knowledge()
}

fn public_events(game: &Game) -> Vec<String> {
    game.all_events
        .iter()
        .map(|e| format!("{:?} {:?}", e.last_action, e.callback))
        .collect()
}

#[test]
fn hidden_cards_do_not_change_what_a_seat_knows() {
    let mut swaps = 0;
    for seed in 0..50_000u64 {
        if swaps >= REQUIRED_SWAPS {
            break;
        }
        let mut rng = StdRng::seed_from_u64(seed);
        let mut deck = get_all_cards();
        deck.shuffle(&mut rng);
        let hands: [Vec<Card>; 4] = hands_from_deck(&deck);
        let mut game = game_from(&hands);
        let mut actions = vec![];
        let stop = rng.random_range(1..60);
        while !game.ended() && actions.len() < stop {
            let legal = game.legal_actions();
            let action = legal[rng.random_range(0..legal.len())];
            game.apply_action_mut(action);
            actions.push(action);
        }

        let seat: Seat = rng.random_range(0..4);
        let (x, y) = {
            let others: Vec<u8> = (0..4).filter(|&o| o != seat).collect();
            let i = rng.random_range(0..3);
            (others[i], others[(i + 1 + rng.random_range(0..2)) % 3])
        };
        let touched = actions.iter().fold(CardSet::EMPTY, |set, a| match &a.action_type {
            ActionType::Pass(cards) => set | CardSet::from_cards(cards),
            ActionType::CardPlayed(card) => set | CardSet::of(card),
            _ => set,
        });
        let known = knowledge_of(&game, seat);
        let candidates = |from: u8, to: u8| -> Vec<Card> {
            hands[from as usize]
                .iter()
                .filter(|c| !touched.contains(c.index()) && known.may_hold(to).contains(c.index()))
                .cloned()
                .collect()
        };
        let (from_x, from_y) = (candidates(x, y), candidates(y, x));
        if from_x.is_empty() || from_y.is_empty() {
            continue;
        }
        let a = from_x[rng.random_range(0..from_x.len())];
        let b = from_y[rng.random_range(0..from_y.len())];
        let mut swapped = hands.clone();
        for card in swapped[x as usize].iter_mut() {
            if *card == a {
                *card = b;
            }
        }
        for card in swapped[y as usize].iter_mut() {
            if *card == b {
                *card = a;
            }
        }

        let Some(other) = replay(&swapped, &actions) else {
            continue;
        };
        if public_events(&other) != public_events(&game) {
            continue;
        }
        assert_ne!(start_hand(&other, x), start_hand(&game, x));
        assert_eq!(
            seat_events(&other, seat),
            seat_events(&game, seat),
            "seed {seed}: seat {seat} sees hidden cards"
        );
        assert_eq!(
            knowledge_of(&other, seat),
            known,
            "seed {seed}: seat {seat} knows hidden cards"
        );
        swaps += 1;
    }
    assert!(swaps >= REQUIRED_SWAPS, "only {swaps} valid swaps");
}

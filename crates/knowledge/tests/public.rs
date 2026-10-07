//! Guard tests for the public instance and the fact epoch (S9, F9, F12).
//! - A public `Tracker`, fed the public events (every pass hidden), never contradicts the true
//!   hands, and knows no more than any seat does.
//! - The fact epoch moves exactly when the facts change (and on undo), never repeats.

use engine::{get_all_cards, hands_from_deck, ActionType, CardSet, Game, GameAction};
use knowledge::{Tracker, PUBLIC};
use observation::{start_hand, SeatEvent, SeatFilter};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

fn pick_action(game: &Game, rng: &mut StdRng) -> GameAction {
    let legal = game.legal_actions();
    let (undo, normal): (Vec<&GameAction>, Vec<&GameAction>) = legal
        .iter()
        .partition(|a| matches!(a.action_type, ActionType::UndoRequest));
    if normal.is_empty() || (!undo.is_empty() && rng.random_bool(0.05)) {
        return legal[rng.random_range(0..legal.len())];
    }
    *normal[rng.random_range(0..normal.len())]
}

/// The public version of a seat's event: pass cards removed.
pub fn public(e: &SeatEvent) -> SeatEvent {
    match *e {
        SeatEvent::Pass { from, to, .. } => SeatEvent::Pass { from, to, cards: None },
        ref other => other.clone(),
    }
}

#[test]
fn public_instance_is_sound_and_knows_no_more_than_a_seat() {
    let mut seen_public_certain = 0;
    let mut epochs_moved = (0u32, 0u32);
    for seed in 0..1_000u64 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut deck = get_all_cards();
        deck.shuffle(&mut rng);
        let mut game = Game::new(
            "public".into(),
            ["S0", "S1", "S2", "S3"].map(String::from),
            Some(hands_from_deck(&deck)),
        );
        let mut filters: Vec<SeatFilter> = (0..4).map(SeatFilter::new).collect();
        let mut seats: Vec<Tracker> = (0..4).map(|s| Tracker::new(s, start_hand(&game, s))).collect();
        let mut public_tracker = Tracker::public();
        assert_eq!(public_tracker.knowledge().me(), PUBLIC);
        let mut used = std::collections::HashSet::new();
        for _ in 0..600 {
            if game.ended() {
                break;
            }
            game.apply_action_mut(pick_action(&game, &mut rng));
            let event = game.all_events.last().unwrap();
            let mut seat0 = None;
            for s in 0..4u8 {
                if let Some(e) = filters[s as usize].filter(event) {
                    let before = (seats[s as usize].fact_epoch(), *seats[s as usize].knowledge());
                    seats[s as usize].apply(&e).unwrap();
                    let after = seats[s as usize].knowledge();
                    let moved = seats[s as usize].fact_epoch() != before.0;
                    let changed = e == SeatEvent::Undone || !after.same_facts(&before.1);
                    assert_eq!(
                        moved, changed,
                        "seed {seed}: epoch moved {moved} but facts changed {changed}"
                    );
                    if s == 0 {
                        epochs_moved.0 += moved as u32;
                        epochs_moved.1 += 1;
                        if moved {
                            assert!(used.insert(seats[0].fact_epoch()), "seed {seed}: epoch reused");
                        }
                        seat0 = Some(e);
                    }
                }
            }
            if let Some(e) = seat0 {
                public_tracker.apply(&public(&e)).unwrap();
            }
            let p = public_tracker.knowledge();
            for x in 0..4u8 {
                let truth = CardSet::from_cards(&game.state.players[x as usize].cards);
                assert!(
                    p.certain(x).is_subset(truth) && truth.is_subset(p.may_hold(x)),
                    "seed {seed}: the public instance is wrong about seat {x}"
                );
                assert_eq!(p.hand_len(x) as u32, truth.len());
                for s in 0..4u8 {
                    let k = seats[s as usize].knowledge();
                    assert!(
                        p.certain(x).is_subset(k.certain(x)) && k.may_hold(x).is_subset(p.may_hold(x)),
                        "seed {seed}: the public instance knows more than seat {s} about seat {x}"
                    );
                }
                seen_public_certain += p.certain(x).len();
            }
        }
    }
    eprintln!(
        "public certain cards seen: {seen_public_certain}; fact epoch moved on {} of {} events of seat 0",
        epochs_moved.0, epochs_moved.1
    );
    assert!(
        seen_public_certain > 1_000,
        "the public instance deduces too little: {seen_public_certain}"
    );
    assert!(
        epochs_moved.0 < epochs_moved.1,
        "the epoch should stay on bids and questions"
    );
}

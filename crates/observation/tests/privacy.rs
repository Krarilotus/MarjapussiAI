//! Guard test (I11): a pass shows its cards to the giver and the receiver only.

use engine::get_all_cards;
use engine::ActionType;
use engine::Game;
use engine::{hands_from_deck, partner, CardSet};
use observation::{SeatEvent, SeatFilter};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

#[test]
fn a_called_half_callback_preserves_public_half_evidence() {
    use engine::{AnswerType, GameAction, GameCallback, GameEvent, PlaceAtTable, Suit};
    let event = GameEvent {
        last_action: GameAction {
            player: PlaceAtTable(3),
            action_type: ActionType::Answer(AnswerType::YesHalf(Suit::Acorns)),
        },
        callback: Some(GameCallback::StillTrump(Suit::Acorns)),
        player_at_turn: PlaceAtTable(1),
        time: 0,
    };
    for seat in 0..4 {
        assert_eq!(
            SeatFilter::new(seat).filter(&event),
            Some(SeatEvent::YesHalf {
                seat: 3,
                suit: Suit::Acorns,
                asker_has_half: true,
            })
        );
    }
}

#[test]
fn passed_cards_are_visible_to_giver_and_receiver_only() {
    let mut passes = 0;
    for seed in 0..500u64 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut deck = get_all_cards();
        deck.shuffle(&mut rng);
        let hands = hands_from_deck(&deck);
        let mut game = Game::new(
            "privacy".into(),
            ["S0", "S1", "S2", "S3"].map(String::from),
            Some(hands),
        );
        let mut filters: Vec<SeatFilter> = (0..4).map(SeatFilter::new).collect();
        while !game.ended() {
            let legal = game.legal_actions();
            game.apply_action_mut(legal[rng.random_range(0..legal.len())]);
            let event = game.all_events.last().unwrap();
            for seat in 0..4u8 {
                let seen = filters[seat as usize].filter(event);
                if let ActionType::Pass(cards) = &event.last_action.action_type {
                    let from = event.last_action.player.0;
                    let involved = seat == from || seat == partner(from);
                    let expected = involved.then(|| CardSet::from_cards(cards));
                    assert_eq!(
                        seen,
                        Some(SeatEvent::Pass {
                            from,
                            to: partner(from),
                            cards: expected
                        }),
                        "seed {seed}: seat {seat} for a pass by {from}"
                    );
                    passes += 1;
                }
            }
        }
    }
    assert!(passes >= 1_000, "too few passes checked: {passes}");
}

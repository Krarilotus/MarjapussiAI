//! Guard test (I11): a pass shows its cards to the giver and the receiver only.

use engine::bits::{partner, CardSet};
use engine::game::cards::get_all_cards;
use engine::game::gameevent::ActionType;
use engine::game::Game;
use observation::{SeatEvent, SeatFilter};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

#[test]
fn passed_cards_are_visible_to_giver_and_receiver_only() {
    let mut passes = 0;
    for seed in 0..500u64 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut deck = get_all_cards();
        deck.shuffle(&mut rng);
        let hands = [
            deck[0..9].to_vec(),
            deck[9..18].to_vec(),
            deck[18..27].to_vec(),
            deck[27..36].to_vec(),
        ];
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

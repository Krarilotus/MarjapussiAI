use super::*;
use engine::{AnswerType, Card, PlaceAtTable, QuestionType};

fn action(player: u8, action_type: ActionType) -> GameAction {
    GameAction {
        player: PlaceAtTable(player),
        action_type,
    }
}

#[test]
fn phase_budgets_follow_observed_exchange() {
    let params = Params::default();
    let hand = |start| (start..start + 9).fold(CardSet::EMPTY, |h, c| h | CardSet::single(c));
    let giver = Knowledge::new(0, hand(0));
    let cards = std::array::from_fn(|i| Card::from_index(i as u8));
    assert_eq!(giver.first_passer(), None);
    assert_eq!(budget(&params, &giver, &[action(0, ActionType::Pass(cards))]).as_millis(), 800);

    // The receiver's observed forth pass establishes the return phase. Neither
    // the auction length nor any hidden opponent hand is used for classification.
    let mut receiver = Knowledge::new(2, hand(9));
    receiver
        .apply(&SeatEvent::Pass {
            from: 0,
            to: 2,
            cards: Some(CardSet::from_cards(&cards)),
        })
        .unwrap();
    assert_eq!(receiver.first_passer(), Some(0));
    assert_eq!(receiver.hand_len(2), 13);
    assert_eq!(budget(&params, &receiver, &[action(2, ActionType::Pass(cards))]).as_millis(), 4000);

    let bid = action(2, ActionType::NewBid(140));
    let card = action(2, ActionType::CardPlayed(Card::from_index(9)));
    assert_eq!(budget(&params, &receiver, &[bid]).as_millis(), 800);
    assert_eq!(budget(&params, &receiver, &[bid, card]).as_millis(), 4000);
    for play in [
        card,
        action(2, ActionType::Question(QuestionType::Yours)),
        action(2, ActionType::Answer(AnswerType::NoPair)),
    ] {
        assert_eq!(budget(&params, &receiver, &[play]).as_millis(), 80);
    }
}

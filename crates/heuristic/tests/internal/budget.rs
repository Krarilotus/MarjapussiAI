use super::*;
use engine::{AnswerType, Card, PlaceAtTable, QuestionType};

fn action(player: u8, action_type: ActionType) -> GameAction {
    GameAction {
        player: PlaceAtTable(player),
        action_type,
    }
}

#[test]
fn phase_budgets_follow_the_owners_hard_limits() {
    let params = Params::default();
    let hand = |start| (start..start + 9).fold(CardSet::EMPTY, |h, c| h | CardSet::single(c));
    let giver = Knowledge::new(0, hand(0));
    let cards = std::array::from_fn(|i| Card::from_index(i as u8));
    assert_eq!(giver.first_passer(), None);
    // Each exchange step (Schieben) has the owner's 5 s limit.
    assert_eq!(
        DecisionClass::of(&[action(0, ActionType::Pass(cards))])
            .budget(&params)
            .as_millis(),
        4000
    );

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
    assert_eq!(
        DecisionClass::of(&[action(2, ActionType::Pass(cards))])
            .budget(&params)
            .as_millis(),
        4000
    );

    let bid = action(2, ActionType::NewBid(140));
    let card = action(2, ActionType::CardPlayed(Card::from_index(9)));
    assert_eq!(DecisionClass::of(&[bid]).budget(&params).as_millis(), 160);
    // The raise-or-lead choice is a bid and a card play: the stricter play limit applies.
    assert_eq!(DecisionClass::of(&[bid, card]).budget(&params).as_millis(), 80);
    for play in [
        card,
        action(2, ActionType::Question(QuestionType::Yours)),
        action(2, ActionType::Answer(AnswerType::NoPair)),
    ] {
        assert_eq!(DecisionClass::of(&[play]).budget(&params).as_millis(), 80);
    }
}

#[test]
fn only_exchange_steps_sample_the_exchange_worlds() {
    let params = Params {
        outlook_worlds: 64,
        exchange_worlds: 512,
        ..Params::default()
    };
    let cards = std::array::from_fn(|i| Card::from_index(i as u8));
    let bid = action(2, ActionType::NewBid(140));
    let card = action(2, ActionType::CardPlayed(Card::from_index(9)));
    assert_eq!(DecisionClass::of(&[action(0, ActionType::Pass(cards))]).worlds(&params), 512);
    for legal in [vec![bid], vec![bid, card], vec![card]] {
        assert_eq!(DecisionClass::of(&legal).worlds(&params), 64);
    }
}

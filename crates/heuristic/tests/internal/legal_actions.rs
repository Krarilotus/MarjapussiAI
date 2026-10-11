use super::*;
use engine::{Card, PlaceAtTable, Suit};

fn action(action_type: ActionType) -> GameAction {
    GameAction {
        action_type,
        player: PlaceAtTable(0),
    }
}
#[test]
fn ordering_is_explicit_permutation_invariant_and_idempotent() {
    let expected = vec![
        action(ActionType::StopBidding),
        action(ActionType::NewBid(120)),
        action(ActionType::NewBid(125)),
        action(ActionType::AnnounceTrump(Suit::Green)),
        action(ActionType::AnnounceTrump(Suit::Red)),
        action(ActionType::Question(QuestionType::Yours)),
        action(ActionType::Question(QuestionType::YourHalf(Suit::Green))),
        action(ActionType::Question(QuestionType::YourHalf(Suit::Red))),
        action(ActionType::CardPlayed(Card::from_index(0))),
        action(ActionType::CardPlayed(Card::from_index(35))),
    ];
    let mut shuffled = expected.clone();
    shuffled.reverse();
    shuffled.push(action(ActionType::UndoRequest));
    assert_eq!(eligible(&shuffled), expected);
    assert_eq!(eligible(&expected), expected);
    let doubled = vec![expected[0], expected[0]];
    assert_eq!(
        eligible(&doubled),
        doubled,
        "ordering does not alter domain multiplicity"
    );
}
#[test]
fn pass_combination_order_preserves_exact_engine_arrays() {
    let left = action(ActionType::Pass([8, 6, 4, 0].map(Card::from_index)));
    let right = action(ActionType::Pass([8, 7, 4, 0].map(Card::from_index)));
    assert_eq!(eligible(&[right, left]), vec![left, right]);
    // An invalid array is not silently transformed into a legal action.
    let invalid = action(ActionType::Pass([0, 4, 6, 8].map(Card::from_index)));
    assert_eq!(eligible(&[invalid]), vec![invalid]);
}

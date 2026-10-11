//! One policy-domain order for live decisions and hypothetical actors.
//!
//! Engine legality and Table enumeration remain unchanged. Canonical ordering
//! deliberately replaces hand-position/phase-enumeration tie breaks. Authored
//! programme alternatives and nominated candidate order are separate owners.

use engine::{ActionType, AnswerType, GameAction, QuestionType};

/// Eligible actions in ascending (kind, typed payload, player) order. Pass
/// arrays are already strictly descending by the engine's legality contract;
/// preserve their exact representation rather than repair an illegal array.
/// Duplicates are not removed and no eligible action is added.
pub(crate) fn eligible(actions: &[GameAction]) -> Vec<GameAction> {
    let mut actions: Vec<_> = actions
        .iter()
        .copied()
        .filter(|a| a.action_type != ActionType::UndoRequest)
        .collect();
    actions.sort_unstable_by_key(key);
    actions
}

fn key(action: &GameAction) -> (u8, [i32; 4], u8) {
    use ActionType::*;
    let (kind, payload) = match action.action_type {
        Start => (0, [0; 4]),
        StopBidding => (1, [0; 4]),
        NewBid(value) => (2, [value, 0, 0, 0]),
        Pass(cards) => (3, cards.map(|card| i32::from(card.index()))),
        AnnounceTrump(suit) => (4, [suit as i32, 0, 0, 0]),
        Question(QuestionType::Yours) => (5, [0, 0, 0, 0]),
        Question(QuestionType::YourHalf(suit)) => (5, [1, suit as i32, 0, 0]),
        Answer(AnswerType::NoPair) => (6, [0, 0, 0, 0]),
        Answer(AnswerType::YesPair(suit)) => (6, [1, suit as i32, 0, 0]),
        Answer(AnswerType::NoHalf(suit)) => (6, [2, suit as i32, 0, 0]),
        Answer(AnswerType::YesHalf(suit)) => (6, [3, suit as i32, 0, 0]),
        CardPlayed(card) => (7, [i32::from(card.index()), 0, 0, 0]),
        UndoDecline => (8, [0; 4]),
        UndoAccept => (9, [0; 4]),
        UndoRequest => (10, [0; 4]), // excluded before sorting
    };
    (kind, payload, action.player.0)
}

#[cfg(test)]
mod tests {
    include!("../tests/internal/legal_actions.rs");
}

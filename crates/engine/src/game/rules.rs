//! Public rule transitions as pure functions, one owner each. `Game`, `FastState`, the
//! knowledge layer and the agents call them instead of restating the rules.

use crate::bits::{CardSet, SUITS};
use crate::game::cards::Suit;
use crate::game::gameevent::{AnswerType, QuestionType};
use crate::game::player::{PlayerTrumpPossibilities, TRICKS};
use crate::game::points::{CARD_POINTS, LAST_TRICK_BONUS, PAIR_POINTS};

/// Uncalled whole pairs in a hand, as suit bits.
#[inline]
pub fn uncalled_pairs(hand: CardSet, called: u8) -> u8 {
    hand.pair_suits().fold(0, |bits, suit| bits | 1 << suit as u8) & !called
}

/// All truthful answers. A player with several uncalled pairs chooses which to show;
/// answering does not depend on or spend the respondent's own question rights.
pub fn question_answers(hand: CardSet, called: u8, question: QuestionType) -> impl Iterator<Item = AnswerType> {
    let mut answers = [None; 4];
    match question {
        QuestionType::Yours => {
            let pairs = uncalled_pairs(hand, called);
            if pairs == 0 {
                answers[0] = Some(AnswerType::NoPair);
            } else {
                for (i, suit) in SUITS.into_iter().enumerate() {
                    if pairs >> i & 1 != 0 {
                        answers[i] = Some(AnswerType::YesPair(suit));
                    }
                }
            }
        }
        QuestionType::YourHalf(suit) => {
            answers[0] = Some(if (hand & CardSet::halves(suit)).is_empty() {
                AnswerType::NoHalf(suit)
            } else {
                AnswerType::YesHalf(suit)
            });
        }
    }
    answers.into_iter().flatten()
}

/// Trump established by a legal answer: only an uncalled suit can become trump.
/// A positive half answer also needs a half in the asker's hand.
#[inline]
pub fn answer_trump(asker: CardSet, answer: AnswerType, already_called: bool) -> Option<Suit> {
    if already_called {
        return None;
    }
    match answer {
        AnswerType::YesPair(suit) => Some(suit),
        AnswerType::YesHalf(suit) if !(asker & CardSet::halves(suit)).is_empty() => Some(suit),
        _ => None,
    }
}

impl PlayerTrumpPossibilities {
    /// May announce an own pair: only while the right is still `Own`.
    pub const fn may_announce(self) -> bool {
        matches!(self, PlayerTrumpPossibilities::Own)
    }

    /// May ask the partner "yours?": not after a half question (`Ours`).
    pub const fn may_ask_pair(self) -> bool {
        !matches!(self, PlayerTrumpPossibilities::Ours)
    }

    /// The asker's right after asking `question`: "yours?" turns `Own` into `Yours`; a half
    /// question makes it `Ours`.
    pub const fn after_question(self, question: QuestionType) -> Self {
        match (question, self) {
            (QuestionType::Yours, PlayerTrumpPossibilities::Own) => PlayerTrumpPossibilities::Yours,
            (QuestionType::Yours, right) => right,
            (QuestionType::YourHalf(_), _) => PlayerTrumpPossibilities::Ours,
        }
    }
}

/// Points of a finished trick (its four cards as bit indices): card points, plus
/// `LAST_TRICK_BONUS` for the last trick. `tricks_done` counts the tricks finished before this
/// one (0 for the first). Table lookups: this is on the solver's hot path.
#[inline]
pub fn trick_points(trick: [u8; 4], tricks_done: u8) -> i32 {
    let bonus = if tricks_done + 1 == TRICKS { LAST_TRICK_BONUS } else { 0 };
    trick.iter().map(|&c| CARD_POINTS[c as usize] as i32).sum::<i32>() + bonus
}

/// Points credited when `suit` becomes trump (an announcement, or a pair shown in an answer):
/// the pair's points the first time, nothing when the suit was called before.
#[inline]
pub const fn trump_points(suit: Suit, already_called: bool) -> i32 {
    if already_called {
        0
    } else {
        PAIR_POINTS[suit as usize]
    }
}

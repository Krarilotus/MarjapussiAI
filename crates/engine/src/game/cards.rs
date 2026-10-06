//! Cards, suits and values: the leaf of the engine. Owns the card codes used by marjapussi.de
//! (`r-A`, `g-6`, …) and the card index layout (`suit * 9 + value`, 36 cards).

use std::fmt;

use serde::Serialize;
use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::game::errors::GameError;
use crate::inline::Filler;

/// Cards in the deck.
pub const CARDS: usize = 36;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, DeserializeFromStr, SerializeDisplay)]
pub struct Card {
    /// Only compare cards with same color
    pub suit: Suit,
    pub value: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Hash, Serialize)]
pub enum Suit {
    Green,
    Acorns,
    Bells,
    Red,
}

impl fmt::Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Hash, Serialize)]
pub enum Value {
    Six,
    Seven,
    Eight,
    Nine,
    Unter,
    Ober,
    King,
    Ten,
    Ace,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Suits in enum (index) order.
pub const SUITS: [Suit; 4] = [Suit::Green, Suit::Acorns, Suit::Bells, Suit::Red];

/// Values in enum (index) order, low to high.
pub const VALUES: [Value; 9] = [
    Value::Six,
    Value::Seven,
    Value::Eight,
    Value::Nine,
    Value::Unter,
    Value::Ober,
    Value::King,
    Value::Ten,
    Value::Ace,
];

const SUIT_CODES: [char; 4] = ['g', 'e', 's', 'r'];
const VALUE_CODES: [char; 9] = ['6', '7', '8', '9', 'U', 'O', 'K', 'Z', 'A'];

impl Suit {
    /// The site's code: `g` (Grün), `e` (Eichel), `s` (Schell), `r` (Rot/Herz).
    pub const fn code(self) -> char {
        SUIT_CODES[self as usize]
    }

    pub fn from_code(code: char) -> Option<Suit> {
        SUIT_CODES.iter().position(|&c| c == code).map(|i| SUITS[i])
    }
}

impl Value {
    /// The site's code: `6`–`9`, `U`, `O`, `K`, `Z` (Zehn), `A`.
    pub const fn code(self) -> char {
        VALUE_CODES[self as usize]
    }

    pub fn from_code(code: char) -> Option<Value> {
        VALUE_CODES.iter().position(|&c| c == code).map(|i| VALUES[i])
    }
}

impl Card {
    /// Bit index `suit * 9 + value` (enum order), 0–35.
    #[inline]
    pub const fn index(self) -> u8 {
        self.suit as u8 * 9 + self.value as u8
    }

    /// The card with bit index `index` (0–35).
    #[inline]
    pub const fn from_index(index: u8) -> Card {
        Card {
            suit: SUITS[(index / 9) as usize],
            value: VALUES[(index % 9) as usize],
        }
    }
}

impl Filler for Card {
    const FILL: Card = Card::from_index(0);
}

impl Filler for Suit {
    const FILL: Suit = Suit::Green;
}

/// The 36 cards in index order.
pub fn get_all_cards() -> Vec<Card> {
    (0..CARDS as u8).map(Card::from_index).collect()
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}-{}", self.suit.code(), self.value.code())
    }
}

impl fmt::Debug for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self) // call method from Display
    }
}

impl std::str::FromStr for Card {
    type Err = GameError;

    /// Parses the site's card code, e.g. `r-A`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut chars = s.chars();
        match (chars.next(), chars.next(), chars.next(), chars.next()) {
            (Some(suit), Some('-'), Some(value), None) => Ok(Card {
                suit: Suit::from_code(suit).ok_or(GameError::InvalidCode)?,
                value: Value::from_code(value).ok_or(GameError::InvalidCode)?,
            }),
            _ => Err(GameError::InvalidCode),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip() {
        for c in get_all_cards() {
            assert_eq!(c.to_string().parse::<Card>(), Ok(c));
            assert_eq!(Card::from_index(c.index()), c);
        }
        assert_eq!(
            "r-A".parse(),
            Ok(Card {
                suit: Suit::Red,
                value: Value::Ace
            })
        );
        assert_eq!(
            "g-O".parse(),
            Ok(Card {
                suit: Suit::Green,
                value: Value::Ober
            })
        );
        assert_eq!(
            "s-6".parse(),
            Ok(Card {
                suit: Suit::Bells,
                value: Value::Six
            })
        );
        assert_eq!(
            "e-Z".parse(),
            Ok(Card {
                suit: Suit::Acorns,
                value: Value::Ten
            })
        );
        for bad in ["", "r", "rA", "r-", "x-A", "r-X", "r-AA", "r_A"] {
            assert_eq!(bad.parse::<Card>(), Err(GameError::InvalidCode), "{bad}");
        }
        assert_eq!(get_all_cards().len(), CARDS);
    }
}

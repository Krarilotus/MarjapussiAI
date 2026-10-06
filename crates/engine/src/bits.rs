//! Bitset view of cards and the owner of the card-play rule.
//!
//! `play_levels` (which cards may be played) and `takes` / `trick_winner` (who wins a trick)
//! are the single implementation of these rules: `Game` and `engine::fast` call them. Their
//! behaviour is pinned by the golden trace and the human replay.

use std::fmt;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not, Sub, SubAssign};

use crate::game::cards::{Card, Suit, Value};

pub(crate) use crate::game::cards::{SUITS, VALUES};

/// Seat helpers (seats are 0–3 clockwise; partners sit opposite).
pub const fn partner(seat: u8) -> u8 {
    (seat + 2) % 4
}

pub const fn next_seat(seat: u8) -> u8 {
    (seat + 1) % 4
}

/// A set of cards as 36 bits.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CardSet(pub u64);

impl CardSet {
    pub const EMPTY: CardSet = CardSet(0);
    pub const ALL: CardSet = CardSet((1 << 36) - 1);

    pub const fn single(index: u8) -> CardSet {
        CardSet(1 << index)
    }

    pub fn of(card: &Card) -> CardSet {
        CardSet::single(card.index())
    }

    pub fn from_cards<'a>(cards: impl IntoIterator<Item = &'a Card>) -> CardSet {
        cards.into_iter().fold(CardSet::EMPTY, |set, c| set | CardSet::of(c))
    }

    pub const fn suit(suit: Suit) -> CardSet {
        CardSet(0x1FF << (suit as u64 * 9))
    }

    /// King and Ober of a suit.
    pub const fn halves(suit: Suit) -> CardSet {
        let base = suit as u64 * 9;
        CardSet((1 << (base + Value::King as u64)) | (1 << (base + Value::Ober as u64)))
    }

    /// The four cards of one value.
    pub const fn value(value: Value) -> CardSet {
        let v = value as u64;
        CardSet((1 << v) | (1 << (v + 9)) | (1 << (v + 18)) | (1 << (v + 27)))
    }

    pub const fn contains(self, index: u8) -> bool {
        self.0 & (1 << index) != 0
    }

    pub const fn len(self) -> u32 {
        self.0.count_ones()
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn is_subset(self, other: CardSet) -> bool {
        self.0 & !other.0 == 0
    }

    /// Bit indices in ascending order.
    pub fn iter(self) -> impl Iterator<Item = u8> {
        let mut bits = self.0;
        std::iter::from_fn(move || {
            if bits == 0 {
                return None;
            }
            let i = bits.trailing_zeros() as u8;
            bits &= bits - 1;
            Some(i)
        })
    }

    pub fn cards(self) -> Vec<Card> {
        self.iter().map(Card::from_index).collect()
    }
}

impl BitOr for CardSet {
    type Output = CardSet;
    fn bitor(self, rhs: CardSet) -> CardSet {
        CardSet(self.0 | rhs.0)
    }
}

impl BitAnd for CardSet {
    type Output = CardSet;
    fn bitand(self, rhs: CardSet) -> CardSet {
        CardSet(self.0 & rhs.0)
    }
}

impl Sub for CardSet {
    type Output = CardSet;
    fn sub(self, rhs: CardSet) -> CardSet {
        CardSet(self.0 & !rhs.0)
    }
}

impl Not for CardSet {
    type Output = CardSet;
    fn not(self) -> CardSet {
        CardSet(!self.0 & CardSet::ALL.0)
    }
}

impl BitOrAssign for CardSet {
    fn bitor_assign(&mut self, rhs: CardSet) {
        self.0 |= rhs.0;
    }
}

impl BitAndAssign for CardSet {
    fn bitand_assign(&mut self, rhs: CardSet) {
        self.0 &= rhs.0;
    }
}

impl SubAssign for CardSet {
    fn sub_assign(&mut self, rhs: CardSet) {
        self.0 &= !rhs.0;
    }
}

impl fmt::Debug for CardSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.cards()).finish()
    }
}

/// The play rule for one decision: a hand may play `hand ∩ levels[k]` for the first
/// level `k` that meets the hand. Unused levels are `ALL` (a fixed length keeps `allowed`
/// a short unrolled loop).
#[derive(Clone, Copy, Debug)]
pub struct PlayLevels {
    levels: [CardSet; 6],
}

impl PlayLevels {
    /// The cards `hand` may play.
    #[inline]
    pub fn allowed(&self, hand: CardSet) -> CardSet {
        for level in self.levels {
            let allowed = level & hand;
            if !allowed.is_empty() {
                return allowed;
            }
        }
        hand
    }

    /// Cards the player cannot hold, given that they legally played `played`: every
    /// level before the first one that contains it was empty in their hand.
    pub fn excluded_by(&self, played: u8) -> CardSet {
        let mut excluded = CardSet::EMPTY;
        for level in self.levels {
            if level.contains(played) {
                return excluded;
            }
            excluded |= level;
        }
        excluded
    }
}

/// The seat that wins `trick` (bit indices in play order) led by `leader`, seats 0–3.
#[inline]
pub fn winning_seat(leader: u8, trick: &[u8], trump: Option<Suit>) -> u8 {
    (leader + trick_winner(trick, trump) as u8) & 3
}

/// Whether `challenger` takes the trick from the current `winner`.
#[inline]
pub fn takes(challenger: u8, winner: u8, trump: Option<Suit>) -> bool {
    let (cs, ws) = (challenger / 9, winner / 9);
    if cs == ws {
        return challenger > winner;
    }
    trump.is_some_and(|t| t as u8 == cs)
}

/// Position (0–3) of the card that currently wins `trick` (bit indices in play order).
/// The first card leads; panics on an empty trick.
#[inline]
pub fn trick_winner(trick: &[u8], trump: Option<Suit>) -> usize {
    let mut best = 0;
    for (i, &c) in trick.iter().enumerate().skip(1) {
        if takes(c, trick[best], trump) {
            best = i;
        }
    }
    best
}

/// The play rule: which cards may be played into `trick` (bit indices of the cards already
/// in the current trick, 0–3), given the trump and whether this is the game's first trick.
#[inline]
pub fn play_levels(trick: &[u8], trump: Option<Suit>, first_trick: bool) -> PlayLevels {
    let aces = CardSet::value(Value::Ace);
    let green = CardSet::suit(Suit::Green);
    let Some((&first, rest)) = trick.split_first() else {
        return if first_trick {
            PlayLevels {
                levels: [aces, green, CardSet::ALL, CardSet::ALL, CardSet::ALL, CardSet::ALL],
            }
        } else {
            PlayLevels {
                levels: [CardSet::ALL; 6],
            }
        };
    };
    let winner = rest.iter().fold(first, |w, &c| if takes(c, w, trump) { c } else { w });
    let led = first / 9;
    let led_suit = CardSet::suit(SUITS[led as usize]);
    let winner_suit = winner / 9;

    // Cards that take the trick: higher cards of the winner's suit, and every trump if the
    // winner is not a trump. (The former `Vec` rule compared the new high card with the old one
    // by `Card`'s derived order; for trumps over a non-trump that could exclude them there, but
    // the trump level below admits the same cards, so the allowed set is identical.)
    let above_winner = CardSet(CardSet::suit(SUITS[winner_suit as usize]).0 & !((2u64 << winner) - 1));
    let trumps_over = match trump {
        Some(t) if t as u8 != winner_suit => CardSet::suit(t),
        _ => CardSet::EMPTY,
    };
    let higher = above_winner | trumps_over;
    let led_ace = if first_trick {
        CardSet::single(led * 9 + Value::Ace as u8)
    } else {
        CardSet::EMPTY
    };
    let trump_suit = trump.map(CardSet::suit).unwrap_or(CardSet::EMPTY);
    PlayLevels {
        levels: [led_ace, higher & led_suit, led_suit, higher, trump_suit, CardSet::ALL],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::cards::{get_all_cards, CARDS};
    use rand::rngs::StdRng;
    use rand::seq::SliceRandom;
    use rand::{Rng, SeedableRng};

    #[test]
    fn index_round_trips() {
        for c in get_all_cards() {
            assert_eq!(Card::from_index(c.index()), c);
        }
        assert_eq!(CardSet::from_cards(&get_all_cards()), CardSet::ALL);
    }

    /// Properties of the play rule (its behaviour is pinned by the golden trace and the human
    /// replay): the allowed cards are held, never empty for a non-empty hand, and a legal play
    /// never excludes a held card.
    #[test]
    fn play_levels_properties() {
        let mut rng = StdRng::seed_from_u64(7);
        let trumps = [
            None,
            Some(Suit::Green),
            Some(Suit::Acorns),
            Some(Suit::Bells),
            Some(Suit::Red),
        ];
        for _ in 0..50_000 {
            let mut deck: Vec<u8> = (0..CARDS as u8).collect();
            deck.shuffle(&mut rng);
            let trick_len = rng.random_range(0..4);
            let hand_len = rng.random_range(1..10);
            let trick = &deck[..trick_len];
            let hand = deck[trick_len..trick_len + hand_len]
                .iter()
                .fold(CardSet::EMPTY, |h, &c| h | CardSet::single(c));
            let levels = play_levels(trick, trumps[rng.random_range(0..5)], rng.random_bool(0.3));
            let allowed = levels.allowed(hand);
            assert!(!allowed.is_empty() && allowed.is_subset(hand));
            for c in allowed.iter() {
                assert!(levels.excluded_by(c).is_subset(!hand));
            }
        }
    }
}

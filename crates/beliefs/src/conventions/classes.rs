//! The hand classes the conventions talk about (the hypothesis space of the bid model, §1.4),
//! and the card groups the pass models score.

use engine::{points_pair, CardSet, Suit, Value, SUITS};

/// The small cards ("Luschen"): 6, 7, 8, 9 and Unter (owner, C5). HP-8's "small cards".
pub const LUSCHE: CardSet = CardSet(
    CardSet::value(Value::Six).0
        | CardSet::value(Value::Seven).0
        | CardSet::value(Value::Eight).0
        | CardSet::value(Value::Nine).0
        | CardSet::value(Value::Unter).0,
);
pub const BIG_PAIRS: [Suit; 2] = [Suit::Bells, Suit::Red];
pub const SMALL_PAIRS: [Suit; 2] = [Suit::Green, Suit::Acorns];

pub fn pair_value(suit: Suit) -> i32 {
    points_pair(suit).0
}

pub fn len_in(set: CardSet, suit: Suit) -> u32 {
    (set & CardSet::suit(suit)).len()
}

/// The bidding-relevant class of a hand: an ace or not, and per suit no half / one half /
/// the pair. 2 × 3⁴ = 162 classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandClass {
    pub ace: bool,
    /// Per suit (engine order): 0 = no half, 1 = one half, 2 = the pair.
    pub halves: [u8; 4],
}

pub const N_CLASSES: usize = 162;

impl HandClass {
    pub fn of(hand: CardSet) -> HandClass {
        HandClass {
            ace: !(hand & CardSet::ACES).is_empty(),
            halves: SUITS.map(|s| (hand & CardSet::halves(s)).len() as u8),
        }
    }

    pub fn index(self) -> usize {
        let h = self.halves;
        (self.ace as usize) * 81 + h[0] as usize * 27 + h[1] as usize * 9 + h[2] as usize * 3 + h[3] as usize
    }

    pub fn from_index(i: usize) -> HandClass {
        HandClass {
            ace: i >= 81,
            halves: [(i / 27) % 3, (i / 9) % 3, (i / 3) % 3, i % 3].map(|x| x as u8),
        }
    }

    pub fn has_pair(self, suit: Suit) -> bool {
        self.halves[suit as usize] == 2
    }

    pub fn n_halves(self) -> u32 {
        self.halves.iter().map(|&h| h as u32).sum()
    }

    pub fn any_pair(self) -> bool {
        self.halves.contains(&2)
    }
}

/// Class probabilities from independent per-card holding probabilities.
pub fn class_prior(q: &[f32; 36]) -> [f32; N_CLASSES] {
    let no_ace: f32 = CardSet::ACES.iter().map(|a| 1.0 - q[a as usize]).product();
    let per_suit = SUITS.map(|s| {
        let [o, k] = [Value::Ober, Value::King].map(|value| q[engine::Card { suit: s, value }.index() as usize]);
        [(1.0 - o) * (1.0 - k), o * (1.0 - k) + k * (1.0 - o), o * k]
    });
    let mut prior = [0.0; N_CLASSES];
    for (i, p) in prior.iter_mut().enumerate() {
        let c = HandClass::from_index(i);
        *p = if c.ace { 1.0 - no_ace } else { no_ace };
        for s in 0..4 {
            *p *= per_suit[s][c.halves[s] as usize];
        }
    }
    prior
}

/// Features of a class distribution: P(ace), P(pair) and P(≥ 1 half) per suit.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClassFeatures {
    pub ace: f32,
    pub pair: [f32; 4],
    pub half: [f32; 4],
}

pub fn class_features(post: &[f32; N_CLASSES]) -> ClassFeatures {
    let mut b = ClassFeatures::default();
    for (i, &w) in post.iter().enumerate() {
        let c = HandClass::from_index(i);
        if c.ace {
            b.ace += w;
        }
        for s in 0..4 {
            if c.halves[s] == 2 {
                b.pair[s] += w;
            }
            if c.halves[s] >= 1 {
                b.half[s] += w;
            }
        }
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_round_trip_and_prior_sums_to_one() {
        for i in 0..N_CLASSES {
            assert_eq!(HandClass::from_index(i).index(), i);
        }
        let prior = class_prior(&[0.25; 36]);
        assert!((prior.iter().sum::<f32>() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn lusche_is_six_to_unter() {
        assert_eq!(LUSCHE.len(), 20);
        assert!((LUSCHE & (CardSet::HALVES | CardSet::ACES | CardSet::TENS)).is_empty());
    }
}

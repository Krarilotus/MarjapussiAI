//! C42 pass features and fitted weights. Pure calculations from the giver's hand
//! and its estimates of the partner: no inference, action selection, or hidden state.

use beliefs::conventions::{pair_value, LUSCHE};
use engine::{points_set, suit_of, CardSet, Suit, SUITS};

use crate::hand::top;

/// The giver's current estimates, not the recipient's private information.
#[derive(Clone, Debug)]
pub struct Partner {
    pub cards: [f32; engine::CARDS],
    pub ace: f32,
    pub void: [f32; 4],
    pub half: [f32; 4],
}

/// The pass forth's features, in the order of `PASSING_WEIGHTS.forth`.
pub const FORTH_FEATURES: [&str; 13] = [
    "lusche_from_kept_suit",
    "pair_broken",
    "blank_with_lusche",
    "blank_without_lusche",
    "points",
    "tens_guarded",
    "tens_blank",
    "aces",
    "aces_x_vh_lacks_ace",
    "pairs_known",
    "pairs_unknown",
    "halves_x_completes",
    "green",
];

/// The pass back's features, in the order of `PASSING_WEIGHTS.back`.
pub const BACK_FEATURES: [&str; 17] = [
    "standing_after",
    "aces_entry_low_kept",
    "pair_broken",
    "lusche_from_kept_suit",
    "voids_made",
    "halves",
    "side_suits_complete",
    "aces",
    "points",
    "pairs",
    "halves_x_completes",
    "plan_top_trump_kept",
    "plan_trump_given",
    "aces_x_mh_holds_suit",
    "green",
    "tens",
    "aces_back_over_half",
];

/// KB `passing_weights`, per unit of each feature.
pub struct PassingWeights {
    pub forth: [f64; 13],
    pub back: [f64; 17],
}

/// From the human choice analysis (conditional logit over every option of the 1,575 human
/// passes each way, `tools/kbstudy/passfit.py`, variants `broken`, and `acesplit` for the pass back), owner-requested ("We should
/// try it, i'll review it through the policy comparison to humans"). Empirically fitted;
/// held-out playing strength has not been established.
pub const PASSING_WEIGHTS: PassingWeights = PassingWeights {
    forth: [
        -3.60, -5.37, 2.68, 2.70, 0.16, -1.10, -2.30, -0.73, 1.12, -1.13, 0.84, 0.28, -0.20,
    ],
    back: [
        1.46, 2.58, -2.06, -1.47, 1.72, -0.67, 1.02, -0.59, -0.04, 1.14, 1.68, -0.29, 0.13, -0.13, 0.04, -0.05,
        -0.61,
    ],
};

/// The other half of the pair of half `h`.
fn other_half(h: u8) -> u8 {
    let pair = CardSet::halves(suit_of(h));
    (pair - CardSet::single(h))
        .iter()
        .next()
        .expect("a pair has two halves")
}

/// How the pass reads suit by suit (F8/F9): suits made blank with a Lusche among the passed
/// cards, suits made blank without one, and suits I keep from which I pass a Lusche.
fn lusche_counts(hand: CardSet, x: CardSet) -> (f64, f64, f64) {
    SUITS.into_iter().fold((0.0, 0.0, 0.0), |(with, without, kept), s| {
        let held = hand & CardSet::suit(s);
        let blank = !held.is_empty() && (held - x).is_empty();
        let lusche = !(x & held & LUSCHE).is_empty();
        (
            with + (blank && lusche) as u8 as f64,
            without + (blank && !lusche) as u8 as f64,
            kept + (!blank && lusche) as u8 as f64,
        )
    })
}

/// `pair_broken`: my pairs of which the option passes exactly one half (our derivation from
/// the rules: a broken pair can't be announced; the owner's F7 "keep known or big pairs").
fn pair_broken(hand: CardSet, x: CardSet) -> f64 {
    SUITS
        .into_iter()
        .filter(|&s| CardSet::halves(s).is_subset(hand) && (x & CardSet::halves(s)).len() == 1)
        .count() as f64
}

/// The pass forth's features of passing `x` (MH; the partner is VH), with `known_pairs` from my bids.
pub fn forth_features(hand: CardSet, partner: &Partner, known_pairs: [bool; 4], x: CardSet) -> [f64; 13] {
    let (blank_with, blank_without, lusche_kept) = lusche_counts(hand, x);
    let aces = (x & CardSet::ACES).len() as f64;
    let completes: f64 = (x & CardSet::HALVES)
        .iter()
        .map(|h| partner.cards[other_half(h) as usize] as f64)
        .sum();
    let pairs = |known: bool| {
        SUITS
            .into_iter()
            .filter(|&s| CardSet::halves(s).is_subset(x) && known_pairs[s as usize] == known)
            .count() as f64
    };
    let blank_ten = |t: u8| (hand & CardSet::suit(suit_of(t))).len() == 1;
    let tens = x & CardSet::TENS;
    [
        lusche_kept,
        pair_broken(hand, x),
        blank_with,
        blank_without,
        points_set(x) as f64,
        tens.iter().filter(|&t| !blank_ten(t)).count() as f64,
        tens.iter().filter(|&t| blank_ten(t)).count() as f64,
        aces,
        aces * (1.0 - partner.ace as f64),
        pairs(true),
        pairs(false),
        completes,
        (x & CardSet::suit(Suit::Green)).len() as f64,
    ]
}

/// The highest-value held pair, else a single half the partner likely completes.
/// The estimate is P(partner holds a half of that suit), preserving the existing rule.
pub fn back_plan(hand: CardSet, partner_half: [f32; 4], min_confidence: f32) -> Option<Suit> {
    hand.pair_suits().max_by_key(|&s| pair_value(s)).or_else(|| {
        SUITS
            .into_iter()
            .filter(|&s| (hand & CardSet::halves(s)).len() == 1 && partner_half[s as usize] >= min_confidence)
            .max_by_key(|&s| pair_value(s))
    })
}

/// The pass back's features of giving back `y` (VH; the partner is MH) under the selected trump plan.
pub fn back_features(hand: CardSet, partner: &Partner, trump: Option<Suit>, y: CardSet) -> [f64; 17] {
    let kept = hand - y;
    let emptied = |s: Suit| !(hand & CardSet::suit(s)).is_empty() && (kept & CardSet::suit(s)).is_empty();
    let (trump_given, top_kept) = match trump {
        None => (0, false),
        Some(t) => {
            let held = hand & CardSet::suit(t);
            // My highest card of the plan suit: the one no other of mine beats.
            ((y & held).len(), top(held).is_some_and(|c| kept.contains(c)))
        }
    };
    let aces = y & CardSet::ACES;
    let halves = y & CardSet::HALVES;
    let (_, _, lusche_kept) = lusche_counts(hand, y);
    [
        knowledge::own_standing_of(kept).len() as f64,
        aces.iter()
            .filter(|&a| !(kept & CardSet::suit(suit_of(a))).is_empty())
            .count() as f64,
        pair_broken(hand, y),
        lusche_kept,
        SUITS.into_iter().filter(|&s| emptied(s)).count() as f64,
        halves.len() as f64,
        SUITS.into_iter().filter(|&s| Some(s) != trump && emptied(s)).count() as f64,
        aces.len() as f64,
        points_set(y) as f64,
        SUITS.into_iter().filter(|&s| CardSet::halves(s).is_subset(y)).count() as f64,
        halves
            .iter()
            .map(|h| partner.cards[other_half(h) as usize] as f64)
            .sum(),
        top_kept as u8 as f64,
        trump_given as f64,
        aces.iter()
            .map(|a| 1.0 - partner.void[suit_of(a) as usize] as f64)
            .sum(),
        (y & CardSet::suit(Suit::Green)).len() as f64,
        (y & CardSet::TENS).len() as f64,
        // K5b (owner): aces go back for schnippeln, about half of mine rounded up.
        (aces.len() as f64 - (hand & CardSet::ACES).len().div_ceil(2) as f64).max(0.0),
    ]
}

/// The score Σ wᵢ·fᵢ.
pub fn score<const N: usize>(weights: &[f64; N], features: &[f64; N]) -> f64 {
    weights.iter().zip(features).map(|(w, f)| w * f).sum()
}

//! Passing (KB batch 2, C42): every legal option x gets the concept features of the human
//! choice analysis (`tools/kbstudy/passfit.py`, `findings/kb/03-passing-data.md`), score(x) =
//! Σ wᵢ·fᵢ(x) with `PASSING_WEIGHTS`, and P(x) = softmax(score). HP1 passes the argmax (the
//! first legal option on a tie, as the analysis does), or samples by `Params.selection`.
//!
//! The reading of a partner's pass still inverts the old pass models
//! (`beliefs::conventions::passes`) until the concept-level reading R1–R4 is built.

use beliefs::conventions::passes::{plan, BackCtx, ForthCtx, Plan};
use beliefs::conventions::{forth_ctx, LUSCHE};
use beliefs::Mind;
use engine::{partner, points_set, suit_of, takes, ActionType, CardSet, GameAction, Suit, HAND_SIZE, SUITS};

use crate::rules::RuleId;
use crate::{decision, fact, Ctx, Decision};

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
pub const BACK_FEATURES: [&str; 16] = [
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
];

/// KB `passing_weights`, per unit of each feature.
pub struct PassingWeights {
    pub forth: [f64; 13],
    pub back: [f64; 16],
}

/// From the human choice analysis (conditional logit over every option of the 1,575 human
/// passes each way, `tools/kbstudy/passfit.py`, variant `broken`), owner-requested ("We should
/// try it, i'll review it through the policy comparison to humans"). Not tuned.
pub const PASSING_WEIGHTS: PassingWeights = PassingWeights {
    forth: [
        -3.60, -5.37, 2.68, 2.70, 0.16, -1.10, -2.30, -0.73, 1.12, -1.13, 0.84, 0.28, -0.20,
    ],
    back: [
        1.46, 2.56, -2.06, -1.47, 1.71, -0.67, 1.03, -0.62, -0.04, 1.14, 1.68, -0.29, 0.13, -0.14, 0.04, -0.05,
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

/// The pass forth's features of passing `x` (MH; the partner is VH), with `ctx` from my bids.
pub fn forth_features(mind: &Mind, ctx: &ForthCtx, x: CardSet) -> [f64; 13] {
    let hand = mind.knowledge().certain(mind.seat());
    let vh = partner(mind.seat());
    let (blank_with, blank_without, lusche_kept) = lusche_counts(hand, x);
    let aces = (x & CardSet::ACES).len() as f64;
    let completes: f64 = (x & CardSet::HALVES)
        .iter()
        .map(|h| mind.p_holds(vh, other_half(h)) as f64)
        .sum();
    let pairs = |known: bool| {
        SUITS
            .into_iter()
            .filter(|&s| CardSet::halves(s).is_subset(x) && ctx.partner_knows_pair[s as usize] == known)
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
        aces * (1.0 - mind.features(vh).ace as f64),
        pairs(true),
        pairs(false),
        completes,
        (x & CardSet::suit(Suit::Green)).len() as f64,
    ]
}

/// VH's plan for the pass back (§5.2): a trump suit or none.
pub fn back_plan(mind: &Mind, params: &beliefs::conventions::Params) -> Plan {
    let ctx = BackCtx {
        partner_half: mind.features(partner(mind.seat())).half,
    };
    plan(mind.knowledge().certain(mind.seat()), &ctx, params)
}

/// The pass back's features of giving back `y` (VH; the partner is MH) under `plan`.
pub fn back_features(mind: &Mind, plan: Plan, y: CardSet) -> [f64; 16] {
    let hand = mind.knowledge().certain(mind.seat());
    let mh = partner(mind.seat());
    let kept = hand - y;
    let trump = match plan {
        Plan::Trump(t) => Some(t),
        Plan::NoTrump => None,
    };
    let emptied = |s: Suit| !(hand & CardSet::suit(s)).is_empty() && (kept & CardSet::suit(s)).is_empty();
    let (trump_given, top_kept) = match trump {
        None => (0, false),
        Some(t) => {
            let held = hand & CardSet::suit(t);
            // My highest card of the plan suit: the one no other of mine beats.
            let top = held.iter().find(|&c| held.iter().all(|b| b == c || !takes(b, c, None)));
            ((y & held).len(), top.is_some_and(|c| kept.contains(c)))
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
        halves.iter().map(|h| mind.p_holds(mh, other_half(h)) as f64).sum(),
        top_kept as u8 as f64,
        trump_given as f64,
        aces.iter().map(|a| 1.0 - mind.p_void(mh, suit_of(a)) as f64).sum(),
        (y & CardSet::suit(Suit::Green)).len() as f64,
        (y & CardSet::TENS).len() as f64,
    ]
}

/// The score Σ wᵢ·fᵢ.
pub fn score(weights: &[f64], features: &[f64]) -> f64 {
    weights.iter().zip(features).map(|(w, f)| w * f).sum()
}

/// Acting: score every legal pass and take the best (or sample, `Params.selection`).
pub(crate) fn decide(ctx: &Ctx, legal: &[GameAction]) -> Decision {
    let p = ctx.params.conv();
    let hand = ctx.k.certain(ctx.me);
    let opts: Vec<(&GameAction, CardSet)> = legal
        .iter()
        .filter_map(|a| match &a.action_type {
            ActionType::Pass(cards) => Some((a, CardSet::from_cards(cards))),
            _ => None,
        })
        .collect();
    let w = &PASSING_WEIGHTS;
    let (features, weights, names, rule): (Vec<Vec<f64>>, &[f64], &[&'static str], RuleId) =
        if hand.len() as usize == HAND_SIZE {
            let fctx = forth_ctx(&ctx.mind.record().seats, ctx.me, p);
            (
                opts.iter()
                    .map(|&(_, x)| forth_features(ctx.mind, &fctx, x).to_vec())
                    .collect(),
                &w.forth,
                &FORTH_FEATURES,
                RuleId::PForth,
            )
        } else {
            let plan = back_plan(ctx.mind, p);
            (
                opts.iter()
                    .map(|&(_, y)| back_features(ctx.mind, plan, y).to_vec())
                    .collect(),
                &w.back,
                &BACK_FEATURES,
                RuleId::PBack,
            )
        };
    let scores: Vec<f64> = features.iter().map(|f| score(weights, f)).collect();
    let i = if ctx.params.selection() == crate::Selection::Argmax {
        // The first of the best, in f64 as the analysis computes it.
        (0..scores.len())
            .max_by(|&a, &b| scores[a].total_cmp(&scores[b]).then(b.cmp(&a)))
            .unwrap_or(0)
    } else {
        ctx.select(&scores.iter().map(|&s| s as f32).collect::<Vec<_>>())
    };
    let mut why = vec![fact("score", scores[i])];
    why.extend(
        names
            .iter()
            .zip(weights.iter().zip(&features[i]))
            .filter(|(_, (_, &f))| f != 0.0)
            .map(|(&n, (&wt, &f))| fact(n, wt * f)),
    );
    decision(opts[i].0, rule, why)
}

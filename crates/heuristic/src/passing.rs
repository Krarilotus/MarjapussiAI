//! C42 pass choice: extract the giver's information, score legal options, then select.
//! The active belief model does not yet infer intent from observed pass choices.

mod features;
pub use features::{
    back_features, back_plan, forth_features, score, Partner, PassingWeights, BACK_FEATURES, FORTH_FEATURES,
    PASSING_WEIGHTS,
};

use beliefs::conventions::{BidObs, Message};
use beliefs::Mind;
use engine::{partner, ActionType, CardSet, GameAction, Suit, HAND_SIZE, SUITS};

use crate::rules::RuleId;
use crate::{decision, fact, Ctx, Decision};

impl Partner {
    /// Snapshot only the acting giver's estimates. Features never receive the table's hands.
    pub fn of(mind: &Mind) -> Self {
        let seat = partner(mind.seat());
        let features = mind.features(seat);
        Self {
            cards: mind.holds_row(seat),
            ace: features.ace,
            void: SUITS.map(|s| mind.p_void(seat, s)),
            half: features.half,
        }
    }
}

/// Suits covered by my own nominal pair messages. This is the existing convention's
/// "partner knows" feature, not a claim that the partner is certain I hold that pair.
pub fn known_pairs(bids: &[BidObs]) -> [bool; 4] {
    let sent = bids.iter().fold(0u8, |a, o| a | o.nominal.map_or(0, |m| m.bit()));
    SUITS.map(|s| match s {
        Suit::Green | Suit::Acorns => sent & Message::SmallOrHalves.bit() != 0,
        Suit::Bells => sent & Message::BigPair.bit() != 0,
        Suit::Red => sent & (Message::BigPair.bit() | Message::RedPair.bit()) != 0,
    })
}

pub(crate) fn decide(ctx: &Ctx, legal: &[GameAction]) -> Decision {
    let hand = ctx.k.certain(ctx.me);
    let partner = Partner::of(ctx.mind);
    if hand.len() as usize == HAND_SIZE {
        let known = known_pairs(&ctx.mind.record().seats[ctx.me as usize].bids);
        choose(
            ctx,
            legal,
            &PASSING_WEIGHTS.forth,
            &FORTH_FEATURES,
            RuleId::PForth,
            |x| forth_features(hand, &partner, known, x),
        )
        .0
    } else {
        let trump = back_plan(hand, partner.half, ctx.params.partner_half_conf);
        choose(ctx, legal, &PASSING_WEIGHTS.back, &BACK_FEATURES, RuleId::PBack, |y| {
            back_features(hand, &partner, trump, y)
        })
        .0
    }
}

/// Keep scores for selection, and calculate an explanation only for the chosen option.
fn choose<const N: usize>(
    ctx: &Ctx,
    legal: &[GameAction],
    weights: &[f64; N],
    names: &[&'static str; N],
    rule: RuleId,
    features: impl Fn(CardSet) -> [f64; N],
) -> (Decision, Vec<(GameAction, CardSet, f64)>) {
    let opts: Vec<_> = legal
        .iter()
        .filter_map(|a| match &a.action_type {
            ActionType::Pass(cards) => {
                let given = CardSet::from_cards(cards);
                Some((*a, given, score(weights, &features(given))))
            }
            _ => None,
        })
        .collect();
    let i = if ctx.params.selection() == crate::Selection::Argmax {
        // Preserve f64 precision and the first legal option on ties.
        (0..opts.len())
            .max_by(|&a, &b| opts[a].2.total_cmp(&opts[b].2).then(b.cmp(&a)))
            .unwrap_or(0)
    } else {
        ctx.select(&opts.iter().map(|o| o.2 as f32).collect::<Vec<_>>())
    };
    let chosen = decision(&opts[i].0, rule, explain(weights, names, features(opts[i].1)));
    (chosen, opts)
}

fn explain<const N: usize>(weights: &[f64; N], names: &[&'static str; N], selected: [f64; N]) -> Vec<crate::Fact> {
    let mut why = vec![fact("score", score(weights, &selected))];
    why.extend(
        names
            .iter()
            .zip(weights.iter().zip(selected))
            .filter(|(_, (_, f))| *f != 0.0)
            .map(|(&name, (&weight, value))| fact(name, weight * value)),
    );
    why
}

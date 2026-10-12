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

impl Partner {
    /// What a giver holding `reader` reads about its partner from the auction alone: the
    /// partner's class likelihood (`Mind::class_likelihood`) over the uniform deal of the
    /// cards the giver doesn't see (`beliefs::conventions::read_class`); voids by counting.
    /// The level-0 exchange of the outlook's take-or-defend routes uses it.
    pub fn read(reader: CardSet, likelihood: Option<&[f32; beliefs::conventions::N_CLASSES]>) -> Self {
        let (features, cards) = beliefs::conventions::read_class(reader, HAND_SIZE, likelihood);
        Self {
            cards,
            ace: features.ace,
            void: Self::uniform(reader).void,
            half: features.half,
        }
    }

    /// What a giver can estimate about its partner from its own hand alone: the partner holds 9
    /// of the 27 cards it doesn't see, uniformly (P(card) = 1/3; an ace, a void, a half by
    /// counting).
    pub fn uniform(hand: CardSet) -> Self {
        let unseen = 36 - hand.len() as usize;
        let held = HAND_SIZE;
        // P(the partner holds none of `n` given unseen cards) = C(unseen - n, 9) / C(unseen, 9).
        let none = |n: u32| -> f32 {
            (0..held)
                .map(|i| (unseen - n as usize).saturating_sub(i) as f32 / (unseen - i) as f32)
                .product()
        };
        let p = held as f32 / unseen as f32;
        let mut cards = [0.0; engine::CARDS];
        for c in (CardSet::ALL - hand).iter() {
            cards[c as usize] = p;
        }
        Self {
            cards,
            ace: 1.0 - none((CardSet::ACES - hand).len()),
            void: SUITS.map(|s| none((CardSet::suit(s) - hand).len())),
            half: SUITS.map(|s| 1.0 - none((CardSet::halves(s) - hand).len())),
        }
    }
}

/// The best option of `hand` by the passing store: every 4-card set, scored by `features`.
fn best_pass<const N: usize>(hand: CardSet, weights: &[f64; N], features: impl Fn(CardSet) -> [f64; N]) -> CardSet {
    let cards: Vec<u8> = hand.iter().collect();
    let mut best = (f64::NEG_INFINITY, CardSet::EMPTY);
    for a in 0..cards.len() {
        for b in a + 1..cards.len() {
            for c in b + 1..cards.len() {
                for d in c + 1..cards.len() {
                    let x = [cards[a], cards[b], cards[c], cards[d]]
                        .iter()
                        .fold(CardSet::EMPTY, |s, &k| s | CardSet::single(k));
                    let v = score(weights, &features(x));
                    if v > best.0 {
                        best = (v, x);
                    }
                }
            }
        }
    }
    best.1
}

/// The level-0 exchange of a deal in which `declarer` plays: its partner passes forth and it
/// passes back by the passing store, each reading the other only from the auction
/// (`Partner::read` with `likelihood[seat]`, the seat's class likelihood as its partner reads
/// it) and knowing its own pair messages (`known[seat]`). Returns the hands after the exchange.
pub fn level0_exchange(
    mut hands: [CardSet; 4],
    declarer: u8,
    partner_half_conf: f32,
    likelihood: &[Option<Box<[f32; beliefs::conventions::N_CLASSES]>>; 4],
    known: [[bool; 4]; 4],
) -> [CardSet; 4] {
    let (d, p) = (declarer as usize, partner(declarer) as usize);
    let giver = hands[p];
    let reads_declarer = Partner::read(giver, likelihood[d].as_deref());
    let forth = best_pass(giver, &PASSING_WEIGHTS.forth, |x| {
        forth_features(giver, &reads_declarer, known[p], x)
    });
    hands[p] -= forth;
    hands[d] |= forth;
    let taker = hands[d];
    // The giver's dealt hand as the declarer reads it from its dealt cards; the cards received
    // are certain now.
    let mut estimate = Partner::read(taker - forth, likelihood[p].as_deref());
    forth.iter().for_each(|c| estimate.cards[c as usize] = 0.0);
    let trump = back_plan(taker, estimate.half, partner_half_conf);
    let back = best_pass(taker, &PASSING_WEIGHTS.back, |y| {
        back_features(taker, &estimate, trump, y)
    });
    hands[d] -= back;
    hands[p] |= back;
    hands
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

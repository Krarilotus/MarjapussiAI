//! Reading a visible pass (§1.4, C5): the public context the giver chose its pass in, and
//! `P(pass | the giver's hand)` from the same HP/HK score the giver acts with. Moved from
//! `heuristic::beliefs::pass_read` (D-d).

use engine::{partner, CardSet, Suit, SUITS};
use knowledge::Knowledge;
use observation::Seat;

use super::bids::{class_posterior, Message};
use super::classes::class_features;
use super::passes::{self, BackCtx, ForthCtx, PassModel};
use super::{Params, SeatSignals};
use crate::record::PassEvent;

/// The public context of MH's pass forth (the same for MH and every reader, §1.4).
pub fn forth_ctx(seats: &[SeatSignals; 4], mh: Seat, params: &Params) -> ForthCtx {
    let vh = partner(mh);
    let theirs = public_features(seats, vh, params);
    let sent = |y: Seat| {
        seats[y as usize]
            .bids
            .iter()
            .fold(0u8, |a, o| a | o.nominal.map_or(0, |m| m.bit()))
    };
    let (my_sent, vh_sent) = (sent(mh), sent(vh));
    ForthCtx {
        partner_knows_ace: my_sent & Message::Ace.bit() != 0,
        // HP-6 "the partner knows of my pair" (C3: unless it "has absolutely no clue it exists"):
        // I sent a message that covers a pair of that suit (SmallOrHalves: Green/Acorns, BigPair:
        // Bells/Red, RedPair: Red).
        partner_knows_pair: SUITS.map(|s| match s {
            Suit::Green | Suit::Acorns => my_sent & Message::SmallOrHalves.bit() != 0,
            Suit::Bells => my_sent & Message::BigPair.bit() != 0,
            Suit::Red => my_sent & (Message::BigPair.bit() | Message::RedPair.bit()) != 0,
        }),
        partner_showed_ace: vh_sent & Message::Ace.bit() != 0,
        partner_showed_halves: vh_sent & !Message::Ace.bit() != 0,
        partner_pair: theirs.pair.map(|p| p >= 0.5),
        partner_bid: seats[vh as usize]
            .bids
            .iter()
            .rev()
            .find_map(|o| o.bid)
            .expect("VH is the declarer, so it has bid"),
    }
}

/// The context of VH's pass back as a reader rebuilds it: MH's halves from its public signals.
pub fn back_ctx(seats: &[SeatSignals; 4], mh: Seat, params: &Params) -> BackCtx {
    BackCtx {
        partner_half: public_features(seats, mh, params).half,
    }
}

/// A seat's class features as everyone reads them from its bids alone (a uniform card prior).
fn public_features(seats: &[SeatSignals; 4], y: Seat, params: &Params) -> super::ClassFeatures {
    let s = &seats[y as usize];
    class_features(&class_posterior(
        &[0.25; 36],
        &s.bids,
        params.trust(s.kind, true),
        s.kind,
        params,
    ))
}

/// The visible pass a seat reads (C5): VH reads MH's pass forth, MH reads VH's pass back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PassReading {
    /// VH reads MH's (the giver's) pass forth `x`.
    Forth { giver: Seat, x: CardSet, ctx: ForthCtx },
    /// MH reads VH's (the giver's) pass back `y`, after MH gave `x`.
    Back {
        giver: Seat,
        x: CardSet,
        y: CardSet,
        ctx: BackCtx,
    },
}

impl PassReading {
    /// The pass `k.me()` can read now, if any.
    pub fn of(k: &Knowledge, passes: &[PassEvent], seats: &[SeatSignals; 4], params: &Params) -> Option<PassReading> {
        let me = k.me();
        let giver = partner(me);
        if k.declarer() == Some(me) {
            let x = passes.first().filter(|p| p.from == giver)?.cards?;
            Some(PassReading::Forth {
                giver,
                x,
                ctx: forth_ctx(seats, giver, params),
            })
        } else if k.declarer() == Some(giver) {
            let x = passes.first().filter(|p| p.from == me)?.cards?;
            let y = passes.get(1)?.cards?;
            Some(PassReading::Back {
                giver,
                x,
                y,
                ctx: back_ctx(seats, me, params),
            })
        } else {
            None
        }
    }

    pub fn giver(&self) -> Seat {
        match *self {
            PassReading::Forth { giver, .. } | PassReading::Back { giver, .. } => giver,
        }
    }

    /// The cards the giver passed.
    pub fn passed(&self) -> CardSet {
        match *self {
            PassReading::Forth { x, .. } => x,
            PassReading::Back { y, .. } => y,
        }
    }

    /// The giver's hand the pass was chosen from, given its dealt hand.
    fn chosen_from(&self, dealt: CardSet) -> CardSet {
        match *self {
            PassReading::Forth { .. } => dealt,
            PassReading::Back { x, .. } => dealt | x,
        }
    }

    /// `P(the observed pass | the giver's dealt hand)` under the giver's own pass model
    /// (softmax at `pass_temperature`), mixed with the trust θ and floored at ε × uniform: an
    /// odd pass only down-weights a hand. `None` if the pass is impossible from that hand.
    /// `trust`: the reader's trust in the giver (`Params::trust` of its kind).
    pub fn likelihood(&self, dealt: CardSet, trust: f32, params: &Params, scores: &mut Vec<f32>) -> Option<f32> {
        let hand = self.chosen_from(dealt);
        match *self {
            PassReading::Forth { x, ctx, .. } => pass_likelihood(
                &passes::ForthModel::new(hand, &ctx, params),
                hand,
                x,
                trust,
                params,
                scores,
            ),
            PassReading::Back { y, ctx, .. } => pass_likelihood(
                &passes::BackModel::new(hand, passes::plan(hand, &ctx, params), params),
                hand,
                y,
                trust,
                params,
                scores,
            ),
        }
    }
}

fn pass_likelihood<M: PassModel>(
    model: &M,
    hand: CardSet,
    passed: CardSet,
    trust: f32,
    params: &Params,
    scores: &mut Vec<f32>,
) -> Option<f32> {
    scores.clear();
    let mut chosen = None;
    passes::for_each_option(hand, |o| {
        if o == passed {
            chosen = Some(scores.len());
        }
        scores.push(model.total(o));
    });
    let p = passes::softmax_prob(scores, chosen?, params.pass_temperature);
    let uniform = 1.0 / scores.len() as f32;
    let read = trust * p + (1.0 - trust) * uniform;
    Some(read.max(params.convention_floor * uniform))
}

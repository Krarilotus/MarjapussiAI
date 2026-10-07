//! The planned leads of M3 (Judge item 5): each names its condition and why it gains over the
//! fallback HL-F (the cheapest lead). `play::lead` asks `before_standing` before cashing a
//! standing card (HL-2) and `after_standing` after it, before HL-7 / HL-F.

use engine::opponents;
use engine::CARD_POINTS;
use engine::{partner, CardSet, SUITS};
use engine::{Card, Suit, Value};

use crate::hand::{higher, top};
use crate::rules::RuleId;
use crate::valuation::Valuation;
use crate::{fact, Ctx, Fact};

type Lead = (u8, RuleId, Vec<Fact>);

fn ace_of(suit: Suit) -> u8 {
    Card {
        suit,
        value: Value::Ace,
    }
    .index()
}

fn lowest(set: CardSet) -> Option<u8> {
    set.iter().min_by_key(|&c| (CARD_POINTS[c as usize], c))
}

/// Trumps that may still beat `c` from the opponents (tier 0).
fn opp_trump_over(ctx: &Ctx, c: u8) -> CardSet {
    let [l, r] = opponents(ctx.me);
    (ctx.k.may_hold(l) | ctx.k.may_hold(r)) & higher(c)
}

pub(crate) fn before_standing(ctx: &Ctx, legal: CardSet) -> Option<Lead> {
    schnippeln(ctx, legal).or_else(|| last_trick(ctx, legal))
}

pub(crate) fn after_standing(ctx: &Ctx, legal: CardSet, val: &Valuation) -> Option<Lead> {
    destroy_pair(ctx, legal, val)
        .or_else(|| partner_ruffs(ctx, legal, val))
        .or_else(|| draw_trumps(ctx, legal))
}

/// HL-1 (P9, schnippeln): trick 1, no ace to lead, and the partner **certainly** holds the ace
/// of a suit I can lead low (tier 0, e.g. I gave it back). *Why:* the next opponent must
/// overtake my low card, and the partner must play the led suit's ace in trick 1: our trick,
/// with the opponent's higher card in it.
fn schnippeln(ctx: &Ctx, legal: CardSet) -> Option<Lead> {
    if !ctx.params.lead_schnippeln || !ctx.k.played().is_empty() || !(legal & CardSet::ACES).is_empty() {
        return None;
    }
    let mate = partner(ctx.me);
    let (s, c) = SUITS
        .into_iter()
        .filter(|&s| ctx.k.certain(mate).contains(ace_of(s)))
        .filter_map(|s| lowest(legal & CardSet::suit(s)).map(|c| (s, c)))
        .min_by_key(|&(_, c)| (CARD_POINTS[c as usize], c))?;
    Some((c, RuleId::HL1, vec![fact("suit", s as u8)]))
}

/// HL-6: two cards left and one of them is the top trump: lead the other, so the top trump
/// takes the last trick (+20). *Why:* the top trump wins trick 9 whoever leads it; cashed now,
/// my last card would have to win trick 9 on its own.
fn last_trick(ctx: &Ctx, legal: CardSet) -> Option<Lead> {
    let t = ctx.k.trump()?;
    if !ctx.params.lead_last_trick || ctx.k.hand_len(ctx.me) != 2 || legal.len() != 2 {
        return None;
    }
    let trumps = legal & CardSet::suit(t);
    let topt = trumps.iter().find(|&c| opp_trump_over(ctx, c).is_empty())?;
    let other = (legal - CardSet::single(topt)).iter().next()?;
    if trumps.contains(other) && opp_trump_over(ctx, other).is_empty() {
        return None;
    }
    Some((other, RuleId::HL6, vec![fact("kept", topt)]))
}

/// HL-5: an opponent likely holds an uncalled pair in a side suit where I hold the ace or the
/// ten: lead it. *Why:* the opponent must follow suit, and with few cards there it gives up K
/// or O; the pair can no longer be announced and its points are in our trick.
fn destroy_pair(ctx: &Ctx, legal: CardSet, val: &Valuation) -> Option<Lead> {
    if !ctx.params.lead_destroy_pair {
        return None;
    }
    let trump = ctx.k.trump();
    let (p, c) = SUITS
        .into_iter()
        .filter(|&s| Some(s) != trump && !ctx.k.is_called(s))
        .filter_map(|s| {
            let p = opponents(ctx.me)
                .iter()
                .map(|&o| ctx.mind.features(o).pair[s as usize])
                .fold(0.0, f32::max);
            let big = (legal & CardSet::suit(s) & (CardSet::ACES | CardSet::TENS)) - val.halves;
            top(big).map(|c| (p, c))
        })
        .filter(|&(p, _)| p >= ctx.params.destroy_pair_conf)
        .max_by(|a, b| a.0.total_cmp(&b.0))?;
    Some((c, RuleId::HL5, vec![fact("p_opponent_pair", p)]))
}

/// HL-3 (plan suit): trump is set, the partner is certainly void in a side suit and likely
/// holds trump: lead my lowest card there. *Why:* the next opponent must follow or overtake,
/// and the partner trumps it (the pass-back plan HK-2: "the opponent needs to go higher when
/// you can take the trick with trump").
fn partner_ruffs(ctx: &Ctx, legal: CardSet, val: &Valuation) -> Option<Lead> {
    let t = ctx.k.trump()?;
    if !ctx.params.lead_plan_suit {
        return None;
    }
    let mate = partner(ctx.me);
    let p_trump = 1.0 - ctx.mind.p_void(mate, t);
    if p_trump < ctx.params.partner_trump_conf {
        return None;
    }
    let (s, c) = SUITS
        .into_iter()
        .filter(|&s| s != t && ctx.k.is_void(mate, s))
        .filter_map(|s| lowest((legal & CardSet::suit(s)) - val.halves).map(|c| (s, c)))
        .min_by_key(|&(_, c)| (CARD_POINTS[c as usize], c))?;
    Some((
        c,
        RuleId::HL3,
        vec![fact("suit", s as u8), fact("p_partner_trump", p_trump)],
    ))
}

/// HL-4: lead trump only with a reason: I hold the top trump and another one, and an opponent
/// may still hold trump. *Why:* drawing their trumps protects our standing side cards from
/// being ruffed.
fn draw_trumps(ctx: &Ctx, legal: CardSet) -> Option<Lead> {
    let t = ctx.k.trump()?;
    if !ctx.params.lead_draw_trump {
        return None;
    }
    let mine = legal & CardSet::suit(t);
    let [l, r] = opponents(ctx.me);
    let theirs = (ctx.k.may_hold(l) | ctx.k.may_hold(r)) & CardSet::suit(t);
    if mine.len() < 2 || theirs.is_empty() {
        return None;
    }
    let c = mine.iter().find(|&c| opp_trump_over(ctx, c).is_empty())?;
    Some((c, RuleId::HL4, vec![fact("their_trumps", theirs.len())]))
}

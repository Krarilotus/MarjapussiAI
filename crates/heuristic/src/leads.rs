//! The planned leads of M3 (Judge item 5): each names its condition and why it gains over the
//! fallback HL-F (the cheapest lead). `play::lead` asks `before_standing` before cashing a
//! standing card (HL-2) and `after_standing` after it, before HL-7 / HL-F.

use engine::opponents;
use engine::CARD_POINTS;
use engine::{partner, CardSet, SUITS};
use engine::{Card, Suit, Value};

use crate::hand::higher;
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

/// The existing standing-card order, shared by ordinary leads and announcement timing.
pub(crate) fn cash_card(legal: CardSet, standing: CardSet) -> Option<u8> {
    standing
        .iter()
        .filter(|&c| (higher(c) & standing).is_empty())
        .max_by_key(|&c| {
            (
                CARD_POINTS[c as usize],
                (legal & CardSet::suit(engine::suit_of(c))).len(),
            )
        })
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
    schnippeln_line(ctx, legal, val)
        .or_else(|| destroy_pair(ctx, legal, val))
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

/// HL-1b (`schnippeln_line`, owner #235–#237): after my standing cards, a low card toward the
/// ace the partner **certainly** holds (e.g. one I passed it): the opponent in between must go
/// over it, the partner takes it with the ace and has the lead. Never a half of my live pair.
/// The entry must hold with P >= `partner_lead_conf` (the same joint entry estimate as HL-3).
/// The live planner's overrides show the capture alone pays (91-100 % of such tricks won,
/// +14 to +20 card points net), so no further purpose is required.
fn schnippeln_line(ctx: &Ctx, legal: CardSet, val: &Valuation) -> Option<Lead> {
    if !ctx.params.lead_schnippeln || ctx.k.played().is_empty() {
        return None;
    }
    let mate = partner(ctx.me);
    let mates_aces = ctx.k.certain(mate) & CardSet::ACES;
    let pair = SUITS
        .into_iter()
        .filter(|&s| !ctx.k.is_called(s))
        .map(|s| ctx.mind.features(mate).pair[s as usize])
        .fold(0.0, f32::max);
    let (s, c, entry) = SUITS
        .into_iter()
        .filter(|&s| mates_aces.contains(ace_of(s)))
        .filter_map(|s| lowest((legal & CardSet::suit(s)) - val.halves).map(|c| (s, c, ctx.entry(c))))
        .filter(|(_, _, e)| e.lead[mate as usize] >= ctx.params.partner_lead_conf)
        .max_by(|a, b| {
            a.2.lead[mate as usize]
                .total_cmp(&b.2.lead[mate as usize])
                .then((CARD_POINTS[b.1 as usize], b.1).cmp(&(CARD_POINTS[a.1 as usize], a.1)))
        })?;
    Some((
        c,
        RuleId::HL1,
        vec![
            fact("suit", s as u8),
            fact("p_partner_entry", entry.lead[mate as usize]),
            fact("partner_pair", pair),
            fact("partner_aces", mates_aces.len()),
        ],
    ))
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

/// HL-5: lead the side-suit card on which an opponent likely must give up a half of its
/// uncalled pair (the owner: destroy their pair with the non-trump play). *Why:* the pair can no
/// longer be announced. Whether a half must fall depends on the guards it may play instead and
/// on the obligation to overtake (Astra's review 2026-10-13, §3), so the believed worlds play the
/// trick out (`outlook::forced_half`); the most likely card, the higher at equal likelihood.
fn destroy_pair(ctx: &Ctx, legal: CardSet, val: &Valuation) -> Option<Lead> {
    if !ctx.params.lead_destroy_pair {
        return None;
    }
    let trump = ctx.k.trump();
    let side = SUITS
        .into_iter()
        .filter(|&s| Some(s) != trump && !ctx.k.is_called(s))
        .fold(CardSet::EMPTY, |set, s| set | CardSet::suit(s));
    let (c, p) = crate::outlook::forced_half(ctx, (legal & side) - val.halves)
        .into_iter()
        .filter(|&(_, p)| p >= f64::from(ctx.params.destroy_pair_conf))
        .max_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))?;
    Some((c, RuleId::HL5, vec![fact("p_forced_half", p)]))
}

/// HL-3: lead into partner's known void only when partner can win the entry.
/// Forced following and overruffs are resolved jointly by the engine's legal play.
fn partner_ruffs(ctx: &Ctx, legal: CardSet, val: &Valuation) -> Option<Lead> {
    let t = ctx.k.trump()?;
    if !ctx.params.lead_plan_suit {
        return None;
    }
    let mate = partner(ctx.me);
    let (s, c, entry) = SUITS
        .into_iter()
        .filter(|&s| s != t && ctx.k.is_void(mate, s))
        .filter_map(|s| lowest((legal & CardSet::suit(s)) - val.halves).map(|c| (s, c, ctx.entry(c))))
        .filter(|(_, _, e)| e.lead[mate as usize] > 0.0 && e.lead[mate as usize] >= ctx.params.partner_trump_conf)
        .max_by(|a, b| {
            a.2.lead[mate as usize]
                .total_cmp(&b.2.lead[mate as usize])
                .then((CARD_POINTS[b.1 as usize], b.1).cmp(&(CARD_POINTS[a.1 as usize], a.1)))
        })?;
    Some((
        c,
        RuleId::HL3,
        vec![
            fact("suit", s as u8),
            fact("p_partner_entry", entry.lead[mate as usize]),
            fact("p_both_opponents_follow", entry.both_follow),
            fact("remaining_suit", (CardSet::suit(s) - ctx.k.played()).len()),
        ],
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

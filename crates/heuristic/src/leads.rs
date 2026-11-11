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
}

/// A rank label alone is not an activation resource: after the companion half
/// has been played, this card can no longer complete that pair. `others` is the
/// actor's conservative set of possible remaining cards, never a sampled hand.
fn live_half(cash: u8, hand: CardSet, others: CardSet) -> bool {
    CardSet::HALVES.contains(cash)
        && !((CardSet::halves(engine::suit_of(cash)) - CardSet::single(cash)) & (hand | others)).is_empty()
}

/// Leading `cash` would spoil a possible split pair: I hold one half of its open suit, not
/// `cash` itself, and the other half is the partner's only possible card there, so the partner
/// must follow with it before a half question can count the pair. A conditional resource loss,
/// not knowledge that the partner owns that half: the half and the partner's known cards must
/// fit one partner hand. `may` is each seat's possible cards (no sampled hand); `mate_held` the
/// partner's cards I know from our exchange.
pub(crate) fn cash_spoils_split_half(
    me: u8,
    hand: CardSet,
    called: u8,
    may: &[CardSet; 4],
    mate_held: CardSet,
    cash: u8,
) -> bool {
    let suit = engine::suit_of(cash);
    let mine = hand & CardSet::halves(suit);
    if called & (1 << suit as u8) != 0 || !hand.contains(cash) || mine.len() != 1 || mine.contains(cash) {
        return false;
    }
    let mate = partner(me) as usize;
    let possible = (may[mate] - hand) | mate_held;
    let companion = CardSet::halves(suit) - mine;
    if possible & CardSet::suit(suit) != companion {
        return false;
    }
    let elsewhere = (0..4)
        .filter(|&s| s != mate && s != me as usize)
        .fold(hand, |cards, s| cards | (may[s] - hand - mate_held));
    let known = mate_held | (possible - elsewhere);
    let required = known | companion;
    (known & hand).is_empty()
        && required.is_subset(possible)
        && required.len() <= hand.len()
        && hand.len() <= possible.len()
}

/// A specific cash before Yours: retain the lead and leave partner at least one
/// spare beyond its pair. This is not a guard on own announcements or AskHalf.
pub(crate) fn cash_before_question(hand: CardSet, trump: Option<Suit>, others: CardSet, cash: u8) -> bool {
    if hand.len() < 3 || !hand.contains(cash) || !(higher(cash) & others).is_empty() {
        return false;
    }
    let suit = engine::suit_of(cash);
    let mine = hand & CardSet::suit(suit);
    if let Some(trump) = trump {
        return suit == trump && mine.len() >= 2;
    }
    // Holding a half excludes partner's full pair in the led suit; its other
    // potential pairs have a spare discard. A future half question is separate.
    let outside = others & CardSet::suit(suit);
    let standing = mine.iter().filter(|&c| (higher(c) & outside).is_empty()).count() as u32;
    !live_half(cash, hand, others)
        && !(mine & CardSet::HALVES).is_empty()
        && mine.len() >= 2
        && standing >= outside.len()
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

#[cfg(test)]
mod control_tests {
    use super::*;

    fn cards(names: &[&str]) -> CardSet {
        CardSet::from_cards(&names.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>())
    }

    #[test]
    fn a_pairless_run_can_precede_activation_but_an_unsafe_long_suit_cannot() {
        let hand = cards(&["e-A", "e-Z", "e-O", "e-9", "e-8", "e-7", "e-6", "s-A", "s-8"]);
        let cash = "e-A".parse::<Card>().unwrap().index();
        assert!(cash_before_question(hand, None, CardSet::ALL - hand, cash));
        assert!(!cash_before_question(hand, Some(Suit::Red), CardSet::ALL - hand, cash));
        let unsafe_hand = hand - cards(&["e-A"]);
        assert!(!cash_before_question(
            unsafe_hand,
            None,
            CardSet::ALL - unsafe_hand,
            cash
        ));
    }

    #[test]
    fn standing_half_can_be_cashed_after_its_companion_was_played() {
        // g-O (the King's companion), g-A and g-Z are played: g-K stands and pairs nothing.
        let hand = cards(&["g-K", "g-U", "g-9", "g-8", "r-O"]);
        let cash = "g-K".parse::<Card>().unwrap().index();
        let spent = cards(&["g-A", "g-Z", "g-O"]);
        let others = CardSet::ALL - hand - spent;
        assert!(cash_before_question(hand, None, others, cash));
        // Companion still out: g-K is a live half and is kept.
        assert!(!cash_before_question(hand, None, others | cards(&["g-O"]), cash));
        // A higher card still out: g-K does not stand.
        assert!(!cash_before_question(hand, None, others | cards(&["g-Z"]), cash));
        assert!(!cash_before_question(hand, Some(Suit::Bells), others, cash));
    }

    /// Seat 0 holds g-O; the partner (seat 2) may hold g-K and no other Green card.
    fn split() -> (CardSet, [CardSet; 4]) {
        let hand = cards(&["g-A", "g-O", "g-8", "e-A", "e-O", "s-6"]);
        let rest = CardSet::ALL - hand;
        let green = CardSet::suit(Suit::Green) - cards(&["g-K"]);
        (hand, [hand, rest, rest - green, rest])
    }

    fn spoils(hand: CardSet, called: u8, may: &[CardSet; 4], held: CardSet, name: &str) -> bool {
        cash_spoils_split_half(0, hand, called, may, held, name.parse::<Card>().unwrap().index())
    }

    #[test]
    fn cashing_over_the_partners_only_possible_card_spoils_the_split_half() {
        let (hand, may) = split();
        assert!(spoils(hand, 0, &may, CardSet::EMPTY, "g-A"));
        // Another suit, where the partner may follow with a spare: no loss.
        assert!(!spoils(hand, 0, &may, CardSet::EMPTY, "e-A"));
    }

    #[test]
    fn a_spare_a_called_suit_an_absent_half_or_no_own_half_keep_the_cash() {
        let (hand, may) = split();
        let mut spare = may;
        spare[2] |= cards(&["g-6"]);
        assert!(!spoils(hand, 0, &spare, CardSet::EMPTY, "g-A"));
        assert!(!spoils(hand, 0, &may, cards(&["g-K", "g-6"]), "g-A"));
        assert!(!spoils(hand, 1 << Suit::Green as u8, &may, CardSet::EMPTY, "g-A"));
        let mut absent = may;
        absent[2] -= cards(&["g-K"]);
        assert!(!spoils(hand, 0, &absent, CardSet::EMPTY, "g-A"));
        let lost_own = (hand - cards(&["g-O"])) | cards(&["r-6"]);
        let rest = CardSet::ALL - lost_own;
        let lost_may = [lost_own, rest, rest - (CardSet::suit(Suit::Green) - cards(&["g-K"])), rest];
        assert!(!spoils(lost_own, 0, &lost_may, CardSet::EMPTY, "g-A"));
    }

    #[test]
    fn the_half_and_the_partners_known_cards_must_fit_one_hand() {
        let (hand, may) = split();
        let full = cards(&["e-7", "e-8", "e-9", "s-7", "s-8", "r-7"]);
        assert!(!spoils(hand, 0, &may, full, "g-A"));
        assert!(spoils(hand, 0, &may, full - cards(&["r-7"]), "g-A"));
    }
}

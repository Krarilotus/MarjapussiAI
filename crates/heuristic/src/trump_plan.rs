//! Announcement timing: retain a specific own lead, or use the opportunity now.
//! Reach is the existing approximate longer-horizon activation model.

use engine::Suit;
use engine::{partner, CardSet, SUITS};
use engine::{ActionType, GameAction, QuestionType};

use beliefs::conventions::pair_value;

use crate::rules::RuleId;
use crate::valuation::Valuation;
use crate::{decision, fact, Ctx, Decision};
use engine::PlayerTrumpPossibilities;

/// What our team can still reach, from my view.
pub(crate) struct Reach {
    /// My uncalled pairs.
    pub my_pairs: Vec<Suit>,
    /// P(the partner holds an uncalled pair); 0 after its "no pair".
    pub partner_pair: f32,
    /// My live standalone halves: (suit, P(the partner holds the other half)), best first.
    pub halves: Vec<(Suit, f32)>,
    /// Access estimate: the existing live-Ace proxy, supplemented by concrete
    /// next-trick entries when partner access is required. Not a guarantee.
    pub p_partner_lead: f32,
}

impl Reach {
    pub fn new(ctx: &Ctx, legal: &[GameAction]) -> Reach {
        let k = ctx.k;
        let mate = partner(ctx.me);
        let hand = k.certain(ctx.me);
        let uncalled = SUITS.into_iter().filter(|&s| !k.is_called(s));
        let mine = k.pairs(ctx.me).certain;
        let my_pairs: Vec<Suit> = uncalled.clone().filter(|&s| mine & 1 << s as u8 != 0).collect();
        // The believed pair is 1 or 0 where the facts decide (`beliefs`).
        let belief = ctx.mind.features(mate);
        let pair_p = |s: Suit| belief.pair[s as usize];
        let partner_pair = 1.0 - uncalled.clone().map(|s| 1.0 - pair_p(s)).product::<f32>();
        let mut halves: Vec<(Suit, f32)> = uncalled
            .filter_map(|s| {
                let mine = hand & CardSet::halves(s);
                if mine.len() != 1 {
                    return None;
                }
                let other = (CardSet::halves(s) - mine).iter().next()?;
                Some((s, ctx.mind.p_holds(mate, other)))
            })
            .collect();
        halves.sort_by(|a, b| {
            (b.1 * pair_value(b.0) as f32)
                .total_cmp(&(a.1 * pair_value(a.0) as f32))
                .then((a.0 as u8).cmp(&(b.0 as u8)))
        });
        let mut p_partner_lead = ctx.mind.features(mate).ace;
        let requires_partner =
            (!my_pairs.is_empty() && !k.rights(ctx.me).may_announce() && k.rights(mate).may_ask_pair())
                || (partner_pair > 0.0 && !k.rights(ctx.me).may_ask_pair() && k.rights(mate).may_announce());
        if requires_partner {
            for action in legal {
                if let ActionType::CardPlayed(c) = action.action_type {
                    p_partner_lead = p_partner_lead.max(ctx.entry(c.index()).lead[mate as usize]);
                }
            }
        }
        Reach {
            my_pairs,
            partner_pair,
            halves,
            p_partner_lead,
        }
    }

    /// Leads our team still needs to activate the pair of `s` I hold a half of: one for my
    /// announcement or my half question, plus one for "Yours" first if the partner may hold a
    /// pair (the order mine → yours → ours).
    pub fn leads_needed(&self, ctx: &Ctx, s: Suit) -> u32 {
        let split = (ctx.k.certain(ctx.me) & CardSet::halves(s)).len() == 1;
        1 + (split && self.partner_may(ctx)) as u32
    }

    /// P(the pair of `s` gets activated) given `leads` expected leads of our team (C25: the one
    /// owner of activation). My own pair needs my `Own` right, or the partner's "Yours" after
    /// its lead; a split pair needs a half question, which no right forbids.
    pub fn p_activate(&self, ctx: &Ctx, s: Suit, leads: f32) -> f32 {
        let mate = partner(ctx.me);
        let mine = ctx.k.certain(ctx.me) & CardSet::halves(s);
        let rights = match mine.len() {
            2 if ctx.k.rights(ctx.me).may_announce() => 1.0,
            2 if ctx.k.rights(mate).may_ask_pair() => self.p_partner_lead,
            2 => 0.0,
            1 => 1.0, // A split pair can still be asked at every rights level.
            _ if ctx.k.rights(ctx.me).may_ask_pair() => 1.0,
            _ if ctx.k.rights(mate).may_announce() => self.p_partner_lead,
            _ => 0.0,
        };
        rights * (leads / self.leads_needed(ctx, s) as f32).min(1.0)
    }

    /// Whether the partner may hold a pair worth a "Yours" (P10a "may need").
    fn partner_may(&self, ctx: &Ctx) -> bool {
        self.partner_pair > 0.0 && self.partner_pair >= ctx.params.partner_pair_min
    }

    /// Existing attempt model: uncertain questions before activation. The timing
    /// guard compares it with a certified run, not independent team winners.
    fn attempts(&self, ctx: &Ctx) -> u32 {
        let yours = self.partner_may(ctx) as u32;
        if !self.my_pairs.is_empty() || ctx.k.pairs(partner(ctx.me)).certain != 0 {
            0
        } else if self.halves.iter().any(|h| h.1 >= 1.0) {
            1 + yours
        } else {
            self.open_halves().count() as u32 + yours
        }
    }

    /// Live halves the partner may complete (HC-13: a known "no" is no question).
    fn open_halves(&self) -> impl Iterator<Item = &(Suit, f32)> {
        self.halves.iter().filter(|h| h.1 > 0.0)
    }
}

/// P10a: asking `ask` leaves every pair the team may still need as activatable as before. Only
/// rights the question spends count: a question that changes nothing (my right is spent
/// already) is never blocked.
fn keeps_needed_rights(ctx: &Ctx, reach: &Reach, ask: QuestionType) -> bool {
    let me = ctx.me;
    let mate = partner(me);
    let (before, theirs) = (ctx.k.rights(me), ctx.k.rights(mate));
    let after = before.after_question(ask);
    // The partner will plausibly win a lead before the end (P10a).
    let lead = reach.p_partner_lead >= ctx.params.partner_lead_conf;
    // My own uncalled pair: I announce it (Own), or the partner asks me "Yours" after a lead.
    let my_pair =
        |r: PlayerTrumpPossibilities| reach.my_pairs.is_empty() || r.may_announce() || (theirs.may_ask_pair() && lead);
    // The partner's possible pair: my "Yours" (asking it now is its activation), or the
    // partner's own announcement after a lead.
    let partner_pair =
        |r: PlayerTrumpPossibilities| !reach.partner_may(ctx) || r.may_ask_pair() || (theirs.may_announce() && lead);
    let lost_mine = my_pair(before) && !my_pair(after);
    let lost_theirs = ask != QuestionType::Yours && partner_pair(before) && !partner_pair(after);
    !lost_mine && !lost_theirs
}

fn find(legal: &[GameAction], t: ActionType) -> Option<&GameAction> {
    legal.iter().find(|a| a.action_type == t)
}

/// The plan at a lead where questions are legal.
pub(crate) fn timing(ctx: &Ctx, legal: &[GameAction], reach: &Reach, val: &Valuation) -> Option<Decision> {
    let planned = action(ctx, legal, reach)?;
    let cards = legal
        .iter()
        .filter_map(|a| match a.action_type {
            ActionType::CardPlayed(c) => Some(c.index()),
            _ => None,
        })
        .fold(CardSet::EMPTY, |s, c| s | CardSet::single(c));
    let retained = ctx.k.retaining_lead(ctx.me) & (cards - val.plan_cards);
    // With no trump, all these cards remain own winners. Under trump, only
    // the trump run plus ONE side winner is certified; later side rounds can
    // become ruffable. Cash that side card first, then the trump run survives.
    let (first, count) = match ctx.k.trump() {
        None => (retained, retained.len()),
        Some(t) => {
            let trumps = retained & CardSet::suit(t);
            let side = retained - trumps;
            (
                if side.is_empty() { trumps } else { side },
                trumps.len() + u32::from(!side.is_empty()),
            )
        }
    };
    let count = count.min(ctx.k.hand_len(ctx.me).saturating_sub(1) as u32);
    let attempts = reach.attempts(ctx);
    if ctx.params.lead_cash_standing && attempts < count {
        if let Some(c) = crate::leads::cash_card(cards, first) {
            let a = find(legal, ActionType::CardPlayed(engine::Card::from_index(c)))?;
            return Some(decision(
                a,
                RuleId::HC4,
                vec![
                    fact("retains_own_lead", 1),
                    fact("retained_leads", count),
                    fact("attempts", attempts),
                ],
            ));
        }
    }
    Some(planned)
}

fn action(ctx: &Ctx, legal: &[GameAction], reach: &Reach) -> Option<Decision> {
    let mut why = Vec::new();
    // Mine: my most valuable pair (HC-5 when there are several).
    let announce = reach
        .my_pairs
        .iter()
        .filter_map(|&s| find(legal, ActionType::AnnounceTrump(s)).map(|a| (pair_value(s), a)))
        .max_by_key(|&(v, _)| v);
    if let Some((v, a)) = announce {
        why.push(fact("pair_value", v));
        let rule = if reach.my_pairs.len() > 1 {
            RuleId::HC5
        } else {
            RuleId::HC2
        };
        return Some(decision(a, rule, why));
    }
    // Yours.
    if reach.partner_may(ctx) && keeps_needed_rights(ctx, reach, QuestionType::Yours) {
        if let Some(a) = find(legal, ActionType::Question(QuestionType::Yours)) {
            why.push(fact("partner_pair", reach.partner_pair));
            return Some(decision(a, RuleId::HQ3, why));
        }
    }
    // Ours: the half the partner most likely completes, by pair value.
    let skipped = reach.halves.len() - reach.open_halves().count();
    if skipped > 0 {
        why.push(fact("hc13_skipped", skipped as u32));
    }
    for &(s, p) in reach.open_halves() {
        if !keeps_needed_rights(ctx, reach, QuestionType::YourHalf(s)) {
            why.push(fact("p10a_blocked", 1));
            break;
        }
        if let Some(a) = find(legal, ActionType::Question(QuestionType::YourHalf(s))) {
            why.push(fact("p_other_half", p));
            return Some(decision(a, RuleId::HQ3, why));
        }
    }
    None
}

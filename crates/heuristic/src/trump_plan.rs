//! Announcing and asking (M3 P8, P10, P10a; replaces the M1 stopgap HQ-0).
//!
//! Why: a pair counts only once announced, and every announcement or question needs a lead.
//! Schmiddi's attempt counting (`marjapussi-py-schmiddi/marjapussi/probabilistic_policy.py`,
//! l. 487-575) says how many of our leads it still takes to reach a pair:
//! - 0 if I hold an uncalled pair or the partner certainly does;
//! - the number of my standalone halves (+1 if the partner may hold a pair) if our team surely
//!   holds a pair split between us;
//! - otherwise 1 ("Yours") + my standalone halves.
//!
//! When I lead and hold more sure leads than that (standing cards), I cash one first (HC-4): the
//! next lead, and with it the next question, is guaranteed. Otherwise I act now (HC-2), in the
//! order mine → yours → ours (HQ-3), skipping questions whose answer is known (HC-13), and never
//! spending a question right the team still needs (P10a).

use engine::Suit;
use engine::{partner, CardSet, SUITS};
use engine::{ActionType, GameAction, QuestionType};

use beliefs::conventions::pair_value;

use crate::rules::RuleId;
use crate::valuation::Valuation;
use crate::{decision, fact, Ctx, Decision, Fact};
use engine::PlayerTrumpPossibilities;

/// What our team can still reach, from my view.
pub(crate) struct Reach {
    /// My uncalled pairs.
    pub my_pairs: Vec<Suit>,
    /// P(the partner holds an uncalled pair); 0 after its "no pair".
    pub partner_pair: f32,
    pub partner_pair_certain: bool,
    /// My live standalone halves: (suit, P(the partner holds the other half)), best first.
    pub halves: Vec<(Suit, f32)>,
    /// A suit whose pair our team surely holds split between us.
    pub certain_split: Option<Suit>,
    /// P(the partner wins a lead before the end): 1 with a certain standing card, else
    /// P(it holds a live ace).
    pub p_partner_lead: f32,
}

impl Reach {
    pub fn new(ctx: &Ctx) -> Reach {
        let k = ctx.k;
        let mate = partner(ctx.me);
        let hand = k.certain(ctx.me);
        let uncalled = SUITS.into_iter().filter(|&s| !k.is_called(s));
        let mine = k.pairs(ctx.me).certain;
        let my_pairs: Vec<Suit> = uncalled.clone().filter(|&s| mine & 1 << s as u8 != 0).collect();
        // The believed pair is 1 or 0 where the facts decide (`beliefs`).
        let belief = ctx.mind.features(mate);
        let pair_p = |s: Suit| belief.pair[s as usize];
        let partner_pair_certain = uncalled.clone().any(|s| pair_p(s) >= 1.0);
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
        let certain_split = halves.iter().find(|h| h.1 >= 1.0).map(|h| h.0);
        let p_partner_lead = if !k.standing(mate).is_empty() {
            1.0
        } else {
            ctx.mind.features(mate).ace
        };
        Reach {
            my_pairs,
            partner_pair,
            partner_pair_certain,
            halves,
            certain_split,
            p_partner_lead,
        }
    }

    /// Leads our team still needs to activate the pair of `s` I hold a half of: one for my
    /// announcement or my half question, plus one for "Yours" first if the partner may hold a
    /// pair (the order mine → yours → ours).
    pub fn leads_needed(&self, ctx: &Ctx, s: Suit) -> u32 {
        let own = self.my_pairs.contains(&s);
        1 + (!own && self.partner_may(ctx)) as u32
    }

    /// P(the pair of `s` gets activated) given `leads` expected leads of our team (C25: the one
    /// owner of activation). My own pair needs my `Own` right, or the partner's "Yours" after
    /// its lead; a split pair needs a half question, which no right forbids.
    pub fn p_activate(&self, ctx: &Ctx, s: Suit, leads: f32) -> f32 {
        let mate = partner(ctx.me);
        let rights = if !self.my_pairs.contains(&s) || ctx.k.rights(ctx.me).may_announce() {
            1.0
        } else if ctx.k.rights(mate).may_ask_pair() {
            self.p_partner_lead
        } else {
            0.0
        };
        rights * (leads / self.leads_needed(ctx, s) as f32).min(1.0)
    }

    /// Whether the partner may hold a pair worth a "Yours" (P10a "may need").
    fn partner_may(&self, ctx: &Ctx) -> bool {
        self.partner_pair >= ctx.params.partner_pair_min
    }

    /// Live halves the partner may complete (HC-13: a known "no" is no question).
    fn open_halves(&self) -> impl Iterator<Item = &(Suit, f32)> {
        self.halves.iter().filter(|h| h.1 > 0.0)
    }

    /// Schmiddi's attempt count; `None` when nothing is left to reach.
    pub fn attempts(&self, ctx: &Ctx) -> Option<u32> {
        let yours = self.partner_may(ctx) as u32;
        if !self.my_pairs.is_empty() || self.partner_pair_certain {
            Some(0)
        } else if self.certain_split.is_some() {
            Some(1 + yours)
        } else {
            let n = self.open_halves().count() as u32;
            (n + yours > 0).then_some(n + yours)
        }
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

pub(crate) enum Timing {
    /// Cash a standing card first (HC-4): the next lead is guaranteed.
    Cash(Vec<Fact>),
    /// Announce or ask now.
    Act(Decision),
    /// Nothing to announce or ask.
    Nothing,
}

fn find(legal: &[GameAction], t: ActionType) -> Option<&GameAction> {
    legal.iter().find(|a| a.action_type == t)
}

/// The plan at a lead where questions are legal.
pub(crate) fn timing(ctx: &Ctx, legal: &[GameAction], reach: &Reach, val: &Valuation) -> Timing {
    let Some(attempts) = reach.attempts(ctx) else {
        return Timing::Nothing;
    };
    let sure_leads = (ctx.k.standing(ctx.me) - val.plan_cards).len();
    let mut why = vec![fact("attempts", attempts), fact("sure_leads", sure_leads)];
    if attempts < sure_leads {
        return Timing::Cash(why);
    }
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
        return Timing::Act(decision(a, rule, why));
    }
    // Yours.
    if reach.partner_may(ctx) && keeps_needed_rights(ctx, reach, QuestionType::Yours) {
        if let Some(a) = find(legal, ActionType::Question(QuestionType::Yours)) {
            why.push(fact("partner_pair", reach.partner_pair));
            return Timing::Act(decision(a, RuleId::HQ3, why));
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
            return Timing::Act(decision(a, RuleId::HQ3, why));
        }
    }
    Timing::Nothing
}

//! Card play (§6.0, §7). At a lead where questions are legal, `trump_plan` decides first whether
//! to announce, ask, or cash a standing card (M3 P8). Then the planned leads (`leads.rs`) and
//! HL-2 (cash standing cards first); every other card choice is the best `value_of_play` of
//! `valuation` (C25: one owner of keep and give value), labelled by the valuation term that
//! separates it from the runner-up.

use engine::CARD_POINTS;
use engine::{partner, suit_of, takes, CardSet};
use engine::{ActionType, AnswerType, GameAction};

use crate::hand::higher;
use crate::leads;
use crate::rules::RuleId;
use crate::trump_plan::{self, Reach, Timing};
use crate::valuation::{Best, Choice, Term, Valuation};
use crate::{decision, fact, Ctx, Decision, Fact};

fn cards_of(legal: &[GameAction]) -> Vec<(u8, &GameAction)> {
    legal
        .iter()
        .filter_map(|a| match &a.action_type {
            ActionType::CardPlayed(c) => Some(((c).index(), a)),
            _ => None,
        })
        .collect()
}

pub(crate) fn decide(ctx: &Ctx, legal: &[GameAction]) -> Decision {
    let reach = Reach::new(ctx);
    let val = Valuation::new(ctx, &reach);
    let can_ask = legal
        .iter()
        .any(|a| matches!(a.action_type, ActionType::Question(_) | ActionType::AnnounceTrump(_)));
    let mut cash = None;
    if can_ask {
        match trump_plan::timing(ctx, legal, &reach, &val) {
            Timing::Act(d) => return d,
            Timing::Cash(why) => cash = Some(why),
            Timing::Nothing => {}
        }
    }
    // F10: the last tricks go to the solver over the worlds I believe in. Its moves are cards
    // and announcements, not questions, so while a question may still activate one of our
    // pairs (a half of mine carries activation value) the valuation decides.
    if val.halves.is_empty() {
        if let Some(d) = crate::endgame::decide(ctx, legal, &val) {
            return d;
        }
    }
    let cards = cards_of(legal);
    let set = cards.iter().fold(CardSet::EMPTY, |a, &(c, _)| a | CardSet::single(c));
    let (card, mut rule, mut why) = if ctx.k.trick().is_empty() {
        lead(ctx, set, &val)
    } else {
        follow(ctx, set, &val)
    };
    // HC-4: the standing card was cashed because the attempts still fit the sure leads (P8).
    if let (Some(plan_why), RuleId::HL2) = (cash, rule) {
        rule = RuleId::HC4;
        why.extend(plan_why);
    }
    let action = cards.iter().find(|&&(c, _)| c == card).map_or(&legal[0], |&(_, a)| a);
    decision(action, rule, why)
}

/// HC-6: answer "Yours" with the pair whose meld is most at risk if not named now (its worth
/// × P(I don't activate it later myself)); the other stays mine to announce.
pub(crate) fn answer(ctx: &Ctx, legal: &[GameAction]) -> Decision {
    let reach = Reach::new(ctx);
    let val = Valuation::new(ctx, &reach);
    let risk = |a: &GameAction| match a.action_type {
        ActionType::Answer(AnswerType::YesPair(s)) => val.at_risk[s as usize],
        _ => -1.0,
    };
    let best = legal
        .iter()
        .max_by(|a, b| risk(a).total_cmp(&risk(b)))
        .unwrap_or(&legal[0]);
    decision(best, RuleId::HC6, vec![fact("at_risk", risk(best))])
}

/// The rule the deciding valuation term stands for.
fn term_rule(ctx: &Ctx, choice: &Choice, best: &Best, set: CardSet, val: &Valuation) -> RuleId {
    let trump = ctx.k.trump();
    let to_them = |c: u8| choice.p_ours(ctx, c) < 0.5;
    // HC-8: every legal card beats the partner's winning card.
    let forced_over = choice
        .winner
        .is_some_and(|(s, w)| s == partner(ctx.me) && set.iter().all(|x| takes(x, w, trump)));
    match best.term {
        _ if forced_over => RuleId::HC8,
        Term::Trick if to_them(best.card) => RuleId::HF2,
        Term::Trick => RuleId::HV0,
        Term::Future
            if best
                .runner_up
                .is_some_and(|r| to_them(r) && CARD_POINTS[r as usize] >= 10) =>
        {
            RuleId::HC7
        }
        Term::Future => RuleId::HV1,
        Term::Pair if val.meld_needed => RuleId::HC9,
        Term::Pair => RuleId::HC1,
        Term::Plan => RuleId::HV2,
        Term::Win => RuleId::HL7,
    }
}

fn lead(ctx: &Ctx, legal: CardSet, val: &Valuation) -> (u8, RuleId, Vec<Fact>) {
    // The suit of a pair the partner certainly holds isn't led before its announcement,
    // unless I hold nothing else.
    let legal = match legal - val.protect {
        open if open.is_empty() => legal,
        open => open,
    };
    if let Some(l) = leads::before_standing(ctx, legal) {
        return l;
    }
    // HL-2: side-suit standing cards outside our reachable pairs' suits (P7), highest first.
    let stand = if ctx.params.lead_cash_standing {
        (ctx.k.standing(ctx.me) & legal) - val.plan_cards
    } else {
        CardSet::EMPTY
    };
    if let Some(c) = stand
        .iter()
        .filter(|&c| (higher(c) & stand).is_empty())
        .max_by_key(|&c| (CARD_POINTS[c as usize], (legal & CardSet::suit(suit_of(c))).len()))
    {
        return (c, RuleId::HL2, vec![fact("points", CARD_POINTS[c as usize])]);
    }
    if let Some(l) = leads::after_standing(ctx, legal, val) {
        return l;
    }
    choose(ctx, legal, val, true)
}

fn follow(ctx: &Ctx, legal: CardSet, val: &Valuation) -> (u8, RuleId, Vec<Fact>) {
    choose(ctx, legal, val, false)
}

fn choose(ctx: &Ctx, legal: CardSet, val: &Valuation, leading: bool) -> (u8, RuleId, Vec<Fact>) {
    let choice = Choice::new(ctx);
    let best = choice.best(ctx, val, legal, leading);
    let c = best.card;
    let mut why = vec![fact("value", best.value), fact("p_ours", choice.p_ours(ctx, c))];
    if !leading {
        why.push(fact("trick_points", choice.trick_points));
    }
    (c, term_rule(ctx, &choice, &best, legal, val), why)
}

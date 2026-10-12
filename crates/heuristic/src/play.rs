//! Card play (§6.0, §7). Outside the endgame, at a lead where questions are legal, `trump_plan` decides whether
//! to announce, ask, or cash a standing card (M3 P8). Then the planned leads (`leads.rs`) and
//! HL-2 (cash standing cards first); every other card choice is the best `value_of_play` of
//! `valuation` (C25: one owner of keep and give value), labelled by the valuation term that
//! separates it from the runner-up.

use engine::CARD_POINTS;
use engine::{partner, takes, CardSet};
use engine::{ActionType, AnswerType, GameAction, Move, PlayPhase};

use crate::leads;
use crate::rules::RuleId;
use crate::trump_plan::{self, Reach};
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
    let reach = Reach::new(ctx, legal);
    let val = Valuation::new(ctx, &reach);
    if let Some(d) = crate::endgame::decide(ctx, legal, &val) {
        return d;
    }
    let can_ask = legal
        .iter()
        .any(|a| matches!(a.action_type, ActionType::Question(_) | ActionType::AnnounceTrump(_)));
    if can_ask {
        if let Some(d) = trump_plan::timing(ctx, legal, &reach, &val) {
            return d;
        }
    }
    let cards = cards_of(legal);
    let set = cards.iter().fold(CardSet::EMPTY, |a, &(c, _)| a | CardSet::single(c));
    let (card, rule, why) = if ctx.k.trick().is_empty() {
        lead(ctx, set, &val)
    } else {
        follow(ctx, set, &val)
    };
    let action = cards.iter().find(|&&(c, _)| c == card).map_or(&legal[0], |&(_, a)| a);
    decision(action, rule, why)
}

/// HC-6: answer "Yours" with the pair whose meld is most at risk if not named now (its worth
/// × P(I don't activate it later myself)); the other stays mine to announce.
pub(crate) fn answer(ctx: &Ctx, legal: &[GameAction]) -> Decision {
    let reach = Reach::new(ctx, legal);
    let val = Valuation::new(ctx, &reach);
    if let Some(d) = crate::endgame::decide(ctx, legal, &val) {
        return d;
    }
    let risk = |a: &GameAction| match a.action_type {
        ActionType::Answer(AnswerType::YesPair(s)) => val.at_risk[s as usize],
        _ => -1.0,
    };
    let best = legal
        .iter()
        .max_by(|a, b| risk(a).total_cmp(&risk(b)))
        .unwrap_or(&legal[0]);
    // HO-2: another answer only where the route is clearly better than HC-6's.
    if ctx.params.outlook_activation {
        let moves: Vec<Move> = legal.iter().filter_map(|a| Move::from_action(&a.action_type)).collect();
        let planned = Move::from_action(&best.action_type);
        if let Some(Some(d)) = crate::outlook::route_choice(ctx, legal, PlayPhase::from_actions(legal), &moves, planned)
        {
            return d;
        }
    }
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
    if let Some(c) = leads::cash_card(legal, stand) {
        return (c, RuleId::HL2, vec![fact("points", CARD_POINTS[c as usize])]);
    }
    if let Some(l) = leads::after_standing(ctx, legal, val) {
        return l;
    }
    // HO-1: the plan layer's chain (the trick, then the rest played out with both teams'
    // activations) over the believed worlds, among the cards that keep my live pairs' halves.
    // It overrules the valuation's lead only where it is clearly better.
    let valued = choose(ctx, legal, val, true);
    if ctx.params.outlook_leads {
        let candidates = match legal - val.halves {
            open if open.is_empty() => legal,
            open => open,
        };
        if let Some(values) = crate::outlook::lead_values(ctx, candidates) {
            let best = values.best();
            let clear = values
                .index(valued.0)
                .is_none_or(|v| v != best && values.clearly_better(best, v));
            if clear {
                let mean = values.mean(best);
                let over = values.index(valued.0).map_or(0.0, |v| mean - values.mean(v));
                return (
                    values.options[best],
                    RuleId::HO1,
                    vec![fact("expected_difference", mean), fact("over_valuation", over)],
                );
            }
        }
    }
    valued
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

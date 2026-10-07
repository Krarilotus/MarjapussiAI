//! Passing (§5): HP1 acts with the pass conventions' scores (the HP score forth, the HK score
//! back; the models are `beliefs::conventions::passes`, which every reader inverts too) and
//! takes the best option. Each score term is a rule (HP-1 … HP-8, HK-1 … HK-7).

use beliefs::conventions::forth_ctx;
use beliefs::conventions::passes::{best, plan, BackCtx, BackModel, ForthModel, Score};
use engine::points_set;
use engine::HAND_SIZE;
use engine::{partner, CardSet};
use engine::{ActionType, GameAction};

use crate::rules::RuleId;

pub const FORTH_RULES: [RuleId; 8] = [
    RuleId::HP1,
    RuleId::HP2,
    RuleId::HP3,
    RuleId::HP4,
    RuleId::HP5,
    RuleId::HP6,
    RuleId::HP7,
    RuleId::HP8,
];
pub const BACK_RULES: [RuleId; 7] = [
    RuleId::HK1,
    RuleId::HK2,
    RuleId::HK3,
    RuleId::HK4,
    RuleId::HK5,
    RuleId::HK6,
    RuleId::HK7,
];

/// Acting: score every legal pass and take the best (or sample, `Params.sample_temperature`).
pub(crate) fn decide(ctx: &crate::Ctx, legal: &[GameAction]) -> crate::Decision {
    let p = ctx.params.conv();
    let hand = ctx.k.certain(ctx.me);
    let opts: Vec<(&GameAction, CardSet)> = legal
        .iter()
        .filter_map(|a| match &a.action_type {
            ActionType::Pass(cards) => Some((a, CardSet::from_cards(cards))),
            _ => None,
        })
        .collect();
    let forth = hand.len() as usize == HAND_SIZE;
    let (scored, rules): (Vec<Score>, &[RuleId]) = if forth {
        let model = ForthModel::new(hand, &forth_ctx(&ctx.mind.record().seats, ctx.me, p), p);
        (opts.iter().map(|&(_, x)| model.score(x)).collect(), &FORTH_RULES)
    } else {
        let bctx = BackCtx {
            partner_half: ctx.mind.features(partner(ctx.me)).half,
        };
        let model = BackModel::new(hand, plan(hand, &bctx, p), p);
        (opts.iter().map(|&(_, y)| model.score(y)).collect(), &BACK_RULES)
    };
    let pairs: Vec<(CardSet, Score)> = opts.iter().zip(&scored).map(|(&(_, x), &s)| (x, s)).collect();
    let i = if ctx.params.selection() != crate::Selection::Argmax {
        ctx.select(&scored.iter().map(|s| s.total).collect::<Vec<_>>())
    } else {
        best(&pairs)
    };
    let s = scored[i];
    let why = rules
        .iter()
        .zip(s.terms)
        .filter(|(_, t)| *t != 0.0)
        .map(|(r, t)| crate::fact(r.code(), t))
        .collect();
    // The label is the rule that separates the chosen option from the runner-up (Judge M1
    // item 1); all terms stay in `why`.
    let runner_up = (0..pairs.len()).filter(|&j| j != i).max_by(|&a, &b| {
        let (xa, sa) = &pairs[a];
        let (xb, sb) = &pairs[b];
        sa.total
            .total_cmp(&sb.total)
            .then(points_set(*xb).cmp(&points_set(*xa)))
            .then(xb.0.cmp(&xa.0))
    });
    let term = runner_up.map_or_else(|| s.main_term(), |j| s.deciding_term(&scored[j]));
    crate::decision(opts[i].0, rules[term.min(rules.len() - 1)], why)
}

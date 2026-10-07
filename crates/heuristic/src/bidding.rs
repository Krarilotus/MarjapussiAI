//! Bidding, KB batches 1 and 1b (`findings/kb/concepts.md`): the owner's convention, with no
//! estimate of the game. In this order:
//! 1. **B-gate, part 2:** both teams have told something (value 125–135, `team_plan_formed`,
//!    the opponents stepped) and my only live messages are pair messages that would cross 140:
//!    bid 140 instead (it hides the pair from the opponents; I play it anyway).
//! 2. **B-tell:** while `info_channel_open`, the first message of `info_to_tell` (with a pair,
//!    the ace first, then the pair; 3+ halves without a pair before the ace; a +10/+15 landing
//!    on 140 goes to 145, a +5 stays 140). A step across 140 only if `team_pair_cover(target)`
//!    ≥ `over_140_cover` (**B-over140**), else that message is skipped.
//! 3. **B-defend:** no ace, no pair and no 3+ halves in the hand, the partner's first call was a
//!    pass, the opponents are bidding, and my hand is near hopeless (`schwarz_defend`, from
//!    `standing_against_us`): first 140, then +5, just enough to stay ahead, up to its limit
//!    and never above `opp_reach` (beyond it the opponents overreach).
//!    Being played schwarz at their value costs us twice that value; declaring and failing
//!    costs it once, and we get the partner's 4 cards (owner: "a very seldom applying rule").
//! 4. **B-gate, part 1:** nothing told, my first or second call, value ≤ 125, no pair of my
//!    own, and strong cards or many halves: bid 140 (it closes the opponents' exchange; they
//!    lose if they overbid without a pair).
//! 5. **B-quiet:** pass.
//!
//! Taking the game (HB-8), claiming 140 (HB-140c), the barriers and raising come with the game
//! estimate in batch 2; until then they are off.

use beliefs::conventions::{follow_target, info_to_tell, next_context, HandClass, Message, PAIR_LINE, PAIR_MESSAGES};
use engine::{ActionType, CardSet, GameAction, SUITS};

use crate::rules::RuleId;
use crate::{decision, fact, Ctx, Decision};

// The 140 gate's thresholds: the orchestrator's proposal (`findings/kb/concepts.md`, batch 1b),
// read from the human 140 jumpers (KB-01 P2: 2.5 standing cards, 0.6 A+10, 1.7 aces on
// average) and the owner's example ("4 halves … first turn or second, directly 140"). To be
// confirmed by the owner.
/// Part 1: only on my first or second call.
const GATE1_CALLS: usize = 2;
/// Part 1: only up to this value.
const GATE1_MAX_VALUE: i32 = 125;
/// Part 1: strong cards: at least this many cards standing from my hand alone,
const GATE1_STANDING: u32 = 3;
/// or an ace and ten of one suit with at least this many aces,
const GATE1_ACES_WITH_ACE_TEN: u32 = 2;
/// or at least this many halves.
const GATE1_HALVES: u32 = 4;
/// Part 2: the values at which both teams' telling is done and the gate replaces a pair step.
const GATE2_MIN_VALUE: i32 = 125;
const GATE2_MAX_VALUE: i32 = 135;

/// B-defend's limit by `standing_against_us` (at least the count, the limit): 27 → 240,
/// 25–26 → 220, 24 → 200, 23 → 180, 21–22 → 140; 20 or fewer, no protective bid.
/// Owner-confirmed 2026-10-07 (`findings/kb/concepts.md`, batch 1b addendum 7).
const DEFEND_LIMITS: [(u32, i32); 5] = [(27, 240), (25, 220), (24, 200), (23, 180), (21, 140)];

fn find(legal: &[GameAction], bid: Option<i32>) -> Option<&GameAction> {
    legal.iter().find(|a| match (&a.action_type, bid) {
        (ActionType::NewBid(v), Some(b)) => *v == b,
        (ActionType::StopBidding, None) => true,
        _ => false,
    })
}

/// KB `schwarz_defend`: how high I bid to keep the opponents from playing us schwarz, or None
/// for a hand that needs no protective bid. In one place so the owner's rule can be refined.
fn schwarz_defend(ctx: &Ctx) -> Option<i32> {
    let standing = ctx.k.standing_against_us(ctx.me);
    DEFEND_LIMITS
        .iter()
        .find(|&&(n, _)| standing >= n)
        .map(|&(_, limit)| limit)
}

/// B-gate part 1's hand: strong cards or many halves, and no pair of my own.
fn gate_part1_hand(ctx: &Ctx, class: HandClass) -> bool {
    let (k, me) = (ctx.k, ctx.me);
    let hand = k.certain(me);
    let aces = (hand & CardSet::ACES).len();
    !class.any_pair()
        && (k.own_standing(me).len() >= GATE1_STANDING
            || (SUITS.into_iter().any(|s| k.ace_ten(me, s)) && aces >= GATE1_ACES_WITH_ACE_TEN)
            || (hand & CardSet::HALVES).len() >= GATE1_HALVES)
}

pub(crate) fn decide(ctx: &Ctx, legal: &[GameAction]) -> Decision {
    let p = ctx.params.conv();
    let class = HandClass::of(ctx.k.certain(ctx.me));
    // Read the record once (the believed reads below recompute the Mind's cache).
    let (bctx, calls, plan_formed, opponents_stepped) = {
        let record = ctx.mind.record();
        (
            next_context(&record.auction, ctx.me, p),
            record.calls(ctx.me),
            record.team_plan_formed(ctx.me),
            record.opponents_stepped(ctx.me),
        )
    };
    let v = bctx.value;
    let mut why = vec![fact("value", v)];
    let tell: Vec<Message> = info_to_tell(class, &bctx, p).collect();
    let gate1 = calls < GATE1_CALLS && v <= GATE1_MAX_VALUE && gate_part1_hand(ctx, class);
    // B-gate, part 2.
    let only_pairs = !tell.is_empty() && tell.iter().all(|m| PAIR_MESSAGES & m.bit() != 0);
    if (GATE2_MIN_VALUE..=GATE2_MAX_VALUE).contains(&v)
        && plan_formed
        && opponents_stepped
        && only_pairs
        && follow_target(tell[0], &bctx, p) > PAIR_LINE
    {
        if let Some(a) = find(legal, Some(PAIR_LINE)) {
            why.push(fact("gate_part", 2));
            return decision(a, RuleId::BGate, why);
        }
    }
    // B-tell (with B-over140).
    let mut skipped = false;
    if bctx.info_channel_open() {
        for &m in &tell {
            let target = follow_target(m, &bctx, p);
            if target > PAIR_LINE {
                let cover = ctx.mind.team_pair_cover(target);
                why.push(fact("team_pair_cover", cover));
                if cover < ctx.params.over_140_cover {
                    skipped = true;
                    continue;
                }
            }
            if let Some(a) = find(legal, Some(target)) {
                why.push(fact("message", m as u8));
                why.push(fact("step", target - v));
                if gate1 {
                    // Part 1's hand, but telling comes first (see the batch 1b report).
                    why.push(fact("gate_part1_hand", 1));
                }
                return decision(a, RuleId::BTell, why);
            }
        }
    }
    // B-defend: only a hand with no ace, no pair and no 3+ halves at all ([O] "with 3 halves,
    // pair or ace we have already a rule to follow"; "weak hands are those where the good
    // cards are mostly missing"); the first bid is 140 (addendum 8), then +5 steps.
    let nothing_in_hand = !class.ace && !class.any_pair() && class.n_halves() < 3;
    if nothing_in_hand && bctx.partner_passed_first && opponents_stepped {
        let bid = if v < PAIR_LINE { PAIR_LINE } else { v + 5 };
        // Never above what the opponents can reach (addendum 10): beyond it they overreach.
        let reach = ctx.mind.opp_reach(ctx.params.over_140_cover);
        if let Some(limit) = schwarz_defend(ctx).filter(|&l| bid <= l.min(reach)) {
            if let Some(a) = find(legal, Some(bid)) {
                why.push(fact("standing_against_us", ctx.k.standing_against_us(ctx.me)));
                why.push(fact("limit", limit));
                why.push(fact("opp_reach", reach));
                return decision(a, RuleId::BDefend, why);
            }
        }
    }
    // B-gate, part 1.
    if gate1 {
        if let Some(a) = find(legal, Some(PAIR_LINE)) {
            why.push(fact("gate_part", 1));
            return decision(a, RuleId::BGate, why);
        }
    }
    // B-quiet (B-over140 when it held a message back).
    let rule = if skipped { RuleId::BOver140 } else { RuleId::BQuiet };
    decision(find(legal, None).unwrap_or(&legal[0]), rule, why)
}

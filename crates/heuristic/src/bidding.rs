//! Bidding, KB batches 1 and 1b (`findings/kb/concepts.md`): the owner's convention, with no
//! estimate of the game. In this order:
//! 1. **B-gate, part 2:** both teams have told something (value 125–135, `team_plan_formed`,
//!    the opponents stepped) and my only live messages are pair messages that would cross 140:
//!    bid 140 instead (it hides the pair from the opponents; I play it anyway).
//! 2. **B-tell:** while `info_channel_open`, the first message of `info_to_tell` (with a pair,
//!    the ace first, then the pair; 3+ halves without a pair before the ace; a +10/+15 landing
//!    on 140 goes to 145, a +5 stays 140). A step across 140 only if `team_pair_cover(target)`
//!    ≥ `over_140_cover` (**B-over140**), else that message is skipped. **The tie-break**
//!    (batch 1c), when the message is a +5/+10 landing on 135 or below, in this order:
//!    - over 140 (**B-over140**): no own pair message left to tell (my pair goes by its own
//!      message) and `team_pair_cover` at the lowest bid above 140 ≥ `over_140_cover`, the team
//!      pair being derived from my single halves and the partner's signals: that bid;
//!    - 140 (**B-gate**) if `game_drive` ≥ 0.5;
//!    - else the step.
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

use beliefs::conventions::{
    follow_target, info_to_tell, next_context, pair_value, BidContext, BidObs, HandClass, Message, PAIR_LINE,
    PAIR_MESSAGES,
};
use engine::{ActionType, CardSet, GameAction, Suit, SUITS};

use crate::rules::RuleId;
use crate::{decision, fact, Ctx, Decision, Fact};

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

/// KB `game_drive` (batch 1c): the weights of P(prefer 140 over the step) = σ(Σ wᵢ·xᵢ −
/// `threshold`), in standing-card units. From the joint human-game analysis, owner-requested
/// (`findings/kb/concepts.md`, "Batch 1c"; the derived-pair weight is provisional). Not tuned.
struct DriveWeights {
    /// Per card standing from my hand alone (`own_standing`).
    standing: f32,
    /// Per suit with its ace and ten (`ace_ten`).
    ace_ten: f32,
    /// Per 100 of my own complete pairs' value.
    own_pair_per_100: f32,
    /// Per 100 of `derived_pair_value` (provisional).
    derived_pair_per_100: f32,
    /// The step lands on 135.
    lands_on_135: f32,
    /// The partner's last call was a pass.
    partner_passed: f32,
    /// The partner has not called yet.
    partner_not_acted: f32,
    /// The partner sent a pair message.
    partner_pair: f32,
    /// Messages still to tell (`info_to_tell`), each once.
    tell_two_halves: f32,
    tell_ace: f32,
    tell_small_pair: f32,
    tell_three_halves: f32,
    /// A big (or red) pair message whose bid crosses 140.
    tell_big_pair_over_140: f32,
    /// Σ ≥ this ⇔ P ≥ 0.5.
    threshold: f32,
}

const GAME_DRIVE: DriveWeights = DriveWeights {
    standing: 1.0,
    ace_ten: 0.9,
    own_pair_per_100: 1.9,
    derived_pair_per_100: 1.9,
    lands_on_135: 1.9,
    partner_passed: 0.3,
    partner_not_acted: -0.8,
    partner_pair: -0.6,
    tell_two_halves: -2.1,
    tell_ace: -1.2,
    tell_small_pair: -0.9,
    tell_three_halves: -0.4,
    tell_big_pair_over_140: 3.2,
    threshold: 4.3,
};

/// KB `p_take_140` (C43b): the log-odds of P(take the game at 140 rather than pass) when
/// nothing is left to tell. From the human-game analysis (B-quiet: 140 vs pass, 1,261
/// decisions at 125–135, `tools/kbstudy/quietfit.py`), owner-requested. Not tuned.
struct QuietWeights {
    intercept: f32,
    /// Per card standing from my hand alone (`own_standing`).
    standing: f32,
    /// Per suit with its ace and ten.
    ace_ten: f32,
    /// Per 100 of my own complete pairs' value.
    own_pair_per_100: f32,
    /// `team_pair_cover_derivable(145)`.
    cover_145: f32,
    /// Per card of `standing_potential`.
    potential: f32,
    /// The value is 135.
    at_135: f32,
    /// The partner's last call was a pass.
    partner_passed: f32,
    /// The partner has not called yet.
    partner_not_acted: f32,
    /// Per ace.
    aces: f32,
}

const P_TAKE_140: QuietWeights = QuietWeights {
    intercept: -4.85,
    standing: 0.39,
    ace_ten: 0.10,
    own_pair_per_100: 1.07,
    cover_145: 3.21,
    potential: 0.42,
    at_135: 0.84,
    partner_passed: -0.48,
    partner_not_acted: -0.21,
    aces: 0.76,
};

/// KB `take_vs_defend` (B-take): the log-odds of P(bid on rather than pass) at 140 or more
/// against an opponent's bid. From the human-game analysis (3,124 decisions, 35 % bid on;
/// `tools/kbstudy/overbid.py`), owner-requested. Not tuned. The bid level itself had no
/// measurable weight; `contract_estimate` bounds it instead.
struct TakeWeights {
    intercept: f32,
    standing: f32,
    aces: f32,
    own_pair_per_100: f32,
    /// `team_pair_cover(145)`.
    cover_145: f32,
    potential: f32,
    partner_passed: f32,
}

const TAKE_OVER_140: TakeWeights = TakeWeights {
    intercept: -5.30,
    standing: 0.22,
    aces: 0.31,
    own_pair_per_100: 0.46,
    cover_145: 5.45,
    potential: 0.24,
    partner_passed: -0.25,
};

/// B-take: bid the next step over an opponent's contract at 140 or more when
/// P(take) >= 0.5 and `contract_estimate` gives at least an even chance to make it.
fn take_over_140(
    ctx: &Ctx,
    legal: &[GameAction],
    class: HandClass,
    value: i32,
    mate: PartnerCalls,
    holder: Option<observation::Seat>,
    why: &mut Vec<Fact>,
) -> Option<Decision> {
    let w = &TAKE_OVER_140;
    let (k, me) = (ctx.k, ctx.me);
    let hand = k.certain(me);
    let own_pairs = uncalled_pair_value(k, class);
    let x = w.intercept
        + w.standing * k.own_standing(me).len() as f32
        + w.aces * (hand & CardSet::ACES).len() as f32
        + w.own_pair_per_100 * own_pairs as f32 / 100.0
        + w.cover_145 * ctx.mind.team_pair_cover(PAIR_LINE + 5)
        + w.potential * knowledge::standing_potential_of(hand) as f32
        + if mate.passed { w.partner_passed } else { 0.0 };
    let p_take = 1.0 / (1.0 + (-x).exp());
    let bid = value + 5;
    let estimate = crate::raising::contract_estimate(hand, own_pairs, ctx.mind.derived_pair_value());
    let p_make = crate::raising::p_make(estimate, bid);
    why.extend([
        fact("p_take", p_take),
        fact("contract_estimate", estimate),
        fact("p_make", p_make),
    ]);
    let rule_takes = p_take >= 0.5 && p_make >= 0.5;
    // The route (take_vs_defend over the believed worlds) overrules the fitted rule only where
    // it is clearly better.
    let takes = match holder.filter(|_| ctx.params.route_take) {
        Some(theirs) => match crate::outlook::take_or_defend(ctx, bid, theirs, value) {
            Some(v) => {
                why.extend([fact("take_value", v.mean(0)), fact("defend_value", v.mean(1))]);
                let (take, defend) = (0, 1);
                if rule_takes {
                    !v.clearly_better(defend, take)
                } else {
                    v.clearly_better(take, defend)
                }
            }
            None => rule_takes,
        },
        None => rule_takes,
    };
    if !takes {
        return None;
    }
    find(legal, Some(bid)).map(|a| decision(a, RuleId::BTake, why.clone()))
}

fn uncalled_pair_value(k: &knowledge::Knowledge, class: HandClass) -> i32 {
    SUITS
        .into_iter()
        .filter(|&s| class.has_pair(s) && !k.is_called(s))
        .map(pair_value)
        .sum()
}

/// KB `p_take_140` (C43b): P(bid 140 to take the game rather than pass), a probability; the
/// terms go to `why`.
fn p_take_140(ctx: &Ctx, class: HandClass, value: i32, mate: PartnerCalls, why: &mut Vec<Fact>) -> f32 {
    let w = &P_TAKE_140;
    let (k, me) = (ctx.k, ctx.me);
    let hand = k.certain(me);
    let own_pairs = uncalled_pair_value(k, class);
    let terms = [
        ("standing", w.standing * k.own_standing(me).len() as f32),
        (
            "ace_ten",
            w.ace_ten * SUITS.into_iter().filter(|&s| k.ace_ten(me, s)).count() as f32,
        ),
        ("own_pairs", w.own_pair_per_100 * own_pairs as f32 / 100.0),
        (
            "cover_145",
            w.cover_145 * ctx.mind.team_pair_cover_derivable(PAIR_LINE + 5),
        ),
        (
            "standing_potential",
            w.potential * knowledge::standing_potential_of(hand) as f32,
        ),
        ("at_135", if value == PAIR_LINE - 5 { w.at_135 } else { 0.0 }),
        ("partner_passed", if mate.passed { w.partner_passed } else { 0.0 }),
        (
            "partner_not_acted",
            if mate.not_acted { w.partner_not_acted } else { 0.0 },
        ),
        ("aces", w.aces * (hand & CardSet::ACES).len() as f32),
    ];
    let x = w.intercept + terms.iter().map(|t| t.1).sum::<f32>();
    why.extend(terms.iter().filter(|t| t.1 != 0.0).map(|&(n, v)| fact(n, v)));
    let p = 1.0 / (1.0 + (-x).exp());
    why.push(fact("p_take_140", p));
    p
}

/// The partner in the auction so far: no call yet, its last call a pass, a pair message sent.
#[derive(Clone, Copy)]
struct PartnerCalls {
    not_acted: bool,
    passed: bool,
    pair: bool,
}

/// What the tie-break looks at: my hand class, the messages I still have to tell, the auction
/// context, the step's bid, and the partner's calls.
struct TieBreak<'a> {
    class: HandClass,
    tell: &'a [Message],
    bctx: &'a BidContext,
    target: i32,
    mate: PartnerCalls,
}

/// KB `game_drive` (batch 1c): P(prefer 140 over the step), a probability (the store's
/// principle). v1 bids 140 at P ≥ 0.5.
fn game_drive(ctx: &Ctx, t: &TieBreak) -> f32 {
    let w = &GAME_DRIVE;
    let p = ctx.params.conv();
    let (k, me) = (ctx.k, ctx.me);
    let own_pairs = uncalled_pair_value(k, t.class);
    let mut x = w.standing * k.own_standing(me).len() as f32
        + w.ace_ten * SUITS.into_iter().filter(|&s| k.ace_ten(me, s)).count() as f32
        + w.own_pair_per_100 * own_pairs as f32 / 100.0
        + w.derived_pair_per_100 * ctx.mind.derived_pair_value() / 100.0;
    if t.target == PAIR_LINE - 5 {
        x += w.lands_on_135;
    }
    if t.mate.not_acted {
        x += w.partner_not_acted;
    } else if t.mate.passed {
        x += w.partner_passed;
    }
    if t.mate.pair {
        x += w.partner_pair;
    }
    let small_pair = !t.class.has_pair(Suit::Red)
        && !t.class.has_pair(Suit::Bells)
        && (t.class.has_pair(Suit::Acorns) || t.class.has_pair(Suit::Green));
    for &m in t.tell {
        x += match m {
            Message::TwoHalves => w.tell_two_halves,
            Message::Ace => w.tell_ace,
            Message::SmallOrHalves if small_pair => w.tell_small_pair,
            Message::SmallOrHalves => w.tell_three_halves,
            Message::BigPair | Message::RedPair if follow_target(m, t.bctx, p) > PAIR_LINE => w.tell_big_pair_over_140,
            Message::BigPair | Message::RedPair => 0.0,
        };
    }
    1.0 / (1.0 + (w.threshold - x).exp())
}

/// The tie-break (batch 1c) for a +5/+10 message landing on 135 or below: over 140, else 140,
/// else None (the step).
fn tie_break(ctx: &Ctx, legal: &[GameAction], t: &TieBreak, why: &mut Vec<Fact>) -> Option<Decision> {
    why.push(fact("tie_break", 1));
    if let Some(d) = over_140(ctx, legal, t, why) {
        return Some(d);
    }
    // 2. 140 if the game drive says so.
    let drive = game_drive(ctx, t);
    why.push(fact("game_drive", drive));
    if drive >= 0.5 {
        return Some(decision(find(legal, Some(PAIR_LINE))?, RuleId::BGate, why.clone()));
    }
    None
}

/// Step 1 of the chain: over 140, "we have a pair together", once my own pair and halves
/// messages are told (C43), on my complete pairs and the pairs derivable from the partner's
/// messages: the lowest legal bid above 140.
fn over_140(ctx: &Ctx, legal: &[GameAction], t: &TieBreak, why: &mut Vec<Fact>) -> Option<Decision> {
    let own_to_tell = t
        .tell
        .iter()
        .any(|m| (PAIR_MESSAGES | Message::TwoHalves.bit()) & m.bit() != 0);
    let over = legal
        .iter()
        .filter_map(|a| match a.action_type {
            ActionType::NewBid(v) if v > PAIR_LINE => Some(v),
            _ => None,
        })
        .min();
    if let (false, Some(v)) = (own_to_tell, over) {
        let cover = ctx.mind.team_pair_cover_derivable(v);
        why.push(fact("team_pair_cover", cover));
        why.push(fact("derived_pair_value", ctx.mind.derived_pair_value()));
        if cover >= ctx.params.over_140_cover {
            return Some(decision(find(legal, Some(v))?, RuleId::BOver140, why.clone()));
        }
    }
    None
}

fn find(legal: &[GameAction], bid: Option<i32>) -> Option<&GameAction> {
    legal.iter().find(|a| match (&a.action_type, bid) {
        (ActionType::NewBid(v), Some(b)) => *v == b,
        (ActionType::StopBidding, None) => true,
        _ => false,
    })
}

/// B-control and B-offer, the ownership bids at 140 or more (the owner's deals 848 and 573 with
/// Astra's review 2026-10-13): one ownership step per auction. Truthful untold information
/// does not spend that allowance; own control/keep steps do, even if a raw +5 also fits a message.
/// - **B-control:** a complete pair of mine runs from the top (`knowledge::control_run`): as the
///   declarer I lead its ace, announce and keep the lead through the suit, which defending can't
///   give me. Against an opponent's bid when my partner passed while telling was cheap (the owner:
///   "its really dangeorously if partner has gone weg first turn").
/// - **B-offer:** my partner holds the game with the Red pair shown and no ace shown in my team; I
///   haven't bid and hold an ace (the owner: "if you haven't said anything and have an ace and
///   partner signals red pair definitively without ace then you might as well give the 5 most of
///   the time"). It reads as the ace message. The keep: after my partner's offer over my pair
///   message, I keep the game with +5 while I hold that pair, so the pair's hand receives the
///   offered ace in the pass (the owner: the pair holder "could still signal the take of the
///   game if he wants it after the offer from the partner by overbidding by 5").
fn own_the_game(
    ctx: &Ctx,
    legal: &[GameAction],
    class: HandClass,
    bctx: &BidContext,
    why: &mut Vec<Fact>,
) -> Option<Decision> {
    let me = ctx.me;
    let record = ctx.mind.record();
    let hand = ctx.k.certain(me);
    let run = hand.pair_suits().find(|&s| knowledge::control_run(hand, s));
    let mine = &record.seats[me as usize].bids;
    let challenged = own_challenged(mine, class, run.is_some(), ctx.params.conv());
    why.push(fact("own_challenged", challenged as u8));
    if challenged || bctx.holder == Some(me) {
        return None;
    }
    let bid = find(legal, Some(bctx.value + 5))?;
    if let Some(s) = run {
        if bctx.holder.is_some_and(|h| h % 2 != me % 2) && bctx.partner_passed_first {
            why.push(fact("control_run", s as u8));
            return Some(decision(bid, RuleId::BControl, std::mem::take(why)));
        }
    }
    // The shared context retains the offer through an intervening opponent bid.
    // Keep only the Red pair actually offered to, not an unrelated pair after a bluff.
    if bctx.offer_reply && class.has_pair(Suit::Red) {
        why.push(fact("keep", 1));
        return Some(decision(bid, RuleId::BOffer, std::mem::take(why)));
    }
    if bctx.ace_offer && class.ace {
        why.push(fact("aces", (hand & CardSet::ACES).len()));
        return Some(decision(bid, RuleId::BOffer, std::mem::take(why)));
    }
    None
}

/// Bounded reconstruction from our unchanged auction hand and our own calls only.
/// Public increments never establish another actor's ownership intent. Recognise
/// the current policy's control/keep before testing truthful, live message content;
/// otherwise a control bid coinciding with an untold Ace could be reused indefinitely.
fn own_challenged(
    bids: &[BidObs],
    class: HandClass,
    control_run: bool,
    params: &beliefs::conventions::Params,
) -> bool {
    let mut told = 0;
    for obs in bids {
        let Some(bid) = obs.bid else {
            continue;
        };
        let mut ctx = obs.ctx;
        ctx.sent |= told;
        ctx.team_ace |= told & Message::Ace.bit() != 0;
        let high_step = ctx.value >= PAIR_LINE && bid == ctx.value + 5;
        let control = high_step
            && control_run
            && ctx.partner_passed_first
            && ctx.holder.is_some_and(|h| h % 2 != ctx.speaker % 2);
        let keep = high_step && ctx.offer_reply && class.has_pair(Suit::Red);
        if control || keep {
            return true;
        }
        let information = info_to_tell(class, &ctx, params).find(|&m| follow_target(m, &ctx, params) == bid);
        if let Some(m) = information {
            told |= m.bit();
        } else if ctx.value >= PAIR_LINE {
            return true;
        }
    }
    false
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
    let (bctx, calls, plan_formed, opponents_stepped, mate, opponent_holds, holder) = {
        let record = ctx.mind.record();
        let holder = record.auction.iter().rev().find_map(|&(s, b)| b.map(|_| s));
        let bids = &record.seats[engine::partner(ctx.me) as usize].bids;
        (
            next_context(&record.auction, ctx.me, p),
            record.calls(ctx.me),
            record.team_plan_formed(ctx.me),
            record.opponents_stepped(ctx.me),
            PartnerCalls {
                not_acted: bids.is_empty(),
                passed: bids.last().is_some_and(|b| b.bid.is_none()),
                pair: bids
                    .iter()
                    .any(|b| b.nominal.is_some_and(|m| PAIR_MESSAGES & m.bit() != 0)),
            },
            holder.is_some_and(|s| s % 2 != ctx.me % 2),
            holder,
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
                if m.step() <= 10 && target < PAIR_LINE {
                    let t = TieBreak {
                        class,
                        tell: &tell,
                        bctx: &bctx,
                        target,
                        mate,
                    };
                    if let Some(d) = tie_break(ctx, legal, &t, &mut why) {
                        return d;
                    }
                }
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
    // B-quiet with nothing left to tell (C43, C43b; owner on seed 42: "should have gone for the
    // 140 to take the game"), while 140 is still open: over 140, else 140 if `p_take_140` ≥ 0.5
    // (the real alternative here is the pass), else pass.
    if tell.is_empty() && v < PAIR_LINE {
        let t = TieBreak {
            class,
            tell: &tell,
            bctx: &bctx,
            target: v,
            mate,
        };
        why.push(fact("tie_break", 1));
        if let Some(d) = over_140(ctx, legal, &t, &mut why) {
            return d;
        }
        if p_take_140(ctx, class, v, mate, &mut why) >= ctx.params.take_140_min {
            if let Some(a) = find(legal, Some(PAIR_LINE)) {
                return decision(a, RuleId::BGate, why);
            }
        }
    }
    if v >= PAIR_LINE {
        if let Some(d) = own_the_game(ctx, legal, class, &bctx, &mut why) {
            return d;
        }
    }
    // B-take (KB `take_vs_defend`; owner: "often better to take than be played"): at 140 or
    // more against an opponent's bid.
    if v >= PAIR_LINE && opponent_holds && ctx.params.b_take {
        if let Some(d) = take_over_140(ctx, legal, class, v, mate, holder, &mut why) {
            return d;
        }
    }
    // B-quiet (B-over140 when it held a message back).
    let rule = if skipped { RuleId::BOver140 } else { RuleId::BQuiet };
    decision(find(legal, None).unwrap_or(&legal[0]), rule, why)
}

#[cfg(test)]
mod ownership_tests {
    use super::*;
    use beliefs::conventions::{walk_auction, Params};

    #[test]
    fn own_information_is_truthful_and_consumed_once_even_when_publicly_ambiguous() {
        let params = Params {
            tell_window: 0,
            ..Params::default()
        };
        let read = walk_auction(
            &[
                (0, Some(130)),
                (1, Some(145)),
                (2, Some(150)),
                (3, None),
                (0, Some(155)),
                (1, Some(160)),
                (2, Some(165)),
            ],
            &params,
        );
        let ace = HandClass {
            ace: true,
            halves: [0; 4],
        };
        assert_eq!(read[2].nominal, None); // other actors cannot know the purpose
        assert!(!own_challenged(&[read[2]], ace, false, &params));
        assert!(own_challenged(&[read[2], read[6]], ace, false, &params));
        assert!(own_challenged(&[read[2]], HandClass { ace: false, ..ace }, false, &params));
    }

    #[test]
    fn own_control_is_not_reclassified_as_an_untold_ace() {
        let params = Params {
            tell_window: 0,
            ..Params::default()
        };
        let read = walk_auction(
            &[
                (0, None),
                (1, Some(135)),
                (2, Some(150)),
                (3, None),
                (1, Some(155)),
                (2, Some(160)),
            ],
            &params,
        );
        let mut class = HandClass {
            ace: true,
            halves: [0; 4],
        };
        class.halves[Suit::Bells as usize] = 2;
        assert!(own_challenged(&[read[2], read[5]], class, true, &params));
        assert!(!own_challenged(&[read[2], read[5]], class, false, &params));
    }
}

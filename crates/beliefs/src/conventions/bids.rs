//! The bidding convention: ONE generative model `P(bid | hand class, public context)` (§1.4).
//!
//! The speaker picks a message by HB-1…HB-5 among the messages that are still live for it
//! (HB-0), and bids its target with the 140 shift (HB-140a). Humans do not always follow the
//! rule, so the likelihood mixes the rule (confidence by context, HR-2), a literal reading
//! without the shift, a generic step and a block component (HR-7). Acting uses `message` and
//! `follow_target`; reading inverts `bid_likelihood` (or HP1's own policy, `hp1::bid_prob`) over
//! the 162 hand classes (`bid_tables`). Moved from `heuristic::conventions` (D-d).

use engine::{next_seat, partner, Suit};
use observation::Seat;

use super::classes::{class_prior, pair_value, HandClass, BIG_PAIRS, N_CLASSES, SMALL_PAIRS};
use super::{hp1, Params, SeatKind};

/// What the convention can say with one step. Passing says "nothing (more) to tell".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    /// +5: "I hold an ace", only while no ace was shown in my team.
    Ace,
    /// +5 after my team showed an ace: "I hold 2 halves".
    TwoHalves,
    /// +10: a small pair (Green, Acorns) or 3–4 halves.
    SmallOrHalves,
    /// +15: a big pair (Bells, Red).
    BigPair,
    /// +20 as a player's first step: the Red pair.
    RedPair,
}

pub const MESSAGES: [Message; 5] = [
    Message::Ace,
    Message::TwoHalves,
    Message::SmallOrHalves,
    Message::BigPair,
    Message::RedPair,
];

impl Message {
    pub fn step(self) -> i32 {
        match self {
            Message::Ace | Message::TwoHalves => 5,
            Message::SmallOrHalves => 10,
            Message::BigPair => 15,
            Message::RedPair => 20,
        }
    }

    pub fn bit(self) -> u8 {
        1 << self as u8
    }

    /// Whether a hand of this class has the message's content.
    /// `halves_in_window`: 3 halves still count for `SmallOrHalves` (see `BidContext::live`).
    pub fn holds(self, class: HandClass, strict_two_halves: bool, halves_in_window: bool) -> bool {
        match self {
            Message::RedPair => class.has_pair(Suit::Red),
            Message::BigPair => BIG_PAIRS.iter().any(|&s| class.has_pair(s)),
            Message::SmallOrHalves => {
                SMALL_PAIRS.iter().any(|&s| class.has_pair(s)) || (halves_in_window && class.n_halves() >= 3)
            }
            Message::Ace => class.ace,
            Message::TwoHalves => {
                class.n_halves() >= 2
                    && (!strict_two_halves
                        || engine::SUITS
                            .iter()
                            .filter(|&&s| class.halves[s as usize] > 0)
                            .map(|&s| pair_value(s))
                            .sum::<i32>()
                            >= PAIR_LINE)
            }
        }
    }
}

/// The public context of one bidding action, the same for every observer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BidContext {
    pub speaker: Seat,
    /// The game value before the action (115 before the first bid).
    pub value: i32,
    /// The speaker has not bid yet (passing is final, so this is its first step).
    pub first_step: bool,
    /// The speaker's team has shown `Ace`.
    pub team_ace: bool,
    /// The speaker's team has bid at all.
    pub team_any: bool,
    /// Messages the speaker has sent (bit set of `Message::bit`).
    pub sent: u8,
    /// The partner's first action was a pass.
    pub partner_passed_first: bool,
    /// The other team has shown `BigPair` or `RedPair`.
    pub other_big: bool,
    /// The speaker's team has shown a pair message (`SmallOrHalves`, `BigPair`, `RedPair`).
    pub team_pair_shown: bool,
    /// Bidding actions (bids and passes) before this one.
    pub n_before: u32,
    /// KB `opponents_exchanging`: an opponent made an information step (a message, below 140)
    /// and the information channel is still open.
    pub opponents_exchanging: bool,
}

impl BidContext {
    /// KB `info_channel_open`: telling still reaches the partner below 140 (a 140 gate by the
    /// other team closes it, and then the value is 140 or more).
    pub fn info_channel_open(&self) -> bool {
        self.value < PAIR_LINE
    }

    /// HB-0: the messages that are still worth telling the partner. A message drops out when
    /// the team already told it, when I sent it, or when its chance has passed: the ace and the
    /// halves are told only in the first `tell_window` actions, and an ace alone is not told at
    /// 140 or more without a pair shown in the team (the owner's `_update_to_be_communicated`
    /// idea in refactor_step2, a lesson per C11).
    pub fn live(&self, params: &Params) -> u8 {
        let mut live = 0;
        let sent = |m: Message| self.sent & m.bit() != 0;
        let in_window = params.tell_window == 0 || self.n_before < params.tell_window;
        let ace_viable = !(params.ace_at_140_needs_pair && self.value >= PAIR_LINE && !self.team_pair_shown);
        if !self.team_ace {
            if in_window && ace_viable {
                live |= Message::Ace.bit();
            }
        } else if !sent(Message::TwoHalves) && in_window {
            live |= Message::TwoHalves.bit();
        }
        if !sent(Message::SmallOrHalves) {
            live |= Message::SmallOrHalves.bit();
        }
        if !sent(Message::BigPair) && !sent(Message::RedPair) {
            live |= Message::BigPair.bit();
        }
        if self.first_step {
            live |= Message::RedPair.bit();
        }
        live
    }
}

/// KB `info_to_tell`: the messages a hand of `class` holds that are still live (HB-0), in the
/// order they are told: with a pair, the ace first (+5), then the pair; with 3+ halves and no
/// pair, the halves (+10) before the ace ([O] Q1a; the owner's correction of 2026-10-07, C39:
/// "halves first" is for halves only).
pub fn info_to_tell(class: HandClass, ctx: &BidContext, params: &Params) -> impl Iterator<Item = Message> {
    let live = ctx.live(params);
    let in_window = params.tell_window == 0 || ctx.n_before < params.tell_window;
    let strict = params.strict_two_halves;
    let order = if class.any_pair() {
        [
            Message::Ace,
            Message::RedPair,
            Message::BigPair,
            Message::SmallOrHalves,
            Message::TwoHalves,
        ]
    } else {
        [
            Message::RedPair,
            Message::BigPair,
            Message::SmallOrHalves,
            Message::Ace,
            Message::TwoHalves,
        ]
    };
    order
        .into_iter()
        .filter(move |&m| live & m.bit() != 0 && m.holds(class, strict, in_window))
}

/// HB-1 … HB-5: the first message of `info_to_tell`.
pub fn message(class: HandClass, ctx: &BidContext, params: &Params) -> Option<Message> {
    info_to_tell(class, ctx, params).next()
}

/// The messages whose landing on 140 makes the shift (owner, verbatim file #12: "you probably skip the 140 and
/// make a 15"; the answer to #16: "some humans … don't care for the +5 step for the two
/// halves"): +10 and +15. A +5 (ace, two halves) landing on 140 stays 140 and shifts nothing.
const SHIFTS_OFF_140: u8 = 1 << Message::SmallOrHalves as u8 | 1 << Message::BigPair as u8;

/// HB-140a: the bid that carries `m`, for the encoder and the reading alike. If a live +10 or
/// +15 message's raw target lands exactly on 140, it and every live message with a larger raw
/// target shift by +5 (from 130: +10 → 145, +15 → 150; from 125: +15 → 145, +20 → 150). A +5
/// neither shifts nor makes a shift (from 135: +5 → 140, +10 → 145, +15 → 150). The variant
/// `skip_on_any_cross` shifts every step that crosses 140 instead.
pub fn follow_target(m: Message, ctx: &BidContext, params: &Params) -> i32 {
    let raw = ctx.value + m.step();
    let shift = if params.skip_on_any_cross {
        ctx.value < PAIR_LINE && raw >= PAIR_LINE
    } else {
        let live = ctx.live(params) & SHIFTS_OFF_140;
        let hits_140 = MESSAGES
            .iter()
            .any(|&l| live & l.bit() != 0 && ctx.value + l.step() == PAIR_LINE);
        hits_140 && raw >= PAIR_LINE
    };
    raw + if shift { 5 } else { 0 }
}

/// HR-2: how likely the speaker follows the step rules here.
pub fn confidence(m: Option<Message>, ctx: &BidContext, params: &Params) -> f32 {
    let mut c = params.conf_general;
    if m == Some(Message::TwoHalves) {
        c = c.min(params.conf_two_halves);
    }
    if !ctx.team_any {
        c = c.min(params.conf_team_first);
    }
    if ctx.partner_passed_first {
        c = c.min(params.conf_partner_passed);
    }
    c
}

/// Messages that show a pair (or 3 halves) in the team.
pub const PAIR_MESSAGES: u8 =
    1 << Message::SmallOrHalves as u8 | 1 << Message::BigPair as u8 | 1 << Message::RedPair as u8;

/// The pair line: bids above it promise a pair in the team (owner convention; HB-140a/b/c,
/// HR-3, HR-4, HZ-3). The one place this number lives.
pub const PAIR_LINE: i32 = 140;

pub const BARRIERS: [i32; 4] = [PAIR_LINE, 180, 200, 220];

/// `P(action | class, ctx)`: the generative bid model for seats that do not play HP1 (humans;
/// P3: a Red pair mostly as +15). HP1's own encoder sends `message` at `follow_target`; HP1
/// seats are read with `bidding::hp1_bid_prob`. `bid` is `None` for a pass; `trust` is the
/// observer's trust θ in this speaker (§3.4).
pub fn bid_likelihood(bid: Option<i32>, class: HandClass, ctx: &BidContext, trust: f32, params: &Params) -> f32 {
    let m = message(class, ctx, params);
    let c = confidence(m, ctx, params);
    let (follow, literal) = match m {
        None => (None, None),
        Some(m) => (Some(follow_target(m, ctx, params)), Some(ctx.value + m.step())),
    };
    // HB-140b from the speaker's side: without a pair of its own it may not cross 140.
    let p_follow = match follow {
        Some(t) if t > PAIR_LINE && !class.any_pair() => {
            params.cross_without_own_pair * (bid == follow) as u8 as f32
                + (1.0 - params.cross_without_own_pair) * bid.is_none() as u8 as f32
        }
        // P3, humans: a Red pair is mostly shown as a big pair (+15), less often by +20.
        _ if m == Some(Message::RedPair) => {
            let big = follow_target(Message::BigPair, ctx, params);
            (1.0 - params.human_red_as_big) * (bid == follow) as u8 as f32
                + params.human_red_as_big * (bid == Some(big)) as u8 as f32
        }
        _ => (bid == follow) as u8 as f32,
    };
    let base = generic_step(bid, ctx, params);
    // HR-4: unexplained bids above 140 come mostly from seats with a pair.
    let generic = if bid.is_some_and(|b| b > PAIR_LINE) && !class.any_pair() {
        base * params.generic_cross_no_pair
    } else {
        base
    };
    // HR-3: a literal step that lands exactly on 140 is not read as that message.
    let literal = literal.filter(|&t| t != PAIR_LINE);
    let conv = c * p_follow
        + (1.0 - c) * (params.literal_share * (bid == literal) as u8 as f32 + (1.0 - params.literal_share) * generic);
    // Trust θ (§3.4): P_read = θ·P_conv + (1−θ)·P_base, the base being class-independent.
    let read = trust * conv + (1.0 - trust) * base;
    // HR-7: a barrier bid can be a block, more likely right after the other team showed a big
    // pair; HR-3: 140 itself is also a plain claim or defence (class-independent mass).
    let block = if ctx.other_big {
        params.block_prior
    } else {
        params.block_base
    };
    let barrier = bid.is_some_and(|b| BARRIERS.contains(&b));
    let n_barriers = BARRIERS.iter().filter(|&&b| b > ctx.value).count().max(1) as f32;
    let claim = if bid == Some(PAIR_LINE) && ctx.value < PAIR_LINE {
        params.claim_140_prior
    } else {
        0.0
    };
    // The floor ε is applied by the reader, relative to the most likely hand (`beliefs`).
    (1.0 - block - claim) * read + block * barrier as u8 as f32 / n_barriers + claim
}

/// The public reading of an action, used only to track the context: the live message whose
/// shifted target (else whose literal target) equals the bid.
pub fn nominal(bid: Option<i32>, ctx: &BidContext, params: &Params) -> Option<Message> {
    let bid = bid?;
    let live = ctx.live(params);
    let candidates = || MESSAGES.into_iter().filter(move |&m| live & m.bit() != 0);
    candidates()
        .find(|&m| follow_target(m, ctx, params) == bid)
        .or_else(|| candidates().find(|&m| ctx.value + m.step() == bid))
}

/// One observed bidding action with its public context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BidObs {
    pub bid: Option<i32>,
    pub ctx: BidContext,
    /// The public reading of the action (context tracking only).
    pub nominal: Option<Message>,
}

/// The public auction state, replayed action by action.
#[derive(Clone, Debug)]
struct Auction {
    value: i32,
    sent: [u8; 4],
    has_bid: [bool; 4],
    /// `Some(true)`: the seat's first action was a pass.
    first_action: [Option<bool>; 4],
    /// The seat made an information step (a message, below 140).
    informed: [bool; 4],
    n: u32,
}

impl Auction {
    fn new() -> Auction {
        Auction {
            value: super::NO_BID,
            sent: [0; 4],
            has_bid: [false; 4],
            first_action: [None; 4],
            informed: [false; 4],
            n: 0,
        }
    }

    fn context(&self, seat: Seat) -> BidContext {
        let (s, mate) = (seat as usize, partner(seat) as usize);
        let team_sent = self.sent[s] | self.sent[mate];
        let left = next_seat(seat);
        let right = partner(left);
        let other_sent = self.sent[left as usize] | self.sent[right as usize];
        let exchanging = self.informed[left as usize] || self.informed[right as usize];
        BidContext {
            speaker: seat,
            value: self.value,
            first_step: !self.has_bid[s],
            team_ace: team_sent & Message::Ace.bit() != 0,
            team_any: self.has_bid[s] || self.has_bid[mate],
            sent: self.sent[s],
            partner_passed_first: self.first_action[mate] == Some(true),
            other_big: other_sent & (Message::BigPair.bit() | Message::RedPair.bit()) != 0,
            team_pair_shown: team_sent & PAIR_MESSAGES != 0,
            n_before: self.n,
            opponents_exchanging: exchanging && self.value < PAIR_LINE,
        }
    }

    fn apply(&mut self, seat: Seat, bid: Option<i32>, params: &Params) -> BidObs {
        let ctx = self.context(seat);
        let nominal = nominal(bid, &ctx, params);
        let s = seat as usize;
        if let Some(m) = nominal {
            self.sent[s] |= m.bit();
            self.informed[s] |= bid.is_some_and(|b| b < PAIR_LINE);
        }
        self.first_action[s].get_or_insert(bid.is_none());
        if let Some(b) = bid {
            self.value = b;
            self.has_bid[s] = true;
        }
        self.n += 1;
        BidObs { bid, ctx, nominal }
    }
}

/// Walks the auction (`(seat, bid or pass)` in order) and attaches the public context.
pub fn walk_auction(actions: &[(Seat, Option<i32>)], params: &Params) -> Vec<BidObs> {
    let mut auction = Auction::new();
    actions
        .iter()
        .map(|&(seat, bid)| auction.apply(seat, bid, params))
        .collect()
}

/// The public context of `seat`'s next bidding action.
pub fn next_context(actions: &[(Seat, Option<i32>)], seat: Seat, params: &Params) -> BidContext {
    let mut auction = Auction::new();
    for &(s, bid) in actions {
        auction.apply(s, bid, params);
    }
    auction.context(seat)
}

/// Per observed bid, its likelihood for every hand class, with the floor ε relative to the most
/// likely class: a bluff only down-weights a hand, it never rules it out (RQ-16). P3: a seat
/// playing HP1 is read with HP1's own bid policy (`hp1::bid_prob`) if `read_hp1_as_hp1`; any
/// other seat with the human-calibrated convention model (`bid_likelihood`).
pub fn bid_tables(bids: &[BidObs], trust: f32, kind: SeatKind, params: &Params) -> Vec<[f32; N_CLASSES]> {
    bids.iter().map(|obs| bid_table(obs, trust, kind, params)).collect()
}

/// The class-independent step distribution: +5 … +20 or a pass, else a rare jump.
pub fn generic_step(bid: Option<i32>, ctx: &BidContext, params: &Params) -> f32 {
    match bid {
        None => (1.0 - params.generic_eps) / 5.0,
        Some(b) if [5, 10, 15, 20].contains(&(b - ctx.value)) => (1.0 - params.generic_eps) / 5.0,
        Some(_) => params.generic_eps / 20.0,
    }
}

/// One observed bid's likelihood table (see `bid_tables`). An HP1 seat's policy is mixed with
/// the class-independent step by the trust θ, like the human model (D-f), so a bid its policy
/// does not predict (HB-9) reads as uninformative, never as impossible.
pub fn bid_table(obs: &BidObs, trust: f32, kind: SeatKind, params: &Params) -> [f32; N_CLASSES] {
    let generic = generic_step(obs.bid, &obs.ctx, params);
    let mut table: [f32; N_CLASSES] = std::array::from_fn(|i| {
        let class = HandClass::from_index(i);
        if kind == SeatKind::Heuristic && params.read_hp1_as_hp1 {
            trust * hp1::bid_prob(obs.bid, class, &obs.ctx, params) + (1.0 - trust) * generic
        } else {
            bid_likelihood(obs.bid, class, &obs.ctx, trust, params)
        }
    });
    let max = table.iter().cloned().fold(0.0, f32::max);
    if max <= 0.0 {
        return [1.0; N_CLASSES];
    }
    table.iter_mut().for_each(|x| *x = x.max(params.convention_floor * max));
    table
}

/// Class posterior of a seat's dealt hand from its bids (exact over the 162 classes): the prior
/// from per-card probabilities times the likelihood of every observed action.
pub fn class_posterior(
    prior_q: &[f32; 36],
    bids: &[BidObs],
    trust: f32,
    kind: SeatKind,
    params: &Params,
) -> [f32; N_CLASSES] {
    let mut post = class_prior(prior_q);
    for table in bid_tables(bids, trust, kind, params) {
        for (w, l) in post.iter_mut().zip(table) {
            *w *= l;
        }
    }
    let sum: f32 = post.iter().sum();
    if sum > 0.0 {
        post.iter_mut().for_each(|x| *x /= sum);
    }
    post
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(value: i32, team_ace: bool) -> BidContext {
        BidContext {
            value,
            first_step: true,
            team_ace,
            team_any: true,
            ..Default::default()
        }
    }

    #[test]
    fn shift_from_125_moves_every_message_from_the_140_hit_upwards() {
        let p = Params::default();
        let c = ctx(125, false);
        assert_eq!(follow_target(Message::Ace, &c, &p), 130);
        assert_eq!(follow_target(Message::SmallOrHalves, &c, &p), 135);
        assert_eq!(follow_target(Message::BigPair, &c, &p), 145);
        assert_eq!(follow_target(Message::RedPair, &c, &p), 150);
    }

    #[test]
    fn no_shift_from_135_when_the_ace_was_shown() {
        let p = Params::default();
        let c = BidContext {
            first_step: false,
            ..ctx(135, true)
        };
        // TwoHalves was sent too; no +10/+15 lands on 140.
        let c = BidContext {
            sent: Message::TwoHalves.bit(),
            ..c
        };
        assert_eq!(follow_target(Message::BigPair, &c, &p), 150);
        assert_eq!(follow_target(Message::SmallOrHalves, &c, &p), 145);
    }

    #[test]
    fn a_plus_5_neither_shifts_nor_makes_a_shift() {
        // Owner (verbatim file #12, and the answer in #16; C39 addendum): only a +10/+15 landing on 140 shifts,
        // with every larger message; a +5 landing on 140 stays 140.
        let p = Params::default();
        let at = |c: &BidContext| MESSAGES.map(|m| (m, follow_target(m, c, &p)));
        // 135, first step, no ace in the team: +5 → 140, +10 → 145, +15 → 150, +20 → 155.
        let c = ctx(135, false);
        assert_eq!(
            at(&c)[..1].iter().chain(&at(&c)[2..]).map(|t| t.1).collect::<Vec<_>>(),
            [140, 145, 150, 155]
        );
        // 135 after the team's ace: two halves +5 → 140, and no shift for the rest.
        let c = BidContext {
            first_step: false,
            ..ctx(135, true)
        };
        assert_eq!(follow_target(Message::TwoHalves, &c, &p), 140);
        assert_eq!(follow_target(Message::SmallOrHalves, &c, &p), 145);
        assert_eq!(follow_target(Message::BigPair, &c, &p), 150);
        // 130: +10 → 145, +15 → 150.
        let c = ctx(130, false);
        assert_eq!(follow_target(Message::SmallOrHalves, &c, &p), 145);
        assert_eq!(follow_target(Message::BigPair, &c, &p), 150);
        // 125: +15 → 145, +20 → 150.
        let c = ctx(125, false);
        assert_eq!(follow_target(Message::BigPair, &c, &p), 145);
        assert_eq!(follow_target(Message::RedPair, &c, &p), 150);
        // The reading takes the same targets.
        assert_eq!(nominal(Some(140), &ctx(135, false), &p), Some(Message::Ace));
        assert_eq!(nominal(Some(145), &ctx(130, false), &p), Some(Message::SmallOrHalves));
    }

    #[test]
    fn any_cross_variant_shifts_each_crossing_step() {
        let p = Params {
            skip_on_any_cross: true,
            ..Params::default()
        };
        let c = BidContext {
            sent: Message::TwoHalves.bit(),
            first_step: false,
            ..ctx(135, true)
        };
        assert_eq!(follow_target(Message::BigPair, &c, &p), 155);
    }
}

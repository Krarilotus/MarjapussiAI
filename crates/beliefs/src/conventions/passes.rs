//! The passing conventions as generative models (§1.4, §5): the HP score (passing forth, MH)
//! and the HK score (passing back, VH) define `P(option | hand) = softmax(score / τ)`. Acting
//! takes the argmax; `PassReading` inverts the same scores to read a pass. Moved from
//! `heuristic::passing` (D-d); the rule IDs of the score terms stay with the heuristic.
//!
//! Every input of a score is the giver's hand plus public context, so any observer who
//! hypothesises the giver's hand can evaluate it.

use engine::Suit;
use engine::{points_set, CARD_POINTS};
use engine::{suit_of, CardSet, SUITS};

use super::bids::PAIR_LINE;
use super::classes::{len_in, pair_value, LUSCHE};
use super::Params;

/// Public context of the pass forth, from the giver's (MH's) side.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ForthCtx {
    /// What the partner (VH) believes from my bids: an ace, and each pair.
    pub partner_knows_ace: bool,
    pub partner_knows_pair: [bool; 4],
    /// VH showed an ace in the bidding.
    pub partner_showed_ace: bool,
    /// VH showed meaningful halves (TwoHalves, SmallOrHalves or a pair message).
    pub partner_showed_halves: bool,
    /// VH's believed pairs (from its bids).
    pub partner_pair: [bool; 4],
    /// VH's last bid.
    pub partner_bid: i32,
}

/// Public context of the pass back, from the giver's (VH's) side.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BackCtx {
    /// P(MH holds a half of s), for the plan P-T(s) via MH.
    pub partner_half: [f32; 4],
}

/// VH's game plan (§5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    Trump(Suit),
    NoTrump,
}

/// A scored option: the total and the contribution of each rule.
#[derive(Clone, Copy, Debug, Default)]
pub struct Score {
    pub total: f32,
    pub terms: [f32; 8],
}

/// Where a score's rule terms go: the full breakdown (`Score`) or just the total (`f32`,
/// for the readers' softmax). One scoring body serves both, so totals are bit-identical.
pub trait Sink: Default {
    fn add(&mut self, term: usize, value: f32);
}

impl Sink for Score {
    fn add(&mut self, term: usize, value: f32) {
        self.terms[term] += value;
        self.total += value;
    }
}

#[derive(Default)]
pub struct Total(pub f32);

impl Sink for Total {
    fn add(&mut self, _term: usize, value: f32) {
        self.0 += value;
    }
}

impl Score {
    /// The rule that most favours this option over `other` (largest term difference); if no
    /// term favours it (a tie broken by points), the largest term.
    pub fn deciding_term(&self, other: &Score) -> usize {
        let best = (0..8)
            .max_by(|&a, &b| (self.terms[a] - other.terms[a]).total_cmp(&(self.terms[b] - other.terms[b])))
            .unwrap_or(0);
        if self.terms[best] - other.terms[best] > 0.0 {
            best
        } else {
            self.main_term()
        }
    }

    /// The rule that contributed most to this option.
    pub fn main_term(&self) -> usize {
        let best = (0..8)
            .max_by(|&a, &b| self.terms[a].total_cmp(&self.terms[b]))
            .unwrap_or(0);
        if self.terms[best] > 0.0 {
            best
        } else {
            (0..8)
                .max_by(|&a, &b| self.terms[a].abs().total_cmp(&self.terms[b].abs()))
                .unwrap_or(0)
        }
    }
}

/// All 4-card subsets of `hand`.
pub fn options(hand: CardSet) -> Vec<CardSet> {
    let mut out = Vec::with_capacity(715);
    options_into(hand, &mut out);
    out
}

/// All 4-card subsets of `hand` into `out` (cleared first), in the order of `options`.
pub fn options_into(hand: CardSet, out: &mut Vec<CardSet>) {
    out.clear();
    for_each_option(hand, |x| out.push(x));
}

/// Calls `f` for every 4-card subset of `hand`, in ascending card order.
pub fn for_each_option(hand: CardSet, mut f: impl FnMut(CardSet)) {
    let mut cards = [0u64; 13];
    let mut n = 0;
    for c in hand.iter().take(13) {
        cards[n] = 1 << c;
        n += 1;
    }
    for a in 0..n {
        for b in a + 1..n {
            let ab = cards[a] | cards[b];
            for c in b + 1..n {
                let abc = ab | cards[c];
                for &card in &cards[c + 1..n] {
                    f(CardSet(abc | card));
                }
            }
        }
    }
}

/// HP-7: no option blanks two suits ("if you can't comfortably pass two colours").
pub fn awkward(hand: CardSet) -> bool {
    // The two shortest non-empty suits together hold more than 4 cards.
    let (mut a, mut b, mut n) = (u32::MAX, u32::MAX, 0);
    for s in SUITS {
        let l = len_in(hand, s);
        if l == 0 {
            continue;
        }
        n += 1;
        if l < a {
            b = a;
            a = l;
        } else if l < b {
            b = l;
        }
    }
    n < 2 || a + b > 4
}

/// Per lane, the sum of the cards' byte-lane words over the cards of `x` (both pass models score
/// an option from these sums).
fn lane_sums<const N: usize>(x: CardSet, lanes: [&[u64; 36]; N]) -> [u64; N] {
    let mut sums = [0u64; N];
    let mut bits = x.0;
    while bits != 0 {
        let i = bits.trailing_zeros() as usize;
        for (sum, lane) in sums.iter_mut().zip(lanes) {
            *sum += lane[i];
        }
        bits &= bits - 1;
    }
    sums
}

/// A pass model as its readers use it: the total score of one 4-card option.
pub trait PassModel {
    fn total(&self, option: CardSet) -> f32;
}

impl PassModel for ForthModel<'_> {
    fn total(&self, x: CardSet) -> f32 {
        ForthModel::total(self, x)
    }
}

impl PassModel for BackModel<'_> {
    fn total(&self, y: CardSet) -> f32 {
        BackModel::total(self, y)
    }
}

/// The HP score for one MH hand (§5.1, C3, C4). Everything that depends only on the hand and
/// the context is computed once. Per option, the integer counts the terms need are summed from
/// per-card byte lanes (four lookups instead of a dozen bit counts); the float arithmetic is the
/// same as written per term below.
pub struct ForthModel<'a> {
    p: &'a Params,
    /// Per card, byte lanes: 0 unknown half, 1 ace, 2 half, 3 needed half, 4 green, 5 points,
    /// 6..7 pairs 0..1.
    lane_a: [u64; 36],
    /// Per card: lanes 0..3 Lusche of suit s, 4..7 tens and halves of suit s.
    lane_b: [u64; 36],
    /// Per card: lanes 0..1 pairs 2..3, 2..5 cards of suit s.
    lane_c: [u64; 36],
    /// Cards per suit in the hand.
    suit_len: [u64; 4],
    info_ace: bool,
    info_complete: bool,
    need_ace: bool,
    boost: f32,
    awkward: bool,
    /// Penalty or bonus for passing both halves of my pair i.
    pair_both: [f32; 4],
    n_pairs: usize,
}

fn lane(i: usize) -> u64 {
    1 << (8 * i)
}

fn byte(x: u64, i: usize) -> u32 {
    ((x >> (8 * i)) & 0xFF) as u32
}

impl<'a> ForthModel<'a> {
    pub fn new(hand: CardSet, ctx: &ForthCtx, p: &'a Params) -> ForthModel<'a> {
        let awkward = awkward(hand);
        let unknown_halves = SUITS
            .iter()
            .filter(|&&t| !ctx.partner_knows_pair[t as usize])
            .fold(CardSet::EMPTY, |a, &t| a | CardSet::halves(t));
        let need_halves = if ctx.partner_bid > PAIR_LINE && !ctx.partner_pair.contains(&true) {
            SUITS
                .iter()
                .filter(|&&t| pair_value(t) >= ctx.partner_bid - PAIR_LINE)
                .fold(CardSet::EMPTY, |a, &t| a | CardSet::halves(t))
        } else {
            CardSet::EMPTY
        };
        let mut pair_both = [0.0; 4];
        let mut pair_halves = [CardSet::EMPTY; 4];
        let mut n_pairs = 0;
        for t in hand.pair_suits() {
            let big = matches!(t, Suit::Bells | Suit::Red);
            pair_both[n_pairs] = if big || ctx.partner_knows_pair[t as usize] {
                -p.w_keep_pair
            } else {
                let acorns = if t == Suit::Acorns { p.w_pass_acorns } else { 0.0 };
                let needed = if ctx.partner_showed_halves {
                    0.0
                } else {
                    p.w_pass_needed
                };
                acorns + needed
            };
            pair_halves[n_pairs] = CardSet::halves(t);
            n_pairs += 1;
        }
        let green = CardSet::suit(Suit::Green);
        let (mut lane_a, mut lane_b, mut lane_c) = ([0u64; 36], [0u64; 36], [0u64; 36]);
        for c in hand.iter() {
            let s = suit_of(c) as usize;
            let mut a = CARD_POINTS[c as usize] as u64 * lane(5);
            for (i, set) in [unknown_halves, CardSet::ACES, CardSet::HALVES, need_halves, green]
                .into_iter()
                .enumerate()
            {
                if set.contains(c) {
                    a += lane(i);
                }
            }
            let mut b = 0;
            if LUSCHE.contains(c) {
                b += lane(s);
            } else if !CardSet::ACES.contains(c) {
                b += lane(4 + s);
            }
            let mut cc = lane(2 + s);
            for (i, halves) in pair_halves[..n_pairs].iter().enumerate() {
                if halves.contains(c) {
                    if i < 2 {
                        a += lane(6 + i);
                    } else {
                        cc += lane(i - 2);
                    }
                }
            }
            lane_a[c as usize] = a;
            lane_b[c as usize] = b;
            lane_c[c as usize] = cc;
        }
        ForthModel {
            p,
            lane_a,
            lane_b,
            lane_c,
            suit_len: SUITS.map(|s| len_in(hand, s) as u64),
            info_ace: !ctx.partner_knows_ace,
            info_complete: ctx.partner_showed_halves,
            need_ace: !ctx.partner_showed_ace,
            boost: if awkward { p.w_awkward } else { 1.0 },
            awkward,
            pair_both,
            n_pairs,
        }
    }

    pub fn score(&self, x: CardSet) -> Score {
        self.score_into(x)
    }

    /// The total only (the readers' softmax).
    pub fn total(&self, x: CardSet) -> f32 {
        self.score_into::<Total>(x).0
    }

    fn score_into<S: Sink>(&self, x: CardSet) -> S {
        let p = self.p;
        let mut s = S::default();
        let [a, b, c] = lane_sums(x, [&self.lane_a, &self.lane_b, &self.lane_c]);
        let mut blank = 0;
        let mut lusche: i32 = 0;
        for t in 0..4 {
            let len = self.suit_len[t];
            if len > 0 && byte(c, 2 + t) as u64 == len {
                blank += 1;
                // HP-8: Lusche signal the void; a blank made of tens and halves does not (C4,
                // C5). A passed ace is HP-4's business, so it is neither.
                lusche += byte(b, t) as i32 - byte(b, 4 + t) as i32;
            }
        }
        // HP-1 (with the structural prior "MH blanks 2 suits", C5)
        s.add(
            0,
            p.w_blank * blank as f32 + if blank >= 2 { p.w_blank_two } else { 0.0 },
        );
        // HP-2: what the partner does not know yet
        let mut info = p.w_info_half * byte(a, 0) as f32;
        if self.info_ace {
            info += p.w_info_ace * byte(a, 1) as f32;
        }
        if self.info_complete {
            info += p.w_info_complete * byte(a, 2) as f32;
        }
        s.add(1, info);
        // HP-3 and HP-4, boosted by HP-7 on awkward hands
        let quality = p.w_quality * byte(a, 5) as f32;
        let mut need = p.w_need_half * byte(a, 3) as f32;
        if self.need_ace {
            need += p.w_need_ace * byte(a, 1) as f32;
        }
        s.add(2, quality);
        s.add(3, need);
        if self.awkward {
            s.add(6, (self.boost - 1.0) * (quality + need));
        }
        // HP-5: a very weak tie-breaker
        s.add(4, -p.w_green * byte(a, 4) as f32);
        // HP-6: pairs
        for (i, &both) in self.pair_both[..self.n_pairs].iter().enumerate() {
            let given = if i < 2 { byte(a, 6 + i) } else { byte(c, i - 2) };
            match given {
                1 => s.add(5, -p.w_split),
                2 => s.add(5, both),
                _ => {}
            }
        }
        // HP-8: blank a suit with its Lusche
        s.add(7, p.w_small_blank * lusche as f32);
        s
    }
}

/// VH's plan from its 13 cards (§5.2): its best pair, else a half MH likely completes.
pub fn plan(hand: CardSet, ctx: &BackCtx, p: &Params) -> Plan {
    if let Some(t) = hand.pair_suits().max_by_key(|&t| pair_value(t)) {
        return Plan::Trump(t);
    }
    SUITS
        .into_iter()
        .filter(|&t| (hand & CardSet::halves(t)).len() == 1 && ctx.partner_half[t as usize] >= p.partner_half_conf)
        .max_by_key(|&t| pair_value(t))
        .map_or(Plan::NoTrump, Plan::Trump)
}

/// The HK score for one VH hand of 13 cards under `plan` (§5.2). As in `ForthModel`, the
/// integer counts per option come from per-card byte lanes; the float arithmetic per term is
/// as written below.
pub struct BackModel<'a> {
    p: &'a Params,
    trump: Option<usize>,
    /// Per card, byte lanes: 0 trump, 1 a half to keep (my pairs, my half of the planned split
    /// pair), 2 in a suit without A or 10, 3 ace, 4 ten backed by its ace.
    lane_a: [u64; 36],
    /// Per card: lane s = cards of suit s.
    lane_b: [u64; 36],
    suit_len: [u64; 4],
    /// Side suits (not trump) with at least 2 cards: HK-2 candidates.
    side: [bool; 4],
    aces: u64,
    /// HK-7 (P9): my aces, if few enough to give all back for schnippeln.
    schnippeln_aces: CardSet,
}

impl<'a> BackModel<'a> {
    pub fn new(hand: CardSet, plan: Plan, p: &'a Params) -> BackModel<'a> {
        let own_pair_halves = hand.pair_suits().fold(CardSet::EMPTY, |a, t| a | CardSet::halves(t));
        let trump = match plan {
            Plan::Trump(t) => Some(t),
            Plan::NoTrump => None,
        };
        let plan_halves = own_pair_halves | trump.map_or(CardSet::EMPTY, |t| hand & CardSet::halves(t));
        let topless = SUITS
            .iter()
            .filter(|&&u| (hand & CardSet::suit(u) & (CardSet::ACES | CardSet::TENS)).is_empty())
            .fold(CardSet::EMPTY, |a, &u| a | CardSet::suit(u));
        let backed_tens = SUITS
            .iter()
            .filter(|&&u| !(hand & CardSet::ACES & CardSet::suit(u)).is_empty())
            .fold(CardSet::EMPTY, |a, &u| a | (CardSet::TENS & CardSet::suit(u)));
        let trump_suit = trump.map_or(CardSet::EMPTY, CardSet::suit);
        let (mut lane_a, mut lane_b) = ([0u64; 36], [0u64; 36]);
        for c in hand.iter() {
            let mut a = 0;
            for (i, set) in [trump_suit, plan_halves, topless, CardSet::ACES, backed_tens]
                .into_iter()
                .enumerate()
            {
                if set.contains(c) {
                    a += lane(i);
                }
            }
            lane_a[c as usize] = a;
            lane_b[c as usize] = lane(suit_of(c) as usize);
        }
        BackModel {
            p,
            trump: trump.map(|t| t as usize),
            lane_a,
            lane_b,
            suit_len: SUITS.map(|s| len_in(hand, s) as u64),
            side: SUITS.map(|u| Some(u) != trump && len_in(hand, u) >= 2),
            aces: (hand & CardSet::ACES).len() as u64,
            schnippeln_aces: {
                let aces = hand & CardSet::ACES;
                // Not with a pair of my own: I need my sure leads to announce it (P8).
                let own_pair = !own_pair_halves.is_empty();
                if p.w_schnippeln > 0.0 && aces.len() as u64 <= p.schnippeln_max_aces as u64 && !own_pair {
                    aces
                } else {
                    CardSet::EMPTY
                }
            },
        }
    }

    pub fn score(&self, y: CardSet) -> Score {
        self.score_into(y)
    }

    /// The total only (the readers' softmax).
    pub fn total(&self, y: CardSet) -> f32 {
        self.score_into::<Total>(y).0
    }

    fn score_into<S: Sink>(&self, y: CardSet) -> S {
        let p = self.p;
        let mut s = S::default();
        let [a, b] = lane_sums(y, [&self.lane_a, &self.lane_b]);
        // All of suit u given back.
        let emptied = |u: usize| self.suit_len[u] > 0 && byte(b, u) as u64 == self.suit_len[u];
        // HK-1
        s.add(
            0,
            -p.w_trump_keep * byte(a, 0) as f32 - p.w_half_keep * byte(a, 1) as f32,
        );
        if let Some(t) = self.trump {
            // HK-2 (with the structural prior "VH passes one whole suit back", C5)
            if (0..4).any(|u| self.side[u] && emptied(u)) {
                s.add(1, p.w_side_suit);
            }
            // HK-4
            if byte(b, t) as u64 == self.suit_len[t] {
                s.add(3, -p.w_last_trick);
            }
        } else {
            if self.aces > 0 && byte(a, 3) as u64 == self.aces {
                s.add(3, -p.w_last_trick);
            }
            // HK-6
            s.add(5, p.w_short * byte(a, 2) as f32);
        }
        // HK-3
        s.add(
            2,
            -p.w_keep_ace * byte(a, 3) as f32 - p.w_keep_standing * byte(a, 4) as f32,
        );
        // HK-5
        let voids = (0..4).filter(|&u| Some(u) != self.trump && emptied(u)).count();
        s.add(4, p.w_void * voids as f32);
        // HK-7 (P9): give all my (few) aces back, with the ace of a suit I keep a low card in:
        // in trick 1 I then lead it low (HL-1), the next opponent must overtake, and MH must
        // play the ace. Trick 1 forces green while I hold green, so that suit must be green
        // unless I give back all green.
        if !self.schnippeln_aces.is_empty() && byte(a, 3) as u64 == self.aces {
            let green = Suit::Green as usize;
            let no_green = byte(b, green) as u64 == self.suit_len[green];
            let ok = self.schnippeln_aces.iter().any(|ace| {
                let u = suit_of(ace) as usize;
                (byte(b, u) as u64) < self.suit_len[u] && (no_green || u == green)
            });
            if ok {
                s.add(6, p.w_schnippeln);
            }
        }
        s
    }
}

// The argmax over `scored`; ties go to fewer points given away.
pub fn best(scored: &[(CardSet, Score)]) -> usize {
    (0..scored.len())
        .max_by(|&a, &b| {
            let (xa, sa) = &scored[a];
            let (xb, sb) = &scored[b];
            sa.total
                .total_cmp(&sb.total)
                .then(points_set(*xb).cmp(&points_set(*xa)))
                .then(xb.0.cmp(&xa.0))
        })
        .unwrap_or(0)
}

/// `P(chosen | hand)` under the softmax of `scores` at temperature `tau`.
pub fn softmax_prob(scores: &[f32], chosen: usize, tau: f32) -> f32 {
    let max = scores.iter().cloned().fold(f32::MIN, f32::max);
    let sum: f32 = scores.iter().map(|&s| ((s - max) / tau).exp()).sum();
    ((scores[chosen] - max) / tau).exp() / sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_counts() {
        assert_eq!(options(CardSet((1 << 9) - 1)).len(), 126);
        assert_eq!(options(CardSet((1 << 13) - 1)).len(), 715);
    }
}

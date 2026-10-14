//! The exact feature DP (F5, F7): the joint distribution of every hidden seat's aces and halves
//! (`DealSpace::with_features`), weighted by each seat's bid likelihood of its **dealt** class.
//!
//! - A seat outside a hidden exchange: its dealt hand is K5's dealt-certain cards plus the
//!   unknown cards it holds now, exactly, per leaf.
//! - The two opponents after their hidden exchange: per leaf the team's cards after the
//!   exchange are known, but not who was dealt which. The exchange kernel (`Kernel`) gives, per
//!   feature card, P(it was dealt to the seat that holds it after the exchange); the leaf's
//!   likelihood sums the dealt splits under it (memoised per split signature).
//!
//! Outputs per seat: P(feature) of the hand now and of the dealt hand, with the facts (a
//! feature true in every or in no consistent leaf) kept apart, and the posterior marginals of
//! the feature cards (F6).

use engine::{CardSet, CARDS, SUITS};
use knowledge::{Knowledge, Passed};
use observation::Seat;

use crate::conventions::{HandClass, N_CLASSES};
use crate::Table;

/// Features per seat, in this order: has an ace; ≥ 1, ≥ 2, ≥ 3 halves; a half of each suit;
/// the pair of each suit.
pub const N_FEATURES: usize = 12;
const HALVES_AT_LEAST: usize = 1;
const HALF: usize = 4;
const PAIR: usize = 8;

/// The exchange kernel (F7): P(a feature card was dealt to the seat that holds it after the
/// exchange), `[ace, half][first passer (MH), second passer (VH)]`. C32 (owner: no learning in
/// v1): the uniform-pass value 9/13 (`Kernel::UNIFORM`); the fitted kernels are in git history.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kernel {
    pub stay: [[f32; 2]; 2],
}

pub const UNIFORM_STAY: f32 = 9.0 / 13.0;

impl Kernel {
    /// Uniform passing: no convention about which cards go.
    pub const UNIFORM: Kernel = Kernel {
        stay: [[UNIFORM_STAY; 2]; 2],
    };
}

/// Feature bits and the feature vector of a hand with `ace` and `halves[s]` halves per suit.
fn bits(ace: bool, halves: [u8; 4]) -> u16 {
    let total: u8 = halves.iter().sum();
    let mut b = ace as u16;
    for i in 0..3 {
        b |= ((total as usize > i) as u16) << (HALVES_AT_LEAST + i);
    }
    for (s, &h) in halves.iter().enumerate() {
        b |= ((h >= 1) as u16) << (HALF + s);
        b |= ((h >= 2) as u16) << (PAIR + s);
    }
    b
}

fn class_of(ace: bool, halves: [u8; 4]) -> usize {
    HandClass {
        ace,
        halves: halves.map(|h| h.min(2)),
    }
    .index()
}

/// Per seat: Σ weight · feature, Σ weight, and which features are true in some / every leaf.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Moments {
    pub sum: [f64; N_FEATURES],
    pub any: u16,
    pub all: u16,
}

impl Default for Moments {
    fn default() -> Self {
        Moments {
            sum: [0.0; N_FEATURES],
            any: 0,
            all: u16::MAX,
        }
    }
}

impl Moments {
    fn add(&mut self, w: f64, b: u16) {
        for (i, s) in self.sum.iter_mut().enumerate() {
            if b >> i & 1 == 1 {
                *s += w;
            }
        }
        self.any |= b;
        self.all &= b;
    }

    fn add_weighted(&mut self, w: f64, sum: &[f64; N_FEATURES], any: u16, all: u16) {
        for (s, x) in self.sum.iter_mut().zip(sum) {
            *s += w * x;
        }
        self.any |= any;
        self.all &= all;
    }

    /// The value of feature `i` where it is a fact (true in every consistent leaf).
    pub fn fact(&self, i: usize) -> bool {
        self.all >> i & 1 == 1
    }

    /// Feature `i` is true in every or in no consistent leaf.
    pub fn is_fact(&self, i: usize) -> bool {
        self.all >> i & 1 == 1 || self.any >> i & 1 == 0
    }
}

/// The DP's output.
#[derive(Clone, Debug)]
pub(crate) struct Posterior {
    /// Σ of the leaf weights (deals × likelihood).
    pub z: f64,
    pub now: [Moments; 4],
    /// What the dealt features need (computed on their first read: `dealt`).
    pub dealt: DealtParts,
    /// Posterior P(card in seat) of the feature cards the DP split (`split`); other cells 0.
    pub cards: Table,
    pub split: CardSet,
}

/// The parts of the posterior the dealt features are made of.
#[derive(Clone, Debug)]
pub(crate) struct DealtParts {
    me: Seat,
    me_dealt: u16,
    hidden: [Seat; 3],
    in_exchange: [bool; 3],
    passers: Option<(usize, usize)>,
    /// Per hidden seat outside a hidden exchange: its dealt-class mass.
    mass: Box<[[f64; N_CLASSES]; 3]>,
    /// Per exchange term: Σ of the leaf weights (deals × the other seats' likelihood).
    term_weight: Vec<f64>,
}

/// The dealt-hand moments of every seat (the passers of a hidden exchange through its terms).
pub(crate) fn dealt(parts: &DealtParts, inputs: &ExchangeInputs, terms: &mut ExchangeTerms) -> [Moments; 4] {
    let mut out = [Moments::default(); 4];
    out[parts.me as usize].add(1.0, parts.me_dealt);
    for j in 0..3 {
        if !parts.in_exchange[j] {
            out[parts.hidden[j] as usize] = moments(&parts.mass[j]);
        }
    }
    if let Some((m, v)) = parts.passers {
        let (lm, lv) = (inputs.lm.as_deref(), inputs.lv.as_deref());
        for (id, &tw) in parts.term_weight.iter().enumerate().filter(|(_, &tw)| tw > 0.0) {
            let (tm, tv) = terms.moments(id, lm, lv, inputs.kernel);
            out[parts.hidden[m] as usize].add_weighted(tw, &tm.sum, tm.any, tm.all);
            out[parts.hidden[v] as usize].add_weighted(tw, &tv.sum, tv.any, tv.all);
        }
    }
    out
}

/// Likelihood of a seat's dealt class from its signals, normalised to max 1 (`None`: no signal).
pub(crate) type Likelihood = Option<Box<[f32; N_CLASSES]>>;

#[derive(Clone, Copy)]
enum Role {
    Ace,
    Half(usize),
    Other,
}

fn role(group: CardSet) -> Role {
    let c = group.iter().next().expect("groups are not empty");
    if CardSet::ACES.contains(c) {
        Role::Ace
    } else if CardSet::HALVES.contains(c) {
        Role::Half(engine::suit_of(c) as usize)
    } else {
        Role::Other
    }
}

/// Aces and halves per suit of a card set.
fn counts(set: CardSet) -> (u8, [u8; 4]) {
    (
        (set & CardSet::ACES).len() as u8,
        SUITS.map(|s| (set & CardSet::halves(s)).len() as u8),
    )
}

/// A count distribution over 0..=8 (at most 8 feature cards of one role in a team), with its
/// length.
#[derive(Clone, Copy)]
struct Dist {
    p: [f64; 9],
    len: usize,
}

/// Binomial(n, p) over 0..=n.
fn binomial(n: u8, p: f64) -> Dist {
    let mut d = Dist { p: [0.0; 9], len: 1 };
    d.p[0] = 1.0;
    for _ in 0..n {
        let mut next = [0.0; 9];
        for k in 0..d.len {
            next[k] += d.p[k] * (1.0 - p);
            next[k + 1] += d.p[k] * p;
        }
        d = Dist {
            p: next,
            len: d.len + 1,
        };
    }
    d
}

fn convolve(a: Dist, b: Dist) -> Dist {
    let mut out = Dist {
        p: [0.0; 9],
        len: a.len + b.len - 1,
    };
    for i in 0..a.len {
        for j in 0..b.len {
            out.p[i + j] += a.p[i] * b.p[j];
        }
    }
    out
}

/// The feature bits of every class (the class decides every feature).
fn class_bits() -> &'static [u16; N_CLASSES] {
    static BITS: std::sync::OnceLock<[u16; N_CLASSES]> = std::sync::OnceLock::new();
    BITS.get_or_init(|| {
        std::array::from_fn(|c| {
            let h = HandClass::from_index(c);
            bits(h.ace, h.halves)
        })
    })
}

/// Feature moments of a class distribution.
fn moments(mass: &[f64; N_CLASSES]) -> Moments {
    let table = class_bits();
    let mut m = Moments::default();
    for (c, &w) in mass.iter().enumerate() {
        if w > 0.0 {
            m.add(w, table[c]);
        }
    }
    m
}

/// One hidden exchange's dealt-split sum for a signature of held counts: the likelihood, and
/// (computed on the first dealt read) per seat the likelihood-weighted dealt features.
struct ExchangeTerm {
    key: u32,
    lik: f64,
    moments: Option<(Moments, Moments)>,
}

/// Held (after-exchange) aces 0..=4 of each passer, and per suit the halves 0..=2 of each.
const EXCHANGE_KEYS: usize = 25 * 9 * 9 * 9 * 9;

type Held = (u8, [u8; 4]);

fn exchange_key(held_m: Held, held_v: Held) -> u32 {
    let mut i = held_m.0 as u32 * 5 + held_v.0 as u32;
    for s in 0..4 {
        i = i * 9 + held_m.1[s] as u32 * 3 + held_v.1[s] as u32;
    }
    i
}

fn decode_key(mut key: u32) -> (Held, Held) {
    let (mut hm, mut hv) = ([0u8; 4], [0u8; 4]);
    for s in (0..4).rev() {
        let x = key % 9;
        hm[s] = (x / 3) as u8;
        hv[s] = (x % 3) as u8;
        key /= 9;
    }
    (((key / 5) as u8, hm), ((key % 5) as u8, hv))
}

/// What the exchange terms depend on: the two passers, their dealt-class likelihoods and the
/// kernel. Not the facts, and not the other signals: the terms of equal inputs are equal.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ExchangeInputs {
    passers: Option<(Seat, Seat)>,
    lm: Likelihood,
    lv: Likelihood,
    kernel: Kernel,
}

impl ExchangeInputs {
    pub(crate) fn of(k: &Knowledge, lik: &[Likelihood; 4], kernel: Kernel) -> ExchangeInputs {
        let passers = k.first_passer().map(|m| (m, engine::partner(m)));
        let of = |seat: Option<Seat>| seat.and_then(|y| lik[y as usize].clone());
        ExchangeInputs {
            passers,
            lm: of(passers.map(|p| p.0)),
            lv: of(passers.map(|p| p.1)),
            kernel,
        }
    }
}

/// The exchange terms of one set of `ExchangeInputs`, so card plays, questions and answers
/// reuse them.
#[derive(Default)]
pub(crate) struct ExchangeTerms {
    index: Vec<u32>,
    terms: Vec<ExchangeTerm>,
}

impl ExchangeTerms {
    #[cfg(test)]
    pub(crate) fn cached_terms(&self) -> usize {
        self.terms.len()
    }

    /// Use the feature-DP likelihood for a concrete current deal. Played cards
    /// still belong to the holder's post-exchange hand for this calculation.
    pub(crate) fn likelihood(
        &mut self,
        held: [CardSet; 2],
        lik: [Option<&[f32; N_CLASSES]>; 2],
        kernel: Kernel,
    ) -> f64 {
        let key = exchange_key(counts(held[0]), counts(held[1]));
        let id = self.term(key, lik[0], lik[1], kernel);
        self.terms[id as usize].lik
    }

    fn term(&mut self, key: u32, lm: Option<&[f32; N_CLASSES]>, lv: Option<&[f32; N_CLASSES]>, k: Kernel) -> u32 {
        if self.index.is_empty() {
            self.index = vec![u32::MAX; EXCHANGE_KEYS];
        }
        if self.index[key as usize] != u32::MAX {
            return self.index[key as usize];
        }
        let lik = Self::sum(key, lm, lv, k, None);
        self.terms.push(ExchangeTerm {
            key,
            lik,
            moments: None,
        });
        self.index[key as usize] = (self.terms.len() - 1) as u32;
        self.index[key as usize]
    }

    /// The moments of term `id` (once per term).
    fn moments(
        &mut self,
        id: usize,
        lm: Option<&[f32; N_CLASSES]>,
        lv: Option<&[f32; N_CLASSES]>,
        k: Kernel,
    ) -> (Moments, Moments) {
        if let Some(m) = self.terms[id].moments {
            return m;
        }
        let mut mass = (Box::new([0.0f64; N_CLASSES]), Box::new([0.0f64; N_CLASSES]));
        Self::sum(self.terms[id].key, lm, lv, k, Some((&mut mass.0, &mut mass.1)));
        let m = (moments(&mass.0), moments(&mass.1));
        self.terms[id].moments = Some(m);
        m
    }

    /// The likelihood of a held-count signature summed over the dealt splits, and (if asked)
    /// the class masses of both passers. One loop for both, so they always agree.
    fn sum(
        key: u32,
        lm: Option<&[f32; N_CLASSES]>,
        lv: Option<&[f32; N_CLASSES]>,
        k: Kernel,
        mut mass: Option<(&mut [f64; N_CLASSES], &mut [f64; N_CLASSES])>,
    ) -> f64 {
        let _timing = crate::profile::scope("belief.exchange_sum");
        let (held_m, held_v) = decode_key(key);
        // P(dealt to the first passer) of each count, per role.
        let dealt_m = |role: usize, nm: u8, nv: u8| {
            convolve(
                binomial(nm, k.stay[role][0] as f64),
                binomial(nv, 1.0 - k.stay[role][1] as f64),
            )
        };
        let aces = dealt_m(0, held_m.0, held_v.0);
        let team_aces = held_m.0 + held_v.0;
        let h: [Dist; 4] = std::array::from_fn(|s| dealt_m(1, held_m.1[s], held_v.1[s]));
        let team_halves: [usize; 4] = std::array::from_fn(|s| (held_m.1[s] + held_v.1[s]) as usize);
        // Class index = ace · 81 + halves of suit 0 · 27 + … (`HandClass::index`; halves are at
        // most 2 per suit), built up loop by loop; the product is formed left to right in the
        // same order as one expression `pa · p0 · p1 · p2 · p3 · Lm · Lv`.
        const W: [usize; 4] = [27, 9, 3, 1];
        let lm_of = |c: usize| lm.map_or(1.0, |l| l[c] as f64);
        let lv_of = |c: usize| lv.map_or(1.0, |l| l[c] as f64);
        let mut lik = 0.0;
        for da in 0..aces.len {
            let pa = aces.p[da];
            let (cm_a, cv_a) = (81 * (da > 0) as usize, 81 * ((team_aces as usize) > da) as usize);
            for d0 in 0..h[0].len {
                let x0 = pa * h[0].p[d0];
                let (cm0, cv0) = (cm_a + d0 * W[0], cv_a + (team_halves[0] - d0) * W[0]);
                for d1 in 0..h[1].len {
                    let x1 = x0 * h[1].p[d1];
                    let (cm1, cv1) = (cm0 + d1 * W[1], cv0 + (team_halves[1] - d1) * W[1]);
                    for d2 in 0..h[2].len {
                        let x2 = x1 * h[2].p[d2];
                        let (cm2, cv2) = (cm1 + d2 * W[2], cv1 + (team_halves[2] - d2) * W[2]);
                        for d3 in 0..h[3].len {
                            let (cm, cv) = (cm2 + d3, cv2 + team_halves[3] - d3);
                            let l = x2 * h[3].p[d3] * lm_of(cm) * lv_of(cv);
                            lik += l;
                            if let Some((mm, mv)) = mass.as_mut() {
                                mm[cm] += l;
                                mv[cv] += l;
                            }
                        }
                    }
                }
            }
        }
        lik
    }
}

/// One leaf of the feature DP, reduced to what the signals read: its deals, each hidden
/// seat's class now and as dealt, and the hidden exchange's key.
#[derive(Clone, Copy)]
struct Row {
    w: f64,
    now: [u8; 3],
    dealt: [u8; 3],
    key: u32,
}

/// The facts part of the feature DP, computed once per fact epoch (bids then only reweight it).
pub(crate) struct Leaves {
    me: Seat,
    me_now: u16,
    me_dealt: u16,
    hidden: [Seat; 3],
    groups: Vec<CardSet>,
    in_exchange: [bool; 3],
    /// The first and the second passer of a hidden exchange (hidden indices).
    passers: Option<(usize, usize)>,
    rows: Vec<Row>,
    /// `groups.len()` splits per row.
    splits: Vec<[u8; 3]>,
}

impl Leaves {
    pub(crate) fn new(k: &Knowledge) -> Leaves {
        let _timing = crate::profile::scope("belief.feature_leaves");
        let me = k.me();
        let space = knowledge::DealSpace::with_features(k);
        let (a, h) = counts(k.certain(me));
        let (da, dh) = counts(k.dealt_certain(me));
        let hidden = space.hidden();
        let groups = if space.count() == 0 {
            vec![]
        } else {
            space.group_cards()
        };
        // From the pass forth on, both passers (the giver too) are read through the exchange.
        let in_exchange = hidden.map(|y| k.received(y) == Passed::Hidden || k.given(y) == Passed::Hidden);
        let passers = in_exchange.contains(&true).then(|| {
            let first = k.first_passer().expect("a hidden pass has a first passer");
            let at = |y: Seat| hidden.iter().position(|&h| h == y).expect("both passers are hidden");
            (at(first), at(engine::partner(first)))
        });
        let mut leaves = Leaves {
            me,
            me_now: bits(a > 0, h),
            me_dealt: bits(da > 0, dh),
            hidden,
            groups,
            in_exchange,
            passers,
            rows: vec![],
            splits: vec![],
        };
        if space.count() == 0 {
            return leaves;
        }
        let roles: Vec<Role> = leaves.groups.iter().map(|&g| role(g)).collect();
        let now0 = hidden.map(|y| counts(k.certain(y)));
        let held0 = hidden.map(|y| counts(k.certain(y) | k.played_by(y)));
        let dealt0 = hidden.map(|y| counts(k.dealt_certain(y)));
        for (w, splits) in space.leaves() {
            if w == 0 {
                continue;
            }
            // The unknown feature cards each hidden seat holds now in this leaf.
            let mut add = [(0u8, [0u8; 4]); 3];
            for (g, sp) in splits.iter().enumerate() {
                for j in 0..3 {
                    match roles[g] {
                        Role::Ace => add[j].0 += sp[j],
                        Role::Half(s) => add[j].1[s] += sp[j],
                        Role::Other => {}
                    }
                }
            }
            let plus = |(a, h): Held, j: usize| (a + add[j].0, std::array::from_fn(|s| h[s] + add[j].1[s]));
            let class = |x: Held| class_of(x.0 > 0, x.1) as u8;
            leaves.rows.push(Row {
                w: w as f64,
                now: std::array::from_fn(|j| class(plus(now0[j], j))),
                dealt: std::array::from_fn(|j| class(plus(dealt0[j], j))),
                key: passers.map_or(0, |(m, v)| exchange_key(plus(held0[m], m), plus(held0[v], v))),
            });
            leaves.splits.extend_from_slice(splits);
        }
        leaves
    }
}

/// The feature DP: the leaves weighted by each seat's dealt-class likelihood. `terms` must
/// belong to the same signal epoch (likelihoods and kernel).
pub(crate) fn posterior(
    leaves: &Leaves,
    lik: &[Likelihood; 4],
    kernel: Kernel,
    terms: &mut ExchangeTerms,
) -> Posterior {
    let mut out = Posterior {
        z: 0.0,
        now: [Moments::default(); 4],
        dealt: DealtParts {
            me: leaves.me,
            me_dealt: leaves.me_dealt,
            hidden: leaves.hidden,
            in_exchange: leaves.in_exchange,
            passers: leaves.passers,
            mass: Box::new([[0.0; N_CLASSES]; 3]),
            term_weight: vec![],
        },
        cards: [[0.0; CARDS]; 4],
        split: leaves.groups.iter().fold(CardSet::EMPTY, |a, &g| a | g),
    };
    // My own hand is a fact.
    out.now[leaves.me as usize].add(1.0, leaves.me_now);
    if leaves.rows.is_empty() {
        return out;
    }
    let hidden = leaves.hidden;
    let l: [Option<&[f32; N_CLASSES]>; 3] = hidden.map(|y| lik[y as usize].as_deref());
    // Seats read through their dealt class here (the passers of a hidden exchange through it).
    let direct: [Option<&[f32; N_CLASSES]>; 3] = std::array::from_fn(|j| l[j].filter(|_| !leaves.in_exchange[j]));
    let mut now_mass = [[0.0f64; N_CLASSES]; 3];
    let DealtParts {
        mass: dealt_mass,
        term_weight,
        ..
    } = &mut out.dealt;
    let ng = leaves.groups.len();
    let mut group_post = vec![[0.0f64; 3]; ng];
    for (r, row) in leaves.rows.iter().enumerate() {
        let mut lw = row.w;
        for j in 0..3 {
            if let Some(t) = direct[j] {
                lw *= t[row.dealt[j] as usize] as f64;
            }
        }
        if let Some((m, v)) = leaves.passers {
            let id = terms.term(row.key, l[m], l[v], kernel);
            if term_weight.len() <= id as usize {
                term_weight.resize(id as usize + 1, 0.0);
            }
            term_weight[id as usize] += lw;
            lw *= terms.terms[id as usize].lik;
        }
        out.z += lw;
        for j in 0..3 {
            now_mass[j][row.now[j] as usize] += lw;
            dealt_mass[j][row.dealt[j] as usize] += lw;
        }
        for (gp, sp) in group_post.iter_mut().zip(&leaves.splits[r * ng..(r + 1) * ng]) {
            for j in 0..3 {
                gp[j] += lw * sp[j] as f64;
            }
        }
    }
    for j in 0..3 {
        out.now[hidden[j] as usize] = moments(&now_mass[j]);
    }
    for (g, &cards) in leaves.groups.iter().enumerate() {
        for j in 0..3 {
            let p = group_post[g][j] / (out.z * cards.len() as f64);
            for c in cards.iter() {
                out.cards[hidden[j] as usize][c as usize] = p as f32;
            }
        }
    }
    out
}

/// The feature vector `i` read as a `SeatFeatures` field layout.
pub(crate) fn layout<T: Copy>(f: [T; N_FEATURES]) -> (T, [T; 3], [T; 4], [T; 4]) {
    (
        f[0],
        [f[1], f[2], f[3]],
        [f[HALF], f[HALF + 1], f[HALF + 2], f[HALF + 3]],
        [f[PAIR], f[PAIR + 1], f[PAIR + 2], f[PAIR + 3]],
    )
}

/// Which feature family feature `i` belongs to (F8 calibrates per family).
pub fn family(i: usize) -> crate::Family {
    use crate::Family;
    match i {
        0 => Family::Ace,
        1 => Family::HalvesAtLeast1,
        2 => Family::HalvesAtLeast2,
        3 => Family::HalvesAtLeast3,
        HALF..PAIR => Family::Half,
        _ => Family::Pair,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_follow_the_layout() {
        let b = bits(true, [0, 1, 2, 0]);
        assert_eq!(b & 1, 1);
        assert_eq!(b >> HALVES_AT_LEAST & 0b111, 0b111);
        assert_eq!(b >> HALF & 0xF, 0b0110);
        assert_eq!(b >> PAIR & 0xF, 0b0100);
    }

    #[test]
    fn exchange_counts_sum_to_one_without_signals() {
        let mut x = ExchangeTerms::default();
        let key = exchange_key((2, [1, 0, 2, 0]), (1, [1, 1, 0, 0]));
        assert_eq!(decode_key(key), ((2, [1, 0, 2, 0]), (1, [1, 1, 0, 0])));
        let id = x.term(key, None, None, Kernel::UNIFORM);
        assert!((x.terms[id as usize].lik - 1.0).abs() < 1e-12);
        assert_eq!(x.term(key, None, None, Kernel::UNIFORM), id);
    }
}

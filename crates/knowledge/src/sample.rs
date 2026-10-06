//! Deals consistent with what one seat knows: exact count, exact marginals, exactly uniform
//! samples (tier 1, lazy: built on demand from a `Knowledge`, never on the event path).
//!
//! A deal gives each hidden seat `hand_len - |certain|` of the unknown cards, from the cards it
//! may hold, so that every live constraint holds. Unknown cards are grouped by signature (the
//! hidden seats that may hold them). Inside a class the cards are exchangeable, so a DP over
//! the seats' remaining capacities counts the deals class by class. Cards named in a live
//! constraint break that symmetry. They form smaller groups (same signature, same
//! constraints, e.g. the king and ober of one suit); an outer enumeration splits each group
//! over the seats and checks the constraints, and the DP counts the rest of each split.
//!
//! Counts fit `u64`: three hidden seats and at most 36 cards give at most 3^36 < 2^58 deals.

use engine::bits::CardSet;
use observation::Seat;
use rand::RngCore;

use crate::{Knowledge, MAX_CONSTRAINTS};

/// The four hands of a deal, indexed by seat.
pub type Hands = [CardSet; 4];

/// Remaining capacity 0..=13 per hidden seat.
const DIM: usize = 14;
const TABLE: usize = DIM * DIM;

const BINOM: [[u64; 37]; 37] = {
    let mut t = [[0u64; 37]; 37];
    let mut n = 0;
    while n < 37 {
        t[n][0] = 1;
        let mut k = 1;
        while k <= n {
            t[n][k] = t[n - 1][k - 1] + t[n - 1][k];
            k += 1;
        }
        n += 1;
    }
    t
};

/// Exchangeable unknown cards: same signature (and, for a tied group, same constraints).
/// Its cards are `pool[start..start + n]`.
#[derive(Clone, Copy, Debug)]
struct Class {
    sig: u8,
    n: u8,
    start: u8,
}

/// A live constraint on hidden seat `j`, as bounds on the cards of `mask` dealt to it.
#[derive(Clone, Copy, Debug)]
struct Tie {
    j: usize,
    mask: CardSet,
    lo: u8,
    hi: u8,
}

/// One split of every tied group. It stands for `mult` placements of the tied cards and
/// leaves capacity `rest` per hidden seat.
#[derive(Clone, Copy, Debug)]
struct Leaf {
    rest: [u8; 3],
    mult: u64,
}

/// The deals consistent with a `Knowledge`. Built in about a microsecond; sampling and
/// marginals reuse its tables.
#[derive(Clone, Debug)]
pub struct DealSpace {
    hidden: [Seat; 3],
    known: Hands,
    /// Tied groups, split by the outer enumeration.
    groups: Vec<Class>,
    leaves: Vec<Leaf>,
    /// `groups.len()` splits per leaf.
    leaf_splits: Vec<[u8; 3]>,
    /// Running total of deals up to and including each leaf.
    cumulative: Vec<u64>,
    /// Untied classes, counted by the DP.
    classes: Vec<Class>,
    pool: [u8; 36],
    /// Cards in classes `i..`.
    suffix: Vec<u8>,
    /// `ways[i * TABLE + r0 * DIM + r1]`: deals of classes `i..` into remaining capacity
    /// `(r0, r1, suffix[i] - r0 - r1)`.
    ways: Vec<u64>,
    total: u64,
}

/// Exact tier-1 sets with the meaning of `Knowledge::certain` and `Knowledge::possible`: per
/// seat, the cards it holds in every consistent deal, and those it holds in some but not all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ExactSets {
    pub certain: [CardSet; 4],
    pub possible: [CardSet; 4],
}

impl ExactSets {
    /// Every card `seat` holds in some consistent deal.
    pub fn may_hold(&self, seat: Seat) -> CardSet {
        self.certain[seat as usize] | self.possible[seat as usize]
    }
}

/// A sampled deal with its importance weight (`Belief::weight`; normalise over the sample).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct World {
    pub hands: Hands,
    pub weight: f64,
}

/// A belief over deals as a likelihood up to a constant (the soft layer plugs in here). The
/// sampler draws uniformly from the consistent deals, so this likelihood is the importance
/// weight of each sample.
pub trait Belief {
    fn weight(&self, hands: &Hands) -> f64;
}

/// Every consistent deal equally likely.
#[derive(Clone, Copy, Debug, Default)]
pub struct Uniform;

impl Belief for Uniform {
    fn weight(&self, _: &Hands) -> f64 {
        1.0
    }
}

impl<F: Fn(&Hands) -> f64> Belief for F {
    fn weight(&self, hands: &Hands) -> f64 {
        self(hands)
    }
}

/// Exactly uniform in `0..n` (Lemire's multiply-and-reject), `n > 0`.
fn below<R: RngCore + ?Sized>(rng: &mut R, n: u64) -> u64 {
    let mut m = rng.next_u64() as u128 * n as u128;
    if (m as u64) < n {
        let threshold = n.wrapping_neg() % n;
        while (m as u64) < threshold {
            m = rng.next_u64() as u128 * n as u128;
        }
    }
    (m >> 64) as u64
}

fn minus(r: [u8; 3], split: [u8; 3]) -> [u8; 3] {
    [r[0] - split[0], r[1] - split[1], r[2] - split[2]]
}

fn cell(r: [u8; 3]) -> usize {
    r[0] as usize * DIM + r[1] as usize
}

/// Calls `f(split, ways to pick it)` for every split of `n` cards over the seats in `sig`
/// with `split <= r`, until `f` returns true.
#[inline]
fn for_each_split(sig: u8, n: u8, r: [u8; 3], mut f: impl FnMut([u8; 3], u64) -> bool) {
    let binom = |n: u8, k: u8| BINOM[n as usize][k as usize];
    match sig {
        1 | 2 | 4 => {
            let j = sig.trailing_zeros() as usize;
            if n <= r[j] {
                let mut split = [0; 3];
                split[j] = n;
                f(split, 1);
            }
        }
        3 | 5 | 6 => {
            let (j, k) = (sig.trailing_zeros() as usize, 7 - sig.leading_zeros() as usize);
            for a in n.saturating_sub(r[k])..=n.min(r[j]) {
                let mut split = [0; 3];
                split[j] = a;
                split[k] = n - a;
                if f(split, binom(n, a)) {
                    return;
                }
            }
        }
        7 => {
            for a in 0..=n.min(r[0]) {
                let rest = n - a;
                for b in rest.saturating_sub(r[2])..=rest.min(r[1]) {
                    if f([a, b, rest - b], binom(n, a) * binom(rest, b)) {
                        return;
                    }
                }
            }
        }
        _ => unreachable!("empty signature"),
    }
}

impl DealSpace {
    pub fn new(k: &Knowledge) -> DealSpace {
        let me = k.me;
        let mut space = DealSpace {
            hidden: [1, 2, 3].map(|d| (me + d) % 4),
            known: k.certain,
            groups: vec![],
            leaves: vec![],
            leaf_splits: vec![],
            cumulative: vec![],
            classes: vec![],
            pool: [0; 36],
            suffix: vec![0],
            ways: vec![],
            total: 0,
        };
        if let Some(fixed) = space.fixed_part(k) {
            space.build(k, fixed);
        }
        space
    }

    /// The unknown cards and the hidden seats' capacities, or `None` if the fixed part is
    /// already inconsistent.
    fn fixed_part(&self, k: &Knowledge) -> Option<(CardSet, [u8; 3])> {
        let mut all_known = CardSet::EMPTY;
        for x in 0..4 {
            if !(all_known & k.certain[x]).is_empty() || k.certain[x].len() > k.hand_len[x] as u32 {
                return None;
            }
            all_known |= k.certain[x];
        }
        if !(all_known & k.played).is_empty() {
            return None;
        }
        let unknown = !(all_known | k.played);
        let me = k.me as usize;
        if k.certain[me].len() != k.hand_len[me] as u32 || !(k.possible[me] & unknown).is_empty() {
            return None;
        }
        let cap = self
            .hidden
            .map(|x| k.hand_len[x as usize] - k.certain[x as usize].len() as u8);
        if cap.iter().map(|&c| c as u32).sum::<u32>() != unknown.len() || cap.iter().any(|&c| c as usize >= DIM) {
            return None;
        }
        let me_ok = k.constraints().iter().filter(|c| c.seat == k.me).all(|c| {
            let n = (k.certain[me] & c.mask).len() as u8;
            c.lo <= n && n <= c.hi
        });
        me_ok.then_some((unknown, cap))
    }

    fn build(&mut self, k: &Knowledge, (unknown, cap): (CardSet, [u8; 3])) {
        let may = self.hidden.map(|x| k.possible[x as usize] & unknown);
        if !(unknown - (may[0] | may[1] | may[2])).is_empty() {
            return;
        }
        let mut ties = vec![];
        let mut held = [0u8; MAX_CONSTRAINTS];
        for c in k.constraints().iter().filter(|c| c.seat != k.me) {
            let j = self.hidden.iter().position(|&h| h == c.seat).unwrap();
            held[ties.len()] = (self.known[c.seat as usize] & c.mask).len() as u8;
            if held[ties.len()] > c.hi {
                return;
            }
            ties.push(Tie {
                j,
                mask: c.mask & may[j],
                lo: c.lo,
                hi: c.hi,
            });
        }

        let tied = ties.iter().fold(CardSet::EMPTY, |a, t| a | t.mask);
        let mut groups: Vec<(u8, u16, CardSet)> = vec![];
        for c in tied.iter() {
            let sig = (0..3).filter(|&j| may[j].contains(c)).fold(0, |s, j| s | 1 << j);
            let member = (0..ties.len())
                .filter(|&t| ties[t].mask.contains(c))
                .fold(0, |m, t| m | 1 << t);
            match groups.iter_mut().find(|g| (g.0, g.1) == (sig, member)) {
                Some(g) => g.2 |= CardSet::single(c),
                None => groups.push((sig, member, CardSet::single(c))),
            }
        }
        let mut classes = vec![];
        for sig in 1..8u8 {
            let mut cards = unknown - tied;
            for (j, &m) in may.iter().enumerate() {
                cards = if sig >> j & 1 == 1 { cards & m } else { cards - m };
            }
            if !cards.is_empty() {
                classes.push((sig, cards));
            }
        }
        // The last class is dealt without a choice, so the one with most splits goes last.
        classes.sort_by_key(|&(sig, cards)| (sig.count_ones(), cards.len()));
        let mut start = 0u8;
        let mut lay_out = |sig: u8, cards: CardSet, pool: &mut [u8; 36]| {
            for c in cards.iter() {
                pool[start as usize] = c;
                start += 1;
            }
            let n = cards.len() as u8;
            Class {
                sig,
                n,
                start: start - n,
            }
        };
        self.groups = groups.iter().map(|g| lay_out(g.0, g.2, &mut self.pool)).collect();
        self.classes = classes.iter().map(|c| lay_out(c.0, c.1, &mut self.pool)).collect();
        let members: Vec<u16> = groups.iter().map(|g| g.1).collect();

        let m = self.classes.len();
        self.suffix = vec![0; m + 1];
        for i in (0..m).rev() {
            self.suffix[i] = self.suffix[i + 1] + self.classes[i].n;
        }
        self.ways = vec![0; (m + 1) * TABLE];
        self.ways[m * TABLE] = 1;
        for i in (0..m).rev() {
            let s = self.suffix[i];
            let class = self.classes[i];
            for r0 in 0..=cap[0].min(s) {
                for r1 in 0..=cap[1].min(s - r0) {
                    let r2 = s - r0 - r1;
                    if r2 > cap[2] {
                        continue;
                    }
                    let r = [r0, r1, r2];
                    let mut sum = 0;
                    for_each_split(class.sig, class.n, r, |split, mult| {
                        sum += mult * self.ways(i + 1, minus(r, split));
                        false
                    });
                    self.ways[i * TABLE + cell(r)] = sum;
                }
            }
        }

        self.split_groups(&ties, &members, held, cap, 1, &mut vec![]);
        let mut total = 0u64;
        self.cumulative = self
            .leaves
            .iter()
            .map(|l| {
                total += l.mult * self.ways(0, l.rest);
                total
            })
            .collect();
        self.total = total;
    }

    /// Outer enumeration: every split of the tied groups that satisfies all constraints and
    /// leaves a completable rest. `held[t]`: cards of tie `t` at its seat so far.
    fn split_groups(
        &mut self,
        ties: &[Tie],
        members: &[u16],
        held: [u8; MAX_CONSTRAINTS],
        rest: [u8; 3],
        mult: u64,
        splits: &mut Vec<[u8; 3]>,
    ) {
        let g = splits.len();
        let Some(&group) = self.groups.get(g) else {
            if ties.iter().zip(held).all(|(t, n)| n >= t.lo) && self.ways(0, rest) > 0 {
                self.leaves.push(Leaf { rest, mult });
                self.leaf_splits.extend_from_slice(splits);
            }
            return;
        };
        for_each_split(group.sig, group.n, rest, |split, ways| {
            let mut held = held;
            for (t, tie) in ties.iter().enumerate() {
                if members[g] >> t & 1 == 1 {
                    held[t] += split[tie.j];
                    if held[t] > tie.hi {
                        return false;
                    }
                }
            }
            splits.push(split);
            self.split_groups(ties, members, held, minus(rest, split), mult * ways, splits);
            splits.pop();
            false
        });
    }

    #[inline]
    fn ways(&self, i: usize, r: [u8; 3]) -> u64 {
        self.ways[i * TABLE + cell(r)]
    }

    /// The number of consistent deals (0 if the knowledge is inconsistent).
    pub fn count(&self) -> u64 {
        self.total
    }

    /// One deal, exactly uniform over the consistent deals; `None` if there is none.
    pub fn sample<R: RngCore + ?Sized>(&self, rng: &mut R) -> Option<Hands> {
        if self.total == 0 {
            return None;
        }
        let u = below(rng, self.total);
        let leaf = self.cumulative.partition_point(|&c| c <= u);
        let mut hands = self.known;
        let n_groups = self.groups.len();
        for (group, &split) in self.groups.iter().zip(&self.leaf_splits[leaf * n_groups..]) {
            self.deal(*group, split, &mut hands, rng);
        }
        let mut rest = self.leaves[leaf].rest;
        let last = self.classes.len().saturating_sub(1);
        for (i, class) in self.classes.iter().enumerate() {
            let split = if i == last {
                rest
            } else {
                let mut u = below(rng, self.ways(i, rest));
                let mut chosen = [0; 3];
                for_each_split(class.sig, class.n, rest, |split, mult| {
                    let w = mult * self.ways(i + 1, minus(rest, split));
                    if u < w {
                        chosen = split;
                        return true;
                    }
                    u -= w;
                    false
                });
                chosen
            };
            self.deal(*class, split, &mut hands, rng);
            rest = minus(rest, split);
        }
        Some(hands)
    }

    /// Deals the cards of `class` to the hidden seats by `split`, uniformly.
    fn deal<R: RngCore + ?Sized>(&self, class: Class, split: [u8; 3], hands: &mut Hands, rng: &mut R) {
        let (start, n) = (class.start as usize, class.n as usize);
        let mut cards = [0u8; 36];
        cards[..n].copy_from_slice(&self.pool[start..start + n]);
        let mut left = n;
        for (&seat, &k) in self.hidden.iter().zip(&split).take(2) {
            let seat = seat as usize;
            for _ in 0..k {
                let t = below(rng, left as u64) as usize;
                hands[seat] |= CardSet::single(cards[t]);
                left -= 1;
                cards[t] = cards[left];
            }
        }
        for &c in &cards[..left] {
            hands[self.hidden[2] as usize] |= CardSet::single(c);
        }
    }

    /// `n` uniform samples, each weighted by `belief` (importance weights, not normalised).
    pub fn sample_worlds<R: RngCore + ?Sized, B: Belief + ?Sized>(
        &self,
        rng: &mut R,
        n: usize,
        belief: &B,
    ) -> Vec<World> {
        (0..n)
            .map_while(|_| self.sample(rng))
            .map(|hands| World {
                weight: belief.weight(&hands),
                hands,
            })
            .collect()
    }

    /// `[seat][card]`: the number of consistent deals in which `seat` holds `card`.
    pub fn holder_counts(&self) -> [[u64; 36]; 4] {
        let mut out = [[0u64; 36]; 4];
        if self.total == 0 {
            return out;
        }
        for (out, known) in out.iter_mut().zip(self.known) {
            for c in known.iter() {
                out[c as usize] = self.total;
            }
        }
        let n_groups = self.groups.len();
        let mut held = vec![[0u64; 3]; n_groups];
        let mut prefix = vec![0u64; TABLE];
        for (l, leaf) in self.leaves.iter().enumerate() {
            let deals = leaf.mult * self.ways(0, leaf.rest);
            for (g, split) in self.leaf_splits[l * n_groups..(l + 1) * n_groups].iter().enumerate() {
                for j in 0..3 {
                    held[g][j] += deals * split[j] as u64;
                }
            }
            prefix[cell(leaf.rest)] += leaf.mult;
        }
        for (group, held) in self.groups.iter().zip(&held) {
            self.share(*group, held, &mut out);
        }
        // prefix[r]: placements of the tied cards and of classes before i that leave r.
        for (i, class) in self.classes.iter().enumerate() {
            let s = self.suffix[i];
            let mut next = vec![0u64; TABLE];
            let mut held = [0u64; 3];
            for r0 in 0..=s.min(DIM as u8 - 1) {
                for r1 in 0..=(s - r0).min(DIM as u8 - 1) {
                    let r = [r0, r1, s - r0 - r1];
                    let before = prefix[cell(r)];
                    if before == 0 {
                        continue;
                    }
                    for_each_split(class.sig, class.n, r, |split, mult| {
                        let to = minus(r, split);
                        let after = self.ways(i + 1, to);
                        if after > 0 {
                            let deals = before * mult * after;
                            for j in 0..3 {
                                held[j] += deals * split[j] as u64;
                            }
                            next[cell(to)] += before * mult;
                        }
                        false
                    });
                }
            }
            self.share(*class, &held, &mut out);
            prefix = next;
        }
        out
    }

    /// Cards of one class are exchangeable, so each is held in an equal share of the deals
    /// (`held[j]`: deals times cards of the class at hidden seat `j`).
    fn share(&self, class: Class, held: &[u64; 3], out: &mut [[u64; 36]; 4]) {
        let (start, n) = (class.start as usize, class.n as usize);
        for j in 0..3 {
            debug_assert_eq!(held[j] % n as u64, 0);
            for &c in &self.pool[start..start + n] {
                out[self.hidden[j] as usize][c as usize] = held[j] / n as u64;
            }
        }
    }

    /// `certain` and `possible` made exact. A card in every deal is certain, even if tier 0
    /// still lists it as possible. All empty if there is no consistent deal.
    pub fn exact_sets(&self) -> ExactSets {
        let mut sets = ExactSets::default();
        if self.total == 0 {
            return sets;
        }
        for (x, seat) in self.holder_counts().iter().enumerate() {
            for (c, &n) in seat.iter().enumerate() {
                if n == self.total {
                    sets.certain[x] |= CardSet::single(c as u8);
                } else if n > 0 {
                    sets.possible[x] |= CardSet::single(c as u8);
                }
            }
        }
        sets
    }

    /// `[seat][card]`: the exact probability that `seat` holds `card` under the uniform
    /// distribution over consistent deals (all zero if there is none).
    pub fn marginals(&self) -> [[f64; 36]; 4] {
        let counts = self.holder_counts();
        let total = self.total.max(1) as f64;
        counts.map(|seat| seat.map(|n| n as f64 / total))
    }
}

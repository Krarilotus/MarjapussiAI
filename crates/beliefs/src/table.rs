//! Card tables: the exact n-over-k base under the facts (F4's base), the renormalisation that
//! keeps card and hand-size sums after factors (IPF), and suit-count distributions.

use engine::{CardSet, CARDS};
use knowledge::Knowledge;
use observation::Seat;

use crate::Table;

/// The most cards of one suit a hand can hold.
pub const SUIT_CARDS: usize = 9;

/// The exact base: P(card in seat) over all deals consistent with the facts (the knowledge's
/// deal space; the own hand exact). Zero rows if the facts are inconsistent.
pub fn base(k: &Knowledge) -> Table {
    let space = k.deals();
    let mut t = [[0.0f32; CARDS]; 4];
    let total = space.count();
    if total == 0 {
        return t;
    }
    let counts = space.holder_counts();
    for (row, c) in t.iter_mut().zip(counts) {
        for (p, n) in row.iter_mut().zip(c) {
            // Exact 0 and 1 stay exact (n == 0, n == total); the rest is a belief.
            *p = match n {
                0 => 0.0,
                n if n == total => 1.0,
                n => belief((n as f64 / total as f64) as f32),
            };
        }
    }
    t
}

/// The cells strictly between 0 and 1 (beliefs), per seat.
pub fn open_cells(t: &Table) -> [CardSet; 4] {
    std::array::from_fn(|x| {
        (0..CARDS as u8)
            .filter(|&c| t[x][c as usize] > 0.0 && t[x][c as usize] < 1.0)
            .fold(CardSet::EMPTY, |a, c| a | CardSet::single(c))
    })
}

/// Iterative proportional fitting of the `movable` cells of `t` so that every card sums to 1
/// over the seats and every seat to its hand size; every other cell (facts, pinned beliefs)
/// stays and counts towards both sums. Movable cells end strictly inside (0, 1).
pub fn renormalise(t: &mut Table, sizes: [u8; 4], movable: [CardSet; 4]) {
    let cards = movable.iter().fold(CardSet::EMPTY, |a, &m| a | m);
    let fixed = |t: &Table, x: usize, c: u8| !movable[x].contains(c) as u8 as f32 * t[x][c as usize];
    let cap: [f32; 4] =
        std::array::from_fn(|x| sizes[x] as f32 - (0..CARDS as u8).map(|c| fixed(t, x, c)).sum::<f32>());
    let target: [f32; CARDS] = std::array::from_fn(|c| 1.0 - (0..4).map(|x| fixed(t, x, c as u8)).sum::<f32>());
    for _ in 0..IPF_ROUNDS {
        for c in cards.iter() {
            let s: f32 = (0..4)
                .filter(|&x| movable[x].contains(c))
                .map(|x| t[x][c as usize])
                .sum();
            if s > 0.0 {
                for x in (0..4).filter(|&x| movable[x].contains(c)) {
                    t[x][c as usize] *= target[c as usize].max(0.0) / s;
                }
            }
        }
        for x in 0..4 {
            let s: f32 = movable[x].iter().map(|c| t[x][c as usize]).sum();
            if s > 0.0 {
                for c in movable[x].iter() {
                    t[x][c as usize] *= cap[x].max(0.0) / s;
                }
            }
        }
    }
    for (row, m) in t.iter_mut().zip(movable) {
        for c in m.iter() {
            row[c as usize] = belief(row[c as usize]);
        }
    }
}

/// Rounds of the proportional fitting (S8c: converged well within this).
const IPF_ROUNDS: usize = 30;

/// Poisson-binomial distribution of how many of `cards` the seat holds, under the row's
/// probabilities, truncated to the sound bounds `[lo, hi]` and renormalised.
pub fn count_dist(row: &[f32; CARDS], cards: CardSet, (lo, hi): (u8, u8)) -> [f32; SUIT_CARDS + 1] {
    let mut d = [0.0f64; SUIT_CARDS + 1];
    d[0] = 1.0;
    let mut n = 0;
    for c in cards.iter() {
        let p = row[c as usize] as f64;
        n += 1;
        for k in (0..=n.min(SUIT_CARDS)).rev() {
            let stay = d[k] * (1.0 - p);
            let up = if k > 0 { d[k - 1] * p } else { 0.0 };
            d[k] = stay + up;
        }
    }
    let (lo, hi) = (lo as usize, (hi as usize).min(SUIT_CARDS));
    let inside: f64 = (lo..=hi).map(|k| d[k]).sum();
    let mut out = [0.0f32; SUIT_CARDS + 1];
    if inside > 0.0 {
        for k in lo..=hi {
            out[k] = (d[k] / inside) as f32;
        }
    } else {
        out[lo] = 1.0;
    }
    out
}

/// The smallest distance a belief keeps from 0 and 1 (f32 rounding must not make it a fact).
pub const MARGIN: f32 = 1e-6;

/// A probability that is not decided by the facts: strictly inside (0, 1).
#[inline]
pub fn belief(p: f32) -> f32 {
    p.clamp(MARGIN, 1.0 - MARGIN)
}

/// `known` if the facts decide the read, else `p` kept strictly inside (0, 1).
#[inline]
pub fn read(p: f32, known: Option<bool>) -> f32 {
    match known {
        Some(true) => 1.0,
        Some(false) => 0.0,
        None => belief(p),
    }
}

/// Hand sizes per seat.
pub fn sizes(k: &Knowledge) -> [u8; 4] {
    std::array::from_fn(|x| k.hand_len(x as Seat))
}

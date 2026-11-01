//! Transposition table for trick starts: bounds on a team's future points, keyed by the
//! position. Points and tricks so far are path data and stay out of the key.

use engine::{FastState, CARDS, CARD_POINTS, SUITS, VALUES};

use crate::search::{team_halves, team_pairs};

/// Cards per suit in the engine's index layout (`Card::index` = suit × 9 + rank).
const SUIT_CARDS: u8 = VALUES.len() as u8;
/// The lowest ranks of a suit (6, 7, 8, 9): no points and no pair role, so a position only
/// depends on their order and owners. Gaps among them are deleted; scoring ranks stay literal.
const ZERO_POINT_RANKS: u8 = 4;
/// Their bits within a suit's nine.
const LOW: u64 = (1 << ZERO_POINT_RANKS) - 1;
const LOW_SETS: usize = 1 << ZERO_POINT_RANKS;

// The packing is only exact while those ranks score nothing (pair halves are checked in tests).
const _: () = {
    let mut card = 0;
    while card < CARDS {
        assert!(card % SUIT_CARDS as usize >= ZERO_POINT_RANKS as usize || CARD_POINTS[card] == 0);
        card += 1;
    }
};

/// Per set of live low ranks, each hand's low ranks with the gaps among the live ones deleted.
const PACK_LOW: [[u8; LOW_SETS]; LOW_SETS] = {
    let mut table = [[0; LOW_SETS]; LOW_SETS];
    let mut live = 0;
    while live < LOW_SETS {
        let mut hand = 0;
        while hand < LOW_SETS {
            let mut rank = 0;
            let mut to = 0;
            while rank < ZERO_POINT_RANKS {
                if live & (1 << rank) != 0 {
                    if hand & (1 << rank) != 0 {
                        table[live][hand] |= 1 << to;
                    }
                    to += 1;
                }
                rank += 1;
            }
            hand += 1;
        }
        live += 1;
    }
    table
};

/// A trick-start position, the team the values are for, and the measure (points or tricks).
/// Hands, leader, trump, called suits, trump rights and the phase decide the rest of the play.
/// The last three matter only through a pair that can still score: hands only shrink, and
/// without an uncalled pair a team can neither announce nor ask a question that makes trump
/// (`search::team_pairs`). So the key keeps the called bits of suits whose halves
/// one team holds, all three rights levels of a team's seats while it holds an uncalled pair,
/// and the phase or pending question while the leader's team does. Every field is guarded by
/// `tests/exact.rs` (`transposition_key_separates_every_public_field`,
/// `fields_without_a_scoring_pair_share_cached_bounds`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Key([u64; 4]);

impl Key {
    #[inline]
    pub(crate) fn of(s: &FastState, team: u8, tricks: bool) -> Key {
        let mut h = s.hands().map(|h| h.0);
        let live = s.in_hands().0;
        for suit in SUITS {
            let shift = suit as u8 * SUIT_CARDS;
            let ranks = (live >> shift & LOW) as usize;
            if ranks & (ranks + 1) == 0 {
                continue; // Already packed (including an empty suit).
            }
            for hand in &mut h {
                let packed = PACK_LOW[ranks][(*hand >> shift & LOW) as usize] as u64;
                *hand = (*hand & !(LOW << shift)) | packed << shift;
            }
        }
        let called = s.called() & (team_halves(s, 0) | team_halves(s, 1));
        let open = [0, 1].map(|team| team_pairs(s, team) != 0);
        // Two bits per seat (`FastState::rights_code`): seats 0 and 2 in 0x33, 1 and 3 in 0xCC.
        let rights = s.rights_code() & (if open[0] { 0x33 } else { 0 } | if open[1] { 0xCC } else { 0 });
        let phase = if open[s.leader() as usize & 1] {
            s.phase().code()
        } else {
            0
        };
        let meta = team as u64
            | (s.leader() as u64) << 1
            | (s.trump().map_or(4, |t| t as u64)) << 3
            | (called as u64) << 6
            | (rights as u64) << 10
            | (phase as u64) << 18
            | (tricks as u64) << 21;
        Key([h[0], h[1], h[2], h[3] | meta << CARDS])
    }

    /// Translate the ordering hint along with the zero-point rank abstraction.
    #[inline]
    pub(crate) fn encode_move(s: &FastState, code: u8) -> u8 {
        if code >= CARDS as u8 || code % SUIT_CARDS >= ZERO_POINT_RANKS {
            return code;
        }
        let base = code / SUIT_CARDS * SUIT_CARDS;
        base + ((s.in_hands().0 >> base & LOW) & ((1 << (code - base)) - 1)).count_ones() as u8
    }

    #[inline]
    pub(crate) fn decode_move(s: &FastState, code: u8) -> u8 {
        if code >= CARDS as u8 || code % SUIT_CARDS >= ZERO_POINT_RANKS {
            return code;
        }
        let base = code / SUIT_CARDS * SUIT_CARDS;
        let mut ranks = s.in_hands().0 >> base & LOW;
        for _ in 0..code % SUIT_CARDS {
            ranks &= ranks - 1;
        }
        base + ranks.trailing_zeros() as u8
    }

    #[inline]
    fn hash(&self) -> u64 {
        let k = self.0;
        let mut h = k[0].wrapping_mul(0x9E37_79B9_7F4A_7C15);
        h = (h ^ k[1]).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
        h = (h ^ k[2]).wrapping_mul(0x1656_67B1_9E37_79F9);
        h = (h ^ k[3]).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        h ^ h >> 29
    }
}

/// Bounds `lo <= future points <= hi`, and the best move found (`NO_MOVE` if none).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry {
    key: Key,
    pub(crate) lo: i16,
    pub(crate) hi: i16,
    pub(crate) best: u8,
    // Fits the existing entry padding on the supported 64-bit target. Zero is
    // invalid; stale entries must neither probe nor tighten a new response.
    epoch: u16,
}

pub(crate) const NO_MOVE: u8 = u8::MAX;

const EMPTY: Entry = Entry {
    key: Key([u64::MAX; 4]),
    lo: i16::MIN,
    hi: i16::MAX,
    best: NO_MOVE,
    epoch: 0,
};

/// Fixed-size, direct-mapped, always-replace.
#[derive(Clone, Debug)]
pub(crate) struct TransTable {
    entries: Vec<Entry>,
    shift: u32,
    epoch: u16,
}

impl TransTable {
    pub(crate) fn new(bits: u32) -> TransTable {
        TransTable {
            entries: vec![EMPTY; 1 << bits],
            shift: 64 - bits,
            epoch: 1,
        }
    }

    pub(crate) fn clear(&mut self) {
        // Logical emptiness preserves slots, replacement, bounds and move
        // ordering within a response. Reusing an epoch requires erasing every
        // old entry first, including entries not touched for a whole cycle.
        if self.epoch == u16::MAX {
            self.entries.fill(EMPTY);
            self.epoch = 1;
        } else {
            self.epoch += 1;
        }
    }

    #[inline]
    fn slot(&self, key: &Key) -> usize {
        (key.hash() >> self.shift) as usize
    }

    #[inline]
    pub(crate) fn probe(&self, key: &Key) -> Option<Entry> {
        let e = self.entries[self.slot(key)];
        (e.epoch == self.epoch && e.key == *key).then_some(e)
    }

    /// Stores bounds, tightening those already known for the same key.
    #[inline]
    pub(crate) fn store(&mut self, key: Key, lo: i32, hi: i32, best: u8) {
        let (lo, hi) = (lo.max(i16::MIN as i32), hi.min(i16::MAX as i32));
        let slot = self.slot(&key);
        let e = &mut self.entries[slot];
        if e.epoch == self.epoch && e.key == key {
            e.lo = e.lo.max(lo as i16);
            e.hi = e.hi.min(hi as i16);
            e.best = best;
        } else {
            *e = Entry {
                key,
                lo: lo as i16,
                hi: hi as i16,
                best,
                epoch: self.epoch,
            };
        }
    }
}

#[cfg(test)]
#[path = "../tests/internal/tt.rs"]
mod tests;

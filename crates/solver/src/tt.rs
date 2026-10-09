//! Transposition table for trick starts: bounds on a team's future points, keyed by the
//! position. Points and tricks so far are path data and stay out of the key.

use engine::{FastState, CARDS};

/// A trick-start position, the team the values are for, and the measure (points or tricks).
/// Hands, leader, trump, called suits and trump rights decide the rest of the play;
/// All three rights levels and the pending question/phase matter, even without an own pair.
/// Every field is guarded by `tests/exact.rs`
/// (`transposition_key_separates_every_public_field`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Key([u64; 4]);

impl Key {
    #[inline]
    pub(crate) fn of(s: &FastState, team: u8, tricks: bool) -> Key {
        let h = s.hands();
        let meta = team as u64
            | (s.leader() as u64) << 1
            | (s.trump().map_or(4, |t| t as u64)) << 3
            | (s.called() as u64) << 6
            | (s.rights_code() as u64) << 10
            | (s.phase().code() as u64) << 18
            | (tricks as u64) << 21;
        Key([h[0].0, h[1].0, h[2].0, h[3].0 | meta << CARDS])
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
}

pub(crate) const NO_MOVE: u8 = u8::MAX;

const EMPTY: Entry = Entry {
    key: Key([u64::MAX; 4]),
    lo: i16::MIN,
    hi: i16::MAX,
    best: NO_MOVE,
};

/// Fixed-size, direct-mapped, always-replace.
#[derive(Clone, Debug)]
pub(crate) struct TransTable {
    entries: Vec<Entry>,
    shift: u32,
}

impl TransTable {
    pub(crate) fn new(bits: u32) -> TransTable {
        TransTable {
            entries: vec![EMPTY; 1 << bits],
            shift: 64 - bits,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.entries.fill(EMPTY);
    }

    #[inline]
    fn slot(&self, key: &Key) -> usize {
        (key.hash() >> self.shift) as usize
    }

    #[inline]
    pub(crate) fn probe(&self, key: &Key) -> Option<Entry> {
        let e = self.entries[self.slot(key)];
        (e.key == *key).then_some(e)
    }

    /// Stores bounds, tightening those already known for the same key.
    #[inline]
    pub(crate) fn store(&mut self, key: Key, lo: i32, hi: i32, best: u8) {
        let (lo, hi) = (lo.max(i16::MIN as i32), hi.min(i16::MAX as i32));
        let slot = self.slot(&key);
        let e = &mut self.entries[slot];
        if e.key == key {
            e.lo = e.lo.max(lo as i16);
            e.hi = e.hi.min(hi as i16);
            e.best = best;
        } else {
            *e = Entry {
                key,
                lo: lo as i16,
                hi: hi as i16,
                best,
            };
        }
    }
}

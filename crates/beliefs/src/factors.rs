//! Per-card signal factors (F4): the partner's pass and early leads multiply the cards they
//! inform; discards carry no card information and are not read (S8c). C32 (owner: no learning
//! in v1): every factor is 1 (`Factors::NONE`); the fitted tables of S9 are in git history.

use agent::SeatKind;
use engine::{partner, suit_of, CardSet, CARDS};
use knowledge::{Knowledge, Passed};

use crate::mind::PerKind;

/// Value classes of the factors: ace, ten, king or ober, the rest (U–6).
pub const VALUE_CLASSES: usize = 4;

pub fn value_class(card: u8) -> usize {
    match card % 9 {
        8 => 0,
        7 => 1,
        5 | 6 => 2,
        _ => 3,
    }
}

/// A lead counts as early in the first this-many tricks.
pub const EARLY_TRICKS: usize = 3;

/// The factor tables of one seat kind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Factors {
    /// The partner's pass I saw: `[value class][cards of the card's suit in the pass: 0, 1, ≥ 2]`.
    pub pass: [[f32; 3]; VALUE_CLASSES],
    /// The seat led the card's suit (not trump at the time) in one of the first `EARLY_TRICKS`.
    pub lead: [f32; VALUE_CLASSES],
}

impl Factors {
    /// No factor: every signal reads 1.
    pub const NONE: Factors = Factors {
        pass: [[1.0; 3]; VALUE_CLASSES],
        lead: [1.0; VALUE_CLASSES],
    };
}

/// Which factor applies to a seat's card: the keys of the fit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Pass { class: usize, in_pass: usize },
    Lead { class: usize },
}

/// The cards the partner passed me, if I saw them.
fn partner_pass(k: &Knowledge) -> Option<CardSet> {
    match k.received(k.me()) {
        Passed::Seen(cards) => Some(cards),
        _ => None,
    }
}

/// Suits each seat led early, not as trump.
fn early_leads(k: &Knowledge) -> [u8; 4] {
    let mut out = [0u8; 4];
    for lead in k.leads().take(EARLY_TRICKS) {
        let s = suit_of(lead.card);
        if Some(s) != lead.trump {
            out[lead.seat as usize] |= 1 << s as u8;
        }
    }
    out
}

/// Calls `f(seat, card, key)` for every key that applies to an open card of another seat.
pub fn for_each_key(k: &Knowledge, mut f: impl FnMut(u8, u8, Key)) {
    let me = k.me();
    let leads = early_leads(k);
    let pass = partner_pass(k);
    for y in (0..4u8).filter(|&y| y != me) {
        for c in k.possible(y).iter() {
            let class = value_class(c);
            if let (Some(x), true) = (pass, y == partner(me)) {
                let in_pass = (x & CardSet::suit(suit_of(c))).len().min(2) as usize;
                f(y, c, Key::Pass { class, in_pass });
            }
            if leads[y as usize] >> suit_of(c) as u8 & 1 == 1 {
                f(y, c, Key::Lead { class });
            }
        }
    }
}

impl Factors {
    pub fn of(&self, key: Key) -> f32 {
        match key {
            Key::Pass { class, in_pass } => self.pass[class][in_pass],
            Key::Lead { class } => self.lead[class],
        }
    }

    /// The product of every factor per cell (1 where none applies), each seat read with the
    /// factors of its kind.
    pub fn table(per_kind: &PerKind<Factors>, k: &Knowledge, kinds: [SeatKind; 4], floor: f32) -> [[f32; CARDS]; 4] {
        let mut t = [[1.0f32; CARDS]; 4];
        for_each_key(k, |y, c, key| {
            t[y as usize][c as usize] *= per_kind.of(kinds[y as usize]).of(key).max(floor)
        });
        t
    }
}

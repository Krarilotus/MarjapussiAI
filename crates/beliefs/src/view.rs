//! Lazy views (F9): what I believe another seat `j` believes, from the public facts plus what
//! I know `j` knows (the cards I know `j` holds; for my partner, the cards it passed me that I
//! still hold). Computed on first read per epoch; step 1 has no signals (uniform open cells,
//! renormalised).

use engine::{partner, CardSet, CARDS};
use knowledge::Passed;
use observation::Seat;

use crate::band::Band;
use crate::mind::Mind;
use crate::table::{self, belief};
use crate::Table;

/// Seat `viewer`'s card table, as I believe it believes it.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub viewer: Seat,
    table: Table,
}

impl View {
    /// P(`seat` holds `card`) in the viewer's eyes.
    pub fn p_holds(&self, seat: Seat, card: u8) -> f32 {
        self.table[seat as usize][card as usize]
    }

    pub fn row(&self, seat: Seat) -> [f32; CARDS] {
        self.table[seat as usize]
    }

    /// Whether the viewer knows `card`'s holder (as far as I can tell).
    pub fn knows(&self, seat: Seat, card: u8) -> bool {
        Band::of(self.p_holds(seat, card), Default::default()).is_known()
    }
}

impl Mind {
    /// Seat `j`'s view. `view(me)` is my own table.
    pub fn view(&self, j: Seat) -> View {
        if j == self.seat {
            return View {
                viewer: j,
                table: self.derived().holds,
            };
        }
        let key = (self.public.fact_epoch(), self.epochs());
        if let Some((e, t)) = self.cache.borrow().views[j as usize] {
            if e == key {
                return View { viewer: j, table: t };
            }
        }
        let t = self.view_table(j);
        self.cache.borrow_mut().views[j as usize] = Some((key, t));
        View { viewer: j, table: t }
    }

    fn view_table(&self, j: Seat) -> Table {
        let (k, public) = (self.knowledge(), self.public());
        // Cards j certainly knows the holder of, beyond the public facts.
        let mut known: [CardSet; 4] = std::array::from_fn(|y| public.certain(y as Seat));
        known[j as usize] |= k.certain(j);
        if j == partner(self.seat) {
            if let Passed::Seen(cards) = k.received(self.seat) {
                known[self.seat as usize] |= cards & k.certain(self.seat);
            }
        }
        let decided = known.iter().fold(CardSet::EMPTY, |a, &s| a | s);
        let mut t = [[0.0f32; CARDS]; 4];
        for c in (CardSet::ALL - public.played()).iter() {
            if decided.contains(c) {
                for (y, row) in t.iter_mut().enumerate() {
                    row[c as usize] = known[y].contains(c) as u8 as f32;
                }
                continue;
            }
            let holders = (0..4).filter(|&y| public.may_hold(y as Seat).contains(c));
            let n = holders.clone().count() as f32;
            for y in holders {
                t[y][c as usize] = if n == 1.0 { 1.0 } else { belief(1.0 / n) };
            }
        }
        let movable = table::open_cells(&t);
        table::renormalise(&mut t, table::sizes(public), movable);
        t
    }
}

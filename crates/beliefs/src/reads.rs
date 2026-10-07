//! The read API of `Mind`. Every read is exactly 1 or 0 where the facts decide it (the exact
//! sets, K1–K6) and strictly between otherwise (`table::read`).

use engine::{partner, suit_of, takes, CardSet, PlayerTrumpPossibilities, Suit, CARDS};
use observation::Seat;

use crate::calibrate::Calibration;
use crate::features::{family, layout, Moments, N_FEATURES};
use crate::mind::Mind;
use crate::table::{belief, read, SUIT_CARDS};

/// Distribution of how many cards of a suit a seat holds now.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CountDist {
    pub p: [f32; SUIT_CARDS + 1],
}

impl CountDist {
    pub fn mean(&self) -> f32 {
        self.p.iter().enumerate().map(|(k, p)| k as f32 * p).sum()
    }
}

/// Features of a seat's hand (now, or as dealt): P(an ace), P(at least k halves) for
/// k = 1, 2, 3, P(a half) and P(the pair) per suit.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SeatFeatures {
    pub ace: f32,
    pub halves_at_least: [f32; 3],
    pub half: [f32; 4],
    pub pair: [f32; 4],
}

impl SeatFeatures {
    /// From the DP's moments: facts exact, beliefs calibrated and kept inside (0, 1).
    pub(crate) fn of(m: &Moments, z: f64, cal: &Calibration) -> SeatFeatures {
        let f: [f32; N_FEATURES] = std::array::from_fn(|i| {
            if z <= 0.0 {
                0.0
            } else if m.is_fact(i) {
                m.fact(i) as u8 as f32
            } else {
                belief(cal.apply(family(i), m.sum[i] / z) as f32)
            }
        });
        let (ace, halves_at_least, half, pair) = layout(f);
        SeatFeatures {
            ace,
            halves_at_least,
            half,
            pair,
        }
    }
}

impl Mind {
    /// P(`card` is in `seat`'s hand now).
    pub fn p_holds(&self, seat: Seat, card: u8) -> f32 {
        self.derived().holds[seat as usize][card as usize]
    }

    /// The whole row of `seat` (one lookup for many reads).
    pub fn holds_row(&self, seat: Seat) -> [f32; CARDS] {
        self.derived().holds[seat as usize]
    }

    /// The distribution of `seat`'s count of `suit` now (support inside K4's bounds).
    pub fn count(&self, seat: Seat, suit: Suit) -> CountDist {
        self.derived().counts[seat as usize][suit as usize]
    }

    /// Features of `seat`'s hand now (F5: the exact feature DP under every seat's bids).
    pub fn features(&self, seat: Seat) -> SeatFeatures {
        self.derived().now[seat as usize]
    }

    /// Features of `seat`'s dealt hand, the hand it bid with (F5, F7: the exchange undone).
    pub fn dealt_features(&self, seat: Seat) -> SeatFeatures {
        self.dealt()[seat as usize]
    }

    /// P(`seat` holds no card of `suit`).
    pub fn p_void(&self, seat: Seat, suit: Suit) -> f32 {
        let row = self.holds_row(seat);
        let live = CardSet::suit(suit) - self.knowledge().played();
        let (lo, _) = self.knowledge().count_bounds(seat, suit);
        let known = if live.iter().all(|c| row[c as usize] == 0.0) {
            Some(true)
        } else if lo > 0 || live.iter().any(|c| row[c as usize] == 1.0) {
            Some(false)
        } else {
            None
        };
        read(self.count(seat, suit).p[0], known)
    }

    /// P(my `card` wins its trick if I lead it now). 1 if it stands (K1); 0 if a higher card of
    /// its suit is certainly with an opponent (who must then beat it). Not my card: 0.
    pub fn p_stands(&self, card: u8) -> f32 {
        let me = self.seat;
        let k = self.knowledge();
        if !k.certain(me).contains(card) {
            return 0.0;
        }
        if k.standing(me).contains(card) {
            return 1.0;
        }
        let t = self.derived().holds;
        let mate = partner(me) as usize;
        let suit = suit_of(card);
        let higher = (CardSet::suit(suit) - k.played() - k.certain(me))
            .iter()
            .filter(|&b| takes(b, card, None));
        let mut p = 1.0f32;
        for h in higher {
            let with_mate = t[mate][h as usize];
            if with_mate == 0.0 {
                return 0.0; // with an opponent for sure
            }
            p *= with_mate;
        }
        p *= self.no_ruff(suit);
        read(p, None)
    }

    /// P(no opponent trumps a lead of `suit`): each opponent either has the suit or no trump.
    fn no_ruff(&self, suit: Suit) -> f32 {
        let me = self.seat;
        match self.knowledge().trump() {
            Some(t) if t != suit => [(me + 1) % 4, (me + 3) % 4]
                .iter()
                .map(|&o| 1.0 - self.p_void(o, suit) * (1.0 - self.p_void(o, t)))
                .product(),
            _ => 1.0,
        }
    }

    /// P(my `card` wins a trick before the end), closed form (F10, v0): every higher live card
    /// is with my partner, or falls before it (an opponent's higher card falls while I still
    /// hold other cards of the suit: weight (my length − 1) / my length); times no ruff.
    /// 1 if it stands; never a fact otherwise (the play can still change).
    pub fn future_win(&self, card: u8) -> f32 {
        let me = self.seat;
        let k = self.knowledge();
        if !k.certain(me).contains(card) {
            return 0.0;
        }
        let stands = self.p_stands(card);
        if stands >= 1.0 {
            return 1.0;
        }
        let t = self.derived().holds;
        let mate = partner(me) as usize;
        let suit = suit_of(card);
        let mine = (k.certain(me) & CardSet::suit(suit)).len() as f32;
        let wait = (mine - 1.0) / mine;
        let mut p = 1.0f32;
        for h in (CardSet::suit(suit) - k.played() - k.certain(me))
            .iter()
            .filter(|&b| takes(b, card, None))
        {
            let with_mate = t[mate][h as usize];
            p *= with_mate + (1.0 - with_mate) * wait;
        }
        read((p * self.no_ruff(suit)).max(stands), None)
    }

    fn team_seats(team: u8) -> [Seat; 2] {
        [team & 1, (team & 1) + 2]
    }

    /// P(`team` holds both halves of `suit` now, in one hand or split). K3 decides 1 and 0.
    pub fn p_team_pair(&self, team: u8, suit: Suit) -> f32 {
        let [a, b] = Self::team_seats(team);
        let tp = self.knowledge().team_pairs(a);
        let bit = 1 << suit as u8;
        let known = if tp.certain() & bit != 0 {
            Some(true)
        } else if tp.possible() & bit == 0 {
            Some(false)
        } else {
            None
        };
        let t = self.derived().holds;
        let in_team = |c: u8| t[a as usize][c as usize] + t[b as usize][c as usize];
        let p: f32 = CardSet::halves(suit).iter().map(in_team).product();
        read(p, known)
    }

    /// P(`team` can still announce `suit`): the pair in one hand with a usable right (`Own`, or
    /// the partner may still ask "Yours"), or split (a half question). 0 if K3 rules out every
    /// way or no right is left.
    pub fn p_meldable(&self, team: u8, suit: Suit) -> f32 {
        let k = self.knowledge();
        let [a, b] = Self::team_seats(team);
        let bit = 1 << suit as u8;
        let tp = k.team_pairs(a);
        if k.is_called(suit) || tp.possible() & bit == 0 {
            return 0.0;
        }
        let t = self.derived().holds;
        let [ko, kk] = {
            let mut it = CardSet::halves(suit).iter();
            [it.next().unwrap(), it.next().unwrap()]
        };
        let can_own = |x: Seat| k.rights(x) == PlayerTrumpPossibilities::Own;
        let can_ask_yours = |x: Seat| k.rights(x) != PlayerTrumpPossibilities::Ours;
        let one_hand = |x: Seat, y: Seat| {
            let usable = can_own(x) || can_ask_yours(y);
            if usable {
                t[x as usize][ko as usize] * t[x as usize][kk as usize]
            } else {
                0.0
            }
        };
        let split = t[a as usize][ko as usize] * t[b as usize][kk as usize]
            + t[b as usize][ko as usize] * t[a as usize][kk as usize];
        let p = one_hand(a, b) + one_hand(b, a) + split;
        let certain_usable = (tp.mine.certain & bit != 0 && (can_own(a) || can_ask_yours(b)))
            || (tp.yours.certain & bit != 0 && (can_own(b) || can_ask_yours(a)))
            || tp.ours.certain & bit != 0;
        if certain_usable {
            return 1.0;
        }
        if p == 0.0 {
            return 0.0;
        }
        read(p, None)
    }

    /// KB `has_ace(y)`: P(`y` was dealt an ace), from its steps read with our convention.
    pub fn has_ace(&self, y: Seat) -> f32 {
        self.dealt_features(y).ace
    }

    /// KB `has_halves(y, k)`: P(`y` was dealt at least `k` halves), k = 1, 2, 3.
    pub fn has_halves(&self, y: Seat, k: usize) -> f32 {
        self.dealt_features(y).halves_at_least[k.clamp(1, 3) - 1]
    }

    /// KB `has_pair(y, s)`: P(`y` was dealt the pair of `s`).
    pub fn has_pair(&self, y: Seat, s: Suit) -> f32 {
        self.dealt_features(y).pair[s as usize]
    }

    /// KB `team_pair(s)`: P(my team holds the uncalled pair of `s` now): exact on my hand; with
    /// one half of mine, P(the partner holds the other); with none, P(the partner holds the
    /// pair) from its signals (the C10 completion: its big pair and my Bells half mean Red).
    pub fn team_pair(&self, s: Suit) -> f32 {
        let k = self.knowledge();
        let halves = CardSet::halves(s);
        if k.is_called(s) || !(halves & k.played()).is_empty() {
            return 0.0;
        }
        let mine = k.certain(self.seat) & halves;
        let mate = partner(self.seat);
        match mine.len() {
            2 => 1.0,
            1 => (halves - mine).iter().map(|c| self.p_holds(mate, c)).product(),
            _ => self.features(mate).pair[s as usize],
        }
    }

    /// KB `team_pair_cover(V)`: P(my team holds an uncalled pair worth at least V − 140). Above
    /// 140 the card points cannot reach V, so only a pair can cover the excess.
    pub fn team_pair_cover(&self, v: i32) -> f32 {
        let need = v - crate::conventions::PAIR_LINE;
        1.0 - engine::SUITS
            .into_iter()
            .filter(|&s| crate::conventions::pair_value(s) >= need)
            .map(|s| 1.0 - self.team_pair(s))
            .product::<f32>()
    }

    /// KB `opp_team_pair(s)`: P(the other team holds the uncalled pair of `s` now): one of them
    /// holds both halves (their signals, through the feature DP) or each holds one. 0 when a
    /// half is certainly with my team.
    pub fn opp_team_pair(&self, s: Suit) -> f32 {
        let k = self.knowledge();
        let halves = CardSet::halves(s);
        let ours = k.certain(self.seat) | k.certain(partner(self.seat));
        if k.is_called(s) || !(halves & (k.played() | ours)).is_empty() {
            return 0.0;
        }
        let [l, r] = engine::opponents(self.seat);
        let mut it = halves.iter();
        let (a, b) = (it.next().unwrap(), it.next().unwrap());
        let split = self.p_holds(l, a) * self.p_holds(r, b) + self.p_holds(l, b) * self.p_holds(r, a);
        (self.features(l).pair[s as usize] + self.features(r).pair[s as usize] + split).min(1.0)
    }

    /// KB `opp_reach(cover)`: the highest value the other team can likely play: 140 plus the
    /// value of its likely pair, the largest pair value v with P(they hold a pair worth at least
    /// v) ≥ `cover`. The pair comes from their signals plus our own halves ([O] 2026-10-07): 140
    /// unless one of them has sent a pair message (above 140 only a pair covers the excess).
    pub fn opp_reach(&self, cover: f32) -> i32 {
        let value = crate::conventions::pair_value;
        let signalled = {
            let record = self.record();
            engine::opponents(self.seat).iter().any(|&y| {
                record.seats[y as usize].bids.iter().any(|o| {
                    o.nominal
                        .is_some_and(|m| crate::conventions::PAIR_MESSAGES & m.bit() != 0)
                })
            })
        };
        if !signalled {
            return crate::conventions::PAIR_LINE;
        }
        engine::SUITS
            .into_iter()
            .map(value)
            .filter(|&v| {
                1.0 - engine::SUITS
                    .into_iter()
                    .filter(|&s| value(s) >= v)
                    .map(|s| 1.0 - self.opp_team_pair(s))
                    .product::<f32>()
                    >= cover
            })
            .max()
            .map_or(crate::conventions::PAIR_LINE, |v| crate::conventions::PAIR_LINE + v)
    }
}

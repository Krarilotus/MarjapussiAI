//! The solver: exact future points of one team in a full-information card-play state.
//!
//! The team maximises its future points (cards, the last-trick bonus, its own announced pairs);
//! the other team minimises them. Fail-soft alpha-beta with a transposition table at trick
//! starts (bounds on future points), a static upper bound, safe card equivalence and move
//! ordering; a forced last trick is played out. `value` is one full-window search, `reaches`
//! one zero-window probe (MTD(f) over zero-window probes measured slower for `value`;
//! docs/design/solver.md). The same search counts tricks instead of points for `tricks` and
//! `reaches_tricks` (schwarz).

use engine::{
    next_seat, opponents, partner, points_set, suit_of, takers, takes, trick_winner, uncalled_pairs, value_of, CardSet,
    FastState, Move, PlayPhase, Suit, CARD_POINTS, LAST_TRICK_BONUS, PAIR_POINTS, SUITS, TRICKS,
};

use crate::tt::{Key, TransTable, NO_MOVE};
use std::time::Instant;

/// Limits for a whole decision, including all candidate moves and worlds.
/// A node cap provides deterministic bounded work; a deadline also bounds slow positions.
#[derive(Clone, Copy, Debug, Default)]
pub struct Limits {
    pub deadline: Option<Instant>,
    pub nodes: Option<u64>,
}

pub(crate) struct Budget {
    limits: Limits,
    start_nodes: u64,
}

impl Budget {
    pub(crate) fn new(limits: Limits, solver: &Solver) -> Self {
        Self {
            limits,
            start_nodes: solver.nodes(),
        }
    }

    /// Nodes searched under this budget so far.
    pub(crate) fn used(&self, solver: &Solver) -> u64 {
        solver.nodes().saturating_sub(self.start_nodes)
    }

    /// The node limit, if any.
    pub(crate) fn nodes(&self) -> Option<u64> {
        self.limits.nodes
    }

    pub(crate) fn stopped(&self, solver: &Solver) -> bool {
        self.limits
            .nodes
            .is_some_and(|n| solver.nodes().saturating_sub(self.start_nodes) >= n)
            || self.limits.deadline.is_some_and(|deadline| Instant::now() >= deadline)
    }
}

/// Default transposition table: 2^18 entries of 40 bytes (10 MB).
pub const DEFAULT_TT_BITS: u32 = 18;

/// Uncalled suits whose halves `team` holds between its two hands (suit bits): the only pairs
/// an announcement or a question can still score for it.
#[inline]
fn team_pairs(s: &FastState, team: u8) -> u8 {
    uncalled_pairs(s.hand(team) | s.hand(team + 2), s.called())
}

/// Uncalled pairs that could still score for `team`. This deliberately overestimates
/// activation: a question can reach a partner or split pair despite lost own rights.
#[inline]
fn pair_bound(s: &FastState, team: u8) -> i32 {
    let pairs = team_pairs(s, team);
    SUITS
        .iter()
        .filter(|&&suit| pairs >> suit as u8 & 1 != 0)
        .map(|&suit| PAIR_POINTS[suit as usize])
        .sum()
}

/// The moves searched at a node, best guess first: one card per run of equivalent cards.
struct MoveList {
    codes: [u8; 18],
    scores: [i32; 18],
    len: usize,
}

impl MoveList {
    fn push(&mut self, code: u8, score: i32) {
        self.codes[self.len] = code;
        self.scores[self.len] = score;
        self.len += 1;
    }

    #[inline]
    fn new(s: &FastState, tt_best: u8) -> MoveList {
        let mut list = MoveList {
            codes: [0; 18],
            scores: [0; 18],
            len: 0,
        };
        if s.phase() != PlayPhase::Cards {
            let pairs = team_pairs(s, s.leader() & 1) != 0;
            for m in s.moves().filter(|m| !matches!(m, Move::Card(_))) {
                let score = match m {
                    Move::Announce(suit) => 10_000 + PAIR_POINTS[suit as usize],
                    Move::Question(q) => {
                        // With exposed hands an answer provides no information. If no answer
                        // changes trump or scores a pair, playing directly dominates this
                        // question: same cards and lead, with at least as many rights retained.
                        // Keep all root questions in world evaluation; this is not an
                        // information-set pruning rule. Without a pair in the leader's team, no
                        // answer can.
                        if !pairs || !s.question_activates(q) {
                            continue;
                        }
                        9_000
                    }
                    Move::Answer(_) => 10_000,
                    Move::Card(_) => unreachable!(),
                };
                list.push(m.code(), score);
            }
        }
        let legal = s.legal_cards();
        if list.len == 0 && legal.len() <= 1 {
            for c in legal.iter() {
                list.push(c, 0);
            }
            return list;
        }
        let trick = s.trick();
        let trump = s.trump();
        let mover = s.to_move();
        let (hand, mate) = (s.hand(mover), partner(mover));
        let live = trick.iter().fold(s.in_hands(), |a, &c| a | CardSet::single(c));
        // Whether `seat` can trump a lead of `suit`: void in it, holding a trump.
        let ruffs = |seat: u8, suit: Suit| {
            let held = s.hand(seat);
            trump.is_some_and(|t| {
                t != suit && (held & CardSet::suit(suit)).is_empty() && !(held & CardSet::suit(t)).is_empty()
            })
        };
        // The winning card, and whether it is the partner's and holds against the opponent
        // still to play.
        let lead = trick.split_first().map(|_| {
            let w = trick_winner(trick, trump);
            let holds = trick.len() == 3 || (s.hand(next_seat(mover)) & takers(trick[w], trump)).is_empty();
            (trick[w], (s.leader() as usize + w) % 2 == mover as usize % 2 && holds)
        });
        for c in legal.iter() {
            // Equivalence: the next lower live card of the suit is ours and scores the same, so
            // no live card lies between them and they are interchangeable from here on.
            let below = (live & CardSet::suit(suit_of(c))).0 & ((1u64 << c) - 1);
            if below != 0 {
                let d = 63 - below.leading_zeros() as u8;
                if hand.contains(d) && CARD_POINTS[d as usize] == CARD_POINTS[c as usize] {
                    continue;
                }
            }
            let (p, rank) = (CARD_POINTS[c as usize] as i32, value_of(c) as i32);
            let score = match lead {
                // Lead a winner (no live card of its suit above it, no opponent to trump it), then
                // a card the partner wins (with the suit's top card or a trump), then cheap cards.
                None => {
                    let suit = suit_of(c);
                    let above = (live & CardSet::suit(suit)).0 & !((2u64 << c) - 1);
                    if opponents(mover).iter().any(|&o| ruffs(o, suit)) {
                        -4 * p - rank
                    } else if above == 0 {
                        200 + 4 * p + rank
                    } else if s.hand(mate).contains(63 - above.leading_zeros() as u8) || ruffs(mate, suit) {
                        100 + 4 * p + rank
                    } else {
                        -4 * p - rank
                    }
                }
                Some((winner, partner_wins)) => match (takes(c, winner, trump), partner_wins) {
                    (true, false) => 200 + 4 * p - rank,
                    (false, true) => 100 + 4 * p,
                    (true, true) => 50 + p,
                    (false, false) => -4 * p - rank,
                },
            };
            list.push(c, score);
        }
        for i in 0..list.len {
            if list.codes[i] == tt_best {
                list.scores[i] = i32::MAX;
            }
        }
        for i in 1..list.len {
            let mut j = i;
            while j > 0 && list.scores[j - 1] < list.scores[j] {
                list.scores.swap(j - 1, j);
                list.codes.swap(j - 1, j);
                j -= 1;
            }
        }
        list
    }
}

/// Exact solver with its transposition table (reused across calls and teams).
#[derive(Clone, Debug)]
pub struct Solver {
    tt: TransTable,
    team: u8,
    /// The searched measure: tricks if set, else points.
    tricks: bool,
    nodes: u64,
    /// Card points already awarded plus all still available, including the last bonus.
    card_total: i32,
}

impl Default for Solver {
    fn default() -> Self {
        Solver::with_tt_bits(DEFAULT_TT_BITS)
    }
}

impl Solver {
    pub fn new() -> Solver {
        Solver::default()
    }

    /// A solver with 2^`bits` table entries (40 bytes each).
    pub fn with_tt_bits(bits: u32) -> Solver {
        Solver {
            tt: TransTable::new(bits),
            team: 0,
            tricks: false,
            nodes: 0,
            card_total: 0,
        }
    }

    /// Nodes searched since the solver was made or `reset_nodes` was called.
    pub fn nodes(&self) -> u64 {
        self.nodes
    }

    pub fn reset_nodes(&mut self) {
        self.nodes = 0;
    }

    /// Empties the transposition table.
    pub fn clear(&mut self) {
        self.tt.clear();
    }

    /// The exact future points of `team` (seat % 2) from `s` under best play by both teams:
    /// `team` maximises them, the other team minimises them.
    pub fn value(&mut self, s: &FastState, team: u8) -> i32 {
        self.solve(s, team, false, None, &Budget::new(Limits::default(), self))
            .expect("unlimited search")
    }

    /// The exact number of tricks `team` still takes when it maximises its tricks and the
    /// other team minimises them.
    pub fn tricks(&mut self, s: &FastState, team: u8) -> i32 {
        self.solve(s, team, true, None, &Budget::new(Limits::default(), self))
            .expect("unlimited search")
    }

    /// Whether `team` can take at least `target` more tricks against best defence (one
    /// zero-window probe). Schwarz: `reaches_tricks(s, team, 1)` for a team without a trick.
    pub fn reaches_tricks(&mut self, s: &FastState, team: u8, target: i32) -> bool {
        if target <= 0 {
            return true;
        }
        self.solve(s, team, true, Some(target), &Budget::new(Limits::default(), self))
            .expect("unlimited search")
            >= target
    }

    fn set(&mut self, team: u8, tricks: bool) {
        self.team = team & 1;
        self.tricks = tricks;
    }

    /// The searched measure of `self.team` so far.
    #[inline]
    fn so_far(&self, s: &FastState) -> i32 {
        if self.tricks {
            s.tricks_won(self.team) as i32
        } else {
            s.points(self.team)
        }
    }

    /// An upper bound on the measure still to come.
    #[inline]
    fn most(&self, s: &FastState) -> i32 {
        if !self.tricks {
            self.card_total - s.card_points(0) - s.card_points(1) + pair_bound(s, self.team)
        } else {
            (TRICKS - s.tricks_done()) as i32
        }
    }

    /// Whether `team` can make at least `target` future points against best defence (one
    /// zero-window probe). For a contract: `target = game value - s.points(team)`.
    pub fn reaches(&mut self, s: &FastState, team: u8, target: i32) -> bool {
        if target <= 0 {
            return true;
        }
        self.solve(s, team, false, Some(target), &Budget::new(Limits::default(), self))
            .expect("unlimited search")
            >= target
    }

    pub(crate) fn solve(
        &mut self,
        s: &FastState,
        team: u8,
        tricks: bool,
        target: Option<i32>,
        budget: &Budget,
    ) -> Option<i32> {
        if budget.stopped(self) {
            return None;
        }
        if target.is_some_and(|t| t <= 0) {
            return Some(0);
        }
        self.set(team, tricks);
        // Card points are conserved through every continuation. Compute the total once,
        // rather than summing live ranks again at every node. Pair points are separate.
        let live = s.trick().iter().fold(s.in_hands(), |a, &c| a | CardSet::single(c));
        self.card_total =
            s.card_points(0) + s.card_points(1) + points_set(live) + if s.is_over() { 0 } else { LAST_TRICK_BONUS };
        let (alpha, beta) = target.map_or((-1, self.most(s) + 1), |target| (target - 1, target));
        self.search(s, alpha, beta, budget)
    }

    /// For every legal move (in `FastState::moves` order): the points it gains at once plus
    /// the exact future points after it, for `team`.
    pub fn move_values(&mut self, s: &FastState, team: u8) -> Vec<(Move, i32)> {
        s.moves()
            .map(|m| {
                let child = s.after(m);
                let gain = child.points(team & 1) - s.points(team & 1);
                (m, gain + self.value(&child, team))
            })
            .collect()
    }

    /// The measure still to come in the last trick if its play is forced: no question at its
    /// start could make a trump (both halves of an uncalled suit with the leader's team; no
    /// other question is searched, and one card cannot announce). Its winner takes the rest.
    #[inline]
    fn last_trick(&self, s: &FastState) -> Option<i32> {
        let forced = match s.phase() {
            PlayPhase::Cards => true,
            PlayPhase::Start => team_pairs(s, s.leader() & 1) == 0,
            PlayPhase::Answer(_) => false,
        };
        if !forced {
            return None;
        }
        let mut cards = [0; 4];
        let played = s.trick().len();
        cards[..played].copy_from_slice(s.trick());
        for (i, card) in cards.iter_mut().enumerate().skip(played) {
            *card = s.hand((s.leader() + i as u8) & 3).0.trailing_zeros() as u8;
        }
        let won = (s.leader() as usize + trick_winner(&cards, s.trump())) & 1 == self.team as usize;
        Some(match (won, self.tricks) {
            (false, _) => 0,
            (true, true) => 1,
            (true, false) => self.card_total - s.card_points(0) - s.card_points(1),
        })
    }

    /// Fail-soft alpha-beta on the future measure (points or tricks) of `self.team`.
    fn search(&mut self, s: &FastState, mut alpha: i32, mut beta: i32, budget: &Budget) -> Option<i32> {
        // Check a node cap exactly, and the clock once per 64 nodes. Completed child
        // bounds remain reusable; an interrupted parent never writes an exact entry.
        if budget
            .limits
            .nodes
            .is_some_and(|n| self.nodes.saturating_sub(budget.start_nodes) >= n)
            || self.nodes & 63 == 0 && budget.stopped(self)
        {
            return None;
        }
        self.nodes += 1;
        if s.is_over() {
            return Some(0);
        }
        if s.tricks_done() + 1 == TRICKS {
            if let Some(rest) = self.last_trick(s) {
                return Some(rest);
            }
        }
        let team = self.team;
        let most = self.most(s);
        if most <= alpha {
            return Some(most);
        }
        if beta <= 0 {
            return Some(0);
        }
        let start = s.trick().is_empty();
        let mut key = Key::default();
        let mut tt_best = NO_MOVE;
        if start {
            key = Key::of(s, team, self.tricks);
            if let Some(e) = self.tt.probe(&key) {
                let (lo, hi) = (e.lo as i32, e.hi as i32);
                if lo >= beta || lo == hi {
                    return Some(lo);
                }
                if hi <= alpha {
                    return Some(hi);
                }
                alpha = alpha.max(lo);
                beta = beta.min(hi);
                tt_best = Key::decode_move(s, e.best);
            }
        }
        let (alpha0, beta0) = (alpha, beta);
        let moves = MoveList::new(s, tt_best);
        let maximizing = s.to_move() & 1 == team;
        let base = self.so_far(s);
        let mut best = if maximizing { i32::MIN } else { i32::MAX };
        let mut best_move = NO_MOVE;
        for &code in &moves.codes[..moves.len] {
            let child = s.after(Move::from_code(code));
            let gain = self.so_far(&child) - base;
            let mut v;
            if best_move != NO_MOVE && beta - alpha > 1 {
                let (a, b) = if maximizing {
                    (alpha, alpha + 1)
                } else {
                    (beta - 1, beta)
                };
                v = gain + self.search(&child, a - gain, b - gain, budget)?;
                if v > alpha && v < beta {
                    v = gain + self.search(&child, alpha - gain, beta - gain, budget)?;
                }
            } else {
                v = gain + self.search(&child, alpha - gain, beta - gain, budget)?;
            }
            if maximizing {
                if v > best {
                    best = v;
                    best_move = code;
                }
                alpha = alpha.max(v);
            } else {
                if v < best {
                    best = v;
                    best_move = code;
                }
                beta = beta.min(v);
            }
            if alpha >= beta {
                break;
            }
        }
        if start {
            let (lo, hi) = if best <= alpha0 {
                (0, best)
            } else if best >= beta0 {
                (best, most)
            } else {
                (best, best)
            };
            self.tt.store(key, lo, hi, Key::encode_move(s, best_move));
        }
        Some(best)
    }
}

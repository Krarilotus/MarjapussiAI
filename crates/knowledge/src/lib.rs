//! Sound deduction of hidden cards from one seat's observation (layer 3).
//!
//! For every seat, `Knowledge` keeps the cards it may hold (`possible`) and the cards it
//! certainly holds (`certain`, disjoint from `possible`). Facts about pairs and halves are
//! cardinality constraints `lo <= |hand ∩ mask| <= hi`, updated when a card is played.
//! The design follows the owner's `refactor_step2` GameState (trick rules, pair concepts,
//! set-logic counting); see `docs/design/knowledge.md`. Only `SeatEvent`s go in, so this
//! crate cannot see hidden cards.

use std::cell::Cell;

use engine::is_first_trick;
use engine::Suit;
use engine::HAND_SIZE;
use engine::{
    next_seat, partner, play_levels, suit_of, trick_points, trump_points, winning_seat, CardSet,
    PlayerTrumpPossibilities, QuestionType, NO_BID, SUITS, TRICKS,
};
use observation::{Seat, SeatEvent};

mod allocations;
mod claim;
mod facts;
mod outcome_envelope;
mod sample;

pub use allocations::AllocationVisit;
pub use claim::remaining_claim;
pub use outcome_envelope::{EnvelopeError, IntegerInterval, PublicOutcomeEnvelope, ScoreIntervals};
pub use facts::{
    own_standing_of, standing_potential_of, Pairs, Passed, TeamPairs, BLANK_TEN_FACTOR, STOPPER_CONFIDENCE,
    STOPPER_MAX_HIGHER,
};
pub use sample::{Belief, DealSpace, ExactSets, Hands, Uniform, World};

const MAX_CONSTRAINTS: usize = 16;

/// `lo <= |hand(seat) ∩ mask| <= hi`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Constraint {
    pub seat: Seat,
    pub mask: CardSet,
    pub lo: u8,
    pub hi: u8,
}

/// The observation contradicts the knowledge. With a correct engine and observation this
/// cannot happen; tests treat it as a bug.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contradiction {
    pub seat: Seat,
    pub what: &'static str,
}

fn contradiction(seat: Seat, what: &'static str) -> Contradiction {
    Contradiction { seat, what }
}

fn suit_bit(suit: Suit) -> u8 {
    1 << suit as u8
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Knowledge {
    me: Seat,
    possible: [CardSet; 4],
    certain: [CardSet; 4],
    played: CardSet,
    hand_len: [u8; 4],
    constraints: [Constraint; MAX_CONSTRAINTS],
    n_constraints: u8,
    trump: Option<Suit>,
    called: u8,
    trick: [u8; 4],
    trick_len: u8,
    /// Provenance for the dealt hands (K5): cards each seat played, gave and received.
    played_by: [CardSet; 4],
    /// K6: cards each seat discarded (played neither in the led suit nor in trump).
    discarded: [CardSet; 4],
    given: [Passed; 4],
    received: [Passed; 4],
    /// The seat that passed first (the declarer's partner).
    first_passer: Option<Seat>,
    /// Public facts (K6), undone with the snapshot.
    rights: [PlayerTrumpPossibilities; 4],
    last_bidder: Option<Seat>,
    value: i32,
    trick_leader: Seat,
    trick_winners: [Seat; TRICKS as usize],
    tricks_done: u8,
    /// Per team: card points with the last-trick bonus, and announced pair points.
    card_points: [i32; 2],
    pair_points: [i32; 2],
    /// The per-play context, once per trick (packed `Lead`s): who led which card, under which
    /// trump. Trump only changes before a lead, so it holds for every play of the trick.
    leads: [u16; TRICKS as usize],
}

/// One trick's lead and the trump of its plays (`Knowledge::leads`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lead {
    pub seat: Seat,
    pub card: u8,
    pub trump: Option<Suit>,
}

impl Lead {
    fn pack(self) -> u16 {
        self.card as u16 | (self.seat as u16) << 6 | self.trump.map_or(0, |t| t as u16 + 1) << 8
    }

    fn unpack(x: u16) -> Lead {
        Lead {
            card: (x & 0x3F) as u8,
            seat: (x >> 6 & 3) as Seat,
            trump: match x >> 8 {
                0 => None,
                t => Some(SUITS[t as usize - 1]),
            },
        }
    }
}

/// The "seat" of a public instance (`Knowledge::public`): it holds no hand.
pub const PUBLIC: Seat = 4;

impl Knowledge {
    /// What every seat knows: no own hand, every pass hidden (feed it the public events, e.g.
    /// a seat's events with the pass cards removed). Answers possible/certain/hand sizes and
    /// constraints; the dealt-hand queries (K5) and `deals()` belong to a seat's own instance.
    pub fn public() -> Self {
        let mut k = Knowledge::new(0, CardSet::EMPTY);
        k.me = PUBLIC;
        k.possible = [CardSet::ALL; 4];
        k.certain = [CardSet::EMPTY; 4];
        k
    }

    /// Whether the two states agree on every fact the beliefs read: possible and certain sets,
    /// played cards, hand sizes, constraints, trump and called suits.
    pub fn same_facts(&self, other: &Knowledge) -> bool {
        self.possible == other.possible
            && self.certain == other.certain
            && self.played == other.played
            && self.hand_len == other.hand_len
            && self.constraints() == other.constraints()
            && self.trump == other.trump
            && self.called == other.called
    }

    pub fn new(me: Seat, hand: CardSet) -> Self {
        let mut possible = [!hand; 4];
        possible[me as usize] = CardSet::EMPTY;
        let mut certain = [CardSet::EMPTY; 4];
        certain[me as usize] = hand;
        Knowledge {
            me,
            possible,
            certain,
            played: CardSet::EMPTY,
            hand_len: [HAND_SIZE as u8; 4],
            constraints: [Constraint::default(); MAX_CONSTRAINTS],
            n_constraints: 0,
            trump: None,
            called: 0,
            trick: [0; 4],
            trick_len: 0,
            played_by: [CardSet::EMPTY; 4],
            discarded: [CardSet::EMPTY; 4],
            given: [Passed::Nothing; 4],
            received: [Passed::Nothing; 4],
            first_passer: None,
            rights: [PlayerTrumpPossibilities::Own; 4],
            last_bidder: None,
            value: NO_BID.0,
            trick_leader: 0,
            trick_winners: [0; TRICKS as usize],
            tricks_done: 0,
            card_points: [0; 2],
            pair_points: [0; 2],
            leads: [0; TRICKS as usize],
        }
    }

    pub fn me(&self) -> Seat {
        self.me
    }

    /// Cards `seat` may hold but is not known to hold.
    pub fn possible(&self, seat: Seat) -> CardSet {
        self.possible[seat as usize]
    }

    /// Cards `seat` certainly holds.
    pub fn certain(&self, seat: Seat) -> CardSet {
        self.certain[seat as usize]
    }

    /// Every card `seat` could hold.
    pub fn may_hold(&self, seat: Seat) -> CardSet {
        self.possible[seat as usize] | self.certain[seat as usize]
    }

    pub fn played(&self) -> CardSet {
        self.played
    }

    pub fn hand_len(&self, seat: Seat) -> u8 {
        self.hand_len[seat as usize]
    }

    pub fn trump(&self) -> Option<Suit> {
        self.trump
    }

    pub fn is_called(&self, suit: Suit) -> bool {
        self.called & suit_bit(suit) != 0
    }

    pub fn is_void(&self, seat: Seat, suit: Suit) -> bool {
        (self.may_hold(seat) & CardSet::suit(suit)).is_empty()
    }

    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints[..self.n_constraints as usize]
    }

    /// The lead of every trick started so far, in order.
    pub fn leads(&self) -> impl Iterator<Item = Lead> + '_ {
        let started = self.tricks_done as usize + (self.trick_len > 0) as usize;
        self.leads[..started].iter().map(|&x| Lead::unpack(x))
    }

    /// Bit indices of the cards in the current trick, in play order.
    pub fn trick(&self) -> &[u8] {
        &self.trick[..self.trick_len as usize]
    }

    /// The deals consistent with this knowledge, for counting, marginals and sampling.
    pub fn deals(&self) -> DealSpace {
        DealSpace::new(self)
    }

    /// A private-hand hypothesis used only by grouped counting. Never changes
    /// the observed knowledge or treats the hypothesis as a new observation.
    pub(crate) fn with_current_hand(&self, seat: Seat, hand: CardSet) -> Option<Self> {
        if seat >= 4 || hand.len() != self.hand_len(seat) as u32 || !self.certain(seat).is_subset(hand) {
            return None;
        }
        if !self.constraints().iter().filter(|c| c.seat == seat).all(|c| {
            let n = (hand & c.mask).len() as u8;
            c.lo <= n && n <= c.hi
        }) {
            return None;
        }
        let mut conditioned = *self;
        conditioned.make_certain(seat, hand).ok()?;
        conditioned.possible[seat as usize] = CardSet::EMPTY;
        Some(conditioned)
    }

    pub fn apply(&mut self, event: &SeatEvent) -> Result<(), Contradiction> {
        match *event {
            SeatEvent::Bid { seat, value } => {
                self.last_bidder = Some(seat);
                self.value = value;
                return Ok(());
            }
            SeatEvent::AskPair { seat } => {
                self.ask(seat, QuestionType::Yours);
                return Ok(());
            }
            SeatEvent::AskHalf { seat, suit } => {
                self.ask(seat, QuestionType::YourHalf(suit));
                return Ok(());
            }
            SeatEvent::StopBidding { .. } | SeatEvent::Undone => return Ok(()),
            SeatEvent::Pass { from, to, cards } => self.pass(from, to, cards)?,
            SeatEvent::CardPlayed { seat, card } => self.play(seat, card)?,
            SeatEvent::AnnounceTrump { seat, suit } | SeatEvent::YesPair { seat, suit } => {
                self.make_certain(seat, CardSet::halves(suit) - self.played)?;
                self.meld(seat, suit);
                self.set_trump(suit);
            }
            SeatEvent::NoPair { seat } => {
                for suit in SUITS {
                    let live = CardSet::halves(suit) - self.played;
                    if !self.is_called(suit) && live.len() == 2 {
                        self.add_constraint(Constraint {
                            seat,
                            mask: live,
                            lo: 0,
                            hi: 1,
                        });
                    }
                }
            }
            SeatEvent::NoHalf { seat, suit } => self.exclude(seat, CardSet::halves(suit))?,
            SeatEvent::YesHalf {
                seat,
                suit,
                asker_has_half,
            } => {
                let asker = partner(seat);
                let live = CardSet::halves(suit) - self.played;
                if asker_has_half {
                    self.add_constraint(Constraint {
                        seat,
                        mask: live,
                        lo: 1,
                        hi: 1,
                    });
                    self.add_constraint(Constraint {
                        seat: asker,
                        mask: live,
                        lo: 1,
                        hi: 1,
                    });
                    self.exclude(next_seat(seat), live)?;
                    self.exclude(partner(next_seat(seat)), live)?;
                    if !self.is_called(suit) {
                        self.meld(seat, suit);
                        self.set_trump(suit);
                    }
                } else {
                    self.add_constraint(Constraint {
                        seat,
                        mask: live,
                        lo: 1,
                        hi: 2,
                    });
                    self.exclude(asker, live)?;
                }
            }
        }
        self.propagate()
    }

    /// K6: the asker's right after its question (the engine's `after_question`).
    fn ask(&mut self, seat: Seat, question: QuestionType) {
        let r = &mut self.rights[seat as usize];
        *r = r.after_question(question);
    }

    /// K6: the pair points (the engine's `trump_points`) for the team of the seat whose action
    /// made `suit` trump.
    fn meld(&mut self, seat: Seat, suit: Suit) {
        self.pair_points[(seat % 2) as usize] += trump_points(suit, self.is_called(suit));
    }

    fn set_trump(&mut self, suit: Suit) {
        self.trump = Some(suit);
        self.called |= suit_bit(suit);
    }

    fn add_constraint(&mut self, c: Constraint) {
        // Dropping a constraint loses information but stays sound.
        if (self.n_constraints as usize) < MAX_CONSTRAINTS {
            self.constraints[self.n_constraints as usize] = c;
            self.n_constraints += 1;
        }
    }

    fn drop_constraints_of(&mut self, seat: Seat) {
        let mut i = 0;
        while i < self.n_constraints as usize {
            if self.constraints[i].seat == seat {
                self.remove_constraint(i);
            } else {
                i += 1;
            }
        }
    }

    fn remove_constraint(&mut self, i: usize) {
        self.n_constraints -= 1;
        self.constraints[i] = self.constraints[self.n_constraints as usize];
    }

    /// `seat` holds none of `cards`.
    fn exclude(&mut self, seat: Seat, cards: CardSet) -> Result<(), Contradiction> {
        if !(self.certain[seat as usize] & cards).is_empty() {
            return Err(contradiction(seat, "excluded a certain card"));
        }
        self.possible[seat as usize] -= cards;
        Ok(())
    }

    /// `seat` holds all of `cards`.
    fn make_certain(&mut self, seat: Seat, cards: CardSet) -> Result<(), Contradiction> {
        if !cards.is_subset(self.may_hold(seat)) {
            return Err(contradiction(seat, "certain card not possible"));
        }
        for x in 0..4 {
            self.possible[x] -= cards;
        }
        self.certain[seat as usize] |= cards;
        Ok(())
    }

    fn pass(&mut self, from: Seat, to: Seat, cards: Option<CardSet>) -> Result<(), Contradiction> {
        let (f, t) = (from as usize, to as usize);
        let seen = cards.map_or(Passed::Hidden, Passed::Seen);
        self.first_passer.get_or_insert(from);
        self.given[f] = seen;
        self.received[t] = seen;
        self.drop_constraints_of(from);
        self.drop_constraints_of(to);
        match cards {
            Some(cards) => {
                self.certain[f] -= cards;
                self.possible[f] -= cards;
                for x in 0..4 {
                    if x != t {
                        self.certain[x] -= cards;
                    }
                    self.possible[x] -= cards;
                }
                self.certain[t] |= cards;
            }
            None => {
                let merged = self.may_hold(from) | self.may_hold(to);
                self.possible[f] = merged;
                self.possible[t] = merged;
                self.certain[f] = CardSet::EMPTY;
                self.certain[t] = CardSet::EMPTY;
            }
        }
        if self.hand_len[f] < 4 {
            return Err(contradiction(from, "passed more cards than held"));
        }
        self.hand_len[f] -= 4;
        self.hand_len[t] += 4;
        Ok(())
    }

    fn play(&mut self, seat: Seat, played: u8) -> Result<(), Contradiction> {
        let s = seat as usize;
        let c = CardSet::single(played);
        if !c.is_subset(self.may_hold(seat)) {
            return Err(contradiction(seat, "played a card it cannot hold"));
        }
        let levels = play_levels(self.trick(), self.trump, is_first_trick(self.hand_len[s] as usize));
        self.exclude(seat, levels.excluded_by(played))?;

        for i in 0..self.n_constraints as usize {
            let con = &mut self.constraints[i];
            if con.mask.contains(played) {
                con.mask -= c;
                if con.seat == seat {
                    if con.hi == 0 {
                        return Err(contradiction(seat, "played a card it holds none of"));
                    }
                    con.hi -= 1;
                    con.lo = con.lo.saturating_sub(1);
                }
            }
        }
        for x in 0..4 {
            self.possible[x] -= c;
            self.certain[x] -= c;
        }
        self.played |= c;
        self.played_by[s] |= c;
        self.hand_len[s] -= 1;
        if let Some(&led) = self.trick().first() {
            let suit = suit_of(played);
            if suit != suit_of(led) && Some(suit) != self.trump {
                self.discarded[s] |= c;
            }
        }
        if self.trick_len == 0 {
            self.trick_leader = seat;
            self.leads[self.tricks_done as usize] = Lead {
                seat,
                card: played,
                trump: self.trump,
            }
            .pack();
        }
        self.trick[self.trick_len as usize] = played;
        self.trick_len = (self.trick_len + 1) % 4;
        if self.trick_len == 0 {
            self.finish_trick();
        }
        Ok(())
    }

    /// K6: the trick just completed goes to its winner (the engine's `winning_seat`), with its
    /// points (the engine's `trick_points`, last-trick bonus included).
    fn finish_trick(&mut self) {
        let w = winning_seat(self.trick_leader, &self.trick, self.trump);
        self.card_points[(w % 2) as usize] += trick_points(self.trick, self.tricks_done);
        self.trick_winners[self.tricks_done as usize] = w;
        self.tricks_done += 1;
    }

    /// Applies constraints, hand-size counting and card uniqueness until nothing changes.
    fn propagate(&mut self) -> Result<(), Contradiction> {
        loop {
            let mut changed = false;

            let mut i = 0;
            while i < self.n_constraints as usize {
                let Constraint { seat, mask, lo, hi } = self.constraints[i];
                let s = seat as usize;
                let k = (self.certain[s] & mask).len() as u8;
                let open = self.possible[s] & mask;
                let p = open.len() as u8;
                if k > hi || k + p < lo {
                    return Err(contradiction(seat, "constraint violated"));
                }
                if p > 0 && k == hi {
                    self.possible[s] -= mask;
                    changed = true;
                } else if p > 0 && k + p == lo {
                    self.make_certain(seat, open)?;
                    changed = true;
                } else if p > 0 && !(lo == 0 && k + p <= hi) {
                    i += 1;
                    continue;
                }
                self.remove_constraint(i);
            }

            for x in 0..4u8 {
                if x == self.me {
                    continue;
                }
                let s = x as usize;
                let known = self.certain[s].len() as u8;
                let open = self.possible[s];
                let reachable = known + open.len() as u8;
                if known > self.hand_len[s] || reachable < self.hand_len[s] {
                    return Err(contradiction(x, "hand size impossible"));
                }
                if open.is_empty() {
                    continue;
                }
                if known == self.hand_len[s] {
                    self.possible[s] = CardSet::EMPTY;
                    changed = true;
                } else if reachable == self.hand_len[s] {
                    self.make_certain(x, open)?;
                    changed = true;
                }
            }

            let all_certain = self.certain.iter().fold(CardSet::EMPTY, |a, &b| a | b);
            let all_possible = self.possible.iter().fold(CardSet::EMPTY, |a, &b| a | b);
            if !(!(self.played | all_certain) - all_possible).is_empty() {
                return Err(contradiction(self.me, "a live card has no holder"));
            }
            for x in 0..4u8 {
                let others = (0..4)
                    .filter(|&y| y != x as usize)
                    .fold(CardSet::EMPTY, |a, y| a | self.possible[y]);
                let only_here = self.possible[x as usize] - others - all_certain;
                if !only_here.is_empty() {
                    self.make_certain(x, only_here)?;
                    changed = true;
                }
            }

            if !changed {
                return Ok(());
            }
        }
    }
}

/// `Knowledge` plus the engine's one-step undo, and the lazy exact layer beside it.
/// The cache is a `Cell`, so a `Tracker` is `Send` but not `Sync`: share it across threads
/// by cloning, or share the `Knowledge` (`Copy`).
#[derive(Clone, Debug)]
pub struct Tracker {
    now: Knowledge,
    before_last_undoable: Option<Knowledge>,
    /// The fact epoch (F12): a fresh value whenever the facts change (`same_facts`) or an undo
    /// happens; bids and questions leave it alone. Never reused, so it keys the caches.
    fact_epoch: u32,
    next_epoch: u32,
    exact: Cell<Option<(u32, ExactSets)>>,
}

impl Tracker {
    pub fn new(me: Seat, hand: CardSet) -> Self {
        Tracker {
            now: Knowledge::new(me, hand),
            before_last_undoable: None,
            fact_epoch: 0,
            next_epoch: 1,
            exact: Cell::new(None),
        }
    }

    /// A tracker of the public facts (`Knowledge::public`).
    pub fn public() -> Self {
        Tracker {
            now: Knowledge::public(),
            ..Tracker::new(0, CardSet::EMPTY)
        }
    }

    /// The fact epoch: changes exactly when the facts do (and on undo), never repeats.
    pub fn fact_epoch(&self) -> u32 {
        self.fact_epoch
    }

    pub fn knowledge(&self) -> &Knowledge {
        &self.now
    }

    /// `DealSpace::exact_sets` of the current knowledge, computed on the first call after the
    /// facts change and cached until they change again (keyed on the fact epoch, F12).
    pub fn exact_sets(&self) -> ExactSets {
        if let Some((epoch, sets)) = self.exact.get() {
            if epoch == self.fact_epoch {
                return sets;
            }
        }
        let sets = self.now.deals().exact_sets();
        self.exact.set(Some((self.fact_epoch, sets)));
        sets
    }

    pub fn apply(&mut self, event: &SeatEvent) -> Result<(), Contradiction> {
        let before = self.now;
        let result = self.apply_event(event);
        if *event == SeatEvent::Undone || !self.now.same_facts(&before) {
            self.fact_epoch = self.next_epoch;
            self.next_epoch += 1;
        }
        result
    }

    fn apply_event(&mut self, event: &SeatEvent) -> Result<(), Contradiction> {
        match event {
            SeatEvent::Undone => {
                self.now = self
                    .before_last_undoable
                    .take()
                    .ok_or(contradiction(self.now.me, "undo without an undoable event"))?;
            }
            SeatEvent::Bid { .. } | SeatEvent::StopBidding { .. } | SeatEvent::CardPlayed { .. } => {
                let before = self.now;
                self.now.apply(event)?;
                self.before_last_undoable = Some(before);
            }
            _ => {
                self.now.apply(event)?;
                self.before_last_undoable = None;
            }
        }
        Ok(())
    }
}

//! `engine::fast`: a `Copy` card-play state for search and simulation (documented low-level
//! API, see docs/design/fast-core.md).
//!
//! It covers the play of the cards and the leader's trump announcement at a trick start.
//! Bidding, passing, questions and answers stay in `Game` (questions carry information and are
//! handled at the belief level). The rules come from their owners: legal cards from
//! `bits::play_levels`, the trick winner from `bits::trick_winner`, card and pair values from
//! `game::points`. tests/fast_vs_game.rs checks every card-play state of random games against
//! `Game`.
//!
//! Moves are applied in place; undo by keeping a copy (`let saved = state;` … `state = saved;`).
//! The state is 56 bytes, and copying it measured faster than make/unmake.

use crate::bits::{self, CardSet, SUITS};
use crate::game::cards::Suit;
use crate::game::errors::GameError;
use crate::game::gameevent::{ActionType, GameCallback};
use crate::game::gamestate::GamePhase;
use crate::game::player::PlayerTrumpPossibilities;
use crate::game::points::{CARD_POINTS, LAST_TRICK_BONUS, PAIR_POINTS};
use crate::game::Game;

/// Tricks in a game.
pub const TRICKS: u8 = 9;
const NO_TRUMP: u8 = 4;

/// A move in card play.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Move {
    /// Play the card with this bit index (`bits::index`).
    Card(u8),
    /// The leader announces a pair at a trick start; the suit becomes trump.
    Announce(Suit),
}

/// The card-play state. Seats are 0–3 clockwise; team = seat % 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FastState {
    hands: [CardSet; 4],
    /// Cards of the current trick in play order; `trick[..trick_len]` is valid, the rest is 0.
    trick: [u8; 4],
    trick_len: u8,
    leader: u8,
    /// Trump suit as `Suit as u8`, or `NO_TRUMP`.
    trump: u8,
    /// Suits announced so far (bit `Suit as u8`); each suit can become trump once.
    called: u8,
    /// Seats that may still announce their own pair (bit = seat; `PlayerTrumpPossibilities::Own`).
    may_announce: u8,
    /// A trick start where the leader may still announce (the `Game` phase `StartTrick`).
    announce_open: bool,
    tricks_done: u8,
    tricks: [u8; 2],
    card_points: [i16; 2],
    pair_points: [i16; 2],
}

/// All fields of a `FastState`, for building one from a sampled world (search) or reading it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FastParts {
    pub hands: [CardSet; 4],
    /// Cards already in the current trick, in play order (at most 3).
    pub trick: [u8; 4],
    pub trick_len: u8,
    pub leader: u8,
    pub trump: Option<Suit>,
    /// Suits announced so far, bit `Suit as u8`.
    pub called: u8,
    /// Seats whose trump right is still `Own`, bit = seat.
    pub may_announce: u8,
    /// The leader may announce now (trick start, nothing asked or announced yet).
    pub announce_open: bool,
    pub tricks_done: u8,
    pub tricks: [u8; 2],
    /// Card points per team, including the last-trick bonus.
    pub card_points: [i16; 2],
    /// Announced pair points per team.
    pub pair_points: [i16; 2],
}

impl FastState {
    /// The start of card play: `leader` leads the first trick, no trump, every seat may announce.
    pub fn new(hands: [CardSet; 4], leader: u8) -> FastState {
        FastState {
            hands,
            trick: [0; 4],
            trick_len: 0,
            leader: leader % 4,
            trump: NO_TRUMP,
            called: 0,
            may_announce: 0b1111,
            announce_open: false,
            tricks_done: 0,
            tricks: [0; 2],
            card_points: [0; 2],
            pair_points: [0; 2],
        }
    }

    /// Builds a state from its parts; checks that the cards are disjoint and the hand sizes
    /// fit the trick.
    pub fn from_parts(p: FastParts) -> Result<FastState, GameError> {
        let n = p.trick_len as usize;
        let mut ok = n < 4
            && p.leader < 4
            && (p.tricks_done < TRICKS || p.tricks_done == TRICKS && n == 0)
            && p.tricks[0] as u32 + p.tricks[1] as u32 == p.tricks_done as u32
            && p.trick[..n.min(4)].iter().all(|&c| c < 36);
        if !ok {
            return Err(GameError::InvalidState);
        }
        let in_trick = p.trick[..n].iter().fold(CardSet::EMPTY, |s, &c| s | CardSet::single(c));
        ok &= in_trick.len() == n as u32;
        let mut seen = in_trick;
        let left = (TRICKS - p.tricks_done) as u32;
        for seat in 0..4u8 {
            let hand = p.hands[seat as usize];
            ok &= (hand & seen).is_empty() && hand.is_subset(CardSet::ALL);
            seen |= hand;
            let has_played = ((seat + 4 - p.leader) % 4) < p.trick_len;
            ok &= hand.len() + has_played as u32 == left;
        }
        if !ok {
            return Err(GameError::InvalidState);
        }
        let mut trick = [0; 4];
        trick[..n].copy_from_slice(&p.trick[..n]);
        Ok(FastState {
            hands: p.hands,
            trick,
            trick_len: p.trick_len,
            leader: p.leader,
            trump: p.trump.map_or(NO_TRUMP, |s| s as u8),
            called: p.called & 0b1111,
            may_announce: p.may_announce & 0b1111,
            announce_open: p.announce_open && p.trick_len == 0,
            tricks_done: p.tricks_done,
            tricks: p.tricks,
            card_points: p.card_points,
            pair_points: p.pair_points,
        })
    }

    pub fn parts(&self) -> FastParts {
        FastParts {
            hands: self.hands,
            trick: self.trick,
            trick_len: self.trick_len,
            leader: self.leader,
            trump: self.trump(),
            called: self.called,
            may_announce: self.may_announce,
            announce_open: self.announce_open,
            tricks_done: self.tricks_done,
            tricks: self.tricks,
            card_points: self.card_points,
            pair_points: self.pair_points,
        }
    }

    /// The card-play state of a `Game` (phases `Raising`, `Trick`, `StartTrick`, `Ended`);
    /// `None` in other phases. Pair points are the `NewTrump` callbacks of the events, credited
    /// to the team of the player who caused them (as in `GameFinishedInfo`).
    pub fn from_game(game: &Game) -> Option<FastState> {
        let s = &game.state;
        let announce_open = match s.phase {
            GamePhase::StartTrick => true,
            GamePhase::Raising | GamePhase::Trick | GamePhase::Ended => false,
            _ => return None,
        };
        let trick: &[_] = if s.current_trick.len() == 4 { &[] } else { &s.current_trick };
        let mut fast = FastState::new(
            [0, 1, 2, 3].map(|i| CardSet::from_cards(&s.players[i].cards)),
            (s.player_at_turn.0 + 4 - trick.len() as u8) % 4,
        );
        for (slot, c) in fast.trick.iter_mut().zip(trick) {
            *slot = bits::index(c);
        }
        fast.trick_len = trick.len() as u8;
        fast.trump = s.trump.map_or(NO_TRUMP, |t| t as u8);
        fast.called = s.trump_called.iter().fold(0, |b, &t| b | 1 << t as u8);
        fast.may_announce = (0..4u8)
            .filter(|&i| s.players[i as usize].trump == PlayerTrumpPossibilities::Own)
            .fold(0, |b, i| b | 1 << i);
        fast.announce_open = announce_open;
        fast.tricks_done = s.all_tricks.len() as u8;
        for t in &s.all_tricks {
            let team = (t.winner.0 % 2) as usize;
            fast.tricks[team] += 1;
            fast.card_points[team] += t.points.0 as i16;
        }
        for e in &game.all_events {
            if let Some(GameCallback::NewTrump(suit)) = e.callback {
                fast.pair_points[(e.last_action.player.0 % 2) as usize] += PAIR_POINTS[suit as usize] as i16;
            }
        }
        Some(fast)
    }

    #[inline]
    pub fn hand(&self, seat: u8) -> CardSet {
        self.hands[seat as usize]
    }

    #[inline]
    pub fn hands(&self) -> [CardSet; 4] {
        self.hands
    }

    /// The cards of the current trick, in play order.
    #[inline]
    pub fn trick(&self) -> &[u8] {
        &self.trick[..self.trick_len as usize]
    }

    #[inline]
    pub fn leader(&self) -> u8 {
        self.leader
    }

    #[inline]
    pub fn to_move(&self) -> u8 {
        (self.leader + self.trick_len) & 3
    }

    #[inline]
    pub fn trump(&self) -> Option<Suit> {
        SUITS.get(self.trump as usize).copied()
    }

    /// Suits announced so far, bit `Suit as u8`.
    #[inline]
    pub fn called(&self) -> u8 {
        self.called
    }

    /// Seats that may still announce their own pair, bit = seat.
    #[inline]
    pub fn may_announce(&self) -> u8 {
        self.may_announce
    }

    #[inline]
    pub fn tricks_done(&self) -> u8 {
        self.tricks_done
    }

    #[inline]
    pub fn tricks_won(&self, team: u8) -> u8 {
        self.tricks[team as usize & 1]
    }

    /// Card points of a team so far, including the last-trick bonus.
    #[inline]
    pub fn card_points(&self, team: u8) -> i32 {
        self.card_points[team as usize & 1] as i32
    }

    /// Announced pair points of a team so far.
    #[inline]
    pub fn pair_points(&self, team: u8) -> i32 {
        self.pair_points[team as usize & 1] as i32
    }

    /// Card points plus pair points.
    #[inline]
    pub fn points(&self, team: u8) -> i32 {
        self.card_points(team) + self.pair_points(team)
    }

    /// Cards still in the hands.
    #[inline]
    pub fn in_hands(&self) -> CardSet {
        self.hands[0] | self.hands[1] | self.hands[2] | self.hands[3]
    }

    /// All cards have been played.
    #[inline]
    pub fn is_over(&self) -> bool {
        self.in_hands().is_empty()
    }

    /// The seat currently winning the trick, if a card has been played.
    #[inline]
    pub fn trick_winner(&self) -> Option<u8> {
        (self.trick_len > 0).then(|| (self.leader + bits::trick_winner(self.trick(), self.trump()) as u8) & 3)
    }

    /// The cards the seat to move may play.
    #[inline]
    pub fn legal_cards(&self) -> CardSet {
        let first_trick = self.tricks_done == 0;
        bits::play_levels(self.trick(), self.trump(), first_trick).allowed(self.hands[self.to_move() as usize])
    }

    /// Suits the leader may announce now (bit `Suit as u8`): an own pair (King and Ober in
    /// hand), not announced before, at a trick start, while the leader's trump right is `Own`.
    #[inline]
    pub fn announcements(&self) -> u8 {
        if !self.announce_open || self.may_announce >> self.leader & 1 == 0 {
            return 0;
        }
        let hand = self.hands[self.leader as usize];
        let mut suits = 0;
        for s in SUITS {
            if CardSet::halves(s).is_subset(hand) {
                suits |= 1 << s as u8;
            }
        }
        suits & !self.called
    }

    /// The legal moves: announcements first (in suit order), then cards (ascending index).
    pub fn moves(&self) -> Moves {
        Moves {
            announce: self.announcements(),
            cards: self.legal_cards().0,
        }
    }

    /// Plays `card` for the seat to move. The card must be legal (checked in debug builds).
    #[inline]
    pub fn play(&mut self, card: u8) {
        debug_assert!(self.legal_cards().contains(card), "illegal card {card} in {self:?}");
        let seat = self.to_move() as usize;
        self.hands[seat] -= CardSet::single(card);
        self.trick[self.trick_len as usize] = card;
        self.trick_len += 1;
        self.announce_open = false;
        if self.trick_len == 4 {
            self.finish_trick();
        }
    }

    #[inline]
    fn finish_trick(&mut self) {
        let winner = (self.leader + bits::trick_winner(&self.trick, self.trump()) as u8) & 3;
        let team = (winner & 1) as usize;
        let t = self.trick;
        let mut points = CARD_POINTS[t[0] as usize] as i16
            + CARD_POINTS[t[1] as usize] as i16
            + CARD_POINTS[t[2] as usize] as i16
            + CARD_POINTS[t[3] as usize] as i16;
        if self.tricks_done == TRICKS - 1 {
            points += LAST_TRICK_BONUS as i16;
        }
        self.card_points[team] += points;
        self.tricks[team] += 1;
        self.tricks_done += 1;
        self.leader = winner;
        self.trick = [0; 4]; // canonical: equal states compare and hash equal
        self.trick_len = 0;
        self.announce_open = self.tricks_done < TRICKS;
    }

    /// The leader announces the pair of `suit`; it becomes trump. Must be in `announcements()`
    /// (checked in debug builds).
    #[inline]
    pub fn announce(&mut self, suit: Suit) {
        debug_assert!(self.announcements() >> suit as u8 & 1 == 1, "illegal announcement {suit:?}");
        self.trump = suit as u8;
        self.called |= 1 << suit as u8;
        self.pair_points[(self.leader & 1) as usize] += PAIR_POINTS[suit as usize] as i16;
        self.announce_open = false;
    }

    #[inline]
    pub fn apply(&mut self, m: Move) {
        match m {
            Move::Card(c) => self.play(c),
            Move::Announce(s) => self.announce(s),
        }
    }

    /// A copy with `m` applied.
    #[inline]
    pub fn after(mut self, m: Move) -> FastState {
        self.apply(m);
        self
    }
}

/// Iterator over the legal moves of a state (no allocation).
#[derive(Clone, Copy, Debug)]
pub struct Moves {
    announce: u8,
    cards: u64,
}

impl Iterator for Moves {
    type Item = Move;

    #[inline]
    fn next(&mut self) -> Option<Move> {
        if self.announce != 0 {
            let s = self.announce.trailing_zeros() as usize;
            self.announce &= self.announce - 1;
            return Some(Move::Announce(SUITS[s]));
        }
        if self.cards != 0 {
            let c = self.cards.trailing_zeros() as u8;
            self.cards &= self.cards - 1;
            return Some(Move::Card(c));
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = (self.announce.count_ones() + self.cards.count_ones()) as usize;
        (n, Some(n))
    }
}

impl ExactSizeIterator for Moves {}

/// The `Game` action for a move by `seat`.
pub fn to_action(m: Move, seat: u8) -> crate::game::gameevent::GameAction {
    crate::game::gameevent::GameAction {
        action_type: match m {
            Move::Card(c) => ActionType::CardPlayed(bits::card(c)),
            Move::Announce(s) => ActionType::AnnounceTrump(s),
        },
        player: crate::game::player::PlaceAtTable(seat),
    }
}

/// The move of a `Game` card-play action, if it is one.
pub fn from_action(action: &ActionType) -> Option<Move> {
    match action {
        ActionType::CardPlayed(c) => Some(Move::Card(bits::index(c))),
        ActionType::AnnounceTrump(s) => Some(Move::Announce(*s)),
        _ => None,
    }
}

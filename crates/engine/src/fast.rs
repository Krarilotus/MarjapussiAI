//! `engine::fast`: a `Copy` card-play state for search and simulation (documented low-level
//! API, see docs/design/fast-core.md).
//!
//! It covers cards, announcements, questions and answers. Bidding and passing stay in `Game`.
//! The rules come from their owners: legal cards from
//! `bits::play_levels`, the trick winner from `bits::trick_winner`, card and pair values from
//! `game::points`. tests/fast_vs_game.rs checks every card-play state of random games against
//! `Game`.
//!
//! Moves are applied in place; undo by keeping a copy (`let saved = state;` … `state = saved;`).
//! Copying the compact state avoids a separate undo representation.

use crate::bits::{self, CardSet, SUITS};
use crate::game::cards::{Card, Suit, CARDS};
use crate::game::errors::GameError;
use crate::game::gameevent::{ActionType, AnswerType, GameAction, QuestionType};
use crate::game::gamestate::GamePhase;
use crate::game::player::{PlaceAtTable, PlayerTrumpPossibilities, TRICKS};
use crate::game::rules::{answer_trump, question_answers, trick_points, trump_points, uncalled_pairs};
use crate::game::scoring;
use crate::game::table::Game;

const NO_TRUMP: u8 = 4;

/// A move in card play.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Move {
    /// Play the card with this bit index (`Card::index`).
    Card(u8),
    /// The leader announces a pair at a trick start; the suit becomes trump.
    Announce(Suit),
    Question(QuestionType),
    Answer(AnswerType),
}

/// Public play phase. During an answer the leader waits for its opposite seat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlayPhase {
    Cards,
    Start,
    Answer(QuestionType),
}

impl PlayPhase {
    /// Recover the play phase from the deciding seat's legal actions, without hidden cards.
    pub fn from_actions(actions: &[GameAction]) -> Self {
        for action in actions {
            match action.action_type {
                ActionType::Answer(AnswerType::NoPair | AnswerType::YesPair(_)) => {
                    return Self::Answer(QuestionType::Yours);
                }
                ActionType::Answer(AnswerType::NoHalf(s) | AnswerType::YesHalf(s)) => {
                    return Self::Answer(QuestionType::YourHalf(s));
                }
                ActionType::Question(_) | ActionType::AnnounceTrump(_) => return Self::Start,
                _ => {}
            }
        }
        Self::Cards
    }

    /// Stable compact code for search keys.
    pub const fn code(self) -> u8 {
        match self {
            Self::Cards => 0,
            Self::Start => 1,
            Self::Answer(QuestionType::Yours) => 2,
            Self::Answer(QuestionType::YourHalf(s)) => 3 + s as u8,
        }
    }
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
    /// Suits already called (bit `Suit as u8`); none can become trump again.
    called: u8,
    /// Two bits per seat: Own, Yours, Ours.
    rights: u8,
    phase: PlayPhase,
    tricks_done: u8,
    tricks: [u8; 2],
    card_points: [i16; 2],
    pair_points: [i16; 2],
}

/// The public part of a card-play state: everything except the hands, i.e. what every seat
/// sees. A sampled world adds hands (`FastState::from_public`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PublicState {
    /// Cards already in the current trick, in play order (at most 3); the rest is 0.
    pub trick: [u8; 4],
    pub trick_len: u8,
    pub leader: u8,
    pub trump: Option<Suit>,
    /// Suits announced so far, bit `Suit as u8`.
    pub called: u8,
    pub rights: [PlayerTrumpPossibilities; 4],
    pub phase: PlayPhase,
    pub tricks_done: u8,
    pub tricks: [u8; 2],
    /// Card points per team, including the last-trick bonus.
    pub card_points: [i16; 2],
    /// Announced pair points per team.
    pub pair_points: [i16; 2],
}

impl PublicState {
    /// The seat to move.
    #[inline]
    pub fn to_move(&self) -> u8 {
        if matches!(self.phase, PlayPhase::Answer(_)) {
            bits::partner(self.leader)
        } else {
            seat_to_move(self.leader, self.trick_len)
        }
    }

    /// The cards of the current trick, in play order.
    pub fn trick(&self) -> &[u8] {
        &self.trick[..self.trick_len.min(3) as usize]
    }
}

/// The one formula for the seat to move.
#[inline]
const fn seat_to_move(leader: u8, trick_len: u8) -> u8 {
    (leader + trick_len) & 3
}

/// All of a `FastState`: the hands and the public part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FastParts {
    pub hands: [CardSet; 4],
    pub public: PublicState,
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
            rights: 0,
            phase: PlayPhase::Cards,
            tricks_done: 0,
            tricks: [0; 2],
            card_points: [0; 2],
            pair_points: [0; 2],
        }
    }

    /// Builds a state from its parts; checks that the cards are disjoint and the hand sizes
    /// fit the trick.
    pub fn from_parts(parts: FastParts) -> Result<FastState, GameError> {
        FastState::from_public(parts.public, parts.hands)
    }

    /// A world: the public state with these hands (checked like `from_parts`).
    pub fn from_public(p: PublicState, hands: [CardSet; 4]) -> Result<FastState, GameError> {
        let n = p.trick_len as usize;
        let mut ok = n < 4
            && p.leader < 4
            && (p.tricks_done < TRICKS || p.tricks_done == TRICKS && n == 0)
            && p.tricks[0] as u32 + p.tricks[1] as u32 == p.tricks_done as u32
            && p.trick[..n.min(4)].iter().all(|&c| (c as usize) < CARDS)
            && (p.phase == PlayPhase::Cards || n == 0 && p.tricks_done > 0 && p.tricks_done < TRICKS)
            && match p.phase {
                PlayPhase::Answer(QuestionType::Yours) => {
                    p.rights[p.leader as usize] == PlayerTrumpPossibilities::Yours
                }
                PlayPhase::Answer(QuestionType::YourHalf(_)) => {
                    p.rights[p.leader as usize] == PlayerTrumpPossibilities::Ours
                }
                _ => true,
            };
        if !ok {
            return Err(GameError::InvalidState);
        }
        let in_trick = p.trick[..n].iter().fold(CardSet::EMPTY, |s, &c| s | CardSet::single(c));
        ok &= in_trick.len() == n as u32;
        let mut seen = in_trick;
        let left = (TRICKS - p.tricks_done) as u32;
        for seat in 0..4u8 {
            let hand = hands[seat as usize];
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
            hands,
            trick,
            trick_len: p.trick_len,
            leader: p.leader,
            trump: p.trump.map_or(NO_TRUMP, |s| s as u8),
            called: p.called & 0b1111,
            rights: p
                .rights
                .iter()
                .enumerate()
                .fold(0, |bits, (seat, right)| bits | (*right as u8) << (2 * seat)),
            phase: p.phase,
            tricks_done: p.tricks_done,
            tricks: p.tricks,
            card_points: p.card_points,
            pair_points: p.pair_points,
        })
    }

    pub fn parts(&self) -> FastParts {
        FastParts {
            hands: self.hands,
            public: self.public(),
        }
    }

    /// Everything except the hands.
    pub fn public(&self) -> PublicState {
        PublicState {
            trick: self.trick,
            trick_len: self.trick_len,
            leader: self.leader,
            trump: self.trump(),
            called: self.called,
            rights: [0, 1, 2, 3].map(|seat| self.rights(seat)),
            phase: self.phase,
            tricks_done: self.tricks_done,
            tricks: self.tricks,
            card_points: self.card_points,
            pair_points: self.pair_points,
        }
    }

    /// The play state of a `Game`, including pending answers;
    /// `None` in other phases. Points and tricks per team come from `scoring::team_points`.
    pub fn from_game(game: &Game) -> Option<FastState> {
        let s = &game.state;
        let phase = match s.phase {
            GamePhase::StartTrick => PlayPhase::Start,
            GamePhase::AnsweringPair => PlayPhase::Answer(QuestionType::Yours),
            GamePhase::AnsweringHalf(suit) => PlayPhase::Answer(QuestionType::YourHalf(suit)),
            GamePhase::Raising | GamePhase::Trick | GamePhase::Ended => PlayPhase::Cards,
            _ => return None,
        };
        let trick: &[_] = if s.current_trick.len() == 4 {
            &[]
        } else {
            &s.current_trick
        };
        let mut fast = FastState::new(
            [0, 1, 2, 3].map(|i| CardSet::from_cards(&s.players[i].cards)),
            if matches!(phase, PlayPhase::Answer(_)) {
                bits::partner(s.player_at_turn.0)
            } else {
                (s.player_at_turn.0 + 4 - trick.len() as u8) % 4
            },
        );
        for (slot, c) in fast.trick.iter_mut().zip(trick) {
            *slot = c.index();
        }
        fast.trick_len = trick.len() as u8;
        fast.trump = s.trump.map_or(NO_TRUMP, |t| t as u8);
        fast.called = s.trump_called.iter().fold(0, |b, &t| b | 1 << t as u8);
        fast.rights = (0..4u8).fold(0, |bits, i| bits | (s.players[i as usize].trump as u8) << (2 * i));
        fast.phase = phase;
        fast.tricks_done = s.all_tricks.len() as u8;
        let points = scoring::team_points(&s.all_tricks, &game.all_events);
        fast.tricks = points.tricks;
        fast.card_points = points.cards.map(|p| p as i16);
        fast.pair_points = points.pairs.map(|p| p as i16);
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
        if matches!(self.phase, PlayPhase::Answer(_)) {
            bits::partner(self.leader)
        } else {
            seat_to_move(self.leader, self.trick_len)
        }
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

    #[inline]
    pub fn rights(&self, seat: u8) -> PlayerTrumpPossibilities {
        use PlayerTrumpPossibilities::*;
        [Own, Yours, Ours][((self.rights >> (2 * seat)) & 3) as usize]
    }

    #[inline]
    pub fn rights_code(&self) -> u8 {
        self.rights
    }

    #[inline]
    pub fn phase(&self) -> PlayPhase {
        self.phase
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
        (self.trick_len > 0).then(|| bits::winning_seat(self.leader, self.trick(), self.trump()))
    }

    /// The cards the seat to move may play.
    #[inline]
    pub fn legal_cards(&self) -> CardSet {
        if matches!(self.phase, PlayPhase::Answer(_)) {
            return CardSet::EMPTY;
        }
        let first_trick = self.tricks_done == 0;
        bits::play_levels(self.trick(), self.trump(), first_trick).allowed(self.hands[self.to_move() as usize])
    }

    /// Suits the leader may announce now (bit `Suit as u8`): an own pair (King and Ober in
    /// hand), not announced before, at a trick start, while the leader's trump right is `Own`.
    #[inline]
    pub fn announcements(&self) -> u8 {
        if self.phase != PlayPhase::Start || !self.rights(self.leader).may_announce() {
            return 0;
        }
        uncalled_pairs(self.hand(self.leader), self.called)
    }

    /// Legal announcements/questions/answers first, then cards. No hidden-hand pruning here.
    pub fn moves(&self) -> Moves {
        let mut codes = self.legal_cards().0 | (self.announcements() as u64) << Move::ANNOUNCE;
        match self.phase {
            PlayPhase::Start => {
                if self.rights(self.leader).may_ask_pair() {
                    codes |= 1 << Move::Question(QuestionType::Yours).code();
                }
                for suit in SUITS {
                    codes |= 1 << Move::Question(QuestionType::YourHalf(suit)).code();
                }
            }
            PlayPhase::Answer(question) => {
                for answer in question_answers(self.hand(self.to_move()), self.called, question) {
                    codes |= 1 << Move::Answer(answer).code();
                }
            }
            PlayPhase::Cards => {}
        }
        Moves { codes }
    }

    /// Plays `card` for the seat to move. The card must be legal (checked in debug builds).
    #[inline]
    pub fn play(&mut self, card: u8) {
        debug_assert!(self.legal_cards().contains(card), "illegal card {card} in {self:?}");
        let seat = self.to_move() as usize;
        self.hands[seat] -= CardSet::single(card);
        self.trick[self.trick_len as usize] = card;
        self.trick_len += 1;
        self.phase = PlayPhase::Cards;
        if self.trick_len == 4 {
            self.finish_trick();
        }
    }

    #[inline]
    fn finish_trick(&mut self) {
        let winner = bits::winning_seat(self.leader, &self.trick, self.trump());
        let team = (winner & 1) as usize;
        self.card_points[team] += trick_points(self.trick, self.tricks_done) as i16;
        self.tricks[team] += 1;
        self.tricks_done += 1;
        self.leader = winner;
        self.trick = [0; 4]; // canonical: equal states compare and hash equal
        self.trick_len = 0;
        self.phase = if self.tricks_done < TRICKS {
            PlayPhase::Start
        } else {
            PlayPhase::Cards
        };
    }

    /// The leader announces the pair of `suit`; it becomes trump. Must be in `announcements()`
    /// (checked in debug builds).
    #[inline]
    pub fn announce(&mut self, suit: Suit) {
        debug_assert!(
            self.announcements() >> suit as u8 & 1 == 1,
            "illegal announcement {suit:?}"
        );
        let already_called = self.called >> suit as u8 & 1 == 1;
        self.pair_points[(self.leader & 1) as usize] += trump_points(suit, already_called) as i16;
        self.trump = suit as u8;
        self.called |= 1 << suit as u8;
        self.phase = PlayPhase::Cards;
    }

    #[inline]
    fn question(&mut self, question: QuestionType) {
        debug_assert!(self.moves().any(|m| m == Move::Question(question)), "illegal question");
        let right = self.rights(self.leader).after_question(question) as u8;
        let shift = 2 * self.leader;
        self.rights = (self.rights & !(3 << shift)) | right << shift;
        self.phase = PlayPhase::Answer(question);
    }

    #[inline]
    fn answer(&mut self, answer: AnswerType) {
        debug_assert!(self.moves().any(|m| m == Move::Answer(answer)), "illegal answer");
        let already_called = match answer {
            AnswerType::YesPair(suit) | AnswerType::YesHalf(suit) => self.called >> suit as u8 & 1 != 0,
            _ => false,
        };
        if let Some(suit) = answer_trump(self.hand(self.leader), answer, already_called) {
            self.pair_points[(self.leader & 1) as usize] +=
                trump_points(suit, self.called >> suit as u8 & 1 != 0) as i16;
            self.trump = suit as u8;
            self.called |= 1 << suit as u8;
        }
        self.phase = PlayPhase::Cards;
    }

    #[inline]
    pub fn apply(&mut self, m: Move) {
        match m {
            Move::Card(c) => self.play(c),
            Move::Announce(s) => self.announce(s),
            Move::Question(q) => self.question(q),
            Move::Answer(a) => self.answer(a),
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
    codes: u64,
}

impl Iterator for Moves {
    type Item = Move;

    #[inline]
    fn next(&mut self) -> Option<Move> {
        if self.codes != 0 {
            let special = self.codes & !((1 << Move::ANNOUNCE) - 1);
            let code = if special != 0 {
                special.trailing_zeros()
            } else {
                self.codes.trailing_zeros()
            } as u8;
            self.codes &= !(1 << code);
            return Some(Move::from_code(code));
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.codes.count_ones() as usize;
        (n, Some(n))
    }
}

impl ExactSizeIterator for Moves {}

impl Move {
    /// Codes `0..ANNOUNCE` are cards (their bit index); `ANNOUNCE + suit` an announcement.
    pub const ANNOUNCE: u8 = CARDS as u8;
    /// Number of move codes.
    pub const CODES: usize = CARDS + 22;

    /// A compact code for tables and keys (`Move::from_code` inverts it).
    #[inline]
    pub const fn code(self) -> u8 {
        match self {
            Move::Card(c) => c,
            Move::Announce(s) => Move::ANNOUNCE + s as u8,
            Move::Question(QuestionType::Yours) => 40,
            Move::Question(QuestionType::YourHalf(s)) => 41 + s as u8,
            Move::Answer(AnswerType::NoPair) => 45,
            Move::Answer(AnswerType::YesPair(s)) => 46 + s as u8,
            Move::Answer(AnswerType::NoHalf(s)) => 50 + s as u8,
            Move::Answer(AnswerType::YesHalf(s)) => 54 + s as u8,
        }
    }

    #[inline]
    pub const fn from_code(code: u8) -> Move {
        match code {
            0..=35 => Move::Card(code),
            36..=39 => Move::Announce(SUITS[(code - 36) as usize]),
            40 => Move::Question(QuestionType::Yours),
            41..=44 => Move::Question(QuestionType::YourHalf(SUITS[(code - 41) as usize])),
            45 => Move::Answer(AnswerType::NoPair),
            46..=49 => Move::Answer(AnswerType::YesPair(SUITS[(code - 46) as usize])),
            50..=53 => Move::Answer(AnswerType::NoHalf(SUITS[(code - 50) as usize])),
            54..=57 => Move::Answer(AnswerType::YesHalf(SUITS[(code - 54) as usize])),
            _ => panic!("invalid move code"),
        }
    }

    /// The `Game` action for this move by `seat`.
    pub fn to_action(self, seat: u8) -> GameAction {
        GameAction {
            action_type: match self {
                Move::Card(c) => ActionType::CardPlayed(Card::from_index(c)),
                Move::Announce(s) => ActionType::AnnounceTrump(s),
                Move::Question(q) => ActionType::Question(q),
                Move::Answer(a) => ActionType::Answer(a),
            },
            player: PlaceAtTable(seat),
        }
    }

    /// The move of a `Game` card-play action, if it is one.
    pub fn from_action(action: &ActionType) -> Option<Move> {
        match action {
            ActionType::CardPlayed(c) => Some(Move::Card(c.index())),
            ActionType::AnnounceTrump(s) => Some(Move::Announce(*s)),
            ActionType::Question(q) => Some(Move::Question(*q)),
            ActionType::Answer(a) => Some(Move::Answer(*a)),
            _ => None,
        }
    }
}

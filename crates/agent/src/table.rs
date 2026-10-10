//! A game runner that keeps each seat's observations and legal actions in step.
//! Shared by live/evaluation tables and hypothetical policy rollouts; the policy
//! receives `SeatView`, never the simulated deal.

use crate::{SeatKind, SeatView, SeriesScore};
use engine::{ActionType, AnswerType, Card, CardSet, Clock, Game, GameAction, PlaceAtTable, QuestionType};
use knowledge::Hands;
use knowledge::Tracker;
use observation::{start_hand, Seat, SeatEvent, SeatFilter};

#[derive(Clone)]
struct SeatState {
    filter: SeatFilter,
    tracker: Tracker,
    events: Vec<SeatEvent>,
}

/// A game plus each seat's private view, kept in step event by event.
#[derive(Clone)]
pub struct Table {
    pub game: Game,
    seats: Vec<SeatState>,
    seen: usize,
    pub series: SeriesScore,
    /// Who plays each seat (public metadata, P3).
    pub kinds: [SeatKind; 4],
    /// The engine's legal actions now, refilled once per event (borrowed by `to_act`).
    legal: Vec<GameAction>,
}

impl Table {
    /// Reconstruct a hypothetical table from one seat's canonical history and
    /// remaining hands. Hidden transfers need a separate history hypothesis and
    /// are rejected here. Every transition is replayed through engine legality;
    /// the reconstructed deciding view must equal the supplied view exactly.
    pub fn from_view(view: &SeatView<'_>, mut hands: Hands) -> Option<Self> {
        let mut seen = CardSet::EMPTY;
        for (seat, hand) in hands.iter().enumerate() {
            if hand.len() != view.knowledge.hand_len(seat as u8) as u32 || !(seen & *hand).is_empty() {
                return None;
            }
            seen |= *hand;
        }
        if seen != CardSet::ALL - view.knowledge.played()
            || hands[view.seat as usize] != view.knowledge.certain(view.seat)
        {
            return None;
        }
        for event in view.events.iter().rev() {
            match *event {
                SeatEvent::CardPlayed { seat, card } => hands[seat as usize] |= CardSet::single(card),
                SeatEvent::Pass { from, to, cards } => {
                    let cards = cards?;
                    if !cards.is_subset(hands[to as usize]) {
                        return None;
                    }
                    hands[to as usize] -= cards;
                    hands[from as usize] |= cards;
                }
                SeatEvent::Undone => return None,
                _ => {}
            }
        }
        if hands.iter().any(|hand| hand.len() != engine::HAND_SIZE as u32) {
            return None;
        }
        let mut table = Self::new(&hands.map(|h| h.cards()));
        table.game.clock = Clock::Off;
        table.series = view.series;
        table.kinds = view.kinds;
        for event in view.events {
            let (seat, action_type) = match *event {
                SeatEvent::Bid { seat, value } => (seat, ActionType::NewBid(value)),
                SeatEvent::StopBidding { seat } => (seat, ActionType::StopBidding),
                SeatEvent::Pass {
                    from,
                    cards: Some(cards),
                    ..
                } => (from, ActionType::Pass(cards.cards().try_into().ok()?)),
                SeatEvent::CardPlayed { seat, card } => (seat, ActionType::CardPlayed(Card::from_index(card))),
                SeatEvent::AnnounceTrump { seat, suit } => (seat, ActionType::AnnounceTrump(suit)),
                SeatEvent::AskPair { seat } => (seat, ActionType::Question(QuestionType::Yours)),
                SeatEvent::AskHalf { seat, suit } => (seat, ActionType::Question(QuestionType::YourHalf(suit))),
                SeatEvent::YesPair { seat, suit } => (seat, ActionType::Answer(AnswerType::YesPair(suit))),
                SeatEvent::NoPair { seat } => (seat, ActionType::Answer(AnswerType::NoPair)),
                SeatEvent::YesHalf { seat, suit, .. } => (seat, ActionType::Answer(AnswerType::YesHalf(suit))),
                SeatEvent::NoHalf { seat, suit } => (seat, ActionType::Answer(AnswerType::NoHalf(suit))),
                _ => return None,
            };
            let action = *table.to_act()?.1.iter().find(|a| {
                a.player == PlaceAtTable(seat)
                    && match (a.action_type, action_type) {
                        (ActionType::Pass(a), ActionType::Pass(b)) => {
                            CardSet::from_cards(&a) == CardSet::from_cards(&b)
                        }
                        (a, b) => a == b,
                    }
            })?;
            table.apply(action);
        }
        let reconstructed = table.view(view.seat);
        (reconstructed.events == view.events && reconstructed.knowledge == view.knowledge).then_some(table)
    }

    pub fn new(hands: &[Vec<Card>; 4]) -> Table {
        let mut game = Game::new(
            "table".into(),
            ["S0", "S1", "S2", "S3"].map(String::from),
            Some(hands.clone()),
        );
        for _ in 0..4 {
            let start = game.legal_actions()[0];
            game.apply_action_mut(start);
        }
        let seats = (0..4u8)
            .map(|s| SeatState {
                filter: SeatFilter::new(s),
                tracker: Tracker::new(s, start_hand(&game, s)),
                events: vec![],
            })
            .collect();
        let mut table = Table {
            game,
            seats,
            seen: 0,
            series: SeriesScore::default(),
            kinds: [SeatKind::Unknown; 4],
            legal: Vec::with_capacity(64),
        };
        table.sync();
        table
    }

    fn sync(&mut self) {
        for event in &self.game.all_events[self.seen..] {
            for st in &mut self.seats {
                if let Some(e) = st.filter.filter(event) {
                    st.tracker
                        .apply(&e)
                        .unwrap_or_else(|c| panic!("knowledge contradiction {c:?}"));
                    st.events.push(e);
                }
            }
        }
        self.seen = self.game.all_events.len();
        self.game.legal_actions_into(&mut self.legal);
    }

    /// The seat to act and its legal actions, borrowed (no copy per call, Judge J13 P8). The
    /// engine lists only the acting seat's actions, plus at most one trailing undo request,
    /// which agents never use.
    pub fn to_act(&self) -> Option<(Seat, &[GameAction])> {
        if self.game.ended() {
            return None;
        }
        let seat = self.game.state.player_at_turn.0;
        let all = &self.legal[..];
        let legal = match all.last() {
            Some(a) if a.action_type == ActionType::UndoRequest => &all[..all.len() - 1],
            _ => all,
        };
        debug_assert!(legal.iter().all(|a| a.player.0 == seat));
        Some((seat, legal))
    }

    pub fn view(&self, seat: Seat) -> SeatView<'_> {
        let st = &self.seats[seat as usize];
        SeatView {
            seat,
            knowledge: st.tracker.knowledge(),
            events: &st.events,
            series: self.series,
            kinds: self.kinds,
        }
    }

    pub fn apply(&mut self, action: GameAction) {
        self.game.apply_action_mut(action);
        self.sync();
    }

    pub fn hand(&self, seat: Seat) -> CardSet {
        CardSet::from_cards(&self.game.state.players[seat as usize].cards)
    }
}

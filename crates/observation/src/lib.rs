//! The privacy boundary: turns engine events into what one seat may observe.
//!
//! Everything above this crate sees the game only through `SeatEvent`s and the seat's own
//! starting hand. Hidden passes carry no cards. Never read `Game.info.player_start_cards`
//! (all four starting hands).

use engine::Game;
use engine::Suit;
use engine::{partner, CardSet};
use engine::{ActionType, AnswerType, GameCallback, GameEvent, QuestionType};

pub type Seat = u8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeatEvent {
    Bid {
        seat: Seat,
        value: i32,
    },
    StopBidding {
        seat: Seat,
    },
    /// Cards passed to the partner; `None` for the two seats that do not see them.
    Pass {
        from: Seat,
        to: Seat,
        cards: Option<CardSet>,
    },
    CardPlayed {
        seat: Seat,
        card: u8,
    },
    AnnounceTrump {
        seat: Seat,
        suit: Suit,
    },
    AskPair {
        seat: Seat,
    },
    AskHalf {
        seat: Seat,
        suit: Suit,
    },
    YesPair {
        seat: Seat,
        suit: Suit,
    },
    /// No pair in any suit not called yet.
    NoPair {
        seat: Seat,
    },
    /// The answerer holds a half; `asker_has_half` is the public callback about the asker.
    /// `StillTrump` confirms that half even when this called suit cannot become trump again.
    YesHalf {
        seat: Seat,
        suit: Suit,
        asker_has_half: bool,
    },
    NoHalf {
        seat: Seat,
        suit: Suit,
    },
    /// The last bid, stop or card was taken back.
    Undone,
}

/// The seat's own starting hand, and nothing else from the deal.
pub fn start_hand(game: &Game, seat: Seat) -> CardSet {
    CardSet::from_cards(&game.info.player_start_cards[seat as usize])
}

/// Filters the engine's event stream for one seat. Keeps the undo bookkeeping, because
/// the engine reports an undo only as the second accept.
#[derive(Clone, Debug)]
pub struct SeatFilter {
    seat: Seat,
    undo_accepts: Vec<Seat>,
}

impl SeatFilter {
    pub fn new(seat: Seat) -> Self {
        SeatFilter {
            seat,
            undo_accepts: vec![],
        }
    }

    pub fn seat(&self) -> Seat {
        self.seat
    }

    pub fn filter(&mut self, event: &GameEvent) -> Option<SeatEvent> {
        let seat = event.last_action.player.0;
        let callback = event.callback.as_ref();
        Some(match &event.last_action.action_type {
            ActionType::Start | ActionType::UndoRequest => return None,
            ActionType::UndoDecline => {
                self.undo_accepts.clear();
                return None;
            }
            ActionType::UndoAccept => {
                if !self.undo_accepts.contains(&seat) {
                    self.undo_accepts.push(seat);
                }
                if self.undo_accepts.len() < 2 {
                    return None;
                }
                self.undo_accepts.clear();
                SeatEvent::Undone
            }
            ActionType::NewBid(value) => SeatEvent::Bid { seat, value: *value },
            ActionType::StopBidding => SeatEvent::StopBidding { seat },
            ActionType::Pass(cards) => {
                let to = partner(seat);
                let visible = self.seat == seat || self.seat == to;
                SeatEvent::Pass {
                    from: seat,
                    to,
                    cards: visible.then(|| CardSet::from_cards(cards)),
                }
            }
            ActionType::CardPlayed(card) => SeatEvent::CardPlayed {
                seat,
                card: card.index(),
            },
            ActionType::AnnounceTrump(suit) => SeatEvent::AnnounceTrump { seat, suit: *suit },
            ActionType::Question(QuestionType::Yours) => SeatEvent::AskPair { seat },
            ActionType::Question(QuestionType::YourHalf(suit)) => SeatEvent::AskHalf { seat, suit: *suit },
            ActionType::Answer(AnswerType::YesPair(suit)) => SeatEvent::YesPair { seat, suit: *suit },
            ActionType::Answer(AnswerType::NoPair) => SeatEvent::NoPair { seat },
            ActionType::Answer(AnswerType::NoHalf(suit)) => SeatEvent::NoHalf { seat, suit: *suit },
            ActionType::Answer(AnswerType::YesHalf(suit)) => SeatEvent::YesHalf {
                seat,
                suit: *suit,
                asker_has_half: matches!(callback, Some(GameCallback::NewTrump(_) | GameCallback::StillTrump(_))),
            },
        })
    }
}

/// The whole event stream of `game` as seen by `seat`.
pub fn seat_events(game: &Game, seat: Seat) -> Vec<SeatEvent> {
    let mut filter = SeatFilter::new(seat);
    game.all_events.iter().filter_map(|e| filter.filter(e)).collect()
}

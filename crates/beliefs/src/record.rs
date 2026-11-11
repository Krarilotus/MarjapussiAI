//! The signal record for convention reading: the auction, the passes, the melds and the
//! questions, in the order the seat saw them. Built from the `Mind`'s event list, whose undo
//! `Tracker` decides (one undo owner); the per-play context (who led what under which trump)
//! comes from `Knowledge::leads`, never from here.

use engine::CardSet;
use observation::{Seat, SeatEvent};

use crate::conventions::{walk_auction, Params, SeatKind, SeatSignals};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PassEvent {
    pub from: Seat,
    pub to: Seat,
    /// `None` if this seat did not see the cards.
    pub cards: Option<CardSet>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    /// Pre-exchange bidding actions: `(seat, Some(bid))` or `(seat, None)` for a pass.
    /// A post-return raise updates the public contract, but is not an auction message.
    pub auction: Vec<(Seat, Option<i32>)>,
    pub passes: Vec<PassEvent>,
    /// Per seat: its kind, its bids in their public context, its melds and questions.
    pub seats: [SeatSignals; 4],
}

impl Record {
    pub fn new(events: &[SeatEvent], kinds: [SeatKind; 4], params: &Params) -> Record {
        let mut r = Record::default();
        for (s, kind) in r.seats.iter_mut().zip(kinds) {
            s.kind = kind;
        }
        let mut auction_open = true;
        for e in events {
            match *e {
                SeatEvent::Bid { seat, value } if auction_open => r.auction.push((seat, Some(value))),
                SeatEvent::StopBidding { seat } if auction_open => r.auction.push((seat, None)),
                SeatEvent::Pass { from, to, cards } => {
                    // The first exchange publicly closes bidding, even for seats whose
                    // filtered event hides the cards. Mind retains subsequent raises;
                    // their contract/resource evidence belongs to a separate reader.
                    auction_open = false;
                    r.passes.push(PassEvent { from, to, cards });
                }
                SeatEvent::AnnounceTrump { seat, suit } | SeatEvent::YesPair { seat, suit } => {
                    r.seats[seat as usize].melds |= 1 << suit as u8
                }
                SeatEvent::AskPair { seat } => r.seats[seat as usize].asks.push(None),
                SeatEvent::AskHalf { seat, suit } => r.seats[seat as usize].asks.push(Some(suit)),
                SeatEvent::Bid { .. }
                | SeatEvent::StopBidding { .. }
                | SeatEvent::CardPlayed { .. }
                | SeatEvent::YesHalf { .. }
                | SeatEvent::NoPair { .. }
                | SeatEvent::NoHalf { .. }
                | SeatEvent::Undone => {}
            }
        }
        for obs in walk_auction(&r.auction, params) {
            r.seats[obs.ctx.speaker as usize].bids.push(obs);
        }
        r
    }

    /// KB `team_plan_formed` (batch 1b): the partner of `seat` has told content in the auction
    /// (an ace, halves or a pair), so together with my hand our likely game is clear.
    pub fn team_plan_formed(&self, seat: Seat) -> bool {
        self.seats[engine::partner(seat) as usize]
            .bids
            .iter()
            .any(|o| o.nominal.is_some())
    }

    /// The other team of `seat` has bid at least once.
    pub fn opponents_stepped(&self, seat: Seat) -> bool {
        self.auction.iter().any(|&(y, b)| y % 2 != seat % 2 && b.is_some())
    }

    /// `seat`'s bidding actions so far (bids and passes).
    pub fn calls(&self, seat: Seat) -> usize {
        self.auction.iter().filter(|&&(y, _)| y == seat).count()
    }
}

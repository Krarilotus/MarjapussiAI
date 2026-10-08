//! Bid conventions: messages and targets, likelihoods over the 162 hand classes,
//! and the optional historical HP1 bid/EGP model. Pass choice belongs to `heuristic`;
//! the active `Mind` does not infer intent from the choice of exchanged cards.

mod bids;
mod classes;
pub mod hp1;
mod params;

pub use agent::SeatKind;
pub use bids::{
    bid_likelihood, bid_table, bid_tables, class_posterior, confidence, follow_target, info_to_tell, message,
    next_context, nominal, walk_auction, BidContext, BidObs, Message, BARRIERS, MESSAGES, PAIR_LINE, PAIR_MESSAGES,
};
pub use classes::{
    class_features, class_prior, len_in, pair_value, ClassFeatures, HandClass, BIG_PAIRS, LUSCHE, N_CLASSES,
    SMALL_PAIRS,
};
pub use params::Params;

use engine::Suit;

/// The game value before the first bid (`engine::NO_BID` once this branch is on E7).
pub const NO_BID: i32 = 115;

/// What one seat revealed in public (C1): how it is read, its bidding actions with their
/// public context, its melds and its questions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeatSignals {
    /// Who plays the seat (public table metadata, P3).
    pub kind: SeatKind,
    pub bids: Vec<BidObs>,
    /// Pairs announced or shown by answering (suit bits).
    pub melds: u8,
    /// Questions: `None` = "Yours", `Some(s)` = a half of s.
    pub asks: Vec<Option<Suit>>,
}

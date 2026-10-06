//! Bidding numbers, named once: the value before any bid, the step and the maximum.

use std::iter::StepBy;
use std::ops::RangeInclusive;

use crate::game::points::Points;

/// The game value before any bid. It stays when all pass (the lowest bid is 120), so it also
/// marks a game without contract (`GameState::declarer` is then `None`).
pub const NO_BID: Points = Points(115);

/// Bids are multiples of this step.
pub const BID_STEP: i32 = 5;

/// The highest bid.
pub const MAX_BID: i32 = 420;

/// Whether `bid` is allowed over the current `value`: exactly the members of `bid_range(value)`,
/// in O(1) for the legality check (tests/rules.rs checks the equivalence exhaustively).
#[inline]
pub const fn is_bid(value: Points, bid: i32) -> bool {
    bid > value.0 && bid <= MAX_BID && (bid - value.0) % BID_STEP == 0
}

/// The bids allowed over the current value, lowest first (bid generation; `is_bid` is its
/// membership test).
pub fn bid_range(value: Points) -> StepBy<RangeInclusive<i32>> {
    (value.0 + BID_STEP..=MAX_BID).step_by(BID_STEP as usize)
}

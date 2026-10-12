//! Offline diagnostics for the tools; no decision uses them.

use engine::CardSet;

/// Both teams' final points when `hands` are played out from the first trick with `leader` on
/// lead by the outlook's level-0 actors, each seeing the hands.
pub fn playout_points(hands: [CardSet; 4], leader: u8) -> [i32; 2] {
    crate::outlook::open_playout(hands, leader)
}

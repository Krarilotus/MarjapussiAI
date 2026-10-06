//! The game rules around `Game`. `game.rs` only declares the modules; the crate root
//! re-exports the public API.

mod apply_action;
pub(crate) mod bidding;
pub(crate) mod cards;
pub(crate) mod clock;
pub(crate) mod errors;
pub(crate) mod gameevent;
pub(crate) mod gameinfo;
pub(crate) mod gamestate;
pub(crate) mod legal_actions;
pub(crate) mod meta;
pub(crate) mod parse;
pub(crate) mod player;
pub(crate) mod points;
pub(crate) mod scoring;
pub(crate) mod table;

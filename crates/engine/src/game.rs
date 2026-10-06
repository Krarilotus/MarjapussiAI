//! The game rules around `Game`. Module order is leaf first; `game.rs` only declares the
//! modules and re-exports the main types.

mod apply_action;
pub mod cards;
pub mod clock;
pub mod errors;
pub mod gameevent;
pub mod gameinfo;
pub mod gamestate;
pub mod legal_actions;
pub mod meta;
pub mod parse;
pub mod player;
pub mod points;
pub mod scoring;
pub mod series;
pub mod table;

pub use clock::{current_time_string, format_time, now_millis, Clock};
pub use table::Game;

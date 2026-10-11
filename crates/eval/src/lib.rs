//! Evaluation of any agent (C15): the owner of "how strong is it, and does it play like people".
//!
//! - `table`: runs four `agent::Agent`s on the engine, each seeing only its seat's view;
//! - `suite`: fixed deal suites (named seeds), so every A/B runs on the same deals;
//! - `duplicate`: duplicate deals with team (and optionally seat) rotation, the mean score
//!   difference per deal pair with a 95 % interval, and a sequential test that stops early;
//! - `gamestats`: the side-by-side statistics of any game source (self-play, human games);
//! - `meter`: the fair mistake meter (S7) over believable worlds, with the detectors;
//! - `stats`: small statistics (means, Wilson intervals, the confidence sequence).
//!
//! Scores always come from the engine (`GameFinishedInfo::scores`, i.e. `game::scoring`).

pub mod detectors;
pub mod duplicate;
pub mod gamestats;
pub mod matchstats;
pub mod meter;
pub mod stats;
pub mod suite;
mod table;

pub use table::{deal, play_game, play_game_with, play_table_with, Composite, Logged, RandomAgent, Rng, RunError, Table};

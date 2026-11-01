//! Marjapussi rules engine: the single owner of the game rules.
//! Imported from marjapussi-rs v0.1.3 + Krarilotus rule fixes (see NOTICE).
//!
//! The public API is re-exported here; the modules are private. Layers:
//! - `Game`: one game, event-sourced, with legal actions on demand and the undo snapshot;
//! - rules as bit operations: `CardSet`, `play_levels` (legal cards), `trick_winner`, `takes`;
//! - `FastState`: the `Copy` card-play core for search (docs/design/fast-core.md);
//! - scoring: `game_scores`, `team_points`, `SeriesTally` (marjapussi.de is the source of truth).

mod bits;
mod fast;
mod game;
mod inline;

pub use bits::{
    next_seat, opponents, partner, play_levels, takers, takes, trick_winner, winning_seat, CardSet, PlayLevels,
};
pub use fast::{FastParts, FastState, Move, Moves, PlayPhase, PublicState};
pub use game::bidding::{bid_range, is_bid, BID_STEP, MAX_BID, NO_BID};
pub use game::cards::{get_all_cards, suit_of, value_of, Card, Suit, Value, CARDS, SUITS, VALUES};
pub use game::clock::{current_time_string, format_time, now_millis, Clock};
pub use game::errors::GameError;
pub use game::gameevent::{ActionType, AnswerType, GameAction, GameCallback, GameEvent, QuestionType};
pub use game::gameinfo::GameFinishedInfo;
pub use game::gamestate::{FinishedTrick, GamePhase, GameState};
pub use game::legal_actions::is_first_trick;
pub use game::meta::GameMetaInfo;
pub use game::parse::{parse_action, parse_legacy_format, parse_pass, LegacyGameFormat};
pub use game::player::{hands_from_deck, PlaceAtTable, Player, PlayerTrumpPossibilities, HAND_SIZE, TRICKS};
pub use game::points::{
    points_card, points_pair, points_set, points_trick, Points, CARD_POINTS, CARD_POINTS_TOTAL, LAST_TRICK_BONUS,
    PAIR_POINTS, VALUE_POINTS,
};
pub use game::rules::{trick_points, trump_points, uncalled_pairs};
pub use game::scoring::{
    game_scores, round5, team_points, GameOutcome, SeriesLength, SeriesResult, SeriesSettings, SeriesTally, TeamPoints,
    SCHWARZ_FACTOR_FIFTHS,
};
pub use game::table::Game;
pub use inline::{Filler, InlineVec};

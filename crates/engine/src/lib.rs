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

pub use bits::{next_seat, partner, play_levels, takes, trick_winner, CardSet, PlayLevels};
pub use fast::{FastParts, FastState, Move, Moves, TRICKS};
pub use game::cards::{get_all_cards, Card, Suit, Value, CARDS, SUITS, VALUES};
pub use game::clock::{current_time_string, format_time, now_millis, Clock};
pub use game::errors::GameError;
pub use game::gameevent::{ActionType, AnswerType, GameAction, GameCallback, GameEvent, GameEventPlayer, QuestionType};
pub use game::gameinfo::{GameFinishedInfo, GameInfoPlayer, PublicInfo};
pub use game::gamestate::{FinishedTrick, GamePhase, GameState};
pub use game::legal_actions::is_first_trick;
pub use game::meta::GameMetaInfo;
pub use game::parse::{parse_legacy_format, LegacyGameFormat};
pub use game::player::{PlaceAtTable, Player, PlayerTrumpPossibilities, HAND_SIZE};
pub use game::points::{
    points_card, points_pair, points_set, points_trick, Points, CARD_POINTS, LAST_TRICK_BONUS, PAIR_POINTS,
    VALUE_POINTS,
};
pub use game::scoring::{
    game_scores, round5, team_points, GameOutcome, SeriesLength, SeriesResult, SeriesSettings, SeriesTally, TeamPoints,
    SCHWARZ_FACTOR_FIFTHS,
};
pub use game::table::Game;
pub use inline::{Filler, InlineVec};

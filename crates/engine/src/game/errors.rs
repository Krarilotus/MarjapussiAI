#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameError {
    IllegalAction,
    CannotUndo,
    /// A `fast::FastParts` that is not a reachable card-play state.
    InvalidState,
}

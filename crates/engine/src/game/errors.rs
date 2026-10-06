#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameError {
    IllegalAction,
    CannotUndo,
    /// A `fast::FastParts` that is not a reachable card-play state.
    InvalidState,
    /// Not a card code of the site (`r-A`, `g-6`, …).
    InvalidCode,
}

impl std::fmt::Display for GameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for GameError {}

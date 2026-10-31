//! Known-deal Game Solver (layer 4) on `engine::fast`: trick-control queries and
//! play evaluated over worlds sampled from a seat's knowledge. See docs/design/solver.md.

mod completed;
mod search;
mod trick;
mod tt;
mod worlds;

pub use completed::{
    evaluate_complete, CanonicalEffort, CompletedEvaluation, CompletionFailure, CompletionFailureReason,
    CANONICAL_EFFORT_ALGORITHM,
};
pub use search::{Limits, Solver, DEFAULT_TT_BITS};
pub use trick::{trick_control, TrickControl};
pub use worlds::{evaluate, evaluate_limited, evaluate_moves, Criterion, Decision, Measure, MoveValue, Objective, SolverError};

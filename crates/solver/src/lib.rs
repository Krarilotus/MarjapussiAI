//! Exact card-play solver (layer 4) on `engine::fast`, and a seat's moves judged over worlds
//! sampled from its knowledge. See docs/design/solver.md.

mod search;
mod tt;
mod worlds;

pub use search::{Limits, Solver, DEFAULT_TT_BITS};
pub use worlds::{evaluate, evaluate_limited, Criterion, Decision, Measure, MoveValue, Objective, SolverError};

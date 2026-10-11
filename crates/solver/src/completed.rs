//! Canonical whole-response evaluation for exact completed-result reuse.
//!
//! The reference algorithm starts with an empty transposition table and zero
//! node counter ONCE per evaluation. Normal within-evaluation TT reuse, move
//! ordering, sampling, criteria and limits remain those of `evaluate_limited`.
//! Pin the solver's TT capacity, inputs, belief, RNG state and implementation
//! version when comparing effort. This is not a TT-disabled node count, nor a
//! bound on sampling/belief work or wall time.

use engine::PublicState;
use knowledge::{Belief, Knowledge};
use rand::RngCore;

use crate::{evaluate_limited, Decision, Limits, Objective, Solver, SolverError};

/// Versioned accounting convention; changes to the reference search require
/// compatible version/configuration identities in any external result cache.
pub const CANONICAL_EFFORT_ALGORITHM: &str = "empty-tt-whole-evaluation-nodes-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanonicalEffort {
    /// Search nodes visited by the canonical evaluation, including work in an
    /// unfinished world or secondary criteria pass. Cache hits must debit this
    /// represented cost rather than their actual (possibly zero) search work.
    pub nodes: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompletedEvaluation {
    /// Unmodified existing semantic values, always with `complete == true`.
    pub decision: Decision,
    pub effort: CanonicalEffort,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionFailureReason {
    /// Some worlds or required secondary criteria did not finish. No partial
    /// move values are exposed as a usable completed response.
    Incomplete { completed_worlds: usize },
    /// Preserve the original evaluator's error, including interruption before
    /// the first complete world, inconsistent information and zero weight.
    Solver(SolverError),
    /// The evaluator completed, but the caller's safety deadline had elapsed
    /// by the completion gate (search checks the clock periodically).
    DeadlineElapsed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompletionFailure {
    pub reason: CompletionFailureReason,
    pub effort: CanonicalEffort,
}

impl std::fmt::Display for CompletionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?} after {} canonical search nodes",
            self.reason, self.effort.nodes
        )
    }
}

impl std::error::Error for CompletionFailure {}

/// Evaluate a complete response with canonical search effort.
///
/// Accepts the same inputs and limits as `evaluate_limited`, without changing
/// world count, objective, criteria or solver configuration. The caller owns
/// the RNG and must provide the same initial state for matching cohorts.
/// Clears the reusable solver's TT and resets its counter exactly once before
/// delegating the WHOLE evaluation, including any secondary criteria pass.
/// Limits apply to the entire evaluation; this wrapper invents no node cap.
///
/// On failure, return effort and the original error or incomplete status, not
/// a fallback move. A caller may defer its sequence, but must not cache that
/// fallback as a completed response. Completed results agree at matched inputs
/// and logical effort; a deadline-aborted cold run need not complete merely
/// because a warm completed-result cache can return before the deadline.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_complete<R: RngCore + ?Sized, B: Belief + ?Sized>(
    solver: &mut Solver,
    knowledge: &Knowledge,
    table: &PublicState,
    objective: Objective,
    worlds: usize,
    belief: &B,
    rng: &mut R,
    limits: Limits,
) -> Result<CompletedEvaluation, CompletionFailure> {
    solver.clear();
    solver.reset_nodes();
    let result = evaluate_limited(solver, knowledge, table, objective, worlds, belief, rng, limits);
    let effort = CanonicalEffort { nodes: solver.nodes() };
    match result {
        Err(error) => Err(CompletionFailure {
            reason: CompletionFailureReason::Solver(error),
            effort,
        }),
        Ok(decision) if !decision.complete => Err(CompletionFailure {
            reason: CompletionFailureReason::Incomplete {
                completed_worlds: decision.worlds,
            },
            effort,
        }),
        Ok(_)
            if limits
                .deadline
                .is_some_and(|deadline| std::time::Instant::now() >= deadline) =>
        {
            Err(CompletionFailure {
                reason: CompletionFailureReason::DeadlineElapsed,
                effort,
            })
        }
        Ok(decision) => Ok(CompletedEvaluation { decision, effort }),
    }
}

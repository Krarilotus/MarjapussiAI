//! Completion and canonical solver effort for a single modeled response.
//! Ordinary play does not install this state and keeps its timed partial policy.

use std::{cell::Cell, time::Instant};

use crate::Decision;

#[derive(Clone, Debug, PartialEq)]
pub struct CompletedResponse {
    pub decision: Decision,
    /// Represented endgame search nodes under
    /// `solver::CANONICAL_EFFORT_ALGORITHM`, with the existing solver TT size.
    /// Not a measure of belief, sampling, entry or valuation work. A cache hit
    /// must debit this cost; it cannot claim its actual zero search cost here.
    pub canonical_solver_nodes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseFailureReason {
    /// Modeled responses cannot run recursive forecasts or consume live intent.
    /// Reject before synchronization/evaluation; never alter the caller's policy.
    NonIsolated {
        plan_worlds: u32,
        return_worlds: u32,
        retained_intent: bool,
    },
    /// Original solver failure/completion status, without discarded provenance.
    Solver(solver::CompletionFailureReason),
    /// Shared response/endgame deadline expired outside the solver wrapper.
    DeadlineElapsed,
    /// An applicable completed endgame could not supply a legal policy move.
    NoLegalEndgameDecision,
    /// Defensive accounting failure; never publish saturated effort as exact.
    EffortOverflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResponseFailure {
    pub reason: ResponseFailureReason,
    /// Includes canonical nodes spent before failure, including incomplete
    /// worlds or secondary criteria. Other kernels remain unaccounted here.
    pub canonical_solver_nodes: u64,
}

impl std::fmt::Display for ResponseFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?} after {} canonical solver nodes",
            self.reason, self.canonical_solver_nodes
        )
    }
}
impl std::error::Error for ResponseFailure {}

pub(crate) struct ResponseState {
    nodes: Cell<u64>,
    reason: Cell<Option<ResponseFailureReason>>,
    node_limit: Option<u64>,
}

impl ResponseState {
    pub(crate) fn new(node_limit: Option<u64>) -> Self {
        Self {
            nodes: Cell::new(0),
            reason: Cell::new(None),
            node_limit,
        }
    }

    pub(crate) fn remaining_solver_nodes(&self) -> Option<u64> {
        self.node_limit.map(|limit| limit.saturating_sub(self.nodes.get()))
    }

    pub(crate) fn fail(&self, reason: ResponseFailureReason) {
        // Keep the actual first failure if outer deadline checking also fails.
        if self.reason.get().is_none() {
            self.reason.set(Some(reason));
        }
    }

    pub(crate) fn failure(&self) -> Option<ResponseFailure> {
        self.reason.get().map(|reason| ResponseFailure {
            reason,
            canonical_solver_nodes: self.nodes.get(),
        })
    }

    pub(crate) fn record_solver(
        &self,
        result: Result<solver::CompletedEvaluation, solver::CompletionFailure>,
    ) -> Option<solver::Decision> {
        let nodes = match &result {
            Ok(completed) => completed.effort.nodes,
            Err(failed) => failed.effort.nodes,
        };
        let Some(total) = self.nodes.get().checked_add(nodes) else {
            self.fail(ResponseFailureReason::EffortOverflow);
            return None;
        };
        self.nodes.set(total);
        match result {
            Ok(completed) => Some(completed.decision),
            Err(failed) => {
                self.fail(ResponseFailureReason::Solver(failed.reason));
                None
            }
        }
    }

    pub(crate) fn finish(
        &self,
        decision: Decision,
        deadline: Option<Instant>,
    ) -> Result<CompletedResponse, ResponseFailure> {
        if deadline.is_some_and(|until| Instant::now() >= until) {
            self.fail(ResponseFailureReason::DeadlineElapsed);
        }
        if let Some(failure) = self.failure() {
            Err(failure)
        } else {
            Ok(CompletedResponse {
                decision,
                canonical_solver_nodes: self.nodes.get(),
            })
        }
    }
}

#[cfg(test)]
#[path = "../tests/internal/response_effort.rs"]
mod tests;

//! Mandatory, comparison-local sharing of equal tail dependencies.
//!
//! This is deliberately narrower than general guard normalization. Only Stage
//! siblings that reach Tail/Interrupted without any engine action qualify.
//! The parent `choose_bound` owns one immutable Waiting group: every child is a
//! clone of that EXACT joint input, with only its receipt stage changed. Thus
//! origins, weights, all four private views, pending/accepted state and Context
//! are identical by construction, not inferred from root-visible equality.
//! No entry survives that comparison or authorizes a different information set.

use super::*;

pub(super) struct TailEdge {
    pub consumed: Vec<Consumed>,
    interrupted: bool,
    tail: Stage,
}

/// Refers to outcomes already owned by this comparison; no second trace cache.
pub(super) struct TailDependency {
    pub outcome_index: usize,
    pub read_starts: Vec<(usize, usize)>,
    pub transitions: usize,
    pub solver_nodes: u64,
}

impl<F: FnMut(&SeatView<'_>, &[GameAction], LeafBudget) -> Result<LeafResponse, LeafFailure>> Session<'_, F> {
    /// Pure guard/domain inspection. A non-tail frontier or pending transition
    /// retains the ordinary evaluator path, including all of its certificates.
    pub(super) fn normalized_tail_edges(
        &mut self,
        children: &[Branch],
    ) -> Result<Option<Vec<TailEdge>>, SequenceError> {
        let _timing = beliefs::profile::scope("sequence.tail_normalize");
        let mut edges = Vec::with_capacity(children.len());
        for branch in children {
            self.check()?;
            if branch.table.game.ended()
                || branch.table.to_act().map(|(actor, _)| actor) != Some(self.root)
                || branch.receipt.pending.is_some()
                || !branch.receipt.accepted.matches(&branch.table.view(self.root))
            {
                return Ok(None);
            }
            let inspection = self.inspect(branch)?;
            let interrupted = match inspection.frontier {
                Frontier::Tail => false,
                Frontier::Interrupted => true,
                _ => return Ok(None),
            };
            let information = Information::of(&branch.table.view(self.root));
            let consumed = inspection
                .consumed
                .iter()
                .map(|read| {
                    Ok(Consumed {
                        information: information.clone(),
                        suffix: self.suffix(&branch.receipt.program, read.stage)?,
                        reading: read.meaning.clone(),
                    })
                })
                .collect::<Result<Vec<_>, SequenceError>>()?;
            let tail = branch
                .receipt
                .program
                .nodes
                .iter()
                .position(|node| matches!(node, crate::concepts::sequence::Node::Tail))
                .ok_or(SequenceError::InvalidProgram)?;
            edges.push(TailEdge {
                consumed,
                interrupted,
                tail: Stage(tail),
            });
        }
        Ok(Some(edges))
    }

    /// Exactly drive's Tail/Interrupted dispatch, with its already consumed
    /// reads attached once. The first tail evaluates the entire original group.
    pub(super) fn finish_normalized_tails(
        &mut self,
        children: Vec<Branch>,
        edges: Vec<TailEdge>,
    ) -> Result<Vec<Terminal>, SequenceError> {
        if children.len() != edges.len() {
            return Err(SequenceError::InvalidCohort);
        }
        let mut terminals = Vec::with_capacity(children.len());
        for (mut branch, edge) in children.into_iter().zip(edges) {
            branch.consumed.extend(edge.consumed);
            branch.interrupted |= edge.interrupted;
            branch.receipt.stage = edge.tail;
            terminals.push(self.tail(branch, edge.tail)?);
        }
        terminals.sort_by_key(|terminal| terminal.origin);
        Ok(terminals)
    }

    /// Attach a different authored edge to the same completed dependency.
    /// Preserve its binding/guard reads and interruption bit, never the first
    /// sibling's. The existing first offer/owner and trick prefix are identical:
    /// only Stage siblings of this one unchanged group reach this method.
    pub(super) fn alias_tail(
        &mut self,
        children: &[Branch],
        edges: &[TailEdge],
        dependency: &TailDependency,
        outcomes: &[Vec<Terminal>],
    ) -> Result<Vec<Terminal>, SequenceError> {
        self.check()?;
        let source = outcomes
            .get(dependency.outcome_index)
            .ok_or(SequenceError::InvalidCohort)?;
        if children.len() != edges.len() || source.len() != children.len() {
            return Err(SequenceError::InvalidCohort);
        }
        let mut terminals = Vec::with_capacity(children.len());
        for (branch, edge) in children.iter().zip(edges) {
            let original = source
                .iter()
                .find(|terminal| terminal.origin == branch.origin && terminal.weight == branch.weight)
                .ok_or(SequenceError::InvalidCohort)?;
            let read_start = dependency
                .read_starts
                .iter()
                .find_map(|(origin, start)| (*origin == branch.origin).then_some(*start))
                .ok_or(SequenceError::InvalidCohort)?;
            let dependency_reads = original
                .consumed
                .get(read_start..)
                .ok_or(SequenceError::InvalidCohort)?;
            let mut terminal = original.clone();
            terminal.consumed = branch.consumed.clone();
            terminal.consumed.extend(edge.consumed.iter().cloned());
            terminal.consumed.extend(dependency_reads.iter().cloned());
            terminal.interrupted = branch.interrupted || edge.interrupted;
            terminals.push(terminal);
        }
        terminals.sort_by_key(|terminal| terminal.origin);
        // This shared edge has no engine transition or solver call of its own.
        // Optional cache hits STILL charge the cost of the unique dependency.
        // These counters describe actual aliases (cached parent runs may skip
        // constructing them), not an expansion-equivalent budget allowance.
        // Diagnostic sums saturate; they never participate in allowance checks.
        self.work.tail_aliases = self.work.tail_aliases.saturating_add(1);
        self.work.tail_alias_transitions = self.work.tail_alias_transitions.saturating_add(dependency.transitions);
        self.work.tail_alias_solver_nodes = self
            .work
            .tail_alias_solver_nodes
            .saturating_add(dependency.solver_nodes);
        Ok(terminals)
    }
}

#[cfg(test)]
#[path = "../../../tests/internal/sequence_normalization.rs"]
mod tests;

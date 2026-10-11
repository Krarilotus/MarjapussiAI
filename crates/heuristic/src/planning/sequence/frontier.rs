//! Mandatory sharing of pure-inspection Stage siblings in one choose_bound.
//!
//! The owner keeps the entire joint input immutable; only entry stages vary.
//! This is not a root-information cache. No dependency survives that owner.
//! Singleton Actions, Response, Reconsider and Deferred are deliberately absent:
//! only Choice/multi-Action drive immediately yields, before any action or proof
//! record. Thus finish produces [Local entry(E), independent descendants] and
//! terminal reads P + E + D. Those producer boundaries justify the projection.

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Domain {
    Choice(Vec<Selection>),
    Actions(Vec<Selection>),
}
impl Domain {
    fn alternatives(&self) -> &[Selection] {
        match self {
            Self::Choice(domain) | Self::Actions(domain) => domain,
        }
    }
}

struct EdgeOrigin {
    origin: usize,
    weight: f64,
    stage: Stage,
    consumed: Vec<Consumed>,
    offers: Vec<Offer>,
}

pub(super) struct FrontierEdge {
    domain: Domain,
    information: Information,
    entry: Suffix,
    membership: Membership,
    origins: Vec<EdgeOrigin>,
}

/// Only an index into outcomes already owned by the lexical comparison.
pub(super) struct FrontierDependency {
    pub edge: FrontierEdge,
    pub outcome_index: usize,
    pub transitions: usize,
    pub comparisons: usize,
    pub solver_nodes: u64,
}

impl FrontierEdge {
    fn support(&self) -> Vec<(usize, f64)> {
        self.origins.iter().map(|edge| (edge.origin, edge.weight)).collect()
    }

    fn validates(&self, records: &[PreparedChoice], context: &Context) -> bool {
        let Some(entry) = records.first() else { return false };
        let Some(first) = self.origins.first() else {
            return false;
        };
        entry.scope == ChoiceScope::Local
            && entry.information == self.information
            && entry.semantic_suffix == self.entry
            && &entry.context == context
            && entry.membership == self.membership
            && entry.alternatives == self.domain.alternatives()
            && entry.consumed == first.consumed
            && entry.support == self.support()
            && entry.selected < entry.alternatives.len()
            && entry.assessment.as_ref().is_some_and(|assessment| {
                assessment.cohort_complete
                    && assessment.candidates.len() == entry.alternatives.len()
                    && assessment.selected == entry.selected
            })
    }
}

impl<F: FnMut(&SeatView<'_>, &[GameAction], LeafBudget) -> Result<LeafResponse, LeafFailure>> Session<'_, F> {
    pub(super) fn normalized_frontier(&mut self, children: &[Branch]) -> Result<Option<FrontierEdge>, SequenceError> {
        let _timing = beliefs::profile::scope("sequence.frontier_normalize");
        let mut result: Option<FrontierEdge> = None;
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
            let (domain, offers) = match inspection.frontier {
                Frontier::Choice(stages) => (
                    Domain::Choice(
                        stages
                            .into_iter()
                            .map(|stage| self.selection(&branch.receipt.program, &Alternative::Stage(stage)))
                            .collect::<Result<_, _>>()?,
                    ),
                    Vec::new(),
                ),
                Frontier::Actions(offers) if offers.len() > 1 => (
                    Domain::Actions(
                        offers
                            .iter()
                            .map(|offer| self.selection(&branch.receipt.program, &Alternative::Offer(offer.clone())))
                            .collect::<Result<_, _>>()?,
                    ),
                    offers,
                ),
                _ => return Ok(None),
            };
            let information = Information::of(&branch.table.view(self.root));
            let entry = self.suffix(&branch.receipt.program, branch.receipt.stage)?;
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
            if let Some(edge) = &result {
                // Ordinary finish must form exactly one entry group. Different
                // entry/read shapes retain its ordinary multi-group evaluation.
                if edge.domain != domain
                    || edge.information != information
                    || edge.entry != entry
                    || edge.origins[0].consumed != consumed
                {
                    return Ok(None);
                }
            } else {
                let legal = branch.table.to_act().ok_or(SequenceError::WrongContext)?.1;
                result = Some(FrontierEdge {
                    domain,
                    information,
                    entry,
                    membership: self.membership(&branch.table.view(self.root), legal)?,
                    origins: Vec::with_capacity(children.len()),
                });
            }
            result.as_mut().unwrap().origins.push(EdgeOrigin {
                origin: branch.origin,
                weight: branch.weight,
                stage: branch.receipt.stage,
                consumed,
                offers,
            });
        }
        if let Some(edge) = &mut result {
            edge.origins.sort_by_key(|origin| origin.origin);
            if edge.origins.windows(2).any(|pair| pair[0].origin == pair[1].origin) {
                return Ok(None);
            }
        }
        Ok(result)
    }

    /// None means ordinary evaluation, including all its work and certificates.
    /// No partially projected result or budget credit escapes on a shape miss.
    pub(super) fn alias_frontier(
        &mut self,
        children: &[Branch],
        edge: &FrontierEdge,
        dependency: &FrontierDependency,
        outcomes: &[Vec<Terminal>],
        records: &[Vec<PreparedChoice>],
    ) -> Result<Option<(Vec<Terminal>, Vec<PreparedChoice>)>, SequenceError> {
        self.check()?;
        let representative = &dependency.edge;
        if edge.domain != representative.domain
            || edge.information != representative.information
            || edge.membership != representative.membership
            || edge.support() != representative.support()
        {
            return Ok(None);
        }
        let (Some(source), Some(proofs)) = (
            outcomes.get(dependency.outcome_index),
            records.get(dependency.outcome_index),
        ) else {
            return Ok(None);
        };
        if !representative.validates(proofs, self.context)
            || children.len() != edge.origins.len()
            || source.len() != children.len()
        {
            return Ok(None);
        }
        let selected = proofs[0].selected;
        let mut terminals = Vec::with_capacity(children.len());
        for (own, old) in edge.origins.iter().zip(&representative.origins) {
            let Some(branch) = children
                .iter()
                .find(|branch| branch.origin == own.origin && branch.weight == own.weight)
            else {
                return Ok(None);
            };
            let Some(original) = source
                .iter()
                .find(|terminal| terminal.origin == own.origin && terminal.weight == own.weight)
            else {
                return Ok(None);
            };
            // Exact positional prefix at the known finish/drive boundary, not
            // arbitrary matching/replacement inside descendant proof records.
            let prefix_len = branch.consumed.len() + old.consumed.len();
            if original.consumed.get(..branch.consumed.len()) != Some(branch.consumed.as_slice())
                || original.consumed.get(branch.consumed.len()..prefix_len) != Some(old.consumed.as_slice())
                || !original.tricks.starts_with(&branch.tricks)
                || (branch.interrupted && !original.interrupted)
            {
                return Ok(None);
            }
            let mut terminal = original.clone();
            terminal.consumed = branch.consumed.clone();
            terminal.consumed.extend(own.consumed.iter().cloned());
            terminal
                .consumed
                .extend(original.consumed[prefix_len..].iter().cloned());
            if branch.first.is_some() {
                if original.first != branch.first || original.first_program != branch.first_program {
                    return Ok(None);
                }
            } else if matches!(edge.domain, Domain::Actions(_)) {
                let (Some(old_offer), Some(offer)) = (old.offers.get(selected), own.offers.get(selected)) else {
                    return Ok(None);
                };
                // first_step has no engine effect. Validate the representative
                // owner, then construct the alias's owner from its ORIGINAL entry.
                let mut owner = branch.clone();
                owner.receipt.stage = old.stage;
                match self.first_step(&mut owner, old_offer) {
                    Ok(()) => {}
                    Err(SequenceError::Budget) => return Err(SequenceError::Budget),
                    Err(_) => return Ok(None),
                }
                if original.first != owner.first || original.first_program != owner.first_program {
                    return Ok(None);
                }
                owner.first = None;
                owner.first_program = None;
                owner.receipt.stage = own.stage;
                match self.first_step(&mut owner, offer) {
                    Ok(()) => {}
                    Err(SequenceError::Budget) => return Err(SequenceError::Budget),
                    Err(_) => return Ok(None),
                }
                terminal.first = owner.first;
                terminal.first_program = owner.first_program;
            }
            // Choice inherits the actually selected child's first owner.
            terminals.push(terminal);
        }
        let mut projected = proofs.clone();
        projected[0].semantic_suffix = edge.entry.clone();
        projected[0].consumed = edge.origins[0].consumed.clone();
        if !edge.validates(&projected, self.context) {
            return Ok(None);
        }
        self.work.frontier_aliases = self.work.frontier_aliases.saturating_add(1);
        self.work.frontier_alias_transitions = self
            .work
            .frontier_alias_transitions
            .saturating_add(dependency.transitions);
        self.work.frontier_alias_comparisons = self
            .work
            .frontier_alias_comparisons
            .saturating_add(dependency.comparisons);
        self.work.frontier_alias_solver_nodes = self
            .work
            .frontier_alias_solver_nodes
            .saturating_add(dependency.solver_nodes);
        Ok(Some((terminals, projected)))
    }

    pub(super) fn finish_frontier_child(
        &mut self,
        children: Vec<Branch>,
        dependencies: &mut Vec<FrontierDependency>,
        outcomes: &[Vec<Terminal>],
        records: &[Vec<PreparedChoice>],
        child_records: &mut Vec<PreparedChoice>,
    ) -> Result<Vec<Terminal>, SequenceError> {
        let edge = self.normalized_frontier(&children)?;
        if let Some(edge) = &edge {
            for dependency in dependencies.iter() {
                if let Some((terminals, projected)) =
                    self.alias_frontier(&children, edge, dependency, outcomes, records)?
                {
                    *child_records = projected;
                    return Ok(terminals);
                }
            }
        }
        let before = self.work;
        let terminals = self.finish(children, child_records)?;
        if let Some(edge) = edge.filter(|edge| edge.validates(child_records, self.context)) {
            dependencies.push(FrontierDependency {
                edge,
                outcome_index: outcomes.len(),
                transitions: self.work.represented_transitions - before.represented_transitions,
                comparisons: self.work.represented_comparisons - before.represented_comparisons,
                solver_nodes: self.work.represented_solver_nodes - before.represented_solver_nodes,
            });
        }
        Ok(terminals)
    }
}

#[cfg(test)]
#[path = "../../../tests/internal/sequence_frontier.rs"]
mod tests;

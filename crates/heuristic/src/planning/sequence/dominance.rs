//! Local completion dominance. Only the executor mints authorization; numerical
//! bounds do not establish a competitor's model value, totality or completion.

use super::*;
use crate::concepts::sequence::{Actions, Node};
use crate::concepts::value::bounded::{CompletionBounds, ForwardSnapshot};
use crate::concepts::value::Definition;
use engine::{ActionType, GamePhase};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub enum SelectionProof {
    Complete(Assessment),
    CompletionDominance(CompletionDominance),
}

/// Construction-local scope, not identity of a latent cohort or a memo/rebind
/// key. Equality is diagnostic. The executor retains full private BranchKey;
/// SelectedPlan owns the separate selected-parent reachability certificate.
#[derive(Clone, Debug, PartialEq)]
pub struct DominanceScope {
    pub information: Information,
    pub context: Context,
    pub semantic_suffix: Suffix,
    pub membership: Membership,
    pub alternatives: Vec<Selection>,
    pub selected: usize,
    /// Original group order, noncontiguous origin IDs, exact unnormalised bits.
    pub support: Vec<(usize, u64)>,
    pub opponent_weight_bits: u64,
}

/// Completed selected continuation versus the legal terminal completions of
/// every original competitor. Losers have bounds only, no receipt/Assessment.
/// Private construction prevents a raw numerical ordering node authorizing play.
#[derive(Clone, Debug, PartialEq)]
pub struct CompletionDominance {
    scope: Arc<DominanceScope>,
    values: CompletionBounds,
}
impl CompletionDominance {
    pub fn scope(&self) -> &DominanceScope {
        &self.scope
    }
    pub fn values(&self) -> &CompletionBounds {
        &self.values
    }

    pub(super) fn validates(&self, choice: &PreparedChoice) -> bool {
        let scope = &self.scope;
        choice.scope == ChoiceScope::Local
            && choice.information == scope.information
            && choice.context == scope.context
            && choice.semantic_suffix == scope.semantic_suffix
            && choice.membership == scope.membership
            && choice.alternatives == scope.alternatives
            && choice.selected == scope.selected
            && scope.selected == self.values.selected
            && scope.opponent_weight_bits == Definition::default().opponent_weight.to_bits()
            && support_bits(&choice.support) == scope.support
            && self.values.competitors.len() + 1 == scope.alternatives.len()
            && self
                .values
                .competitors
                .iter()
                .map(|(i, _, _)| *i)
                .eq((0..scope.alternatives.len()).filter(|i| *i != scope.selected))
            && self.values.competitors.iter().all(|(i, _, bound)| {
                let order = bound.upper.total_cmp(&self.values.selected_value.difference);
                order == std::cmp::Ordering::Less || *i > scope.selected && order == std::cmp::Ordering::Equal
            })
    }

    /// Conservative owned-payload ledger. Arc payloads are counted per record,
    /// not silently omitted; cache admission is optional, proof semantics are not.
    pub(super) fn owned_bytes(&self) -> usize {
        use crate::concepts::value::bounded::{BoundOperator, BoundSource, BoundValue};
        use crate::concepts::value::{RuntimeOperator, RuntimeValue};
        let scope = &self.scope;
        std::mem::size_of_val(self)
            + std::mem::size_of_val(scope.as_ref())
            + info_bytes(&scope.information)
            + suffix_bytes(&scope.semantic_suffix)
            + membership_bytes(&scope.membership)
            + vec_bytes(&scope.alternatives)
            + scope.alternatives.iter().map(selection_bytes).sum::<usize>()
            + vec_bytes(&scope.support)
            + vec_bytes(&self.values.competitors)
            + vec_bytes(&self.values.trace)
            + self
                .values
                .trace
                .iter()
                .map(|n| {
                    vec_bytes(&n.dependencies)
                        + match &n.operator {
                            RuntimeOperator::Value(crate::concepts::value::Operator::Best { values }) => {
                                vec_bytes(values)
                            }
                            RuntimeOperator::Value(crate::concepts::value::Operator::Assessment {
                                diagnostics,
                                ..
                            }) => vec_bytes(diagnostics),
                            RuntimeOperator::Bound(BoundOperator::CertifiedBest { competitors, .. }) => {
                                vec_bytes(competitors)
                            }
                            RuntimeOperator::Bound(BoundOperator::ScoreIntervals { source, slots, .. }) => {
                                vec_bytes(slots)
                                    + match source {
                                        BoundSource::PublicForwardEnvelope { snapshot, .. } => {
                                            std::mem::size_of_val(snapshot.as_ref())
                                                + vec_bytes(&snapshot.events)
                                                + vec_bytes(&snapshot.legal)
                                                + vec_bytes(&snapshot.support)
                                        }
                                        #[cfg(test)]
                                        BoundSource::NumericalFixture => 0,
                                    }
                            }
                            _ => 0,
                        }
                        + match &n.value {
                            RuntimeValue::Value(crate::concepts::value::NodeValue::ConditionalOutcomes(v)) => {
                                vec_bytes(v)
                            }
                            RuntimeValue::Value(crate::concepts::value::NodeValue::Weights(v)) => vec_bytes(v),
                            RuntimeValue::Value(crate::concepts::value::NodeValue::ConditionalScores(v)) => {
                                vec_bytes(v)
                            }
                            RuntimeValue::Value(crate::concepts::value::NodeValue::BranchValues(v)) => vec_bytes(v),
                            RuntimeValue::Bound(BoundValue::Scores(v)) => vec_bytes(v),
                            RuntimeValue::Bound(BoundValue::Branches(v)) => vec_bytes(v),
                            RuntimeValue::Bound(BoundValue::Certified { excluded, .. }) => vec_bytes(excluded),
                            _ => 0,
                        }
                })
                .sum::<usize>()
    }
}

fn support_bits(support: &[(usize, f64)]) -> Vec<(usize, u64)> {
    support.iter().map(|(origin, w)| (*origin, w.to_bits())).collect()
}
pub(super) fn support_subsequence(child: &[(usize, f64)], parent: &[(usize, f64)]) -> bool {
    let mut remaining = parent.iter();
    child.iter().all(|(origin, weight)| {
        remaining
            .by_ref()
            .any(|(id, w)| id == origin && w.to_bits() == weight.to_bits())
    })
}

fn literal_tail(suffix: &Suffix) -> bool {
    suffix.nodes.as_slice() == [Node::Tail] && suffix.start == Stage(0)
}
/// Incumbents remain deliberately narrow: a legal card followed by literal Tail,
/// or literal Tail itself. Broader suffixes can legitimately interrupt on an
/// empty binding, which is not admitted as completed selected evidence here.
fn exact_card(selection: &Selection) -> Option<GameAction> {
    match selection {
        Selection::Offer { action, next }
            if matches!(action.action_type, ActionType::CardPlayed(_)) && literal_tail(next) =>
        {
            Some(*action)
        }
        _ => None,
    }
}
fn stage_card(suffix: &Suffix) -> Option<ActionType> {
    if suffix.nodes.len() != 2 {
        return None;
    }
    match suffix.nodes.get(suffix.start.0)? {
        Node::Action {
            actions: Actions::Exact(action @ ActionType::CardPlayed(_)),
            next,
        } if matches!(suffix.nodes.get(next.0), Some(Node::Tail)) => Some(*action),
        _ => None,
    }
}

fn incumbent_admitted(selection: &Selection, root: u8, legal: &[GameAction]) -> bool {
    match selection {
        Selection::Stage(suffix) if literal_tail(suffix) => true,
        Selection::Offer { .. } => {
            exact_card(selection).is_some_and(|action| action.player.0 == root && legal.contains(&action))
        }
        Selection::Stage(suffix) => stage_card(suffix).is_some_and(|kind| {
            legal
                .iter()
                .any(|action| action.player.0 == root && action.action_type == kind)
        }),
    }
}

/// Enclosure of successful forward terminal completions, NOT model totality,
/// future legality, guard truth, feasibility or a promise to finish within Work.
/// Validate normally, then inspect every reachable arm without reading facts or
/// binding actions. All finite admitted paths end at an actual Tail. Thus an
/// empty card binding's existing Interrupted -> Tail escape is also enclosed;
/// it does not become a successful authored action or a losing receipt.
fn card_tail_suffix(suffix: &Suffix) -> bool {
    let program = Program {
        name: "completion-enclosure",
        version: 1,
        start: suffix.start,
        nodes: suffix.nodes.clone(),
    };
    if program.validate().is_err() {
        return false;
    }
    let mut visited = vec![false; program.nodes.len()];
    let mut pending = vec![program.start];
    while let Some(stage) = pending.pop() {
        if visited[stage.0] {
            continue;
        }
        visited[stage.0] = true;
        match &program.nodes[stage.0] {
            Node::Tail => {}
            Node::Action {
                actions: Actions::Exact(ActionType::CardPlayed(_)) | Actions::Cards(_) | Actions::Standing(_),
                next,
            } => pending.push(*next),
            Node::Guard { yes, no, unknown, .. } => pending.extend([*yes, *no, *unknown]),
            Node::Choice(children) => pending.extend(children.iter().copied()),
            _ => return false,
        }
    }
    true
}

fn competitor_admitted(selection: &Selection, root: u8, legal: &[GameAction]) -> bool {
    match selection {
        Selection::Stage(suffix) => card_tail_suffix(suffix),
        Selection::Offer { action, next } => {
            action.player.0 == root
                && matches!(action.action_type, ActionType::CardPlayed(_))
                && legal.contains(action)
                && card_tail_suffix(next)
        }
    }
}
impl<F: FnMut(&SeatView<'_>, &[GameAction], LeafBudget) -> Result<LeafResponse, LeafFailure>> Session<'_, F> {
    pub(super) fn choice_children(&mut self, group: &[Waiting], index: usize) -> Result<Vec<Branch>, SequenceError> {
        let _timing = beliefs::profile::scope("sequence.child_cohort");
        group
            .iter()
            .map(|waiting| {
                let mut branch = waiting.branch.clone();
                match &waiting.alternatives[index] {
                    Alternative::Stage(stage) => branch.receipt.stage = *stage,
                    Alternative::Offer(offer) => self.commit(&mut branch, offer.clone())?,
                    Alternative::Reconsider { .. } | Alternative::Response { .. } => {
                        return Err(SequenceError::InvalidProgram)
                    }
                }
                Ok(branch)
            })
            .collect()
    }

    pub(super) fn dominance_input(
        &mut self,
        group: &[Waiting],
        alternatives: &[Selection],
        scope: ChoiceScope,
        membership: &Membership,
    ) -> Result<Option<Arc<ForwardSnapshot>>, SequenceError> {
        self.check()?;
        if scope != ChoiceScope::Local || alternatives.len() < 2 {
            return Ok(None);
        }
        let first = &group[0].branch;
        if !matches!(first.table.game.state.phase, GamePhase::StartTrick | GamePhase::Trick) {
            return Ok(None);
        }
        let legal = &membership.legal;
        let admitted = incumbent_admitted(&alternatives[0], self.root, legal)
            && alternatives[1..]
                .iter()
                .all(|s| competitor_admitted(s, self.root, legal));
        if !admitted {
            return Ok(None);
        }
        let view = first.table.view(self.root);
        let information = Information::of(&view);
        if group.iter().any(|w| {
            w.branch.interrupted
                || !w.branch.weight.is_finite()
                || w.branch.weight <= 0.0
                || !information.matches(&w.branch.table.view(self.root))
                || w.branch.table.game.state.phase != first.table.game.state.phase
                || !w
                    .branch
                    .table
                    .to_act()
                    .is_some_and(|(seat, domain)| seat == self.root && crate::legal_actions::eligible(domain) == *legal)
        }) {
            return Ok(None);
        }
        if knowledge::PublicOutcomeEnvelope::from_forward_play(
            view.knowledge,
            &first.table.game.state.phase,
            legal,
            view.events,
        )
        .is_err()
        {
            return Ok(None);
        }
        Ok(Some(Arc::new(ForwardSnapshot {
            knowledge: *view.knowledge,
            phase: first.table.game.state.phase.clone(),
            legal: legal.clone(),
            events: view.events.to_vec(),
            support: group
                .iter()
                .map(|w| (w.branch.origin, w.branch.weight.to_bits()))
                .collect(),
            root_team: self.root as usize % 2,
            opponent_weight_bits: Definition::default().opponent_weight.to_bits(),
            provenance: self.provenance,
        })))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn certify_local(
        &mut self,
        snapshot: Arc<ForwardSnapshot>,
        information: &Information,
        suffix: &Suffix,
        membership: &Membership,
        alternatives: &[Selection],
        support: &[(usize, f64)],
        selected: &[Terminal],
    ) -> Result<Option<CompletionDominance>, SequenceError> {
        let _timing = beliefs::profile::scope("sequence.completion_dominance");
        self.charge(0, 1)?;
        self.work.comparisons += 1;
        let envelope = knowledge::PublicOutcomeEnvelope::from_forward_play(
            &snapshot.knowledge,
            &snapshot.phase,
            &snapshot.legal,
            &snapshot.events,
        )
        .map_err(|_| SequenceError::InvalidValue)?;
        if selected.len() != support.len()
            || selected.iter().zip(support).any(|(t, (origin, w))| {
                t.origin != *origin
                    || t.weight.to_bits() != w.to_bits()
                    || t.interrupted
                    || !envelope.contains(&t.outcome)
            })
        {
            return Err(SequenceError::InvalidCohort);
        }
        let outcomes: Vec<_> = selected.iter().map(|t| (t.outcome, t.weight)).collect();
        let slices = [outcomes.as_slice()];
        let mut store = ValueStore::new(&slices, self.root as usize % 2, self.provenance)
            .map_err(|_| SequenceError::InvalidValue)?;
        let Some(values) = store
            .completion_bounds(snapshot, alternatives.len(), 0)
            .map_err(|_| SequenceError::InvalidValue)?
        else {
            return Ok(None);
        };
        self.check()?;
        Ok(Some(CompletionDominance {
            scope: Arc::new(DominanceScope {
                information: information.clone(),
                context: self.context.clone(),
                semantic_suffix: suffix.clone(),
                membership: membership.clone(),
                alternatives: alternatives.to_vec(),
                selected: 0,
                support: support_bits(support),
                opponent_weight_bits: Definition::default().opponent_weight.to_bits(),
            }),
            values,
        }))
    }
}

#[cfg(test)]
#[path = "../../../tests/internal/sequence_dominance.rs"]
mod tests;

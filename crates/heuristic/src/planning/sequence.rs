//! Finite authored continuations over a preserved joint cohort. Suffix choices
//! belong to actor information sets, never individual hidden worlds.

use crate::concepts::sequence::{
    authored_programs, bind_reconsideration, inspect, inspect_with, Frontier, Information, Offer, Program, ReadMeaning,
    Receipt, SequenceError, Stage, Suffix,
};
use crate::concepts::value::{Assessment, Provenance, ValueStore};
use agent::{SeatView, Table};
use beliefs::Mind;
use engine::{GameAction, GameFinishedInfo, GameOutcome, PlayPhase, PublicState};
use std::time::Instant;

mod frontier;
mod normalization;
mod reuse;
pub use reuse::CompletedCache;
mod selection;
pub use selection::{SelectedPlan, SelectionAuthorization};
#[cfg(test)]
#[path = "../../tests/internal/sequence_hard_reads.rs"]
mod hard_read_tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub program: Program,
    pub stage: Stage,
}
impl Candidate {
    pub fn new(program: Program) -> Self {
        let stage = program.start;
        Self { program, stage }
    }
    pub fn suffix(receipt: &Receipt) -> Self {
        Self {
            program: receipt.program.clone(),
            stage: receipt.stage,
        }
    }
}
/// Actual response configuration and caller-owned posterior generation. A new
/// cohort/reweighting must change the generation. Prepared assessments describe
/// the initial joint measure conditioned on a modeled observation, NOT a newly
/// rebuilt live Mind posterior. Keeping a generation requires that same scope.
#[derive(Clone, Debug, PartialEq)]
pub struct Context {
    pub params: crate::Params,
    pub seed: u64,
    pub posterior_revision: u64,
    pub provenance: Provenance,
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Logical work under Work::accounting(), including completed memo hits.
    /// Equal eligible sibling dependencies are charged once, with caches on/off.
    pub transitions: usize,
    pub comparisons: usize,
    /// Canonical completed-response solver work; None imposes no node cap.
    pub solver_nodes: Option<u64>,
    pub memo_entries: usize,
    pub memo_bytes: usize,
    /// Disable completed suffix/tail reuse; exact response reuse remains optional.
    pub reuse_suffixes: bool,
    pub deadline: Option<Instant>,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            transitions: 8192,
            comparisons: 256,
            solver_nodes: None,
            memo_entries: 128,
            memo_bytes: 16 * 1024 * 1024,
            reuse_suffixes: true,
            deadline: None,
        }
    }
}
/// The response leaf must obey this remaining lane. Checking after the callback
/// rejects an overrun; Rust cannot forcibly preempt a callback. This is a soft
/// wall deadline, never a hard process-time guarantee.
/// Ok certifies a deterministic, semantically completed response under the
/// configured model. Incomplete search returns a failure, never a timed prefix
/// or weaker fallback. Both results report actual canonical solver work.
#[derive(Clone, Copy, Debug)]
pub struct LeafBudget {
    pub deadline: Option<Instant>,
    pub remaining_transitions: usize,
    pub remaining_solver_nodes: Option<u64>,
}
/// Completed deterministic primitive result. Effort uses the configured canonical
/// solver algorithm, not wall time; it is replayed on compatible memo hits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeafResponse {
    pub action: GameAction,
    pub canonical_solver_nodes: u64,
}
impl LeafResponse {
    /// A completed primitive/test response which performs no solver search.
    pub fn unit(action: GameAction) -> Self {
        Self {
            action,
            canonical_solver_nodes: 0,
        }
    }
}
/// Unusable response plus actual attempted canonical effort. Never cached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeafFailure {
    pub reason: SequenceError,
    pub canonical_solver_nodes: u64,
}
impl LeafFailure {
    pub fn unit(reason: SequenceError) -> Self {
        Self {
            reason,
            canonical_solver_nodes: 0,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Work {
    /// Actual engine applications; cache hits only debit represented work.
    pub transitions: usize,
    pub represented_transitions: usize,
    /// Actual value-store comparisons, separate from the logical limit.
    pub comparisons: usize,
    pub represented_comparisons: usize,
    /// Canonical nodes actually performed by response callbacks, including failures.
    pub actual_solver_nodes: u64,
    /// Completed hits debit the same canonical work without repeating it.
    pub represented_solver_nodes: u64,
    pub response_calls: usize,
    pub response_hits: usize,
    pub suffix_hits: usize,
    /// Actual sibling edges attached to an already evaluated tail dependency.
    pub tail_aliases: usize,
    /// Expansion work avoided by those edges, not additional serving allowance.
    pub tail_alias_transitions: usize,
    pub tail_alias_solver_nodes: u64,
    /// Actual pure-inspection Choice/multi-Action sibling projections. A cached
    /// parent may skip constructing them; these are diagnostics, not allowances.
    pub frontier_aliases: usize,
    /// Represented expansion avoided by those projections, summed saturating.
    pub frontier_alias_transitions: usize,
    pub frontier_alias_comparisons: usize,
    pub frontier_alias_solver_nodes: u64,
    pub retained_bytes: usize,
    pub completed_candidates: usize,
}
impl Work {
    /// Deterministic bounded-work policy, independent of optional memo capacity.
    pub const fn accounting(&self) -> &'static str {
        "sibling-frontier-dag-v2"
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrickOutcome {
    pub leader: u8,
    pub winner: u8,
    pub cards: [u8; 4],
    pub points: i32,
    pub successor: PublicState,
    pub root: Information,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Consumed {
    pub information: Information,
    pub suffix: Suffix,
    pub reading: ReadMeaning,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Terminal {
    pub origin: usize,
    pub weight: f64,
    pub outcome: GameOutcome,
    pub tricks: Vec<TrickOutcome>,
    pub consumed: Vec<Consumed>,
    pub interrupted: bool,
    pub first: Option<Offer>,
    /// Canonical program owning `first.next`; composed stage ids never escape
    /// paired with an unrelated original candidate's program.
    pub first_program: Option<Program>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    Stage(Suffix),
    Offer { action: GameAction, next: Suffix },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Membership {
    pub legal: Vec<GameAction>,
    pub programs: Vec<Suffix>,
}
impl Membership {
    pub fn from_candidates(legal: &[GameAction], candidates: &[Candidate]) -> Result<Self, SequenceError> {
        let mut programs = Vec::new();
        for candidate in candidates {
            let suffix = candidate.program.suffix(candidate.stage)?;
            if !programs.contains(&suffix) {
                programs.push(suffix);
            }
        }
        Ok(Self {
            legal: crate::legal_actions::eligible(legal),
            programs,
        })
    }
    pub fn authored(view: &SeatView<'_>, legal: &[GameAction]) -> Result<Self, SequenceError> {
        let eligible = crate::legal_actions::eligible(legal);
        let legal = eligible.as_slice();
        Self::from_candidates(
            legal,
            &authored_programs(view, legal)
                .into_iter()
                .map(Candidate::new)
                .collect::<Vec<_>>(),
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceScope {
    Parent,
    Global,
    Local,
    FixedAction,
    Response,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedChoice {
    pub scope: ChoiceScope,
    pub information: Information,
    pub semantic_suffix: Suffix,
    pub context: Context,
    pub membership: Membership,
    pub alternatives: Vec<Selection>,
    pub selected: usize,
    pub assessment: Option<Assessment>,
    pub consumed: Vec<Consumed>,
    /// Original cohort ids and unnormalised joint mass in this information
    /// group. Zero/omitted soft mass is not hard impossibility evidence.
    pub support: Vec<(usize, f64)>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Execution {
    pub candidate: Candidate,
    pub first: Offer,
    pub terminals: Vec<Terminal>,
    pub prepared: Vec<PreparedChoice>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    pub executions: Vec<Execution>,
    pub values: Assessment,
    pub receipt: Receipt,
    pub selection: std::sync::Arc<SelectedPlan>,
    pub work: Work,
}
impl Comparison {
    /// Raw selected execution records, preserving their order and joint support.
    /// SelectedPlan's coalesced authorization records are a different view.
    pub fn prepared(&self) -> &[PreparedChoice] {
        &self.executions[self.values.selected].prepared
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeedsReview;

/// Explicit incomplete coverage. No partial candidate/world values are exposed
/// for ranking; owner retains baseline or reports deferred reassessment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deferred {
    pub reason: SequenceError,
    pub work: Work,
    pub total_candidates: usize,
    pub limit: Option<DeferredLimit>,
}

/// The failed lane, separate from the stable primitive SequenceError contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeferredLimit {
    Deadline,
    RepresentedTransitions {
        required: usize,
        remaining: usize,
    },
    RepresentedComparisons {
        required: usize,
        remaining: usize,
    },
    RepresentedSolverNodes {
        required: u64,
        remaining: u64,
    },
    SolverAccountingOverflow,
    LeafUnavailable {
        actor: u8,
    },
    FiniteContinuation {
        max_steps: usize,
    },
    Reconsideration {
        remaining: u8,
    },
    /// An explicitly authored phase gate exhausted its finite continuation.
    AuthoredGate,
}

/// Same primitive dispatcher as forecasting. A retained selected suffix is only
/// usable in its original conditional-measure scope with exact observed history,
/// configuration, posterior generation and newly bound candidate membership.
pub fn prepared_lookup(
    receipt: &Receipt,
    view: &SeatView<'_>,
    legal: &[GameAction],
    mind: &Mind,
    context: &Context,
    membership: &Membership,
    prepared: &[PreparedChoice],
) -> Result<Offer, NeedsReview> {
    prepared_lookup_with_boundary(receipt, view, legal, mind, context, membership, None, prepared)
}

/// A composed initial comparison has its own complete domain (authored routes
/// plus the retained child). Nested choices remain scoped to ordinary authored
/// membership. Never rewrite stored records to pretend those domains agree.
pub fn prepared_lookup_with_boundary(
    receipt: &Receipt,
    view: &SeatView<'_>,
    legal: &[GameAction],
    mind: &Mind,
    context: &Context,
    authored_membership: &Membership,
    boundary_membership: Option<&Membership>,
    prepared: &[PreparedChoice],
) -> Result<Offer, NeedsReview> {
    let eligible = crate::legal_actions::eligible(legal);
    let legal = eligible.as_slice();
    if !receipt.accepted.matches(view) || receipt.pending.is_some() {
        return Err(NeedsReview);
    }
    let mut stage = receipt.stage;
    let initial = receipt.program.suffix(stage).map_err(|_| NeedsReview)?;
    for _ in 0..crate::concepts::sequence::MAX_STAGES {
        let inspection = inspect(&receipt.program, stage, view, legal, mind).map_err(|_| NeedsReview)?;
        let (offers, bound) = match inspection.frontier {
            Frontier::Actions(offers) => {
                let bound = offers
                    .iter()
                    .map(|offer| {
                        Ok(Selection::Offer {
                            action: offer.action,
                            next: receipt.program.suffix(offer.next).map_err(|_| NeedsReview)?,
                        })
                    })
                    .collect::<Result<Vec<_>, NeedsReview>>()?;
                (Some(offers), bound)
            }
            Frontier::Choice(stages) => (
                None,
                stages
                    .into_iter()
                    .map(|stage| {
                        receipt
                            .program
                            .suffix(stage)
                            .map(Selection::Stage)
                            .map_err(|_| NeedsReview)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Frontier::Tail
            | Frontier::Interrupted
            | Frontier::Reconsider { .. }
            | Frontier::Response { .. }
            | Frontier::Deferred => return Err(NeedsReview),
        };
        // Even a now-singleton choice must validate modeled scope: an
        // opportunity/evidence change cannot silently reuse its old assessment.
        let suffix = receipt.program.suffix(stage).map_err(|_| NeedsReview)?;
        let membership = if suffix == initial {
            boundary_membership.unwrap_or(authored_membership)
        } else {
            authored_membership
        };
        let choice = prepared
            .iter()
            .find(|choice| {
                choice.information.matches(view)
                    && choice.semantic_suffix == suffix
                    && &choice.context == context
                    && &choice.membership == membership
            })
            .ok_or(NeedsReview)?;
        if choice.alternatives != bound || choice.support.is_empty() {
            return Err(NeedsReview);
        }
        let expected_scope = if suffix == initial && boundary_membership.is_some() {
            ChoiceScope::Global
        } else if choice.assessment.is_some() {
            ChoiceScope::Local
        } else {
            ChoiceScope::FixedAction
        };
        if choice.scope != expected_scope {
            return Err(NeedsReview);
        }
        match &choice.assessment {
            Some(assessment)
                if assessment.cohort_complete
                    && assessment.candidates.len() == bound.len()
                    && assessment.selected == choice.selected => {}
            None if bound.len() == 1 && choice.selected == 0 => {}
            _ => return Err(NeedsReview),
        }
        let selected = choice.alternatives.get(choice.selected).ok_or(NeedsReview)?;
        match selected {
            Selection::Stage(suffix) if offers.is_none() => {
                stage = find_stage(&receipt.program, suffix).map_err(|_| NeedsReview)?
            }
            Selection::Offer { action, next } => {
                let offer = Offer {
                    action: *action,
                    next: find_stage(&receipt.program, next).map_err(|_| NeedsReview)?,
                };
                if !offers.as_ref().is_some_and(|offers| offers.contains(&offer)) {
                    return Err(NeedsReview);
                }
                return Ok(offer);
            }
            _ => return Err(NeedsReview),
        }
    }
    Err(NeedsReview)
}

#[derive(Clone)]
struct Branch {
    table: Table,
    origin: usize,
    weight: f64,
    receipt: Receipt,
    first: Option<Offer>,
    first_program: Option<Program>,
    tricks: Vec<TrickOutcome>,
    consumed: Vec<Consumed>,
    interrupted: bool,
}
#[derive(Clone, PartialEq, Eq)]
struct ResponseKey {
    information: Information,
    legal: Vec<GameAction>,
}
/// Private conditional-input identity. Only Table/actor-local leaf sees these
/// four views; root guards/traces receive the root projection exclusively.
#[derive(Clone, PartialEq)]
struct BranchKey {
    views: [Information; 4],
    actor: Option<u8>,
    legal: Vec<GameAction>,
    origin: usize,
    weight: f64,
    suffix: Suffix,
    accepted: Information,
    pending: Option<Selection>,
}
impl BranchKey {
    fn of(branch: &Branch, suffix: Suffix, pending: Option<Selection>) -> Self {
        let (actor, legal) = branch
            .table
            .to_act()
            .map(|(seat, legal)| (Some(seat), crate::legal_actions::eligible(legal)))
            .unwrap_or_default();
        Self {
            views: std::array::from_fn(|seat| Information::of(&branch.table.view(seat as u8))),
            actor,
            legal,
            origin: branch.origin,
            weight: branch.weight,
            suffix,
            accepted: branch.receipt.accepted.clone(),
            pending,
        }
    }
}
struct CachedRun {
    key: Vec<BranchKey>,
    terminals: Vec<Terminal>,
    prepared: Vec<PreparedChoice>,
    transitions: usize,
    comparisons: usize,
    solver_nodes: u64,
}
struct TailRun {
    views: [Information; 4],
    legal: Vec<GameAction>,
    outcome: GameOutcome,
    first: GameAction,
    tricks: Vec<TrickOutcome>,
    consumed: Vec<Consumed>,
    transitions: usize,
    solver_nodes: u64,
}
/// Exact authored input, with validation and demanded canonical stages reused.
/// Slots are filled lazily; no eager compilation of every possible suffix.
struct CanonicalProgram {
    program: Program,
    suffixes: Vec<Option<Suffix>>,
}
struct AuthoredMembership {
    information: Information,
    membership: Membership,
}
struct Session<'a, F> {
    root: u8,
    provenance: Provenance,
    context: &'a Context,
    limits: Limits,
    work: Work,
    leaf: &'a mut F,
    responses: Vec<(ResponseKey, LeafResponse)>,
    hard_reads: Vec<(Information, beliefs::ConceptQuery, Option<bool>)>,
    memo: Vec<CachedRun>,
    tails: Vec<TailRun>,
    canonical: Vec<CanonicalProgram>,
    memberships: Vec<AuthoredMembership>,
    reusable_bytes: usize,
    response_bytes: usize,
    limit: Option<DeferredLimit>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Alternative {
    Stage(Stage),
    Offer(Offer),
    Reconsider { stage: Stage, remaining: u8 },
    Response { next: Stage },
}
struct Waiting {
    branch: Branch,
    alternatives: Vec<Alternative>,
}
fn find_stage(program: &Program, suffix: &Suffix) -> Result<Stage, SequenceError> {
    for stage in 0..program.nodes.len() {
        if program.suffix(Stage(stage))? == *suffix {
            return Ok(Stage(stage));
        }
    }
    Err(SequenceError::InvalidProgram)
}
impl<'context, F: FnMut(&SeatView<'_>, &[GameAction], LeafBudget) -> Result<LeafResponse, LeafFailure>> Session<'context, F> {
    fn limited(&mut self, limit: DeferredLimit) -> SequenceError {
        self.limit = Some(limit);
        SequenceError::Budget
    }
    fn check(&mut self) -> Result<(), SequenceError> {
        if self.limits.deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(self.limited(DeferredLimit::Deadline));
        }
        Ok(())
    }
    /// Completed cached units cost the same logical work as their expansion.
    /// Reject an unaffordable unit before exposing any of its values or trace.
    fn charge(&mut self, transitions: usize, comparisons: usize) -> Result<(), SequenceError> {
        self.charge_work(transitions, comparisons, 0)
    }
    fn remaining_solver_nodes(&self) -> Option<u64> {
        self.limits
            .solver_nodes
            .map(|limit| limit.saturating_sub(self.work.represented_solver_nodes))
    }
    fn charge_solver_nodes(&mut self, nodes: u64) -> Result<(), SequenceError> {
        if let Some(remaining) = self.remaining_solver_nodes() {
            if nodes > remaining {
                return Err(self.limited(DeferredLimit::RepresentedSolverNodes {
                    required: nodes,
                    remaining,
                }));
            }
        }
        self.work.represented_solver_nodes = self
            .work
            .represented_solver_nodes
            .checked_add(nodes)
            .ok_or_else(|| self.limited(DeferredLimit::SolverAccountingOverflow))?;
        Ok(())
    }
    fn charge_work(&mut self, transitions: usize, comparisons: usize, nodes: u64) -> Result<(), SequenceError> {
        self.check()?;
        let remaining = self
            .limits
            .transitions
            .saturating_sub(self.work.represented_transitions);
        if transitions > remaining {
            return Err(self.limited(DeferredLimit::RepresentedTransitions {
                required: transitions,
                remaining,
            }));
        }
        let remaining = self
            .limits
            .comparisons
            .saturating_sub(self.work.represented_comparisons);
        if comparisons > remaining {
            return Err(self.limited(DeferredLimit::RepresentedComparisons {
                required: comparisons,
                remaining,
            }));
        }
        self.charge_solver_nodes(nodes)?;
        self.work.represented_transitions += transitions;
        self.work.represented_comparisons += comparisons;
        Ok(())
    }
    fn retain(&mut self, bytes: usize) -> bool {
        if bytes > self.limits.memo_bytes.saturating_sub(self.work.retained_bytes) {
            return false;
        }
        self.work.retained_bytes += bytes;
        true
    }
    fn retain_reusable(&mut self, bytes: usize) -> bool {
        if !self.retain(bytes) {
            return false;
        }
        self.reusable_bytes += bytes;
        true
    }
    /// Completed deterministic responses may be evicted and recomputed. Cache
    /// admission changes actual work only, never represented candidate coverage.
    /// Reject an oversized payload before evicting or copying anything.
    fn retain_response(&mut self, bytes: usize) -> bool {
        let max_entries = self.limits.memo_entries.saturating_mul(32);
        if max_entries == 0 || bytes > self.limits.memo_bytes {
            return false;
        }
        if self.responses.len() >= max_entries
            || bytes > self.limits.memo_bytes.saturating_sub(self.work.retained_bytes)
        {
            let _timing = beliefs::profile::scope("sequence.eviction");
            #[cfg(feature = "profile")]
            {
                // Fixed event labels only: no keys, payloads or lost-key count.
                // The original pressure condition above retains its short circuit.
                let entries = self.responses.len() >= max_entries;
                let bytes = bytes > self.limits.memo_bytes.saturating_sub(self.work.retained_bytes);
                drop(beliefs::profile::scope(match (entries, bytes) {
                    (true, true) => "sequence.eviction.reason.both",
                    (true, false) => "sequence.eviction.reason.entries",
                    _ => "sequence.eviction.reason.bytes",
                }));
            }
            if self.responses.len() >= max_entries
                && bytes <= self.limits.memo_bytes.saturating_sub(self.work.retained_bytes)
            {
                // Entry-only pressure cannot be relieved by dropping other
                // dependencies. Reclaim only responses, including their capacity.
                self.work.retained_bytes -= self.response_bytes;
                self.reusable_bytes -= self.response_bytes;
                self.responses = Vec::new();
                self.response_bytes = 0;
                return self.retain(bytes);
            }
            #[cfg(feature = "profile")]
            if !self.memo.is_empty() {
                drop(beliefs::profile::scope("sequence.eviction.memo_nonempty"));
            }
            // Reclaim optional traces first; drop capacities as well as elements.
            self.memo = Vec::new();
            self.tails = Vec::new();
            self.hard_reads = Vec::new();
            self.canonical = Vec::new();
            self.memberships = Vec::new();
            self.work.retained_bytes = self.response_bytes;
            self.reusable_bytes = self.response_bytes;
            if self.responses.len() >= max_entries
                || bytes > self.limits.memo_bytes.saturating_sub(self.work.retained_bytes)
            {
                // A simple bounded batch eviction avoids another hot-path index.
                self.responses = Vec::new();
                self.response_bytes = 0;
                self.work.retained_bytes = 0;
                self.reusable_bytes = 0;
            }
        }
        self.retain(bytes)
    }
    fn suffix(&mut self, program: &Program, stage: Stage) -> Result<Suffix, SequenceError> {
        let _timing = beliefs::profile::scope("sequence.suffix");
        let cached = self.canonical_program(program)?;
        self.cached_suffix(program, stage, cached)
    }
    /// One exact lookup/validation for a pure batch over an immutable program.
    /// The iterator borrows the session, so no response/eviction can invalidate
    /// its local index. An empty batch performs no validation or admission.
    fn suffixes<'s, I>(
        &'s mut self,
        program: &'s Program,
        stages: I,
    ) -> impl Iterator<Item = Result<Suffix, SequenceError>> + 's + use<'s, 'context, I, F>
    where
        I: IntoIterator<Item = Stage> + 's,
        I::IntoIter: 's,
    {
        let mut located = None;
        stages.into_iter().map(move |stage| {
            let _timing = beliefs::profile::scope("sequence.suffix");
            let cached = match located {
                Some(cached) => cached,
                None => {
                    let cached = self.canonical_program(program)?;
                    located = Some(cached);
                    cached
                }
            };
            self.cached_suffix(program, stage, cached)
        })
    }
    /// Reuse exact input validation without hashing the entire authored model.
    /// A batch/scan calls this once, retaining the original linear Eq lookup.
    fn canonical_program(&mut self, program: &Program) -> Result<Option<usize>, SequenceError> {
        let cached = {
            let _timing = beliefs::profile::scope("sequence.suffix_lookup");
            self.canonical.iter().position(|entry| entry.program == *program)
        };
        if let Some(index) = cached {
            return Ok(Some(index));
        }
        {
            let _timing = beliefs::profile::scope("sequence.suffix_validate");
            program.validate()?;
        }
        // Even full/disabled caches validate the entire input, but need no
        // allocation ledger traversal when an entry cannot be admitted.
        if self.canonical.len() >= self.limits.memo_entries {
            return Ok(None);
        }
        let _timing = beliefs::profile::scope("sequence.suffix_program_admission");
        let bytes = 2
            * (std::mem::size_of::<CanonicalProgram>()
                + program_bytes(program)
                + std::mem::size_of::<Vec<Option<Suffix>>>()
                + program.nodes.len() * std::mem::size_of::<Option<Suffix>>());
        if self.retain_reusable(bytes) {
            let index = self.canonical.len();
            self.canonical.push(CanonicalProgram {
                program: program.clone(),
                suffixes: vec![None; program.nodes.len()],
            });
            Ok(Some(index))
        } else {
            Ok(None)
        }
    }
    // The index is local to one call/batch/scan: no insertion or eviction occurs
    // between lookup and use. Never keep indices in receipts or other caches.
    fn cached_suffix(&mut self, program: &Program, stage: Stage, cached: Option<usize>) -> Result<Suffix, SequenceError> {
        if let Some(index) = cached {
            if let Some(suffix) = self.canonical[index].suffixes.get(stage.0).and_then(Option::as_ref) {
                let _copy_timing = beliefs::profile::scope("sequence.suffix_copy");
                return Ok(suffix.clone());
            }
        }
        // Either this exact input was validated above, or it equals a validated
        // cache entry. Lack of cache space never skips validation or a stage.
        let suffix = {
            let _timing = beliefs::profile::scope("sequence.suffix_construct");
            program.suffix_validated(stage)?
        };
        if let Some(index) = cached {
            let _timing = beliefs::profile::scope("sequence.suffix_slot_admission");
            if self.retain_reusable(2 * suffix_bytes(&suffix)) {
                self.canonical[index].suffixes[stage.0] = Some(suffix.clone());
            }
        }
        Ok(suffix)
    }
    fn selection(&mut self, program: &Program, alternative: &Alternative) -> Result<Selection, SequenceError> {
        Ok(match alternative {
            Alternative::Stage(stage) => Selection::Stage(self.suffix(program, *stage)?),
            Alternative::Offer(offer) => Selection::Offer {
                action: offer.action,
                next: self.suffix(program, offer.next)?,
            },
            Alternative::Reconsider { .. } | Alternative::Response { .. } => return Err(SequenceError::InvalidProgram),
        })
    }
    fn find_stage(&mut self, program: &Program, suffix: &Suffix) -> Result<Stage, SequenceError> {
        let _timing = beliefs::profile::scope("sequence.suffix");
        // Validate/locate this immutable input once for the whole scan, also
        // when cache admission is disabled. Preserve first matching stage.
        let cached = self.canonical_program(program)?;
        let _scan_timing = beliefs::profile::scope("sequence.suffix_scan");
        for stage in 0..program.nodes.len() {
            let matches = match cached.and_then(|index| self.canonical[index].suffixes[stage].as_ref()) {
                Some(existing) => existing == suffix,
                None => self.cached_suffix(program, Stage(stage), cached)? == *suffix,
            };
            if matches {
                return Ok(Stage(stage));
            }
        }
        Err(SequenceError::InvalidProgram)
    }
    fn membership(&mut self, view: &SeatView<'_>, legal: &[GameAction]) -> Result<Membership, SequenceError> {
        let _timing = beliefs::profile::scope("sequence.membership");
        let eligible = crate::legal_actions::eligible(legal);
        let legal = eligible.as_slice();
        if let Some(entry) = self
            .memberships
            .iter()
            .find(|entry| entry.information.matches(view) && entry.membership.legal == legal)
        {
            return Ok(entry.membership.clone());
        }
        let mut programs = Vec::new();
        for program in authored_programs(view, legal) {
            let suffix = self.suffix(&program, program.start)?;
            if !programs.contains(&suffix) {
                programs.push(suffix);
            }
        }
        let membership = Membership {
            legal: crate::legal_actions::eligible(legal),
            programs,
        };
        let bytes = 2
            * (std::mem::size_of::<AuthoredMembership>()
                + std::mem::size_of::<Information>()
                + std::mem::size_of::<Vec<observation::SeatEvent>>()
                + std::mem::size_of_val(view.events)
                + membership_bytes(&membership));
        if self.memberships.len() < self.limits.memo_entries && self.retain_reusable(bytes) {
            self.memberships.push(AuthoredMembership {
                information: Information::of(view),
                membership: membership.clone(),
            });
        }
        Ok(membership)
    }
    fn response(&mut self, table: &Table) -> Result<GameAction, SequenceError> {
        let _timing = beliefs::profile::scope("sequence.response");
        self.check()?;
        if self.work.represented_transitions >= self.limits.transitions {
            return Err(self.limited(DeferredLimit::RepresentedTransitions {
                required: 1,
                remaining: 0,
            }));
        }
        let (seat, legal) = table.to_act().ok_or(SequenceError::WrongContext)?;
        let eligible = crate::legal_actions::eligible(legal);
        let legal = eligible.as_slice();
        let view = table.view(seat);
        if let Some((_, response)) = self
            .responses
            .iter()
            .find(|(key, _)| key.information.matches(&view) && key.legal == legal)
        {
            let response = *response;
            self.charge_work(0, 0, response.canonical_solver_nodes)?;
            self.work.response_hits += 1;
            return Ok(response.action);
        }
        let budget = LeafBudget {
            deadline: self.limits.deadline,
            remaining_transitions: self
                .limits
                .transitions
                .saturating_sub(self.work.represented_transitions),
            remaining_solver_nodes: self.remaining_solver_nodes(),
        };
        let result = (self.leaf)(&view, legal, budget);
        self.work.response_calls += 1;
        let nodes = match &result {
            Ok(response) => response.canonical_solver_nodes,
            Err(failure) => failure.canonical_solver_nodes,
        };
        // Count actual attempted work even on a deadline/failure. Charge it once
        // before checking wall time; an incomplete leaf never enters any cache.
        self.work.actual_solver_nodes = self
            .work
            .actual_solver_nodes
            .checked_add(nodes)
            .ok_or_else(|| self.limited(DeferredLimit::SolverAccountingOverflow))?;
        self.charge_solver_nodes(nodes)?;
        self.check()?;
        let response = match result {
            Ok(response) => response,
            Err(LeafFailure {
                reason: SequenceError::Budget,
                ..
            }) => {
                return Err(self.limited(DeferredLimit::LeafUnavailable { actor: seat }));
            }
            Err(failure) => return Err(failure.reason),
        };
        if !legal.contains(&response.action) || response.action.player.0 != seat {
            return Err(SequenceError::IllegalAction);
        }
        let bytes = 2
            * (std::mem::size_of::<ResponseKey>()
                + std::mem::size_of_val(view.events)
                + std::mem::size_of_val(legal)
                + std::mem::size_of::<LeafResponse>());
        if self.retain_response(bytes) {
            self.responses.push((
                ResponseKey {
                    information: Information::of(&view),
                    legal: legal.to_vec(),
                },
                response,
            ));
            self.reusable_bytes += bytes;
            self.response_bytes += bytes;
        }
        Ok(response.action)
    }

    fn apply(&mut self, branch: &mut Branch, action: GameAction) -> Result<(), SequenceError> {
        let _timing = beliefs::profile::scope("sequence.apply");
        self.check()?;
        if !branch.table.to_act().is_some_and(|(_, legal)| legal.contains(&action)) {
            return Err(SequenceError::IllegalAction);
        }
        self.charge(1, 0)?;
        let before = branch.table.game.state.all_tricks.len();
        branch.table.apply(action);
        self.work.transitions += 1;
        if branch.table.game.state.all_tricks.len() > before {
            let trick = branch
                .table
                .game
                .state
                .all_tricks
                .last()
                .ok_or(SequenceError::WrongContext)?;
            let view = branch.table.view(self.root);
            let (actor, phase) = branch
                .table
                .to_act()
                .map(|(actor, legal)| (actor, PlayPhase::from_actions(legal)))
                .unwrap_or((self.root, PlayPhase::Cards));
            branch.tricks.push(TrickOutcome {
                leader: trick.leader.0,
                winner: trick.winner.0,
                cards: trick.cards.map(|card| card.index()),
                points: trick.points.0,
                successor: view.knowledge.public_state(actor, phase),
                root: Information::of(&view),
            });
        }
        Ok(())
    }
    fn inspect(&mut self, branch: &Branch) -> Result<crate::concepts::sequence::Inspection, SequenceError> {
        let _timing = beliefs::profile::scope("sequence.inspect");
        let view = branch.table.view(self.root);
        let legal = crate::legal_actions::eligible(branch.table.to_act().ok_or(SequenceError::WrongContext)?.1);
        inspect_with(&branch.receipt.program, branch.receipt.stage, &view, &legal, |query| {
            if let Some((_, _, truth)) = self
                .hard_reads
                .iter()
                .find(|(information, old, _)| information.matches(&view) && old == query)
            {
                return *truth;
            }
            // Table already owns the observer's filtered, tracked Knowledge.
            // Use the same zero-budget proof kernel without replaying history
            // or constructing a Mind; invalid queries remain unresolved here.
            let truth = query.hard(view.knowledge).ok().and_then(|estimate| estimate.known);
            let information = Information::of(&view);
            let bytes = 2 * (info_bytes(&information) + query_bytes(query) + std::mem::size_of::<Option<bool>>());
            if self.hard_reads.len() < self.limits.memo_entries && self.retain_reusable(bytes) {
                self.hard_reads.push((information, query.clone(), truth));
            }
            truth
        })
    }
    fn drive(
        &mut self,
        mut branch: Branch,
        prepared: &mut Vec<PreparedChoice>,
    ) -> Result<Result<Terminal, Waiting>, SequenceError> {
        for _ in 0..128 {
            self.check()?;
            if branch.table.game.ended() {
                let info = GameFinishedInfo::from(branch.table.game);
                return Ok(Ok(Terminal {
                    origin: branch.origin,
                    weight: branch.weight,
                    outcome: info.outcome(),
                    tricks: branch.tricks,
                    consumed: branch.consumed,
                    interrupted: branch.interrupted,
                    first: branch.first,
                    first_program: branch.first_program,
                }));
            }
            let (actor, _) = branch.table.to_act().ok_or(SequenceError::WrongContext)?;
            if actor != self.root {
                let action = self.response(&branch.table)?;
                self.apply(&mut branch, action)?;
                continue;
            }
            branch.receipt.synchronize(&branch.table.view(self.root))?;
            let inspection = self.inspect(&branch)?;
            let information = Information::of(&branch.table.view(self.root));
            let suffixes = self.suffixes(&branch.receipt.program, inspection.consumed.iter().map(|read| read.stage));
            for (read, suffix) in inspection.consumed.iter().zip(suffixes) {
                branch.consumed.push(Consumed {
                    information: information.clone(),
                    suffix: suffix?,
                    reading: read.meaning.clone(),
                });
            }
            let interrupted = inspection.frontier == Frontier::Interrupted;
            match inspection.frontier {
                Frontier::Choice(stages) => {
                    return Ok(Err(Waiting {
                        branch,
                        alternatives: stages.into_iter().map(Alternative::Stage).collect(),
                    }))
                }
                Frontier::Reconsider { stage, remaining, .. } => {
                    branch.receipt.stage = stage;
                    return Ok(Err(Waiting {
                        branch,
                        alternatives: vec![Alternative::Reconsider { stage, remaining }],
                    }));
                }
                Frontier::Response { next } => {
                    // Keep the entry cursor (and preceding guards) in the
                    // semantic key. Full root Information is grouped before
                    // invoking the strict response approximation, never before
                    // a hidden-world-specific optimum is exposed.
                    return Ok(Err(Waiting {
                        branch,
                        alternatives: vec![Alternative::Response { next }],
                    }));
                }
                Frontier::Deferred => return Err(self.limited(DeferredLimit::AuthoredGate)),
                Frontier::Actions(offers) if offers.len() > 1 => {
                    return Ok(Err(Waiting {
                        branch,
                        alternatives: offers.into_iter().map(Alternative::Offer).collect(),
                    }))
                }
                Frontier::Actions(mut offers) => {
                    let offer = offers.remove(0);
                    let legal = branch.table.to_act().ok_or(SequenceError::WrongContext)?.1;
                    let fixed = PreparedChoice {
                        scope: ChoiceScope::FixedAction,
                        information: information.clone(),
                        semantic_suffix: self.suffix(&branch.receipt.program, branch.receipt.stage)?,
                        context: self.context.clone(),
                        membership: self.membership(&branch.table.view(self.root), legal)?,
                        alternatives: vec![self.selection(&branch.receipt.program, &Alternative::Offer(offer.clone()))?],
                        selected: 0,
                        assessment: None,
                        consumed: branch.consumed.clone(),
                        support: vec![(branch.origin, branch.weight)],
                    };
                    prepared.push(fixed);
                    self.commit(&mut branch, offer)?;
                }
                Frontier::Tail | Frontier::Interrupted => {
                    branch.interrupted |= interrupted;
                    let tail = branch
                        .receipt
                        .program
                        .nodes
                        .iter()
                        .position(|node| matches!(node, crate::concepts::sequence::Node::Tail))
                        .ok_or(SequenceError::InvalidProgram)?;
                    branch.receipt.stage = Stage(tail);
                    return self.tail(branch, Stage(tail)).map(Ok);
                }
            }
        }
        Err(self.limited(DeferredLimit::FiniteContinuation { max_steps: 128 }))
    }
    fn commit(&mut self, branch: &mut Branch, offer: Offer) -> Result<(), SequenceError> {
        branch.receipt.propose(
            offer.clone(),
            &branch.table.view(self.root),
            &crate::legal_actions::eligible(branch.table.to_act().ok_or(SequenceError::WrongContext)?.1),
        )?;
        self.first_step(branch, &offer)?;
        self.apply(branch, offer.action)
    }
    fn first_step(&mut self, branch: &mut Branch, offer: &Offer) -> Result<(), SequenceError> {
        if branch.first.is_none() {
            let suffix = self.suffix(&branch.receipt.program, branch.receipt.stage)?;
            let next = self.suffix(&branch.receipt.program, offer.next)?;
            let program = Program {
                name: "evaluated-first-step",
                version: 1,
                start: suffix.start,
                nodes: suffix.nodes,
            };
            branch.first = Some(Offer {
                action: offer.action,
                next: self.find_stage(&program, &next)?,
            });
            branch.first_program = Some(program);
        }
        Ok(())
    }
    /// The named leaf has NO root suffix choices, so its frozen modeled result
    /// can be shared per complete hypothetical input without strategy fusion.
    /// This boundary also shares tails reached through different guard paths.
    fn tail(&mut self, mut branch: Branch, tail: Stage) -> Result<Terminal, SequenceError> {
        let (views, legal, cached) = {
            let _timing = beliefs::profile::scope("sequence.tail_key");
            let views = std::array::from_fn(|seat| Information::of(&branch.table.view(seat as u8)));
            let legal = crate::legal_actions::eligible(branch.table.to_act().ok_or(SequenceError::WrongContext)?.1);
            let cached = self
                .limits
                .reuse_suffixes
                .then(|| {
                    self.tails
                        .iter()
                        .position(|run| run.views == views && run.legal == legal)
                })
                .flatten();
            (views, legal, cached)
        };
        let (outcome, first, tricks, consumed) = if let Some(index) = cached {
            self.charge_work(self.tails[index].transitions, 0, self.tails[index].solver_nodes)?;
            self.work.suffix_hits += 1;
            let _copy_timing = beliefs::profile::scope("sequence.memo_copy");
            let run = &self.tails[index];
            (run.outcome, run.first, run.tricks.clone(), run.consumed.clone())
        } else {
            let before = self.work.represented_transitions;
            let solver_before = self.work.represented_solver_nodes;
            let trick_start = branch.tricks.len();
            let read_start = branch.consumed.len();
            let mut first = None;
            for _ in 0..128 {
                self.check()?;
                if branch.table.game.ended() {
                    break;
                }
                let (actor, _) = branch.table.to_act().ok_or(SequenceError::WrongContext)?;
                if actor == self.root {
                    branch.consumed.push(Consumed {
                        information: Information::of(&branch.table.view(self.root)),
                        suffix: Suffix {
                            start: Stage(0),
                            nodes: vec![crate::concepts::sequence::Node::Tail],
                        },
                        reading: ReadMeaning::ResponseLeaf,
                    });
                }
                let action = self.response(&branch.table)?;
                first.get_or_insert(action);
                self.apply(&mut branch, action)?;
            }
            if !branch.table.game.ended() {
                return Err(self.limited(DeferredLimit::FiniteContinuation { max_steps: 128 }));
            }
            let info = GameFinishedInfo::from(branch.table.game.clone());
            let run = TailRun {
                views,
                legal,
                outcome: info.outcome(),
                first: first.ok_or(SequenceError::WrongContext)?,
                tricks: branch.tricks.split_off(trick_start),
                consumed: branch.consumed.split_off(read_start),
                transitions: self.work.represented_transitions - before,
                solver_nodes: self.work.represented_solver_nodes - solver_before,
            };
            let bytes = {
                let _timing = beliefs::profile::scope("sequence.tail_ledger");
                let bytes = 2
                    * (std::mem::size_of_val(&run)
                        + run.views.iter().map(info_bytes).sum::<usize>()
                        + vec_bytes(&run.legal)
                        + vec_bytes(&run.tricks)
                        + run.tricks.iter().map(|trick| info_bytes(&trick.root)).sum::<usize>()
                        + vec_bytes(&run.consumed)
                        + run.consumed.iter().map(consumed_bytes).sum::<usize>());
                bytes
            };
            if self.limits.reuse_suffixes && self.tails.len() < self.limits.memo_entries && self.retain_reusable(bytes)
            {
                let _copy_timing = beliefs::profile::scope("sequence.memo_copy");
                self.tails.push(TailRun {
                    views: run.views,
                    legal: run.legal,
                    outcome: run.outcome,
                    first: run.first,
                    tricks: run.tricks.clone(),
                    consumed: run.consumed.clone(),
                    transitions: run.transitions,
                    solver_nodes: run.solver_nodes,
                });
            }
            (run.outcome, run.first, run.tricks, run.consumed)
        };
        self.first_step(
            &mut branch,
            &Offer {
                action: first,
                next: tail,
            },
        )?;
        branch.tricks.extend(tricks);
        branch.consumed.extend(consumed);
        Ok(Terminal {
            origin: branch.origin,
            weight: branch.weight,
            outcome,
            tricks: branch.tricks,
            consumed: branch.consumed,
            interrupted: branch.interrupted,
            first: branch.first,
            first_program: branch.first_program,
        })
    }
    fn values<'t>(
        &mut self,
        executions: impl IntoIterator<Item = &'t [Terminal]>,
    ) -> Result<Assessment, SequenceError> {
        let _timing = beliefs::profile::scope("sequence.values");
        self.charge(0, 1)?;
        self.work.comparisons += 1;
        let outcomes: Vec<Vec<_>> = executions
            .into_iter()
            .map(|run| run.iter().map(|t| (t.outcome, t.weight)).collect())
            .collect();
        let slices: Vec<_> = outcomes.iter().map(Vec::as_slice).collect();
        ValueStore::new(&slices, self.root as usize % 2, self.provenance)
            .and_then(|mut store| store.assess())
            .map_err(|_| SequenceError::InvalidValue)
    }
    /// COMPLETE suffix reuse keyed by the entire joint input, weights, semantic
    /// DAG and pending intent. Prefix traces/first actions are reattached. No
    /// individual-world oracle result is transferred into a different group.
    fn finish(
        &mut self,
        mut branches: Vec<Branch>,
        prepared: &mut Vec<PreparedChoice>,
    ) -> Result<Vec<Terminal>, SequenceError> {
        self.check()?;
        let key = {
            let _timing = beliefs::profile::scope("sequence.joint_key");
            branches.sort_by_key(|branch| branch.origin);
            let key = branches
                .iter()
                .map(|branch| {
                    let suffix = self.suffix(&branch.receipt.program, branch.receipt.stage)?;
                    let pending = branch
                        .receipt
                        .pending
                        .as_ref()
                        .map(|offer| self.selection(&branch.receipt.program, &Alternative::Offer(offer.clone())))
                        .transpose()?;
                    Ok(BranchKey::of(branch, suffix, pending))
                })
                .collect::<Result<Vec<_>, SequenceError>>()?;
            key
        };
        let prefixes = {
            let _timing = beliefs::profile::scope("sequence.trace");
            let prefixes: Vec<_> = branches
                .iter_mut()
                .map(|branch| {
                    (
                        branch.origin,
                        branch.first.take(),
                        branch.first_program.take(),
                        std::mem::take(&mut branch.tricks),
                        std::mem::take(&mut branch.consumed),
                        std::mem::take(&mut branch.interrupted),
                    )
                })
                .collect();
            prefixes
        };
        let cached = {
            let _timing = beliefs::profile::scope("sequence.memo_lookup");
            let cached = self.memo.iter().position(|run| run.key == key);
            cached
        };
        // Short event scopes count lookup outcomes, independently of whether a
        // found result can be charged or a missing dependency later completes.
        drop(beliefs::profile::scope(if cached.is_some() {
            "sequence.finish.lookup_hit"
        } else {
            "sequence.finish.lookup_miss"
        }));
        let mut terminals = if let Some(index) = cached {
            self.charge_work(
                self.memo[index].transitions,
                self.memo[index].comparisons,
                self.memo[index].solver_nodes,
            )?;
            self.work.suffix_hits += 1;
            let _copy_timing = beliefs::profile::scope("sequence.finish.hit_copy");
            prepared.extend(self.memo[index].prepared.iter().cloned());
            self.memo[index].terminals.clone()
        } else {
            let before = self.work.represented_transitions;
            let comparisons_before = self.work.represented_comparisons;
            let solver_before = self.work.represented_solver_nodes;
            let mut records = Vec::new();
            let terminals = self.finish_uncached(branches, &mut records)?;
            // Preserve the original left-to-right short circuit: disabled and
            // full-entry cases must not compute run_bytes or attempt retention.
            if !self.limits.reuse_suffixes {
                drop(beliefs::profile::scope("sequence.finish.not_retained.disabled"));
            } else if self.memo.len() >= self.limits.memo_entries {
                drop(beliefs::profile::scope("sequence.finish.not_retained.entry_cap"));
            } else if !self.retain(run_bytes(&key, &terminals, &records)) {
                drop(beliefs::profile::scope("sequence.finish.not_retained.byte_cap"));
            } else {
                let _copy_timing = beliefs::profile::scope("sequence.finish.store_copy");
                self.memo.push(CachedRun {
                    key,
                    terminals: terminals.clone(),
                    prepared: records.clone(),
                    transitions: self.work.represented_transitions - before,
                    comparisons: self.work.represented_comparisons - comparisons_before,
                    solver_nodes: self.work.represented_solver_nodes - solver_before,
                });
                drop(beliefs::profile::scope("sequence.finish.stored"));
            }
            prepared.extend(records);
            terminals
        };
        let _trace_timing = beliefs::profile::scope("sequence.trace");
        for terminal in &mut terminals {
            let (_, first, first_program, tricks, consumed, interrupted) = prefixes
                .iter()
                .find(|prefix| prefix.0 == terminal.origin)
                .ok_or(SequenceError::InvalidCohort)?;
            if let Some(first) = first {
                terminal.first = Some(first.clone());
                terminal.first_program = first_program.clone();
            }
            terminal.tricks.splice(0..0, tricks.iter().cloned());
            terminal.consumed.splice(0..0, consumed.iter().cloned());
            terminal.interrupted |= interrupted;
        }
        if cached.is_some() {
            // A lookup hit rejected by charging or prefix reattachment is not
            // a served result. No event is emitted on those error paths.
            drop(beliefs::profile::scope("sequence.finish.hit_served"));
        }
        Ok(terminals)
    }
    fn finish_uncached(
        &mut self,
        branches: Vec<Branch>,
        prepared: &mut Vec<PreparedChoice>,
    ) -> Result<Vec<Terminal>, SequenceError> {
        let mut completed = Vec::new();
        let mut groups: Vec<(Suffix, Vec<Waiting>)> = Vec::new();
        for branch in branches {
            match self.drive(branch, prepared)? {
                Ok(terminal) => completed.push(terminal),
                Err(waiting) => {
                    let suffix = self.suffix(&waiting.branch.receipt.program, waiting.branch.receipt.stage)?;
                    let group = groups.iter().position(|(old, group)| {
                        group[0].branch.receipt.accepted == waiting.branch.receipt.accepted && *old == suffix
                    });
                    if let Some(group) = group {
                        groups[group].1.push(waiting);
                    } else {
                        groups.push((suffix, vec![waiting]));
                    }
                }
            }
        }
        for (_, group) in groups {
            completed.extend(self.choose(group, prepared)?);
        }
        completed.sort_by_key(|terminal| terminal.origin);
        Ok(completed)
    }
    fn choose(
        &mut self,
        group: Vec<Waiting>,
        prepared: &mut Vec<PreparedChoice>,
    ) -> Result<Vec<Terminal>, SequenceError> {
        if matches!(group[0].alternatives.as_slice(), [Alternative::Response { .. }]) {
            return self.response_group(group, prepared);
        }
        let boundary = match group[0].alternatives.as_slice() {
            [Alternative::Reconsider { stage, remaining }] => Some((*stage, *remaining)),
            _ => None,
        };
        if let Some((stage, remaining)) = boundary {
            return self.reconsider(group, stage, remaining, prepared);
        }
        self.choose_bound(group, prepared, None)
    }
    /// One deterministic response for the whole current root information set.
    /// This is a named approximation, not maximization or knowledge of intent.
    /// Resume the authored successor using real engine actions/observations.
    fn response_group(
        &mut self,
        group: Vec<Waiting>,
        prepared: &mut Vec<PreparedChoice>,
    ) -> Result<Vec<Terminal>, SequenceError> {
        self.check()?;
        let first = &group[0].branch;
        let view = first.table.view(self.root);
        let (actor, legal) = first.table.to_act().ok_or(SequenceError::WrongContext)?;
        let legal = crate::legal_actions::eligible(legal);
        if actor != self.root {
            return Err(SequenceError::WrongContext);
        }
        let information = Information::of(&view);
        let semantic_suffix = self.suffix(&first.receipt.program, first.receipt.stage)?;
        let [Alternative::Response { next }] = group[0].alternatives.as_slice() else {
            return Err(SequenceError::WrongContext);
        };
        let next_suffix = self.suffix(&first.receipt.program, *next)?;
        for waiting in &group {
            let [Alternative::Response { next }] = waiting.alternatives.as_slice() else {
                return Err(SequenceError::WrongContext);
            };
            if !information.matches(&waiting.branch.table.view(self.root))
                || !waiting.branch.table.to_act().is_some_and(|(seat, actions)| {
                    seat == self.root && crate::legal_actions::eligible(actions) == legal
                })
                || self.suffix(&waiting.branch.receipt.program, *next)? != next_suffix
            {
                return Err(SequenceError::WrongContext);
            }
        }
        let action = self.response(&first.table)?;
        prepared.push(PreparedChoice {
            scope: ChoiceScope::Response,
            information,
            semantic_suffix,
            context: self.context.clone(),
            // A fixed response has no strategically compared programme domain.
            // Only its exact eligible legal list and modeled input are certified.
            membership: Membership::from_candidates(&legal, &[])?,
            alternatives: vec![Selection::Offer {
                action,
                next: next_suffix,
            }],
            selected: 0,
            assessment: None,
            consumed: first.consumed.clone(),
            support: group
                .iter()
                .map(|waiting| (waiting.branch.origin, waiting.branch.weight))
                .collect(),
        });
        let mut branches = Vec::with_capacity(group.len());
        for waiting in group {
            let [Alternative::Response { next }] = waiting.alternatives.as_slice() else {
                return Err(SequenceError::WrongContext);
            };
            let offer = Offer { action, next: *next };
            let mut branch = waiting.branch;
            self.commit(&mut branch, offer)?;
            branches.push(branch);
        }
        self.finish(branches, prepared)
    }
    fn reconsider(
        &mut self,
        mut group: Vec<Waiting>,
        stage: Stage,
        remaining: u8,
        prepared: &mut Vec<PreparedChoice>,
    ) -> Result<Vec<Terminal>, SequenceError> {
        self.check()?;
        let membership = {
            let _timing = beliefs::profile::scope("sequence.bind");
            let first = &group[0].branch;
            let view = first.table.view(self.root);
            let legal = crate::legal_actions::eligible(first.table.to_act().ok_or(SequenceError::WrongContext)?.1);
            // finish_uncached grouped full root Information + canonical boundary
            // BEFORE binding/ranking. The binder is pure data expansion, not policy.
            let binding = match bind_reconsideration(&first.receipt.program, stage, &view, &legal) {
                Ok(binding) => binding,
                Err(SequenceError::Budget) => return Err(self.limited(DeferredLimit::Reconsideration { remaining })),
                Err(error) => return Err(error),
            };
            let membership = Membership {
                legal: crate::legal_actions::eligible(&binding.legal),
                programs: self
                    .suffixes(&binding.program, binding.alternatives.iter().copied())
                    .collect::<Result<_, _>>()?,
            };
            for waiting in &mut group {
                if !matches!(waiting.alternatives.as_slice(), [Alternative::Reconsider { remaining: n, .. }] if *n == remaining)
                {
                    return Err(SequenceError::WrongContext);
                }
                let view = waiting.branch.table.view(self.root);
                let legal =
                    crate::legal_actions::eligible(waiting.branch.table.to_act().ok_or(SequenceError::WrongContext)?.1);
                waiting
                    .branch
                    .receipt
                    .install_reconsideration_at_frontier(&binding, &view, &legal)?;
                waiting.alternatives = binding.alternatives.iter().copied().map(Alternative::Stage).collect();
            }
            membership
        };
        self.choose_bound(group, prepared, Some(membership))
    }
    fn choose_bound(
        &mut self,
        group: Vec<Waiting>,
        prepared: &mut Vec<PreparedChoice>,
        membership: Option<Membership>,
    ) -> Result<Vec<Terminal>, SequenceError> {
        let first = &group[0].branch;
        let view = first.table.view(self.root);
        let semantic_suffix = self.suffix(&first.receipt.program, first.receipt.stage)?;
        let alternatives = group[0]
            .alternatives
            .iter()
            .map(|a| self.selection(&first.receipt.program, a))
            .collect::<Result<Vec<_>, _>>()?;
        for waiting in &group {
            let own = waiting
                .alternatives
                .iter()
                .map(|a| self.selection(&waiting.branch.receipt.program, a))
                .collect::<Result<Vec<_>, _>>()?;
            if own != alternatives {
                return Err(SequenceError::WrongContext);
            }
        }
        let information = Information::of(&view);
        let scope = if membership.is_some() {
            ChoiceScope::Global
        } else {
            ChoiceScope::Local
        };
        let membership = match membership {
            Some(membership) => membership,
            None => self.membership(&view, first.table.to_act().ok_or(SequenceError::WrongContext)?.1)?,
        };
        let consumed = first.consumed.clone();
        let mut outcomes = Vec::new();
        let mut records = Vec::new();
        // Mandatory local DAG sharing, not an optional memo: no engine action
        // may occur between this group's input and a shared eligible frontier.
        let mut tail_dependency = None;
        let mut frontier_dependencies = Vec::new();
        let support = group
            .iter()
            .map(|waiting| (waiting.branch.origin, waiting.branch.weight))
            .collect();
        // Lazy depth-first demand. Equal no-action Tail or eligible typed
        // Choice/multi-Action frontiers share one dependency. Every original
        // alternative still has complete values and retains its ordered edge.
        for index in 0..alternatives.len() {
            let children = {
                let _timing = beliefs::profile::scope("sequence.child_cohort");
                let mut children = Vec::new();
                for waiting in &group {
                    let mut branch = waiting.branch.clone();
                    match &waiting.alternatives[index] {
                        Alternative::Stage(stage) => branch.receipt.stage = *stage,
                        Alternative::Offer(offer) => self.commit(&mut branch, offer.clone())?,
                        Alternative::Reconsider { .. } | Alternative::Response { .. } => {
                            return Err(SequenceError::InvalidProgram)
                        }
                    }
                    children.push(branch);
                }
                children
            };
            let mut child_records = Vec::new();
            let normalized = if matches!(group[0].alternatives[index], Alternative::Stage(_)) {
                self.normalized_tail_edges(&children)?
            } else {
                None
            };
            let terminals = if let Some(edges) = normalized {
                if let Some(dependency) = &tail_dependency {
                    self.alias_tail(&children, &edges, dependency, &outcomes)?
                } else {
                    let before = self.work.represented_transitions;
                    let solver_before = self.work.represented_solver_nodes;
                    let read_starts = children
                        .iter()
                        .zip(&edges)
                        .map(|(branch, edge)| (branch.origin, branch.consumed.len() + edge.consumed.len()))
                        .collect();
                    let terminals = self.finish_normalized_tails(children, edges)?;
                    tail_dependency = Some(normalization::TailDependency {
                        outcome_index: outcomes.len(),
                        read_starts,
                        transitions: self.work.represented_transitions - before,
                        solver_nodes: self.work.represented_solver_nodes - solver_before,
                    });
                    terminals
                }
            } else if matches!(group[0].alternatives[index], Alternative::Stage(_)) {
                self.finish_frontier_child(
                    children,
                    &mut frontier_dependencies,
                    &outcomes,
                    &records,
                    &mut child_records,
                )?
            } else {
                self.finish(children, &mut child_records)?
            };
            outcomes.push(terminals);
            records.push(child_records);
        }
        let assessment = self.values(outcomes.iter().map(Vec::as_slice))?;
        let selected = assessment.selected;
        prepared.push(PreparedChoice {
            scope,
            information,
            semantic_suffix,
            context: self.context.clone(),
            membership,
            alternatives,
            selected,
            assessment: Some(assessment),
            consumed,
            support,
        });
        prepared.append(&mut records[selected]);
        Ok(std::mem::take(&mut outcomes[selected]))
    }
}

/// All candidates complete on identical mass or return Budget. Callers keep the
/// baseline/explicit deferred status; no partial estimate pretends full coverage.
pub fn evaluate<F: FnMut(&SeatView<'_>, &[GameAction], LeafBudget) -> Result<LeafResponse, LeafFailure>>(
    candidates: &[Candidate],
    cohort: &[(Table, f64)],
    root: u8,
    provenance: Provenance,
    context: &Context,
    limits: Limits,
    leaf: &mut F,
) -> Result<Comparison, Deferred> {
    evaluate_with_cache(
        candidates,
        cohort,
        root,
        provenance,
        context,
        limits,
        &mut CompletedCache::default(),
        leaf,
    )
}

/// Rebuild all conditional support/value comparisons under this call's measure.
/// Only exact compatible hard dependencies and completed response leaves reuse
/// work across calls. No prepared ranking or weighted suffix optimum persists.
pub fn evaluate_with_cache<F: FnMut(&SeatView<'_>, &[GameAction], LeafBudget) -> Result<LeafResponse, LeafFailure>>(
    candidates: &[Candidate],
    cohort: &[(Table, f64)],
    root: u8,
    provenance: Provenance,
    context: &Context,
    limits: Limits,
    cache: &mut CompletedCache,
    leaf: &mut F,
) -> Result<Comparison, Deferred> {
    let inputs = cache.begin(root, context, limits);
    let mut session = Session {
        root,
        provenance,
        context,
        limits,
        work: Work {
            retained_bytes: inputs.bytes,
            ..Work::default()
        },
        leaf,
        responses: inputs.responses,
        hard_reads: inputs.hard_reads,
        memo: Vec::new(),
        tails: inputs.tails,
        canonical: inputs.canonical,
        memberships: inputs.memberships,
        reusable_bytes: inputs.bytes,
        response_bytes: inputs.response_bytes,
        limit: None,
    };
    let result = (|| -> Result<Comparison, SequenceError> {
        if root >= 4
            || cohort.is_empty()
            || cohort.len() > 64
            || candidates.is_empty()
            || candidates.len() > 64
            || limits.memo_entries > 128
            || context.provenance != provenance
        {
            return Err(SequenceError::InvalidCohort);
        }
        let original = cohort[0].0.view(root);
        let information = Information::of(&original);
        let (_, legal) = cohort[0]
            .0
            .to_act()
            .filter(|(seat, _)| *seat == root)
            .ok_or(SequenceError::WrongContext)?;
        let eligible = crate::legal_actions::eligible(legal);
        let legal = eligible.as_slice();
        let mut mass = 0.0;
        for (table, weight) in cohort {
            if !weight.is_finite()
                || *weight <= 0.0
                || !information.matches(&table.view(root))
                || !table
                    .to_act()
                    .is_some_and(|(seat, actions)| seat == root && crate::legal_actions::eligible(actions) == legal)
            {
                return Err(SequenceError::InvalidCohort);
            }
            mass += weight;
        }
        if !mass.is_finite() || mass <= 0.0 {
            return Err(SequenceError::InvalidCohort);
        }
        let mut executions = Vec::new();
        for candidate in candidates {
            session.suffix(&candidate.program, candidate.stage)?;
            let mut initial_receipt = Receipt::new(candidate.program.clone(), &original)?;
            initial_receipt.stage = candidate.stage;
            let branches = {
                let _timing = beliefs::profile::scope("sequence.initial_cohort");
                let mut branches = Vec::new();
                for (origin, (table, weight)) in cohort.iter().enumerate() {
                    branches.push(Branch {
                        table: table.clone(),
                        origin,
                        weight: *weight,
                        receipt: initial_receipt.clone(),
                        first: None,
                        first_program: None,
                        tricks: Vec::new(),
                        consumed: Vec::new(),
                        interrupted: false,
                    });
                }
                branches
            };
            let mut prepared = Vec::new();
            let terminals = session.finish(branches, &mut prepared)?;
            if terminals.len() != cohort.len() {
                return Err(SequenceError::InvalidCohort);
            }
            let first = terminals[0].first.clone().ok_or(SequenceError::WrongContext)?;
            let first_program = terminals[0].first_program.as_ref().ok_or(SequenceError::WrongContext)?;
            if terminals.iter().any(|terminal| {
                terminal.first.as_ref() != Some(&first) || terminal.first_program.as_ref() != Some(first_program)
            }) {
                return Err(SequenceError::WrongContext);
            }
            executions.push(Execution {
                candidate: candidate.clone(),
                first,
                terminals,
                prepared,
            });
            session.work.completed_candidates += 1;
        }
        let values = session.values(executions.iter().map(|run| run.terminals.as_slice()))?;
        let selected = &executions[values.selected];
        let mut receipt = Receipt::new(
            selected.terminals[0]
                .first_program
                .clone()
                .ok_or(SequenceError::WrongContext)?,
            &original,
        )?;
        receipt.propose(selected.first.clone(), &original, legal)?;
        let selection = std::sync::Arc::new(SelectedPlan::completed(
            information,
            context,
            Membership {
                legal: crate::legal_actions::eligible(legal),
                // Preserve the actual nomination order, including aliases.
                programs: candidates
                    .iter()
                    .map(|candidate| candidate.program.suffix(candidate.stage))
                    .collect::<Result<_, _>>()?,
            },
            &values,
            cohort
                .iter()
                .enumerate()
                .map(|(origin, (_, weight))| (origin, *weight))
                .collect(),
            selected,
        )?);
        Ok(Comparison {
            executions,
            values,
            receipt,
            selection,
            work: session.work,
        })
    })();
    cache.finish(reuse::Inputs {
        responses: session.responses,
        hard_reads: session.hard_reads,
        tails: session.tails,
        canonical: session.canonical,
        memberships: session.memberships,
        bytes: session.reusable_bytes,
        response_bytes: session.response_bytes,
    });
    result.map_err(|reason| Deferred {
        reason,
        work: session.work,
        total_candidates: candidates.len(),
        limit: session.limit,
    })
}

// Retained memo allocation ledger. Counts capacities, nested owned data and
// vector storage (a conservative factor of two covers outer Vec growth). It
// bounds these caches, not transient recursive work or the caller's leaf/TTs.
fn vec_bytes<T>(v: &Vec<T>) -> usize {
    std::mem::size_of::<Vec<T>>() + v.capacity() * std::mem::size_of::<T>()
}
fn info_bytes(info: &Information) -> usize {
    std::mem::size_of::<Information>() + vec_bytes(&info.events)
}
fn query_bytes(query: &beliefs::ConceptQuery) -> usize {
    std::mem::size_of_val(query) + vec_bytes(&query.event) + vec_bytes(&query.given)
}
fn node_bytes(node: &crate::concepts::sequence::Node) -> usize {
    use crate::concepts::sequence::{Guard, Node};
    std::mem::size_of_val(node)
        + match node {
            Node::Choice(v) => vec_bytes(v),
            Node::Guard {
                predicate: Guard::Known(q),
                ..
            } => query_bytes(q),
            _ => 0,
        }
}
fn suffix_bytes(suffix: &Suffix) -> usize {
    std::mem::size_of_val(suffix) + vec_bytes(&suffix.nodes) + suffix.nodes.iter().map(node_bytes).sum::<usize>()
}
fn program_bytes(program: &Program) -> usize {
    std::mem::size_of_val(program) + vec_bytes(&program.nodes) + program.nodes.iter().map(node_bytes).sum::<usize>()
}
fn membership_bytes(membership: &Membership) -> usize {
    std::mem::size_of_val(membership)
        + vec_bytes(&membership.legal)
        + vec_bytes(&membership.programs)
        + membership.programs.iter().map(suffix_bytes).sum::<usize>()
}
fn selection_bytes(selection: &Selection) -> usize {
    std::mem::size_of_val(selection)
        + suffix_bytes(match selection {
            Selection::Stage(s) | Selection::Offer { next: s, .. } => s,
        })
}
fn consumed_bytes(consumed: &Consumed) -> usize {
    std::mem::size_of_val(consumed)
        + info_bytes(&consumed.information)
        + suffix_bytes(&consumed.suffix)
        + match &consumed.reading {
            ReadMeaning::Guard {
                predicate: crate::concepts::sequence::Guard::Known(q),
                ..
            } => query_bytes(q),
            ReadMeaning::Binding { legal_members, .. } => vec_bytes(legal_members),
            ReadMeaning::Alternatives(v) => vec_bytes(v),
            _ => 0,
        }
}
fn assessment_bytes(a: &Assessment) -> usize {
    use crate::concepts::value::{NodeValue, Operator};
    std::mem::size_of_val(a)
        + vec_bytes(&a.candidates)
        + vec_bytes(&a.trace)
        + a.trace
            .iter()
            .map(|node| {
                vec_bytes(&node.dependencies)
                    + match &node.operator {
                        Operator::Best { values } => vec_bytes(values),
                        Operator::Assessment { diagnostics, .. } => vec_bytes(diagnostics),
                        _ => 0,
                    }
                    + match &node.value {
                        NodeValue::ConditionalOutcomes(v) => vec_bytes(v),
                        NodeValue::Weights(v) => vec_bytes(v),
                        NodeValue::ConditionalScores(v) => vec_bytes(v),
                        NodeValue::BranchValues(v) => vec_bytes(v),
                        _ => 0,
                    }
            })
            .sum::<usize>()
}
fn prepared_bytes(p: &PreparedChoice) -> usize {
    std::mem::size_of_val(p)
        + info_bytes(&p.information)
        + suffix_bytes(&p.semantic_suffix)
        + membership_bytes(&p.membership)
        + vec_bytes(&p.alternatives)
        + p.alternatives.iter().map(selection_bytes).sum::<usize>()
        + p.assessment.as_ref().map(assessment_bytes).unwrap_or(0)
        + vec_bytes(&p.consumed)
        + p.consumed.iter().map(consumed_bytes).sum::<usize>()
        + vec_bytes(&p.support)
}
/// Estimate the owned cache payload from borrowed results, BEFORE cloning it.
fn run_bytes(key: &Vec<BranchKey>, terminals: &Vec<Terminal>, prepared: &Vec<PreparedChoice>) -> usize {
    let _timing = beliefs::profile::scope("sequence.run_ledger");
    2 * (std::mem::size_of::<CachedRun>()
        + vec_bytes(key)
        + key
            .iter()
            .map(|k| {
                k.views.iter().map(info_bytes).sum::<usize>()
                    + vec_bytes(&k.legal)
                    + suffix_bytes(&k.suffix)
                    + info_bytes(&k.accepted)
                    + k.pending.as_ref().map(selection_bytes).unwrap_or(0)
            })
            .sum::<usize>()
        + vec_bytes(terminals)
        + terminals
            .iter()
            .map(|t| {
                vec_bytes(&t.tricks)
                    + t.first_program.as_ref().map(program_bytes).unwrap_or(0)
                    + t.tricks.iter().map(|trick| info_bytes(&trick.root)).sum::<usize>()
                    + vec_bytes(&t.consumed)
                    + t.consumed.iter().map(consumed_bytes).sum::<usize>()
            })
            .sum::<usize>()
        + vec_bytes(prepared)
        + prepared.iter().map(prepared_bytes).sum::<usize>())
}

#[cfg(test)]
mod tests {
    // Keep the owner's fixture corrections in their existing source file.
    include!("../../tests/internal/sequence_execution.rs");
    include!("../../tests/internal/canonical_domains.rs");

    fn joint_fixture() -> (u8, Vec<GameAction>, Vec<(Table, f64)>) {
        let table = at_lead();
        let (root, legal) = table.to_act().unwrap();
        let legal = legal.to_vec();
        let view = table.view(root);
        let remaining: [CardSet; 4] = std::array::from_fn(|seat| table.view(seat as u8).knowledge.certain(seat as u8));
        let mut alternate = None;
        'pairs: for left in remaining[1].iter() {
            for right in remaining[3].iter() {
                let mut changed = remaining;
                changed[1] = (changed[1] - CardSet::single(left)) | CardSet::single(right);
                changed[3] = (changed[3] - CardSet::single(right)) | CardSet::single(left);
                if let Some(candidate) = Table::from_view(&view, changed) {
                    alternate = Some(candidate);
                    break 'pairs;
                }
            }
        }
        let alternate = alternate.expect("hidden allocation compatible with the unchanged recorded view");
        (root, legal, vec![(table, 1.0 / 3.0), (alternate, 2.0 / 3.0)])
    }
    #[test]
    fn persistent_leaves_reweight_current_support_and_rebuild_choice_values() {
        let (root, legal, cohort) = joint_fixture();
        let cards: Vec<_> = legal
            .iter()
            .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
            .take(2)
            .copied()
            .collect();
        let choice = Candidate::new(Program {
            name: "reweighted-choice",
            version: 1,
            start: Stage(3),
            nodes: vec![
                Node::Tail,
                Node::Action {
                    actions: Actions::Exact(cards[0].action_type),
                    next: Stage(0),
                },
                Node::Action {
                    actions: Actions::Exact(cards[1].action_type),
                    next: Stage(0),
                },
                Node::Choice(vec![Stage(1), Stage(2)]),
            ],
        });
        let candidates = [choice, direct(cards[0]), direct(cards[1])];
        let mut cache = CompletedCache::default();
        let limits = Limits {
            memo_bytes: 64 * 1024 * 1024,
            ..Limits::default()
        };
        let old_context = context();
        evaluate_with_cache(
            &candidates,
            &cohort,
            root,
            provenance(),
            &old_context,
            limits,
            &mut cache,
            &mut leaf,
        )
        .unwrap();
        let reweighted = [(cohort[1].0.clone(), 1.0 / 5.0), (cohort[0].0.clone(), 4.0 / 5.0)];
        let mut current = old_context.clone();
        current.posterior_revision += 1;
        current.seed += 1; // cohort generation changes, actual response params do not
        let warm = evaluate_with_cache(
            &candidates,
            &reweighted,
            root,
            provenance(),
            &current,
            limits,
            &mut cache,
            &mut leaf,
        )
        .unwrap();
        let cold = evaluate(
            &candidates,
            &reweighted,
            root,
            provenance(),
            &current,
            limits,
            &mut leaf,
        )
        .unwrap();
        assert_eq!(warm.values, cold.values);
        assert_eq!(warm.executions, cold.executions);
        assert_eq!(warm.receipt, cold.receipt);
        assert_eq!(warm.work.response_calls, 0);
        assert!(warm.work.suffix_hits > 0);
        assert_eq!(warm.work.represented_transitions, cold.work.represented_transitions);
        assert_eq!(warm.work.represented_comparisons, cold.work.represented_comparisons);
        let choice = warm.executions[0]
            .prepared
            .iter()
            .find(|p| p.assessment.is_some())
            .unwrap();
        assert_eq!(choice.context, current);
        assert_eq!(choice.support, [(0, 1.0 / 5.0), (1, 4.0 / 5.0)]);
        assert!(cache.retained_bytes() <= limits.memo_bytes);
        // Restore/configuration/model changes cannot transfer a frozen response.
        current.params.seed += 1;
        let changed = evaluate_with_cache(
            &candidates,
            &reweighted,
            root,
            provenance(),
            &current,
            limits,
            &mut cache,
            &mut leaf,
        )
        .unwrap();
        assert!(changed.work.response_calls > 0);
        cache.clear();
        assert_eq!(cache.retained_bytes(), 0);
        fn send<T: Send>() {}
        send::<CompletedCache>();
    }
    #[test]
    fn deferred_diagnostics_distinguish_deadline_logical_lanes_and_leaf_failure() {
        let table = at_lead();
        let (root, legal) = table.to_act().unwrap();
        let card = *legal
            .iter()
            .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
            .unwrap();
        let candidates = [direct(card)];
        let cohort = [(table, 1.0)];
        let context = context();
        let run = |limits| evaluate(&candidates, &cohort, root, provenance(), &context, limits, &mut leaf).unwrap_err();
        assert_eq!(
            run(Limits {
                deadline: Some(Instant::now()),
                ..Limits::default()
            })
            .limit,
            Some(DeferredLimit::Deadline)
        );
        assert_eq!(
            run(Limits {
                transitions: 0,
                ..Limits::default()
            })
            .limit,
            Some(DeferredLimit::RepresentedTransitions {
                required: 1,
                remaining: 0
            })
        );
        assert_eq!(
            run(Limits {
                comparisons: 0,
                ..Limits::default()
            })
            .limit,
            Some(DeferredLimit::RepresentedComparisons {
                required: 1,
                remaining: 0
            })
        );
        let uncached = evaluate(
            &candidates,
            &cohort,
            root,
            provenance(),
            &context,
            Limits {
                memo_bytes: 0,
                memo_entries: 0,
                ..Limits::default()
            },
            &mut leaf,
        )
        .unwrap();
        assert_eq!(uncached.work.retained_bytes, 0);
        let mut unable =
            |_: &SeatView<'_>, _: &[GameAction], _: LeafBudget| Err(LeafFailure::unit(SequenceError::Budget));
        let failed = evaluate(
            &candidates,
            &cohort,
            root,
            provenance(),
            &context,
            Limits::default(),
            &mut unable,
        )
        .unwrap_err();
        assert!(matches!(failed.limit, Some(DeferredLimit::LeafUnavailable { .. })));
    }
    fn two_cards_at_lead() -> Table {
        let mut table = at_lead();
        while table.view(0).knowledge.hand_len(0) > 2 || table.game.state.phase != GamePhase::StartTrick {
            let (actor, legal) = table.to_act().unwrap();
            let action = leaf(
                &table.view(actor),
                legal,
                LeafBudget {
                    deadline: None,
                    remaining_transitions: 100,
                    remaining_solver_nodes: None,
                },
            )
            .unwrap();
            table.apply(action.action);
        }
        table
    }
    #[test]
    fn reconsideration_compares_one_joint_group_and_returns_the_actual_program_owner() {
        let table = two_cards_at_lead();
        let (root, legal) = table.to_act().unwrap();
        let view = table.view(root);
        let program = Program {
            name: "current-group",
            version: 1,
            start: Stage(1),
            nodes: vec![
                Node::Tail,
                Node::Reconsider {
                    remaining: 1,
                    retained: Stage(0),
                },
            ],
        };
        let binding = bind_reconsideration(&program, program.start, &view, &crate::legal_actions::eligible(legal)).unwrap();
        let expected = binding
            .alternatives
            .iter()
            .map(|stage| binding.program.suffix(*stage).unwrap())
            .collect::<Vec<_>>();
        let candidates = [Candidate::new(program)];
        let cohort = [(table.clone(), 1.0 / 3.0), (table, 2.0 / 3.0)];
        let context = context();
        let result = evaluate(
            &candidates,
            &cohort,
            root,
            provenance(),
            &context,
            Limits::default(),
            &mut leaf,
        )
        .unwrap();
        let record = result
            .prepared()
            .iter()
            .find(|p| p.alternatives.len() == expected.len() && p.assessment.is_some())
            .unwrap();
        assert_eq!(
            record.alternatives,
            expected.iter().cloned().map(Selection::Stage).collect::<Vec<_>>()
        );
        assert_eq!(record.membership.programs, expected);
        assert_eq!(record.support, [(0, 1.0 / 3.0), (1, 2.0 / 3.0)]);
        let owner = result.executions[0].terminals[0].first_program.as_ref().unwrap();
        assert_eq!(&result.receipt.program, owner);
        assert!(result
            .receipt
            .program
            .nodes
            .get(result.executions[0].first.next.0)
            .is_some());
        assert_eq!(
            result.executions[0].terminals[0].first,
            result.executions[0].terminals[1].first
        );
        assert_eq!(
            result.executions[0].terminals[0].first_program,
            result.executions[0].terminals[1].first_program
        );
        assert!(result.executions[0].terminals[0]
            .consumed
            .iter()
            .any(|c| matches!(&c.reading, ReadMeaning::Reconsider { .. })));
    }
    #[test]
    fn boundary_membership_is_distinct_from_nested_membership_and_undo_eligibility() {
        let table = two_cards_at_lead();
        let (root, legal) = table.to_act().unwrap();
        let view = table.view(root);
        let eligible = crate::legal_actions::eligible(legal);
        // Table already filters control requests. Inject one to test canonical
        // membership normalization independently of the engine/policy boundary.
        let mut with_control = legal.to_vec();
        with_control.push(GameAction {
            player: engine::PlaceAtTable(root),
            action_type: ActionType::UndoRequest,
        });
        assert_eq!(
            Membership::authored(&view, &with_control).unwrap(),
            Membership::authored(&view, legal).unwrap()
        );
        let cards: Vec<_> = legal
            .iter()
            .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
            .take(2)
            .copied()
            .collect();
        let program = Program {
            name: "additional-retained-connection",
            version: 1,
            start: Stage(3),
            nodes: vec![
                Node::Tail,
                Node::Action {
                    actions: Actions::Exact(cards[1].action_type),
                    next: Stage(0),
                },
                Node::Action {
                    actions: Actions::Exact(cards[0].action_type),
                    next: Stage(1),
                },
                Node::Reconsider {
                    remaining: 1,
                    retained: Stage(2),
                },
            ],
        };
        let mut binding = bind_reconsideration(&program, program.start, &view, legal).unwrap();
        // Declare a complete fixture comparison with the direct card first.
        // All bound routes remain. Stable equal-value ties select an executable
        // action instead of the intentionally unwarm legacy Tail.
        let first = binding
            .alternatives
            .iter()
            .position(|stage| {
                matches!(
            binding.program.nodes[stage.0], Node::Action {
                actions: Actions::Exact(kind), .. } if kind == cards[0].action_type)
            })
            .unwrap();
        let stage = binding.alternatives.remove(first);
        binding.alternatives.insert(0, stage);
        binding.program.nodes[binding.program.start.0] = Node::Choice(binding.alternatives.clone());
        let bound = binding
            .alternatives
            .iter()
            .map(|stage| Candidate {
                program: binding.program.clone(),
                stage: *stage,
            })
            .collect::<Vec<_>>();
        let context = context();
        let result = evaluate(
            &bound,
            &[(table.clone(), 1.0 / 3.0), (table.clone(), 2.0 / 3.0)],
            root,
            provenance(),
            &context,
            Limits::default(),
            &mut leaf,
        )
        .unwrap();
        let authored = Membership::authored(&view, &eligible).unwrap();
        assert_eq!(authored, Membership::authored(&view, legal).unwrap());
        let boundary = Membership::from_candidates(legal, &bound).unwrap();
        assert_ne!(
            boundary, authored,
            "retained connection is additional actual comparison coverage"
        );
        // This certificate uses the actual complete outer value comparison.
        // Descendant records retain their original membership and assessment.
        assert_eq!(result.values.selected, 0);
        let mut prepared = vec![PreparedChoice {
            scope: ChoiceScope::Global,
            information: Information::of(&view),
            semantic_suffix: binding.program.suffix(binding.program.start).unwrap(),
            context: context.clone(),
            membership: boundary.clone(),
            alternatives: binding
                .alternatives
                .iter()
                .map(|stage| Selection::Stage(binding.program.suffix(*stage).unwrap()))
                .collect(),
            selected: result.values.selected,
            assessment: Some(result.values.clone()),
            consumed: vec![],
            support: vec![(0, 1.0 / 3.0), (1, 2.0 / 3.0)],
        }];
        prepared.extend(result.prepared().iter().cloned());
        let receipt = Receipt::new(binding.program.clone(), &view).unwrap();
        let reader = mind(&view);
        assert_eq!(
            prepared_lookup(&receipt, &view, &eligible, &reader, &context, &authored, &prepared),
            Err(NeedsReview)
        );
        let actual = prepared_lookup_with_boundary(
            &receipt,
            &view,
            &eligible,
            &reader,
            &context,
            &authored,
            Some(&boundary),
            &prepared,
        )
        .unwrap();
        assert_eq!(actual.action, result.executions[0].first.action);
        let mut wrong_retained = boundary.clone();
        let retained = binding.program.suffix(binding.retained).unwrap();
        let index = wrong_retained
            .programs
            .iter()
            .position(|suffix| *suffix == retained)
            .unwrap();
        wrong_retained.programs[index] = Program {
            name: "wrong-retained",
            version: 1,
            start: Stage(1),
            nodes: vec![
                Node::Tail,
                Node::Guard {
                    predicate: Guard::OwnCards(CardSet::ALL),
                    yes: Stage(0),
                    no: Stage(0),
                    unknown: Stage(0),
                },
            ],
        }
        .suffix(Stage(1))
        .unwrap();
        assert_eq!(
            prepared_lookup_with_boundary(
                &receipt,
                &view,
                &eligible,
                &reader,
                &context,
                &authored,
                Some(&wrong_retained),
                &prepared
            ),
            Err(NeedsReview)
        );
        assert_eq!(
            prepared_lookup_with_boundary(
                &receipt,
                &view,
                legal,
                &reader,
                &context,
                &authored,
                Some(&boundary),
                &prepared
            )
            .unwrap(),
            actual
        );
        // The stored nested fixed/action records keep the ordinary domain,
        // independently of the extra retained connection at the root Choice.
        assert!(result.prepared().iter().any(|p| p.information.matches(&view)
            && p.semantic_suffix != receipt.program.suffix(receipt.stage).unwrap()
            && p.membership == authored));
    }
    fn guarded_response() -> Candidate {
        Candidate::new(Program {
            name: "fixed-response-with-entry-guard",
            version: 1,
            start: Stage(3),
            nodes: vec![
                Node::Tail,
                Node::Response { next: Stage(0) },
                Node::Deferred,
                Node::Guard {
                    predicate: Guard::PlayReady,
                    yes: Stage(1),
                    no: Stage(2),
                    unknown: Stage(2),
                },
            ],
        })
    }
    #[test]
    fn response_groups_private_hypotheses_and_records_fixed_complete_support() {
        let (root, legal, cohort) = joint_fixture();
        let initial = Information::of(&cohort[0].0.view(root));
        let calls = std::cell::Cell::new(0usize);
        let mut completed = |view: &SeatView<'_>, actions: &[GameAction], budget| {
            if initial.matches(view) {
                assert_eq!(actions, crate::legal_actions::eligible(&legal));
                calls.set(calls.get() + 1);
            }
            leaf(view, actions, budget)
        };
        let candidates = [guarded_response()];
        let mut cache = CompletedCache::default();
        let context = context();
        let result = evaluate_with_cache(
            &candidates,
            &cohort,
            root,
            provenance(),
            &context,
            Limits::default(),
            &mut cache,
            &mut completed,
        )
        .unwrap();
        assert_eq!(
            calls.get(),
            1,
            "one shared response, no hidden-world-specific optimization"
        );
        let record = result.prepared().iter().find(|p| p.information == initial).unwrap();
        assert_eq!(
            record.semantic_suffix,
            candidates[0].program.suffix(candidates[0].stage).unwrap()
        );
        assert_eq!(record.selected, 0);
        assert!(record.assessment.is_none());
        assert!(record.membership.programs.is_empty());
        assert_eq!(record.membership.legal, crate::legal_actions::eligible(&legal));
        assert_eq!(record.support, [(0, 1.0 / 3.0), (1, 2.0 / 3.0)]);
        let [Selection::Offer { action, next }] = record.alternatives.as_slice() else {
            panic!("fixed response must have exactly one executable offer");
        };
        assert_eq!(*action, result.executions[0].first.action);
        assert_eq!(*next, candidates[0].program.suffix(Stage(0)).unwrap());
        assert!(record.consumed.iter().any(|read| matches!(
            &read.reading,
            ReadMeaning::Guard {
                predicate: Guard::PlayReady,
                truth: Some(true)
            }
        )));
        assert!(record
            .consumed
            .iter()
            .any(|read| matches!(&read.reading, ReadMeaning::Response { .. })));
        assert!(result.executions[0].terminals.iter().all(|t| !t.interrupted));
        assert_eq!(result.receipt.pending.as_ref().unwrap().action, *action);
        let receipt_next = result.receipt.pending.as_ref().unwrap().next;
        assert_eq!(result.receipt.program.suffix(receipt_next).unwrap(), *next);
        // Current measure rebuilds the singleton's whole conditional support;
        // the completed deterministic response does not become a cached value.
        let reweighted = [(cohort[0].0.clone(), 2.0 / 5.0), (cohort[1].0.clone(), 3.0 / 5.0)];
        let mut current = context.clone();
        current.posterior_revision += 1;
        let reused = evaluate_with_cache(
            &candidates,
            &reweighted,
            root,
            provenance(),
            &current,
            Limits::default(),
            &mut cache,
            &mut completed,
        )
        .unwrap();
        assert_eq!(calls.get(), 1);
        let record = reused.prepared().iter().find(|p| p.information == initial).unwrap();
        assert_eq!(record.context, current);
        assert_eq!(record.support, [(0, 2.0 / 5.0), (1, 3.0 / 5.0)]);
        assert_eq!(record.selected, 0);
        assert!(record.assessment.is_none());
    }
    #[test]
    fn explicit_gate_defers_without_tail_and_incomplete_root_response_is_not_cached() {
        let table = two_cards_at_lead();
        let root = table.to_act().unwrap().0;
        let initial = Information::of(&table.view(root));
        let context = context();
        let stopped = Candidate::new(Program {
            name: "exhausted-gate",
            version: 1,
            start: Stage(0),
            nodes: vec![Node::Deferred],
        });
        let cohort = [(table, 1.0)];
        let mut must_not_respond = |_: &SeatView<'_>, _: &[GameAction], _: LeafBudget| {
            panic!("gate exhaustion must not downgrade to a response tail");
        };
        let failed = evaluate(
            &[stopped],
            &cohort,
            root,
            provenance(),
            &context,
            Limits::default(),
            &mut must_not_respond,
        )
        .unwrap_err();
        assert_eq!(failed.reason, SequenceError::Budget);
        assert_eq!(failed.limit, Some(DeferredLimit::AuthoredGate));
        assert_eq!(failed.work.transitions, 0);
        assert_eq!(failed.work.completed_candidates, 0);
        let candidates = [guarded_response()];
        let mut cache = CompletedCache::default();
        let mut incomplete =
            |_: &SeatView<'_>, _: &[GameAction], _: LeafBudget| Err(LeafFailure::unit(SequenceError::Budget));
        let failed = evaluate_with_cache(
            &candidates,
            &cohort,
            root,
            provenance(),
            &context,
            Limits::default(),
            &mut cache,
            &mut incomplete,
        )
        .unwrap_err();
        assert_eq!(failed.limit, Some(DeferredLimit::LeafUnavailable { actor: root }));
        assert_eq!(failed.work.transitions, 0);
        let calls = std::cell::Cell::new(0usize);
        let mut completed = |view: &SeatView<'_>, legal: &[GameAction], budget| {
            if initial.matches(view) {
                calls.set(calls.get() + 1);
            }
            leaf(view, legal, budget)
        };
        evaluate_with_cache(
            &candidates,
            &cohort,
            root,
            provenance(),
            &context,
            Limits::default(),
            &mut cache,
            &mut completed,
        )
        .unwrap();
        assert_eq!(
            calls.get(),
            1,
            "an incomplete response cannot populate the exact input cache"
        );
    }
    fn effort_leaf(view: &SeatView<'_>, legal: &[GameAction], budget: LeafBudget) -> Result<LeafResponse, LeafFailure> {
        const NODES: u64 = 5;
        if let Some(remaining) = budget.remaining_solver_nodes.filter(|remaining| *remaining < NODES) {
            return Err(LeafFailure {
                reason: SequenceError::Budget,
                canonical_solver_nodes: remaining,
            });
        }
        let mut response = leaf(view, legal, budget)?;
        response.canonical_solver_nodes = NODES;
        Ok(response)
    }
    #[test]
    fn completed_effort_matches_cold_warm_uncached_and_evicting_comparisons() {
        let table = two_cards_at_lead();
        let (root, legal) = table.to_act().unwrap();
        let card = *legal
            .iter()
            .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
            .unwrap();
        // Duplicate exact programmes exercise complete suffix reuse; the same
        // Table in two origins exercises leaf/tail reuse without oracle choice.
        let candidate = direct(card);
        let candidates = [candidate.clone(), candidate];
        let cohort = [(table.clone(), 1.0 / 3.0), (table, 2.0 / 3.0)];
        let context = context();
        let limits = Limits {
            memo_bytes: 64 * 1024 * 1024,
            ..Limits::default()
        };
        let mut cache = CompletedCache::default();
        let cold = evaluate_with_cache(
            &candidates,
            &cohort,
            root,
            provenance(),
            &context,
            limits,
            &mut cache,
            &mut effort_leaf,
        )
        .unwrap();
        assert!(cold.work.suffix_hits > 0);
        assert!(cold.work.actual_solver_nodes > 0);
        assert!(cold.work.represented_solver_nodes > cold.work.actual_solver_nodes);
        let warm = evaluate_with_cache(
            &candidates,
            &cohort,
            root,
            provenance(),
            &context,
            limits,
            &mut cache,
            &mut effort_leaf,
        )
        .unwrap();
        assert_eq!(warm.work.actual_solver_nodes, 0);
        assert_eq!(warm.values, cold.values);
        assert_eq!(warm.receipt, cold.receipt);
        assert_eq!(warm.prepared(), cold.prepared());
        assert_eq!(warm.executions, cold.executions);
        assert_eq!(warm.work.represented_transitions, cold.work.represented_transitions);
        assert_eq!(warm.work.represented_comparisons, cold.work.represented_comparisons);
        for run in [&cold, &warm] {
            assert_eq!(run.work.represented_solver_nodes, cold.work.represented_solver_nodes);
        }
        let (actor, legal) = cohort[0].0.to_act().unwrap();
        let view = cohort[0].0.view(actor);
        let one_response_bytes = 2
            * (std::mem::size_of::<ResponseKey>()
                + std::mem::size_of_val(view.events)
                + std::mem::size_of_val(legal)
                + std::mem::size_of::<LeafResponse>());
        for restricted in [
            Limits {
                memo_entries: 0,
                memo_bytes: 0,
                ..limits
            },
            Limits {
                memo_bytes: one_response_bytes,
                reuse_suffixes: false,
                ..limits
            },
        ] {
            let run = evaluate(
                &candidates,
                &cohort,
                root,
                provenance(),
                &context,
                restricted,
                &mut effort_leaf,
            )
            .unwrap();
            assert_eq!(run.values, cold.values);
            assert_eq!(run.receipt, cold.receipt);
            assert_eq!(run.prepared(), cold.prepared());
            assert_eq!(run.executions, cold.executions);
            assert_eq!(run.work.represented_transitions, cold.work.represented_transitions);
            assert_eq!(run.work.represented_comparisons, cold.work.represented_comparisons);
            assert_eq!(run.work.represented_solver_nodes, cold.work.represented_solver_nodes);
            assert!(run.work.actual_solver_nodes >= cold.work.actual_solver_nodes);
            assert!(run.work.retained_bytes <= restricted.memo_bytes);
        }
        // Exact full logical allowance must complete with and without reuse.
        let bounded = Limits {
            solver_nodes: Some(cold.work.represented_solver_nodes),
            ..limits
        };
        let bounded_warm = evaluate_with_cache(
            &candidates,
            &cohort,
            root,
            provenance(),
            &context,
            bounded,
            &mut cache,
            &mut effort_leaf,
        )
        .unwrap();
        assert_eq!(bounded_warm.values, cold.values);
        let bounded_cold = evaluate(
            &candidates,
            &cohort,
            root,
            provenance(),
            &context,
            Limits {
                memo_entries: 0,
                memo_bytes: 0,
                ..bounded
            },
            &mut effort_leaf,
        )
        .unwrap();
        assert_eq!(
            bounded_cold.work.represented_solver_nodes,
            bounded_warm.work.represented_solver_nodes
        );
        for use_cache in [false, true] {
            let tight = Limits {
                solver_nodes: Some(cold.work.represented_solver_nodes - 1),
                ..limits
            };
            let failed = if use_cache {
                evaluate_with_cache(
                    &candidates,
                    &cohort,
                    root,
                    provenance(),
                    &context,
                    tight,
                    &mut cache,
                    &mut effort_leaf,
                )
            } else {
                evaluate(
                    &candidates,
                    &cohort,
                    root,
                    provenance(),
                    &context,
                    Limits {
                        memo_entries: 0,
                        memo_bytes: 0,
                        ..tight
                    },
                    &mut effort_leaf,
                )
            }
            .unwrap_err();
            assert_eq!(failed.reason, SequenceError::Budget);
            assert!(failed.work.represented_solver_nodes <= tight.solver_nodes.unwrap());
        }
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use crate::concepts::sequence::{Actions, Node};
    use crate::concepts::value::CohortMethod;
    use engine::{ActionType, Card};

    fn context() -> Context {
        Context {
            params: crate::Params::default(),
            seed: 7,
            posterior_revision: 1,
            provenance: Provenance {
                belief_model: "fixture-joint",
                response_model: "unused",
                cohort: CohortMethod::Enumerated,
            },
        }
    }
    fn no_leaf(_: &SeatView<'_>, _: &[GameAction], _: LeafBudget) -> Result<LeafResponse, LeafFailure> {
        panic!("pure dependency queries must not invoke a response")
    }
    fn session<'a, F>(context: &'a Context, leaf: &'a mut F, limits: Limits) -> Session<'a, F> {
        Session {
            root: 0,
            provenance: context.provenance,
            context,
            limits,
            work: Work::default(),
            leaf,
            responses: Vec::new(),
            hard_reads: Vec::new(),
            memo: Vec::new(),
            tails: Vec::new(),
            canonical: Vec::new(),
            memberships: Vec::new(),
            reusable_bytes: 0,
            response_bytes: 0,
            limit: None,
        }
    }
    fn program() -> Program {
        Program {
            name: "ordered-options",
            version: 1,
            start: Stage(2),
            nodes: vec![
                Node::Tail,
                Node::Action {
                    actions: Actions::Exact(ActionType::CardPlayed(Card::from_index(0))),
                    next: Stage(0),
                },
                Node::Choice(vec![Stage(0), Stage(1)]),
            ],
        }
    }
    #[test]
    fn canonical_cache_preserves_order_and_rejects_changed_invalid_input() {
        let context = context();
        let mut leaf = no_leaf;
        let mut session = session(&context, &mut leaf, Limits::default());
        let program = program();
        for stage in 0..program.nodes.len() {
            assert_eq!(
                session.suffix(&program, Stage(stage)).unwrap(),
                program.suffix(Stage(stage)).unwrap()
            );
        }
        let bytes = session.work.retained_bytes;
        for stage in 0..program.nodes.len() {
            assert_eq!(
                session.suffix(&program, Stage(stage)).unwrap(),
                program.suffix(Stage(stage)).unwrap()
            );
        }
        assert_eq!(session.work.retained_bytes, bytes);
        assert_eq!(session.canonical.len(), 1);
        let mut reordered = program.clone();
        reordered.nodes[2] = Node::Choice(vec![Stage(1), Stage(0)]);
        assert_ne!(
            session.suffix(&reordered, reordered.start).unwrap(),
            program.suffix(program.start).unwrap()
        );
        let mut invalid = program.clone();
        invalid.nodes[2] = Node::Choice(vec![Stage(2)]);
        assert_eq!(
            session.suffix(&invalid, invalid.start),
            Err(SequenceError::InvalidProgram)
        );
        assert_eq!(
            session.suffix(&program, Stage(program.nodes.len())),
            Err(SequenceError::InvalidProgram)
        );
        assert!(session.work.retained_bytes <= session.limits.memo_bytes);
    }
    #[test]
    fn stage_scan_matches_uncached_first_stage_with_full_and_cleared_cache() {
        let context = context();
        let original = program();
        let mut duplicated = original.clone();
        duplicated.nodes.push(Node::Tail);
        let mut renamed = original.clone();
        renamed.name = "same-semantics-different-input";
        for entries in [0, 1, 8] {
            let mut leaf = no_leaf;
            let mut session = session(&context, &mut leaf, Limits { memo_entries: entries, ..Limits::default() });
            for _ in 0..2 {
                for program in [&original, &duplicated, &renamed] {
                    for stage in 0..program.nodes.len() {
                        let suffix = program.suffix(Stage(stage)).unwrap();
                        assert_eq!(session.suffix(program, Stage(stage)).unwrap(), suffix);
                        assert_eq!(session.find_stage(program, &suffix), find_stage(program, &suffix));
                    }
                }
                if entries > 0 {
                    assert_eq!(session.canonical[0].program, original);
                }
                assert!(session.canonical.len() <= entries);
                assert!(session.work.retained_bytes <= session.limits.memo_bytes);
            }
            // Use the real batch eviction path, then demand the same suffixes.
            // A response filling the whole byte allowance evicts reusable data.
            #[cfg(feature = "profile")]
            let before_eviction = beliefs::profile::snapshot();
            assert!(session.retain_response(session.limits.memo_bytes) || entries == 0);
            #[cfg(feature = "profile")]
            {
                let after = beliefs::profile::snapshot();
                let delta = |label: &str| {
                    after.get(label).map_or(0, |timing| timing.calls)
                        - before_eviction.get(label).map_or(0, |timing| timing.calls)
                };
                assert_eq!(delta("sequence.eviction.reason.bytes"), u64::from(entries > 0));
                assert_eq!(delta("sequence.eviction.reason.entries"), 0);
                assert_eq!(delta("sequence.eviction.reason.both"), 0);
                // This fixture clears canonical entries, not completed runs.
                assert_eq!(delta("sequence.eviction.memo_nonempty"), 0);
            }
            assert!(session.canonical.is_empty());
            assert_eq!(session.find_stage(&duplicated, &duplicated.suffix(Stage(3)).unwrap()), Ok(Stage(0)));
            assert_eq!(session.work.represented_transitions, 0);
            assert_eq!(session.work.represented_comparisons, 0);
            assert_eq!(session.work.represented_solver_nodes, 0);
        }
    }
    #[test]
    fn suffix_batches_match_single_reads_under_cache_pressure_and_eviction() {
        let context = context();
        let original = program();
        let mut changed = original.clone();
        changed.nodes[2] = Node::Choice(vec![Stage(1), Stage(0)]);
        let stages = [Stage(2), Stage(0), Stage(1), Stage(2)];
        for (entries, bytes) in [(0, 0), (1, 0), (1, 1024 * 1024), (8, 1024 * 1024)] {
            let limits = Limits {
                memo_entries: entries,
                memo_bytes: bytes,
                ..Limits::default()
            };
            let mut batch_leaf = no_leaf;
            let mut single_leaf = no_leaf;
            let mut batch = session(&context, &mut batch_leaf, limits);
            let mut single = session(&context, &mut single_leaf, limits);
            // The second distinct program misses under a one-entry cap. Both
            // admitted and unadmitted repeats preserve suffix order and ledger.
            for program in [&original, &changed, &changed, &original] {
                let actual = batch.suffixes(program, stages).collect::<Result<Vec<_>, _>>().unwrap();
                let expected = stages
                    .iter()
                    .map(|stage| single.suffix(program, *stage))
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                assert_eq!(actual, expected);
                assert_eq!(batch.work, single.work);
                assert_eq!(batch.canonical.len(), single.canonical.len());
            }
            // No local batch index survives the real response-eviction path.
            assert_eq!(batch.retain_response(bytes), single.retain_response(bytes));
            let actual = batch.suffixes(&changed, stages).collect::<Result<Vec<_>, _>>().unwrap();
            let expected = stages
                .iter()
                .map(|stage| single.suffix(&changed, *stage))
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert_eq!(actual, expected);
            assert_eq!(batch.work, single.work);
        }
    }
    #[test]
    fn suffix_batches_revalidate_changed_inputs_at_full_cap_and_keep_empty_reads_lazy() {
        let context = context();
        let mut leaf = no_leaf;
        let mut session = session(
            &context,
            &mut leaf,
            Limits {
                memo_entries: 1,
                ..Limits::default()
            },
        );
        let mut program = program();
        session
            .suffixes(&program, [Stage(0), Stage(2)])
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(session.canonical.len(), 1);
        // The next batch must reject invalid unreachable authored data even
        // when its requested suffix is otherwise valid and the cache is full.
        program.nodes.push(Node::Choice(vec![Stage(3)]));
        let retained = session.work.retained_bytes;
        assert_eq!(
            session.suffixes(&program, [Stage(0)]).collect::<Result<Vec<_>, _>>(),
            Err(SequenceError::InvalidProgram)
        );
        assert_eq!(session.suffixes(&program, []).count(), 0);
        assert_eq!(session.work.retained_bytes, retained);
        program.nodes.pop();
        assert_eq!(
            session.suffixes(&program, [Stage(99)]).collect::<Result<Vec<_>, _>>(),
            Err(SequenceError::InvalidProgram)
        );
        assert_eq!(session.find_stage(&program, &program.suffix(Stage(0)).unwrap()), Ok(Stage(0)));
    }
    #[test]
    fn membership_reuse_requires_exact_private_view_and_canonical_legal_domain() {
        let context = context();
        let mut leaf = no_leaf;
        let mut session = session(&context, &mut leaf, Limits::default());
        let mut hands: [Vec<Card>; 4] =
            std::array::from_fn(|seat| (0..9).map(|i| Card::from_index((9 * seat + i) as u8)).collect());
        let table = Table::new(&hands);
        let (root, legal) = table.to_act().unwrap();
        let view = table.view(root);
        let expected = Membership::authored(&view, legal).unwrap();
        assert_eq!(session.membership(&view, legal).unwrap(), expected);
        let entries = session.memberships.len();
        let bytes = session.work.retained_bytes;
        assert_eq!(session.membership(&view, legal).unwrap(), expected);
        assert_eq!(session.memberships.len(), entries);
        assert_eq!(session.work.retained_bytes, bytes);
        let mut reordered = legal.to_vec();
        assert!(reordered.len() > 1);
        reordered.reverse();
        assert_eq!(
            session.membership(&view, &reordered).unwrap(),
            Membership::authored(&view, &reordered).unwrap()
        );
        assert_eq!(session.memberships.len(), entries);
        let card = hands[0][0];
        hands[0][0] = hands[1][0];
        hands[1][0] = card;
        let changed = Table::new(&hands);
        let (root, legal) = changed.to_act().unwrap();
        let view = changed.view(root);
        assert_eq!(
            session.membership(&view, legal).unwrap(),
            Membership::authored(&view, legal).unwrap()
        );
        assert_eq!(session.memberships.len(), entries + 1);
    }
    #[test]
    fn dependency_cache_exhaustion_falls_back_without_retaining_or_skipping_validation() {
        let context = context();
        let mut leaf = no_leaf;
        let mut session = session(
            &context,
            &mut leaf,
            Limits {
                memo_bytes: 0,
                ..Limits::default()
            },
        );
        let program = program();
        assert_eq!(
            session.suffix(&program, program.start).unwrap(),
            program.suffix(program.start).unwrap()
        );
        let mut invalid = program;
        invalid.nodes[2] = Node::Choice(vec![Stage(2)]);
        assert_eq!(
            session.suffix(&invalid, invalid.start),
            Err(SequenceError::InvalidProgram)
        );
        let hands: [Vec<Card>; 4] =
            std::array::from_fn(|seat| (0..9).map(|i| Card::from_index((9 * seat + i) as u8)).collect());
        let table = Table::new(&hands);
        let (root, legal) = table.to_act().unwrap();
        let view = table.view(root);
        assert_eq!(
            session.membership(&view, legal).unwrap(),
            Membership::authored(&view, legal).unwrap()
        );
        assert!(session.canonical.is_empty());
        assert!(session.memberships.is_empty());
        assert_eq!(session.work.retained_bytes, 0);
        assert_eq!(session.work.represented_transitions, 0);
        assert_eq!(session.work.represented_comparisons, 0);
    }
    #[test]
    fn response_eviction_recomputes_deterministically_and_charges_every_hit() {
        let context = context();
        let calls = std::cell::Cell::new(0usize);
        let mut leaf = |_: &SeatView<'_>, legal: &[GameAction], _: LeafBudget| {
            calls.set(calls.get() + 1);
            Ok(LeafResponse {
                action: legal[0],
                canonical_solver_nodes: 7,
            })
        };
        let mut session = session(&context, &mut leaf, Limits::default());
        let mut hands: [Vec<Card>; 4] =
            std::array::from_fn(|seat| (0..9).map(|i| Card::from_index((9 * seat + i) as u8)).collect());
        let first_table = Table::new(&hands);
        let first = session.response(&first_table).unwrap();
        let first_bytes = session.response_bytes;
        // One response fits. An optional compiled suffix fits only until the
        // next response arrives; its allocation is reclaimed before eviction.
        session.limits.memo_bytes = 2 * first_bytes - 1;
        let tail = Program {
            name: "optional",
            version: 1,
            start: Stage(0),
            nodes: vec![Node::Tail],
        };
        session.suffix(&tail, tail.start).unwrap();
        assert!(!session.canonical.is_empty());
        let card = hands[0][0];
        hands[0][0] = hands[1][0];
        hands[1][0] = card;
        session.response(&Table::new(&hands)).unwrap();
        assert_eq!(calls.get(), 2);
        assert_eq!(session.responses.len(), 1);
        assert_eq!(session.canonical.capacity(), 0);
        assert_eq!(session.response(&first_table).unwrap(), first);
        assert_eq!(
            calls.get(),
            3,
            "an evicted completed response recomputes the same semantic result"
        );
        assert_eq!(session.response(&first_table).unwrap(), first);
        assert_eq!(calls.get(), 3);
        assert_eq!(session.work.actual_solver_nodes, 21);
        assert_eq!(session.work.represented_solver_nodes, 28);
        assert_eq!(session.work.retained_bytes, session.response_bytes);
        assert_eq!(session.work.retained_bytes, session.reusable_bytes);
        assert!(session.work.retained_bytes <= session.limits.memo_bytes);
    }
    #[test]
    fn tight_solver_lane_rejects_hit_and_partial_miss_without_caching_failure() {
        let context = context();
        let calls = std::cell::Cell::new(0usize);
        let mut leaf = |_: &SeatView<'_>, legal: &[GameAction], budget: LeafBudget| {
            calls.set(calls.get() + 1);
            if let Some(remaining) = budget.remaining_solver_nodes.filter(|remaining| *remaining < 7) {
                return Err(LeafFailure {
                    reason: SequenceError::Budget,
                    canonical_solver_nodes: remaining,
                });
            }
            Ok(LeafResponse {
                action: legal[0],
                canonical_solver_nodes: 7,
            })
        };
        let mut session = session(&context, &mut leaf, Limits::default());
        let hands: [Vec<Card>; 4] =
            std::array::from_fn(|seat| (0..9).map(|i| Card::from_index((9 * seat + i) as u8)).collect());
        let table = Table::new(&hands);
        session.response(&table).unwrap();
        session.work = Work {
            retained_bytes: session.work.retained_bytes,
            ..Work::default()
        };
        session.limits.solver_nodes = Some(3);
        assert_eq!(session.response(&table), Err(SequenceError::Budget));
        assert_eq!(
            session.limit,
            Some(DeferredLimit::RepresentedSolverNodes {
                required: 7,
                remaining: 3
            })
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(session.work.actual_solver_nodes, 0);
        // No cache admits a partial response. Its attempted canonical effort is
        // counted once, and a later larger lane calls the leaf again.
        session.responses = Vec::new();
        session.response_bytes = 0;
        session.reusable_bytes = 0;
        session.work = Work::default();
        session.limit = None;
        assert_eq!(session.response(&table), Err(SequenceError::Budget));
        assert_eq!(session.work.actual_solver_nodes, 3);
        assert_eq!(session.work.represented_solver_nodes, 3);
        assert!(session.responses.is_empty());
        assert_eq!(calls.get(), 2);
        session.limits.solver_nodes = Some(10);
        session.response(&table).unwrap();
        assert_eq!(calls.get(), 3);
        assert_eq!(session.work.actual_solver_nodes, 10);
        assert_eq!(session.work.represented_solver_nodes, 10);
    }
    #[test]
    fn zero_capacity_and_oversized_entry_compute_without_copying_payload() {
        let context = context();
        let calls = std::cell::Cell::new(0usize);
        let mut leaf = |_: &SeatView<'_>, legal: &[GameAction], _: LeafBudget| {
            calls.set(calls.get() + 1);
            Ok(LeafResponse {
                action: legal[0],
                canonical_solver_nodes: 7,
            })
        };
        let hands: [Vec<Card>; 4] =
            std::array::from_fn(|seat| (0..9).map(|i| Card::from_index((9 * seat + i) as u8)).collect());
        let table = Table::new(&hands);
        for limits in [
            Limits {
                memo_entries: 0,
                ..Limits::default()
            },
            Limits {
                memo_bytes: 1,
                ..Limits::default()
            },
        ] {
            let mut session = session(&context, &mut leaf, limits);
            let first = session.response(&table).unwrap();
            assert_eq!(session.response(&table).unwrap(), first);
            assert!(session.responses.is_empty());
            assert_eq!(session.work.retained_bytes, 0);
            assert_eq!(session.work.actual_solver_nodes, 14);
            assert_eq!(session.work.represented_solver_nodes, 14);
        }
        assert_eq!(calls.get(), 4);
    }
}

//! Finite authored continuations over a preserved joint cohort. Suffix choices
//! belong to actor information sets, never individual hidden worlds.

use crate::concepts::sequence::{
    authored_programs, bind_reconsideration, inspect, inspect_with, Frontier, Information, Offer, Program, ReadMeaning,
    Receipt, SequenceError, Stage, Suffix,
};
use crate::concepts::value::{Assessment, Provenance, ValueStore};
use agent::{SeatView, Table};
use beliefs::{BeliefParams, ConceptBudget, Mind};
use engine::{GameAction, GameFinishedInfo, GameOutcome, PlayPhase, PublicState};
use std::time::Instant;

mod reuse;
pub use reuse::CompletedCache;

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
    /// Logical work, including the work represented by completed memo hits.
    pub transitions: usize,
    pub comparisons: usize,
    pub memo_entries: usize,
    pub memo_bytes: usize,
    /// Disable completed suffix/tail reuse without resampling frozen responses.
    pub reuse_suffixes: bool,
    pub deadline: Option<Instant>,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            transitions: 8192,
            comparisons: 256,
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
/// A callback's Ok(action) certifies a deterministic, semantically completed
/// response under Context's model/configuration. Incomplete eligible search
/// must return Err rather than a timed prefix or weaker fallback. The owner
/// enforces this through its completed-response wrapper; the bare action type
/// cannot check the certificate or account canonical solver effort yet.
#[derive(Clone, Copy, Debug)]
pub struct LeafBudget {
    pub deadline: Option<Instant>,
    pub remaining_transitions: usize,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Work {
    /// Actual engine applications; cache hits only debit represented work.
    pub transitions: usize,
    pub represented_transitions: usize,
    /// Actual value-store comparisons, separate from the logical limit.
    pub comparisons: usize,
    pub represented_comparisons: usize,
    pub response_calls: usize,
    pub response_hits: usize,
    pub suffix_hits: usize,
    pub retained_bytes: usize,
    pub completed_candidates: usize,
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
            legal: legal.to_vec(),
            programs,
        })
    }
    pub fn authored(view: &SeatView<'_>, legal: &[GameAction]) -> Result<Self, SequenceError> {
        Self::from_candidates(
            legal,
            &authored_programs(view, legal)
                .into_iter()
                .map(Candidate::new)
                .collect::<Vec<_>>(),
        )
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedChoice {
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
    pub prepared: Vec<PreparedChoice>,
    pub work: Work,
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
    ResponseCache {
        required_bytes: usize,
        remaining_bytes: usize,
        entries: usize,
        max_entries: usize,
    },
    LeafUnavailable {
        actor: u8,
    },
    FiniteContinuation {
        max_steps: usize,
    },
    Reconsideration {
        remaining: u8,
    },
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
    if !receipt.accepted.matches(view) || receipt.pending.is_some() {
        return Err(NeedsReview);
    }
    let mut stage = receipt.stage;
    for _ in 0..crate::concepts::sequence::MAX_STAGES {
        let inspection = inspect(&receipt.program, stage, view, legal, mind).map_err(|_| NeedsReview)?;
        let offers = match inspection.frontier {
            Frontier::Actions(offers) => Some(offers),
            Frontier::Choice(_) => None,
            Frontier::Tail | Frontier::Interrupted | Frontier::Reconsider { .. } => return Err(NeedsReview),
        };
        // Even a now-singleton choice must validate modeled scope: an
        // opportunity/evidence change cannot silently reuse its old assessment.
        let suffix = receipt.program.suffix(stage).map_err(|_| NeedsReview)?;
        let choice = prepared
            .iter()
            .find(|choice| {
                choice.information.matches(view)
                    && choice.semantic_suffix == suffix
                    && &choice.context == context
                    && &choice.membership == membership
            })
            .ok_or(NeedsReview)?;
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
            .map(|(seat, legal)| (Some(seat), legal.to_vec()))
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
}
struct TailRun {
    views: [Information; 4],
    legal: Vec<GameAction>,
    outcome: GameOutcome,
    first: GameAction,
    tricks: Vec<TrickOutcome>,
    consumed: Vec<Consumed>,
    transitions: usize,
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
    responses: Vec<(ResponseKey, GameAction)>,
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
impl<F: FnMut(&SeatView<'_>, &[GameAction], LeafBudget) -> Result<GameAction, SequenceError>> Session<'_, F> {
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
    /// Optional trace/dependency reuse cannot starve frozen response admission.
    /// Eviction recomputes optional results from retained completed responses.
    /// Response eviction is deferred to the canonical leaf-effort interface;
    /// this slice never recomputes a completed response after eviction.
    fn retain_response(&mut self, bytes: usize) -> bool {
        if bytes > self.limits.memo_bytes.saturating_sub(self.work.retained_bytes) {
            // Drop capacities as well as elements before releasing the ledger.
            self.memo = Vec::new();
            self.tails = Vec::new();
            self.hard_reads = Vec::new();
            self.canonical = Vec::new();
            self.memberships = Vec::new();
            self.work.retained_bytes = self.response_bytes;
            self.reusable_bytes = self.response_bytes;
        }
        self.retain(bytes)
    }
    fn suffix(&mut self, program: &Program, stage: Stage) -> Result<Suffix, SequenceError> {
        let mut cached = self.canonical.iter().position(|entry| entry.program == *program);
        if let Some(index) = cached {
            if let Some(suffix) = self.canonical[index].suffixes.get(stage.0).and_then(Option::as_ref) {
                return Ok(suffix.clone());
            }
        } else {
            program.validate()?;
            let bytes = 2
                * (std::mem::size_of::<CanonicalProgram>()
                    + program_bytes(program)
                    + std::mem::size_of::<Vec<Option<Suffix>>>()
                    + program.nodes.len() * std::mem::size_of::<Option<Suffix>>());
            if self.canonical.len() < self.limits.memo_entries && self.retain_reusable(bytes) {
                cached = Some(self.canonical.len());
                self.canonical.push(CanonicalProgram {
                    program: program.clone(),
                    suffixes: vec![None; program.nodes.len()],
                });
            }
        }
        // Either this exact input was validated above, or it equals a validated
        // cache entry. Lack of cache space never skips validation or a stage.
        let suffix = program.suffix_validated(stage)?;
        if let Some(index) = cached {
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
            Alternative::Reconsider { .. } => return Err(SequenceError::InvalidProgram),
        })
    }
    fn find_stage(&mut self, program: &Program, suffix: &Suffix) -> Result<Stage, SequenceError> {
        for stage in 0..program.nodes.len() {
            if self.suffix(program, Stage(stage))? == *suffix {
                return Ok(Stage(stage));
            }
        }
        Err(SequenceError::InvalidProgram)
    }
    fn membership(&mut self, view: &SeatView<'_>, legal: &[GameAction]) -> Result<Membership, SequenceError> {
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
            legal: legal.to_vec(),
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
        self.check()?;
        if self.work.represented_transitions >= self.limits.transitions {
            return Err(self.limited(DeferredLimit::RepresentedTransitions {
                required: 1,
                remaining: 0,
            }));
        }
        let (seat, legal) = table.to_act().ok_or(SequenceError::WrongContext)?;
        let view = table.view(seat);
        if let Some((_, action)) = self
            .responses
            .iter()
            .find(|(key, _)| key.information.matches(&view) && key.legal == legal)
        {
            self.work.response_hits += 1;
            return Ok(*action);
        }
        // Until canonical leaf effort is returned/accounted, keep completed
        // responses mandatory. Exhaustion defers the entire comparison rather
        // than evicting/recomputing responses or dropping difficult worlds.
        let bytes = 2
            * (std::mem::size_of::<Information>()
                + std::mem::size_of::<Vec<observation::SeatEvent>>()
                + std::mem::size_of_val(view.events)
                + std::mem::size_of::<Vec<GameAction>>()
                + std::mem::size_of_val(legal)
                + std::mem::size_of::<GameAction>());
        let max_entries = self.limits.memo_entries.saturating_mul(32);
        if self.responses.len() >= max_entries || !self.retain_response(bytes) {
            let remaining_bytes = self.limits.memo_bytes.saturating_sub(self.work.retained_bytes);
            return Err(self.limited(DeferredLimit::ResponseCache {
                required_bytes: bytes,
                remaining_bytes,
                entries: self.responses.len(),
                max_entries,
            }));
        }
        let result = (self.leaf)(
            &view,
            legal,
            LeafBudget {
                deadline: self.limits.deadline,
                remaining_transitions: self
                    .limits
                    .transitions
                    .saturating_sub(self.work.represented_transitions),
            },
        );
        // Admission is a reservation until a valid completed response exists.
        // Failed leaves must not inflate either this comparison's retained
        // ledger or the persistent cache handed to the next repair.
        self.work.retained_bytes -= bytes;
        self.work.response_calls += 1;
        let action = match result {
            Ok(action) => action,
            Err(SequenceError::Budget) => {
                self.check()?;
                return Err(self.limited(DeferredLimit::LeafUnavailable { actor: seat }));
            }
            Err(error) => return Err(error),
        };
        self.check()?;
        if !legal.contains(&action) || action.player.0 != seat {
            return Err(SequenceError::IllegalAction);
        }
        let key = ResponseKey {
            information: Information::of(&view),
            legal: legal.to_vec(),
        };
        self.responses.push((key, action));
        self.work.retained_bytes += bytes;
        self.reusable_bytes += bytes;
        self.response_bytes += bytes;
        Ok(action)
    }
    fn apply(&mut self, branch: &mut Branch, action: GameAction) -> Result<(), SequenceError> {
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
        let view = branch.table.view(self.root);
        let legal = branch.table.to_act().ok_or(SequenceError::WrongContext)?.1;
        let mut mind = None;
        let mut error = None;
        let result = inspect_with(&branch.receipt.program, branch.receipt.stage, &view, legal, |query| {
            if let Some((_, _, truth)) = self
                .hard_reads
                .iter()
                .find(|(information, old, _)| information.matches(&view) && old == query)
            {
                return *truth;
            }
            // No construction for ordinary guards/bindings. Known queries build
            // one lazy actor-local proof reader, zero DP/weights/traversal.
            if mind.is_none() {
                let mut reader = Mind::new(
                    view.seat,
                    view.knowledge.dealt_certain(view.seat),
                    view.kinds,
                    BeliefParams::base_only(),
                );
                for event in view.events {
                    if reader.apply(event).is_err() {
                        error = Some(SequenceError::WrongContext);
                        return None;
                    }
                }
                if reader.knowledge() != view.knowledge {
                    error = Some(SequenceError::WrongContext);
                    return None;
                }
                mind = Some(reader);
            }
            let truth = mind
                .as_ref()
                .and_then(|mind| mind.concept(query, ConceptBudget { max_nodes: 0 }).ok())
                .and_then(|estimate| estimate.known);
            let information = Information::of(&view);
            let bytes = 2 * (info_bytes(&information) + query_bytes(query) + std::mem::size_of::<Option<bool>>());
            if self.hard_reads.len() < self.limits.memo_entries && self.retain_reusable(bytes) {
                self.hard_reads.push((information, query.clone(), truth));
            }
            truth
        });
        if let Some(error) = error {
            return Err(error);
        }
        result
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
            for read in &inspection.consumed {
                branch.consumed.push(Consumed {
                    information: information.clone(),
                    suffix: self.suffix(&branch.receipt.program, read.stage)?,
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
            branch.table.to_act().ok_or(SequenceError::WrongContext)?.1,
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
        let views = std::array::from_fn(|seat| Information::of(&branch.table.view(seat as u8)));
        let legal = branch.table.to_act().ok_or(SequenceError::WrongContext)?.1.to_vec();
        let cached = self
            .limits
            .reuse_suffixes
            .then(|| {
                self.tails
                    .iter()
                    .position(|run| run.views == views && run.legal == legal)
            })
            .flatten();
        let (outcome, first, tricks, consumed) = if let Some(index) = cached {
            self.charge(self.tails[index].transitions, 0)?;
            self.work.suffix_hits += 1;
            let run = &self.tails[index];
            (run.outcome, run.first, run.tricks.clone(), run.consumed.clone())
        } else {
            let before = self.work.transitions;
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
                transitions: self.work.transitions - before,
            };
            let bytes = 2
                * (std::mem::size_of_val(&run)
                    + run.views.iter().map(info_bytes).sum::<usize>()
                    + vec_bytes(&run.legal)
                    + vec_bytes(&run.tricks)
                    + run.tricks.iter().map(|trick| info_bytes(&trick.root)).sum::<usize>()
                    + vec_bytes(&run.consumed)
                    + run.consumed.iter().map(consumed_bytes).sum::<usize>());
            if self.limits.reuse_suffixes && self.tails.len() < self.limits.memo_entries && self.retain_reusable(bytes)
            {
                self.tails.push(TailRun {
                    views: run.views,
                    legal: run.legal,
                    outcome: run.outcome,
                    first: run.first,
                    tricks: run.tricks.clone(),
                    consumed: run.consumed.clone(),
                    transitions: run.transitions,
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
        let cached = self.memo.iter().position(|run| run.key == key);
        let mut terminals = if let Some(index) = cached {
            self.charge(self.memo[index].transitions, self.memo[index].comparisons)?;
            self.work.suffix_hits += 1;
            prepared.extend(self.memo[index].prepared.iter().cloned());
            self.memo[index].terminals.clone()
        } else {
            let before = self.work.represented_transitions;
            let comparisons_before = self.work.represented_comparisons;
            let mut records = Vec::new();
            let terminals = self.finish_uncached(branches, &mut records)?;
            if self.limits.reuse_suffixes
                && self.memo.len() < self.limits.memo_entries
                && self.retain(run_bytes(&key, &terminals, &records))
            {
                self.memo.push(CachedRun {
                    key,
                    terminals: terminals.clone(),
                    prepared: records.clone(),
                    transitions: self.work.represented_transitions - before,
                    comparisons: self.work.represented_comparisons - comparisons_before,
                });
            }
            prepared.extend(records);
            terminals
        };
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
        let boundary = match group[0].alternatives.as_slice() {
            [Alternative::Reconsider { stage, remaining }] => Some((*stage, *remaining)),
            _ => None,
        };
        if let Some((stage, remaining)) = boundary {
            return self.reconsider(group, stage, remaining, prepared);
        }
        self.choose_bound(group, prepared, None)
    }
    fn reconsider(
        &mut self,
        mut group: Vec<Waiting>,
        stage: Stage,
        remaining: u8,
        prepared: &mut Vec<PreparedChoice>,
    ) -> Result<Vec<Terminal>, SequenceError> {
        self.check()?;
        let first = &group[0].branch;
        let view = first.table.view(self.root);
        let legal = first.table.to_act().ok_or(SequenceError::WrongContext)?.1;
        // finish_uncached grouped full root Information + canonical boundary
        // BEFORE binding/ranking. The binder is pure data expansion, not policy.
        let binding = match bind_reconsideration(&first.receipt.program, stage, &view, legal) {
            Ok(binding) => binding,
            Err(SequenceError::Budget) => return Err(self.limited(DeferredLimit::Reconsideration { remaining })),
            Err(error) => return Err(error),
        };
        let membership = Membership {
            legal: binding.legal.clone(),
            programs: binding
                .alternatives
                .iter()
                .map(|stage| self.suffix(&binding.program, *stage))
                .collect::<Result<_, _>>()?,
        };
        for waiting in &mut group {
            if !matches!(waiting.alternatives.as_slice(), [Alternative::Reconsider { remaining: n, .. }] if *n == remaining)
            {
                return Err(SequenceError::WrongContext);
            }
            let view = waiting.branch.table.view(self.root);
            let legal = waiting.branch.table.to_act().ok_or(SequenceError::WrongContext)?.1;
            waiting
                .branch
                .receipt
                .install_reconsideration_at_frontier(&binding, &view, legal)?;
            waiting.alternatives = binding.alternatives.iter().copied().map(Alternative::Stage).collect();
        }
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
        let membership = match membership {
            Some(membership) => membership,
            None => self.membership(&view, first.table.to_act().ok_or(SequenceError::WrongContext)?.1)?,
        };
        let consumed = first.consumed.clone();
        let mut outcomes = Vec::new();
        let mut records = Vec::new();
        let support = group
            .iter()
            .map(|waiting| (waiting.branch.origin, waiting.branch.weight))
            .collect();
        // Lazy depth-first demand, one child cohort at a time. Equal semantic
        // alternatives/suffixes use finish memo; unfinished children never rank.
        for index in 0..alternatives.len() {
            let mut children = Vec::new();
            for waiting in &group {
                let mut branch = waiting.branch.clone();
                match &waiting.alternatives[index] {
                    Alternative::Stage(stage) => branch.receipt.stage = *stage,
                    Alternative::Offer(offer) => self.commit(&mut branch, offer.clone())?,
                    Alternative::Reconsider { .. } => return Err(SequenceError::InvalidProgram),
                }
                children.push(branch);
            }
            let mut child_records = Vec::new();
            outcomes.push(self.finish(children, &mut child_records)?);
            records.push(child_records);
        }
        let assessment = self.values(outcomes.iter().map(Vec::as_slice))?;
        let selected = assessment.selected;
        prepared.push(PreparedChoice {
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
pub fn evaluate<F: FnMut(&SeatView<'_>, &[GameAction], LeafBudget) -> Result<GameAction, SequenceError>>(
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
pub fn evaluate_with_cache<F: FnMut(&SeatView<'_>, &[GameAction], LeafBudget) -> Result<GameAction, SequenceError>>(
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
            || limits.memo_entries == 0
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
        let mut mass = 0.0;
        for (table, weight) in cohort {
            if !weight.is_finite()
                || *weight <= 0.0
                || !information.matches(&table.view(root))
                || !table
                    .to_act()
                    .is_some_and(|(seat, actions)| seat == root && actions == legal)
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
        let prepared = selected.prepared.clone();
        Ok(Comparison {
            executions,
            values,
            receipt,
            prepared,
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
        assert!(matches!(
            run(Limits {
                memo_bytes: 0,
                ..Limits::default()
            })
            .limit,
            Some(DeferredLimit::ResponseCache { remaining_bytes: 0, .. })
        ));
        let mut unable = |_: &SeatView<'_>, _: &[GameAction], _: LeafBudget| Err(SequenceError::Budget);
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
    #[test]
    fn reconsideration_compares_one_joint_group_and_returns_the_actual_program_owner() {
        let mut table = at_lead();
        while table.view(0).knowledge.hand_len(0) > 2 || table.game.state.phase != GamePhase::StartTrick {
            let (actor, legal) = table.to_act().unwrap();
            let action = leaf(
                &table.view(actor),
                legal,
                LeafBudget {
                    deadline: None,
                    remaining_transitions: 100,
                },
            )
            .unwrap();
            table.apply(action);
        }
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
        let binding = bind_reconsideration(&program, program.start, &view, legal).unwrap();
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
            .prepared
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
    fn no_leaf(_: &SeatView<'_>, _: &[GameAction], _: LeafBudget) -> Result<GameAction, SequenceError> {
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
    fn membership_reuse_requires_exact_private_view_and_ordered_legal_actions() {
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
        assert_eq!(session.memberships.len(), entries + 1);
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
        assert_eq!(session.memberships.len(), entries + 2);
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
    fn mandatory_response_admission_drops_optional_capacity_without_resampling() {
        let context = context();
        let calls = std::cell::Cell::new(0usize);
        let mut leaf = |_: &SeatView<'_>, legal: &[GameAction], _: LeafBudget| {
            let call = calls.get();
            calls.set(call + 1);
            Ok(legal[call % legal.len()])
        };
        let mut session = session(&context, &mut leaf, Limits::default());
        let mut hands: [Vec<Card>; 4] =
            std::array::from_fn(|seat| (0..9).map(|i| Card::from_index((9 * seat + i) as u8)).collect());
        let first_table = Table::new(&hands);
        let frozen = session.response(&first_table).unwrap();
        let first_bytes = session.response_bytes;
        session.limits.memo_bytes = 2 * first_bytes;
        let tail = Program {
            name: "optional",
            version: 1,
            start: Stage(0),
            nodes: vec![Node::Tail],
        };
        session.suffix(&tail, tail.start).unwrap();
        assert!(session.work.retained_bytes > first_bytes);
        assert!(!session.canonical.is_empty());
        let card = hands[0][0];
        hands[0][0] = hands[1][0];
        hands[1][0] = card;
        session.response(&Table::new(&hands)).unwrap();
        assert_eq!(calls.get(), 2);
        assert!(session.canonical.is_empty());
        assert_eq!(session.canonical.capacity(), 0);
        assert_eq!(session.work.retained_bytes, session.response_bytes);
        assert_eq!(session.work.retained_bytes, session.reusable_bytes);
        assert!(session.work.retained_bytes <= session.limits.memo_bytes);
        assert_eq!(session.response(&first_table).unwrap(), frozen);
        assert_eq!(calls.get(), 2, "eviction must not realize an old timed response again");
        // Both frozen keys now fill the response allowance. A new private input
        // defers rather than evicting a response or exposing a partial value.
        hands[0].swap(0, 1); // same card set: this is deliberately NOT new info
        assert_eq!(session.response(&Table::new(&hands)).unwrap(), session.responses[1].1);
        let card = hands[0][1];
        hands[0][1] = hands[1][1];
        hands[1][1] = card;
        assert_eq!(session.response(&Table::new(&hands)), Err(SequenceError::Budget));
        assert!(matches!(session.limit, Some(DeferredLimit::ResponseCache { .. })));
        assert_eq!(calls.get(), 2);
    }
}

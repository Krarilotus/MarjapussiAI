//! Finite authored continuations over a preserved joint cohort. Suffix choices
//! belong to actor information sets, never individual hidden worlds.

use crate::concepts::sequence::{
    authored_programs, inspect, inspect_with, Frontier, Information, Offer, Program, ReadMeaning, Receipt,
    SequenceError, Stage, Suffix,
};
use crate::concepts::value::{Assessment, Provenance, ValueStore};
use agent::{SeatView, Table};
use beliefs::{BeliefParams, ConceptBudget, Mind};
use engine::{GameAction, GameFinishedInfo, GameOutcome, PlayPhase, PublicState};
use std::time::Instant;

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
            Frontier::Tail | Frontier::Interrupted => return Err(NeedsReview),
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
    program: Program,
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
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Alternative {
    Stage(Stage),
    Offer(Offer),
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
    fn check(&self) -> Result<(), SequenceError> {
        if self.limits.deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(SequenceError::Budget);
        }
        Ok(())
    }
    /// Completed cached units cost the same logical work as their expansion.
    /// Reject an unaffordable unit before exposing any of its values or trace.
    fn charge(&mut self, transitions: usize, comparisons: usize) -> Result<(), SequenceError> {
        self.check()?;
        if transitions
            > self
                .limits
                .transitions
                .saturating_sub(self.work.represented_transitions)
            || comparisons
                > self
                    .limits
                    .comparisons
                    .saturating_sub(self.work.represented_comparisons)
        {
            return Err(SequenceError::Budget);
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
            if self.canonical.len() < self.limits.memo_entries && self.retain(bytes) {
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
            if self.retain(2 * suffix_bytes(&suffix)) {
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
        if self.memberships.len() < self.limits.memo_entries && self.retain(bytes) {
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
            return Err(SequenceError::Budget);
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
        let key = ResponseKey {
            information: Information::of(&view),
            legal: legal.to_vec(),
        };
        // Frozen responses cannot be evicted then resampled. Exhaustion defers
        // the entire comparison, retaining difficult worlds rather than dropping.
        let bytes = 2 * (info_bytes(&key.information) + vec_bytes(&key.legal) + std::mem::size_of::<GameAction>());
        if self.responses.len() >= self.limits.memo_entries.saturating_mul(32) || !self.retain(bytes) {
            return Err(SequenceError::Budget);
        }
        let action = (self.leaf)(
            &view,
            legal,
            LeafBudget {
                deadline: self.limits.deadline,
                remaining_transitions: self
                    .limits
                    .transitions
                    .saturating_sub(self.work.represented_transitions),
            },
        )?;
        self.work.response_calls += 1;
        self.check()?;
        if !legal.contains(&action) || action.player.0 != seat {
            return Err(SequenceError::IllegalAction);
        }
        self.responses.push((key, action));
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
            if self.hard_reads.len() < self.limits.memo_entries && self.retain(bytes) {
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
        Err(SequenceError::Budget)
    }
    fn commit(&mut self, branch: &mut Branch, offer: Offer) -> Result<(), SequenceError> {
        branch.receipt.propose(
            offer.clone(),
            &branch.table.view(self.root),
            branch.table.to_act().ok_or(SequenceError::WrongContext)?.1,
        )?;
        branch.first.get_or_insert_with(|| offer.clone());
        self.apply(branch, offer.action)
    }
    /// The named leaf has NO root suffix choices, so its deterministic result
    /// can be shared per complete hypothetical input without strategy fusion.
    /// This boundary also shares tails reached through different guard paths.
    fn tail(&mut self, mut branch: Branch, tail: Stage) -> Result<Terminal, SequenceError> {
        let views = std::array::from_fn(|seat| Information::of(&branch.table.view(seat as u8)));
        let legal = branch.table.to_act().ok_or(SequenceError::WrongContext)?.1.to_vec();
        let cached = self
            .tails
            .iter()
            .position(|run| run.views == views && run.legal == legal);
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
                return Err(SequenceError::Budget);
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
            if self.limits.reuse_suffixes && self.tails.len() < self.limits.memo_entries && self.retain(bytes) {
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
        branch.first.get_or_insert(Offer {
            action: first,
            next: tail,
        });
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
        // A finish cohort comes from one candidate: children vary only stage,
        // intent and observations, never the immutable authored program.
        let program = branches[0].receipt.program.clone();
        debug_assert!(branches.iter().all(|branch| branch.receipt.program == program));
        let prefixes: Vec<_> = branches
            .iter_mut()
            .map(|branch| {
                (
                    branch.origin,
                    branch.first.take(),
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
            let cached_program = self.memo[index].program.clone();
            let mut terminals = self.memo[index].terminals.clone();
            for terminal in &mut terminals {
                if let Some(first) = &mut terminal.first {
                    let suffix = self.suffix(&cached_program, first.next)?;
                    first.next = self.find_stage(&program, &suffix)?;
                }
            }
            terminals
        } else {
            let before = self.work.represented_transitions;
            let comparisons_before = self.work.represented_comparisons;
            let mut records = Vec::new();
            let terminals = self.finish_uncached(branches, &mut records)?;
            if self.limits.reuse_suffixes
                && self.memo.len() < self.limits.memo_entries
                && self.retain(run_bytes(&key, &terminals, &program, &records))
            {
                self.memo.push(CachedRun {
                    key,
                    terminals: terminals.clone(),
                    program,
                    prepared: records.clone(),
                    transitions: self.work.represented_transitions - before,
                    comparisons: self.work.represented_comparisons - comparisons_before,
                });
            }
            prepared.extend(records);
            terminals
        };
        for terminal in &mut terminals {
            let (_, first, tricks, consumed, interrupted) = prefixes
                .iter()
                .find(|prefix| prefix.0 == terminal.origin)
                .ok_or(SequenceError::InvalidCohort)?;
            if let Some(first) = first {
                terminal.first = Some(first.clone());
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
        let membership = self.membership(&view, first.table.to_act().ok_or(SequenceError::WrongContext)?.1)?;
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
    let mut session = Session {
        root,
        provenance,
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
            if terminals.iter().any(|terminal| terminal.first.as_ref() != Some(&first)) {
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
        let mut receipt = Receipt::new(selected.candidate.program.clone(), &original)?;
        receipt.stage = selected.candidate.stage;
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
    result.map_err(|reason| Deferred {
        reason,
        work: session.work,
        total_candidates: candidates.len(),
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
fn run_bytes(
    key: &Vec<BranchKey>,
    terminals: &Vec<Terminal>,
    program: &Program,
    prepared: &Vec<PreparedChoice>,
) -> usize {
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
                    + t.tricks.iter().map(|trick| info_bytes(&trick.root)).sum::<usize>()
                    + vec_bytes(&t.consumed)
                    + t.consumed.iter().map(consumed_bytes).sum::<usize>()
            })
            .sum::<usize>()
        + program_bytes(program)
        + vec_bytes(prepared)
        + prepared.iter().map(prepared_bytes).sum::<usize>())
}

#[cfg(test)]
#[path = "../../tests/internal/sequence_execution.rs"]
mod tests;

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
}

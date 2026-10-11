//! Authored finite continuation data. No node chooses a tactic in a callback.
//! All actions use engine legality and all guards read only the acting view.

use agent::{SeatKind, SeatView, SeriesScore};
use beliefs::{ConceptBudget, ConceptQuery, Mind};
use engine::{ActionType, CardSet, GameAction, PlayPhase, Suit, SUITS};
use knowledge::Knowledge;
use observation::SeatEvent;

/// Structural caps for a composed current-information catalogue. Exceeding a
/// cap defers the complete comparison; it never trims or ranks alternatives.
pub const MAX_STAGES: usize = 4096;
pub const MAX_ALTERNATIVES: usize = 128;
/// Semantic lookahead, independent of wall time and logical-work allowances.
pub const MAX_RECONSIDERATIONS: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stage(pub usize);

/// Binding enumerates legal members; it never ranks or picks a tactic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Actions {
    Exact(ActionType),
    /// All current legal raises, sharing a post-acceptance continuation edge.
    Raises,
    Cards(CardSet),
    /// Rebind known team-winning leads at the current observation. Partner may
    /// overtake; this predicate does NOT prove that this seat retains its lead.
    Standing(CardSet),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Guard {
    OwnLead,
    Leader(u8),
    Trump(Suit),
    OwnCards(CardSet),
    /// Only universal hard truth is a true guard. Unknown has its own edge.
    Known(ConceptQuery),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Action {
        actions: Actions,
        next: Stage,
    },
    Guard {
        predicate: Guard,
        yes: Stage,
        no: Stage,
        unknown: Stage,
    },
    Choice(Vec<Stage>),
    /// Bind a complete current-information catalogue plus the retained edge.
    /// The evaluator ranks that bound Choice on the whole information group.
    Reconsider {
        remaining: u8,
        retained: Stage,
    },
    /// Named response approximation continues to a real terminal outcome.
    Tail,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    pub name: &'static str,
    pub version: u32,
    pub start: Stage,
    pub nodes: Vec<Node>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SequenceError {
    InvalidProgram,
    HistoryChanged,
    WrongContext,
    IllegalAction,
    Budget,
    InvalidCohort,
    InvalidValue,
}

impl Program {
    /// First slice is a finite DAG. Repetition is authored as bounded stages;
    /// cyclic and unbounded programs are deliberately unsupported.
    pub fn validate(&self) -> Result<(), SequenceError> {
        if self.version == 0 || self.name.is_empty() || self.nodes.is_empty() || self.nodes.len() > MAX_STAGES {
            return Err(SequenceError::InvalidProgram);
        }
        fn visit(program: &Program, stage: Stage, marks: &mut [u8]) -> Result<(), SequenceError> {
            let mark = marks.get(stage.0).copied().ok_or(SequenceError::InvalidProgram)?;
            if mark == 1 {
                return Err(SequenceError::InvalidProgram);
            }
            if mark == 2 {
                return Ok(());
            }
            marks[stage.0] = 1;
            match &program.nodes[stage.0] {
                Node::Action { actions, next } => {
                    let valid = match actions {
                        Actions::Raises => true,
                        Actions::Cards(cards) | Actions::Standing(cards) => {
                            !cards.is_empty() && cards.0 & !CardSet::ALL.0 == 0
                        }
                        Actions::Exact(kind) => matches!(
                            kind,
                            ActionType::CardPlayed(_)
                                | ActionType::NewBid(_)
                                | ActionType::Question(_)
                                | ActionType::AnnounceTrump(_)
                                | ActionType::Answer(_)
                        ),
                    };
                    if !valid {
                        return Err(SequenceError::InvalidProgram);
                    }
                    visit(program, *next, marks)?;
                }
                Node::Guard {
                    predicate,
                    yes,
                    no,
                    unknown,
                } => {
                    // Conditional facts are hypotheses, never actual-state
                    // guards. A later conditional operator must bind/verify
                    // its premise explicitly before it can execute an action.
                    if matches!(predicate, Guard::Known(query) if !query.given.is_empty())
                        || matches!(predicate, Guard::Leader(seat) if *seat >= 4)
                    {
                        return Err(SequenceError::InvalidProgram);
                    }
                    for next in [yes, no, unknown] {
                        visit(program, *next, marks)?;
                    }
                }
                Node::Choice(alternatives) => {
                    if alternatives.is_empty() || alternatives.len() > MAX_ALTERNATIVES {
                        return Err(SequenceError::InvalidProgram);
                    }
                    for next in alternatives {
                        visit(program, *next, marks)?;
                    }
                }
                Node::Reconsider { remaining, retained } => {
                    if *remaining == 0 || *remaining > MAX_RECONSIDERATIONS {
                        return Err(SequenceError::InvalidProgram);
                    }
                    visit(program, *retained, marks)?;
                }
                Node::Tail => {}
            }
            marks[stage.0] = 2;
            Ok(())
        }
        // Validate every node, including unreachable authored data.
        let mut marks = vec![0; self.nodes.len()];
        visit(self, self.start, &mut marks)?;
        for i in 0..self.nodes.len() {
            visit(self, Stage(i), &mut marks)?;
        }
        Ok(())
    }

    /// Canonical reachable DAG data. Cosmetic labels, raw indices and unused
    /// nodes are excluded; alternative order remains semantic (stable ties).
    pub fn suffix(&self, start: Stage) -> Result<Suffix, SequenceError> {
        self.validate()?;
        self.suffix_validated(start)
    }

    /// Comparison-local callers may reuse validation of this exact immutable
    /// program. Keep the public entry point validating untrusted authored data.
    pub(crate) fn suffix_validated(&self, start: Stage) -> Result<Suffix, SequenceError> {
        fn copy(
            program: &Program,
            stage: Stage,
            remap: &mut [Option<Stage>],
            nodes: &mut Vec<Node>,
        ) -> Result<Stage, SequenceError> {
            if let Some(mapped) = remap.get(stage.0).copied().flatten() {
                return Ok(mapped);
            }
            let node = match program.nodes.get(stage.0).ok_or(SequenceError::InvalidProgram)? {
                Node::Action { actions, next } => Node::Action {
                    actions: actions.clone(),
                    next: copy(program, *next, remap, nodes)?,
                },
                Node::Guard {
                    predicate,
                    yes,
                    no,
                    unknown,
                } => Node::Guard {
                    predicate: predicate.clone(),
                    yes: copy(program, *yes, remap, nodes)?,
                    no: copy(program, *no, remap, nodes)?,
                    unknown: copy(program, *unknown, remap, nodes)?,
                },
                Node::Choice(alternatives) => Node::Choice(
                    alternatives
                        .iter()
                        .map(|stage| copy(program, *stage, remap, nodes))
                        .collect::<Result<_, _>>()?,
                ),
                Node::Reconsider { remaining, retained } => Node::Reconsider {
                    remaining: *remaining,
                    retained: copy(program, *retained, remap, nodes)?,
                },
                Node::Tail => Node::Tail,
            };
            let mapped = match nodes.iter().position(|old| *old == node) {
                Some(index) => Stage(index),
                None => {
                    let index = nodes.len();
                    nodes.push(node);
                    Stage(index)
                }
            };
            remap[stage.0] = Some(mapped);
            Ok(mapped)
        }
        let mut nodes = Vec::new();
        let start = copy(self, start, &mut vec![None; self.nodes.len()], &mut nodes)?;
        Ok(Suffix { start, nodes })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suffix {
    pub start: Stage,
    pub nodes: Vec<Node>,
}

/// A fully bound comparison domain, not an assessment or selected route.
/// All alternative stages share this one canonical DAG and one observer input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reconsideration {
    pub information: Information,
    pub legal: Vec<GameAction>,
    pub source: Suffix,
    pub program: Program,
    pub alternatives: Vec<Stage>,
    pub retained: Stage,
}

#[derive(Default)]
struct Composition {
    nodes: Vec<Node>,
}
impl Composition {
    fn intern(&mut self, node: Node) -> Result<Stage, SequenceError> {
        if let Some(index) = self.nodes.iter().position(|old| *old == node) {
            return Ok(Stage(index));
        }
        if self.nodes.len() >= MAX_STAGES {
            return Err(SequenceError::Budget);
        }
        let stage = Stage(self.nodes.len());
        self.nodes.push(node);
        Ok(stage)
    }

    fn import(
        &mut self,
        program: &Program,
        stage: Stage,
        remap: &mut [Option<Stage>],
        depth: Option<u8>,
    ) -> Result<Stage, SequenceError> {
        if let Some(mapped) = remap.get(stage.0).copied().flatten() {
            return Ok(mapped);
        }
        let node = match program.nodes.get(stage.0).ok_or(SequenceError::InvalidProgram)? {
            Node::Action { actions, next } => {
                let mut next = self.import(program, *next, remap, depth)?;
                if let Some(remaining) = depth.filter(|remaining| *remaining > 0) {
                    if !matches!(self.nodes[next.0], Node::Reconsider { remaining: n, .. } if n == remaining) {
                        next = self.intern(Node::Reconsider {
                            remaining,
                            retained: next,
                        })?;
                    }
                }
                Node::Action {
                    actions: actions.clone(),
                    next,
                }
            }
            Node::Guard {
                predicate,
                yes,
                no,
                unknown,
            } => Node::Guard {
                predicate: predicate.clone(),
                yes: self.import(program, *yes, remap, depth)?,
                no: self.import(program, *no, remap, depth)?,
                unknown: self.import(program, *unknown, remap, depth)?,
            },
            Node::Choice(alternatives) => {
                let mut bound = Vec::new();
                for alternative in alternatives {
                    bound.push(self.import(program, *alternative, remap, depth)?);
                }
                Node::Choice(bound)
            }
            Node::Reconsider { remaining, retained } => {
                let retained = self.import(program, *retained, remap, depth)?;
                let remaining = depth.map_or(*remaining, |cap| (*remaining).min(cap));
                if remaining == 0 {
                    remap[stage.0] = Some(retained);
                    return Ok(retained);
                }
                Node::Reconsider { remaining, retained }
            }
            Node::Tail => Node::Tail,
        };
        let mapped = self.intern(node)?;
        remap[stage.0] = Some(mapped);
        Ok(mapped)
    }
}

/// Install a finite opportunity to reconsider after each authored action. On
/// expansion the remaining count is reduced on ALL successor alternatives,
/// including the retained suffix, so no path can replenish its semantic depth.
/// Zero denotes the underlying authored routes with no global reconsideration.
pub fn with_reconsideration(program: &Program, remaining: u8) -> Result<Program, SequenceError> {
    if remaining > MAX_RECONSIDERATIONS {
        return Err(SequenceError::InvalidProgram);
    }
    program.validate()?;
    let mut composition = Composition::default();
    let start = composition.import(
        program,
        program.start,
        &mut vec![None; program.nodes.len()],
        Some(remaining),
    )?;
    Ok(Program {
        name: program.name,
        version: program.version,
        start,
        nodes: composition.nodes,
    })
}

/// Complete authored catalogue with an explicit semantic horizon. This binds
/// data only. The caller must jointly rank all returned alternatives or defer.
pub fn authored_programs_with_reconsideration(
    view: &SeatView<'_>,
    legal: &[GameAction],
    remaining: u8,
) -> Result<Vec<Program>, SequenceError> {
    let mut programs = Vec::new();
    let mut identities = Vec::new();
    for program in authored_programs(view, legal) {
        let program = with_reconsideration(&program, remaining)?;
        let identity = program.suffix(program.start)?;
        if !identities.contains(&identity) {
            if programs.len() >= MAX_ALTERNATIVES {
                return Err(SequenceError::Budget);
            }
            identities.push(identity);
            programs.push(program);
        }
    }
    Ok(programs)
}

/// Bind at the actual observed boundary, never at the original decision's view.
/// The resulting Choice must be evaluated on the complete conditional root
/// information group, preserving its mass, before any ancestor can be valued.
pub fn bind_reconsideration(
    program: &Program,
    stage: Stage,
    view: &SeatView<'_>,
    legal: &[GameAction],
) -> Result<Reconsideration, SequenceError> {
    let Some(Node::Reconsider { remaining, retained }) = program.nodes.get(stage.0) else {
        return Err(SequenceError::InvalidProgram);
    };
    if *remaining == 0 {
        return Err(SequenceError::Budget);
    }
    program.validate()?;
    let source = program.suffix(stage)?;
    let old_suffix = program.suffix(*retained)?;
    let retained_program = with_reconsideration(
        &Program {
            name: "retained-current-suffix",
            version: 1,
            start: old_suffix.start,
            nodes: old_suffix.nodes,
        },
        remaining - 1,
    )?;
    let mut programs = authored_programs_with_reconsideration(view, legal, remaining - 1)?;
    programs.push(retained_program);
    let mut composition = Composition::default();
    let mut alternatives = Vec::new();
    let mut retained = None;
    let last = programs.len() - 1;
    for (index, candidate) in programs.iter().enumerate() {
        let start = composition.import(candidate, candidate.start, &mut vec![None; candidate.nodes.len()], None)?;
        if index == last {
            retained = Some(start);
        }
        if !alternatives.contains(&start) {
            if alternatives.len() >= MAX_ALTERNATIVES {
                return Err(SequenceError::Budget);
            }
            alternatives.push(start);
        }
    }
    let start = composition.intern(Node::Choice(alternatives.clone()))?;
    let program = Program {
        name: "current-information-reconsideration",
        version: 1,
        start,
        nodes: composition.nodes,
    };
    program.validate()?;
    Ok(Reconsideration {
        information: Information::of(view),
        legal: legal.to_vec(),
        source,
        program,
        alternatives,
        retained: retained.ok_or(SequenceError::InvalidProgram)?,
    })
}

/// Full equality, not a digest or exposed allocation, defines an information set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Information {
    pub seat: u8,
    pub knowledge: Knowledge,
    pub events: Vec<SeatEvent>,
    pub kinds: [SeatKind; 4],
    pub series: SeriesScore,
}

impl Information {
    pub fn of(view: &SeatView<'_>) -> Self {
        Self {
            seat: view.seat,
            knowledge: *view.knowledge,
            events: view.events.to_vec(),
            kinds: view.kinds,
            series: view.series,
        }
    }

    pub fn matches(&self, view: &SeatView<'_>) -> bool {
        self.seat == view.seat
            && self.knowledge == *view.knowledge
            && self.events == view.events
            && self.kinds == view.kinds
            && self.series == view.series
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub action: GameAction,
    pub next: Stage,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Frontier {
    Actions(Vec<Offer>),
    Choice(Vec<Stage>),
    Reconsider {
        stage: Stage,
        remaining: u8,
        retained: Stage,
    },
    Tail,
    /// An unavailable action is a failed route, not a discarded hidden world.
    Interrupted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inspection {
    pub frontier: Frontier,
    pub consumed: Vec<Read>,
}

/// Consumed dependency data, not an explanation reconstructed after selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read {
    pub stage: Stage,
    pub event_count: usize,
    pub meaning: ReadMeaning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadMeaning {
    Guard {
        predicate: Guard,
        truth: Option<bool>,
    },
    Binding {
        actions: Actions,
        legal_members: Vec<GameAction>,
    },
    Alternatives(Vec<Stage>),
    Reconsider {
        remaining: u8,
        retained: Stage,
    },
    ResponseLeaf,
}

/// Guards read the observer's store; no table or other seat's backend is exposed.
pub fn inspect(
    program: &Program,
    stage: Stage,
    view: &SeatView<'_>,
    legal: &[GameAction],
    mind: &Mind,
) -> Result<Inspection, SequenceError> {
    if mind.seat() != view.seat || mind.knowledge() != view.knowledge {
        return Err(SequenceError::WrongContext);
    }
    inspect_with(program, stage, view, legal, |query| {
        mind.concept(query, ConceptBudget { max_nodes: 0 })
            .ok()
            .and_then(|estimate| estimate.known)
    })
}

/// Internal shared dispatcher. The reader supplies only unconditional hard
/// truth from the same observer. It cannot select an action or rank a tactic.
pub(crate) fn inspect_with(
    program: &Program,
    mut stage: Stage,
    view: &SeatView<'_>,
    legal: &[GameAction],
    mut known: impl FnMut(&ConceptQuery) -> Option<bool>,
) -> Result<Inspection, SequenceError> {
    let mut consumed = Vec::new();
    for _ in 0..MAX_STAGES {
        let node = program.nodes.get(stage.0).ok_or(SequenceError::InvalidProgram)?;
        let frontier = match node {
            Node::Action { actions, next } => {
                let standing = match actions {
                    Actions::Standing(_) => view.knowledge.standing(view.seat),
                    _ => CardSet::EMPTY,
                };
                let offers: Vec<_> = legal.iter().filter(|action| action.player.0 == view.seat && match actions {
                    Actions::Exact(kind) => action.action_type == *kind,
                    Actions::Raises => matches!(action.action_type, ActionType::NewBid(_)),
                    Actions::Cards(cards) => matches!(action.action_type, ActionType::CardPlayed(card) if cards.contains(card.index())),
                    Actions::Standing(cards) => matches!(action.action_type, ActionType::CardPlayed(card) if (*cards & standing).contains(card.index())),
                }).map(|action| Offer { action: *action, next: *next }).collect();
                if offers.len() > MAX_ALTERNATIVES {
                    return Err(SequenceError::Budget);
                }
                consumed.push(Read {
                    stage,
                    event_count: view.events.len(),
                    meaning: ReadMeaning::Binding {
                        actions: actions.clone(),
                        legal_members: offers.iter().map(|offer| offer.action).collect(),
                    },
                });
                if offers.is_empty() {
                    Frontier::Interrupted
                } else {
                    Frontier::Actions(offers)
                }
            }
            Node::Guard {
                predicate,
                yes,
                no,
                unknown,
            } => {
                let truth = match predicate {
                    Guard::OwnLead => {
                        let public = view.knowledge.public_state(view.seat, PlayPhase::from_actions(legal));
                        Some(public.trick_len == 0 && public.leader == view.seat)
                    }
                    Guard::Leader(seat) => Some(
                        view.knowledge
                            .public_state(view.seat, PlayPhase::from_actions(legal))
                            .leader
                            == *seat,
                    ),
                    Guard::Trump(suit) => Some(view.knowledge.trump() == Some(*suit)),
                    Guard::OwnCards(cards) => Some(cards.is_subset(view.knowledge.certain(view.seat))),
                    Guard::Known(query) => {
                        if !query.given.is_empty() {
                            return Err(SequenceError::InvalidProgram);
                        }
                        known(query)
                    }
                };
                consumed.push(Read {
                    stage,
                    event_count: view.events.len(),
                    meaning: ReadMeaning::Guard {
                        predicate: predicate.clone(),
                        truth,
                    },
                });
                stage = match truth {
                    Some(true) => *yes,
                    Some(false) => *no,
                    None => *unknown,
                };
                continue;
            }
            Node::Choice(alternatives) => {
                consumed.push(Read {
                    stage,
                    event_count: view.events.len(),
                    meaning: ReadMeaning::Alternatives(alternatives.clone()),
                });
                Frontier::Choice(alternatives.clone())
            }
            Node::Reconsider { remaining, retained } => {
                if *remaining == 0 {
                    return Err(SequenceError::Budget);
                }
                if *remaining > MAX_RECONSIDERATIONS {
                    return Err(SequenceError::InvalidProgram);
                }
                consumed.push(Read {
                    stage,
                    event_count: view.events.len(),
                    meaning: ReadMeaning::Reconsider {
                        remaining: *remaining,
                        retained: *retained,
                    },
                });
                Frontier::Reconsider {
                    stage,
                    remaining: *remaining,
                    retained: *retained,
                }
            }
            Node::Tail => {
                consumed.push(Read {
                    stage,
                    event_count: view.events.len(),
                    meaning: ReadMeaning::ResponseLeaf,
                });
                Frontier::Tail
            }
        };
        return Ok(Inspection { frontier, consumed });
    }
    Err(SequenceError::InvalidProgram)
}

/// Private semantic state. An offer is pending until its actual observed action
/// is accepted. Cache eviction cannot erase this state or spend its resources.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub program: Program,
    pub stage: Stage,
    pub accepted: Information,
    pub pending: Option<Offer>,
}

impl Receipt {
    pub fn new(program: Program, view: &SeatView<'_>) -> Result<Self, SequenceError> {
        program.validate()?;
        Ok(Self {
            stage: program.start,
            program,
            accepted: Information::of(view),
            pending: None,
        })
    }

    pub fn propose(&mut self, offer: Offer, view: &SeatView<'_>, legal: &[GameAction]) -> Result<(), SequenceError> {
        if !self.accepted.matches(view) || !legal.contains(&offer.action) || offer.action.player.0 != view.seat {
            return Err(SequenceError::WrongContext);
        }
        if offer.next.0 >= self.program.nodes.len() {
            return Err(SequenceError::InvalidProgram);
        }
        self.pending = Some(offer);
        Ok(())
    }

    /// Replace a reached boundary with its complete bound comparison graph.
    /// Installing is not choosing or accepting an action: the evaluator/live
    /// lookup must still select an actually compared alternative and propose it.
    pub fn install_reconsideration(
        &mut self,
        binding: &Reconsideration,
        view: &SeatView<'_>,
        legal: &[GameAction],
        mind: &Mind,
    ) -> Result<(), SequenceError> {
        if self.pending.is_some()
            || !self.accepted.matches(view)
            || !binding.information.matches(view)
            || binding.legal != legal
        {
            return Err(SequenceError::WrongContext);
        }
        let Frontier::Reconsider { stage, .. } = inspect(&self.program, self.stage, view, legal, mind)?.frontier else {
            return Err(SequenceError::WrongContext);
        };
        if self.program.suffix(stage)? != binding.source
            || binding.program.nodes.get(binding.program.start.0) != Some(&Node::Choice(binding.alternatives.clone()))
            || !binding.alternatives.contains(&binding.retained)
        {
            return Err(SequenceError::WrongContext);
        }
        binding.program.validate()?;
        self.program = binding.program.clone();
        self.stage = self.program.start;
        Ok(())
    }

    pub fn synchronize(&mut self, view: &SeatView<'_>) -> Result<(), SequenceError> {
        if self.accepted.seat != view.seat
            || self.accepted.kinds != view.kinds
            || self.accepted.series != view.series
            || !view.events.starts_with(&self.accepted.events)
        {
            return Err(SequenceError::HistoryChanged);
        }
        let delta = &view.events[self.accepted.events.len()..];
        if delta.is_empty() {
            return if self.accepted.matches(view) {
                Ok(())
            } else {
                Err(SequenceError::HistoryChanged)
            };
        }
        if delta.iter().any(|event| matches!(event, SeatEvent::Undone)) {
            return Err(SequenceError::HistoryChanged);
        }
        if let Some(offer) = &self.pending {
            let matches = match (&offer.action.action_type, &delta[0]) {
                (ActionType::NewBid(value), SeatEvent::Bid { seat, value: accepted }) => {
                    *seat == view.seat && value == accepted
                }
                (ActionType::CardPlayed(card), SeatEvent::CardPlayed { seat, card: played }) => {
                    *seat == view.seat && card.index() == *played
                }
                (ActionType::Question(engine::QuestionType::Yours), SeatEvent::AskPair { seat }) => *seat == view.seat,
                (ActionType::Question(engine::QuestionType::YourHalf(s)), SeatEvent::AskHalf { seat, suit }) => {
                    *seat == view.seat && s == suit
                }
                (ActionType::AnnounceTrump(s), SeatEvent::AnnounceTrump { seat, suit }) => {
                    *seat == view.seat && s == suit
                }
                (ActionType::Answer(engine::AnswerType::YesPair(s)), SeatEvent::YesPair { seat, suit }) => {
                    *seat == view.seat && s == suit
                }
                (ActionType::Answer(engine::AnswerType::NoPair), SeatEvent::NoPair { seat }) => *seat == view.seat,
                (ActionType::Answer(engine::AnswerType::YesHalf(s)), SeatEvent::YesHalf { seat, suit, .. }) => {
                    *seat == view.seat && s == suit
                }
                (ActionType::Answer(engine::AnswerType::NoHalf(s)), SeatEvent::NoHalf { seat, suit }) => {
                    *seat == view.seat && s == suit
                }
                _ => false,
            };
            if !matches {
                return Err(SequenceError::HistoryChanged);
            }
            self.stage = offer.next;
            self.pending = None;
        }
        self.accepted = Information::of(view);
        Ok(())
    }
}

/// Bounded authored action families. Binding uses only root's cards and legal
/// actions. No continuation is selected here, and no preservation bonus exists.
/// Every Choice retains a stop/legacy-tail alternative. Omitted families remain
/// a coverage limit; draw repetitions are unrolled at three root leads maximum.
pub fn authored_programs(view: &SeatView<'_>, legal: &[GameAction]) -> Vec<Program> {
    let own = view.knowledge.certain(view.seat);
    let tail = Stage(0);
    let mut programs = vec![Program {
        name: "legacy-tail",
        version: 1,
        start: tail,
        nodes: vec![Node::Tail],
    }];
    let action = |nodes: &mut Vec<Node>, actions, next| {
        let stage = Stage(nodes.len());
        nodes.push(Node::Action { actions, next });
        stage
    };
    let guard = |nodes: &mut Vec<Node>, predicate, yes| {
        let stage = Stage(nodes.len());
        nodes.push(Node::Guard {
            predicate,
            yes,
            no: tail,
            unknown: tail,
        });
        stage
    };
    let choice = |nodes: &mut Vec<Node>, alternatives| {
        let stage = Stage(nodes.len());
        nodes.push(Node::Choice(alternatives));
        stage
    };
    // Non-card roots (announcements/questions/answers) retain direct alternatives.
    for first in legal
        .iter()
        .filter(|a| !matches!(a.action_type, ActionType::UndoRequest))
    {
        if matches!(
            first.action_type,
            ActionType::CardPlayed(_) | ActionType::Question(_) | ActionType::AnnounceTrump(_) | ActionType::Answer(_)
        ) {
            programs.push(Program {
                name: "legal-action-tail",
                version: 1,
                start: Stage(1),
                nodes: vec![
                    Node::Tail,
                    Node::Action {
                        actions: Actions::Exact(first.action_type),
                        next: tail,
                    },
                ],
            });
        }
    }
    // Optional first-play raising is one action family, not a static product
    // of bids and routes. With one reconsideration, opening routes bind AFTER
    // the accepted bid, using the engine's new contract and rights. No-raise
    // card/activation routes remain in the same root catalogue.
    if legal.iter().any(|a| matches!(a.action_type, ActionType::NewBid(_)))
        && legal.iter().any(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
    {
        programs.push(Program {
            name: "optional-raise-then-opening",
            version: 1,
            start: Stage(1),
            nodes: vec![
                Node::Tail,
                Node::Action {
                    actions: Actions::Raises,
                    next: tail,
                },
            ],
        });
    }
    if own.is_empty() {
        return programs;
    }
    // Known team-winning lead -> observed root lead -> legal activation -> one
    // bound side lead. The side lead is a candidate, not a proven entry. The
    // partner's cooperative action remains the actor-local response model.
    for suit in SUITS.into_iter().filter(|&s| !view.knowledge.is_called(s)) {
        let mate = engine::partner(view.seat);
        let halves = CardSet::halves(suit);
        let mine = own & halves;
        let split = mine.len() == 1 && (halves - mine).is_subset(view.knowledge.certain(mate));
        if !(halves.is_subset(own) || halves.is_subset(view.knowledge.may_hold(mate)) || split) {
            continue;
        }
        for side in (own - CardSet::suit(suit)).iter() {
            let mut nodes = vec![Node::Tail];
            let side_action = action(
                &mut nodes,
                Actions::Exact(ActionType::CardPlayed(engine::Card::from_index(side))),
                tail,
            );
            let side_held = guard(&mut nodes, Guard::OwnCards(CardSet::single(side)), side_action);
            let side_lead = guard(&mut nodes, Guard::OwnLead, side_held);
            // Success executes the bound side lead; wrong/failed activation
            // retains all branch mass and falls back to the named tail.
            let activated = guard(&mut nodes, Guard::Trump(suit), side_lead);
            let ask = action(
                &mut nodes,
                Actions::Exact(ActionType::Question(engine::QuestionType::Yours)),
                activated,
            );
            let announce = action(&mut nodes, Actions::Exact(ActionType::AnnounceTrump(suit)), activated);
            let mut activation = vec![tail, ask, announce];
            if split {
                let ask_half = action(
                    &mut nodes,
                    Actions::Exact(ActionType::Question(engine::QuestionType::YourHalf(suit))),
                    activated,
                );
                let half_retained = guard(&mut nodes, Guard::OwnCards(mine), ask_half);
                activation.push(half_retained);
            }
            let activate_or_stop = choice(&mut nodes, activation.clone());
            let retain_lead = guard(&mut nodes, Guard::OwnLead, activate_or_stop);
            let lead_first = action(&mut nodes, Actions::Standing(own - halves), retain_lead);
            activation.push(lead_first);
            let start = choice(&mut nodes, activation);
            programs.push(Program {
                name: "standing-lead-activation-bound-side",
                version: 1,
                start,
                nodes,
            });
        }
    }
    if let Some(trump) = view.knowledge.trump() {
        let trumps = own & CardSet::suit(trump);
        let sides = own - trumps;
        if !trumps.is_empty() && !sides.is_empty() {
            for side in sides.iter() {
                let mut nodes = vec![Node::Tail];
                let side_lead = action(
                    &mut nodes,
                    Actions::Exact(ActionType::CardPlayed(engine::Card::from_index(side))),
                    tail,
                );
                let mut next = choice(&mut nodes, vec![tail, side_lead]);
                for _ in 0..3 {
                    let retain = guard(&mut nodes, Guard::OwnLead, next);
                    let draw = action(&mut nodes, Actions::Standing(trumps), retain);
                    let draws_or_stop = choice(&mut nodes, vec![tail, side_lead, draw]);
                    let gone = beliefs::ConceptQuery {
                        event: engine::opponents(view.seat)
                            .into_iter()
                            .map(|seat| {
                                beliefs::Concept::Count {
                                    seat,
                                    cards: CardSet::suit(trump),
                                    min: 0,
                                    max: 0,
                                }
                                .into()
                            })
                            .collect(),
                        given: vec![],
                    };
                    let stage = Stage(nodes.len());
                    nodes.push(Node::Guard {
                        predicate: Guard::Known(gone),
                        yes: next,
                        no: draws_or_stop,
                        unknown: draws_or_stop,
                    });
                    next = guard(&mut nodes, Guard::OwnLead, stage);
                }
                programs.push(Program {
                    name: "standing-trump-draw-then-side-lead",
                    version: 1,
                    start: next,
                    nodes,
                });
            }
        }
    }
    programs
}

#[cfg(test)]
#[path = "../../tests/internal/sequence_semantics.rs"]
mod tests;

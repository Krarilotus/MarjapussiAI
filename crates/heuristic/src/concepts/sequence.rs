//! Authored finite continuation data. No node chooses a tactic in a callback.
//! All actions use engine legality and all guards read only the acting view.

use agent::{SeatKind, SeatView, SeriesScore};
use beliefs::{ConceptBudget, ConceptQuery, Mind};
use engine::{ActionType, CardSet, GameAction, PlayPhase, Suit, SUITS};
use knowledge::Knowledge;
use observation::SeatEvent;

pub const MAX_STAGES: usize = 128;
pub const MAX_ALTERNATIVES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stage(pub usize);

/// Binding enumerates legal members; it never ranks or picks a tactic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Actions {
    Exact(ActionType),
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
                        Actions::Cards(cards) | Actions::Standing(cards) => {
                            !cards.is_empty() && cards.0 & !CardSet::ALL.0 == 0
                        }
                        Actions::Exact(kind) => matches!(
                            kind,
                            ActionType::CardPlayed(_)
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
                    Actions::Cards(cards) => matches!(action.action_type, ActionType::CardPlayed(card) if cards.contains(card.index())),
                    Actions::Standing(cards) => matches!(action.action_type, ActionType::CardPlayed(card) if (*cards & standing).contains(card.index())),
                }).map(|action| Offer { action: *action, next: *next }).collect();
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

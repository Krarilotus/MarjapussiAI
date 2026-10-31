//! Bounded joint holding queries shared by policy terms.
//! Unresolved probabilities leave the existing estimate in charge; a visited
//! fraction is never renormalized and presented as a complete calculation.

use beliefs::{ConceptBudget, ConceptQuery};
use engine::{CardSet, FastState, Move, PlayPhase};
use std::collections::HashMap;

use crate::Ctx;

/// Authored activation choices, not arbitrary evaluator identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ActivationStep {
    Own,
    Ask,
    Wait,
}

/// Actor activation opportunities fixed by the plan; default activation follows.
pub(crate) const CONTINUATION_STEPS: usize = 2;

/// The selecting actor's private continuation; it never directs its partner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ContinuationPlan {
    pub actor: u8,
    pub steps: [ActivationStep; CONTINUATION_STEPS],
    pub next: usize,
}

/// One concrete move and authored continuation in the decision's fixed world pool.
/// Exact hands identify the world without pointer lifetimes or hash-only equality.
/// Weight is deliberately absent: it affects integration, not this world's score.
///
/// The enclosing fresh Ctx fixes observer information, transfers, public state,
/// contract and model parameters, and owns one lazy immutable Routes pool. These
/// queries MUST NOT be moved to another Ctx or retained across decision epochs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ContinuationQuery {
    pub world: [CardSet; 4],
    pub phase: PlayPhase,
    pub action: Move,
    pub plan: ContinuationPlan,
}

/// Exact engine score difference conditional on a completed modeled trajectory.
/// Its weighted expectation is an estimate, never a proof of playing strength.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TerminalScoreDifference(i32);

impl TerminalScoreDifference {
    pub fn from_terminal(state: &FastState, contract: Option<(u8, i32)>, us: u8) -> Option<Self> {
        if !state.is_over() {
            return None;
        }
        let outcome = engine::GameOutcome {
            playing_team: contract.map(|c| c.0),
            game_value: contract.map_or(0, |c| c.1),
            team_points: [state.points(0), state.points(1)],
            team_tricks: [state.tricks_won(0), state.tricks_won(1)],
        };
        let scores = engine::game_scores(&outcome, engine::SCHWARZ_FACTOR_FIFTHS);
        Some(Self(scores[us as usize] - scores[1 - us as usize]))
    }

    pub fn value(self) -> i32 {
        self.0
    }
}

/// Decision-local results. Neither this store nor its continuation keys are an
/// agent-persistent cache; Ctx and its Routes pool are reconstructed together.
pub(crate) struct Reads {
    remaining: usize,
    results: Vec<(ConceptQuery, Option<f32>)>,
    continuations: HashMap<ContinuationQuery, TerminalScoreDifference>,
    #[cfg(test)]
    continuation_hits: std::cell::Cell<usize>,
}

impl Default for Reads {
    fn default() -> Self {
        Self {
            remaining: 2048,
            results: vec![],
            continuations: HashMap::new(),
            #[cfg(test)]
            continuation_hits: std::cell::Cell::new(0),
        }
    }
}

/// Marks a stored key as a consumed plan prefix: `plan.next = CONSUMED + used`, apart from
/// the unconsumed `next` values `0..=CONTINUATION_STEPS` of exact queries.
const CONSUMED: usize = CONTINUATION_STEPS + 1;

impl ContinuationQuery {
    /// The key of a trajectory from the plan's first step that consumed `used` steps: those
    /// decided every activation, later ones never came up. A query starting mid-plan stays exact.
    fn consumed(mut self, used: usize) -> Self {
        if self.plan.next == 0 {
            self.plan.steps[used..].fill(ActivationStep::Wait);
            self.plan.next = CONSUMED + used;
        }
        self
    }
}

impl Reads {
    /// A completed continuation for `query`: any stored trajectory whose consumed plan steps
    /// agree with the query's. A playout that used only its first steps is the same for every
    /// plan sharing them, so it is played once per distinct consumed prefix.
    pub fn continuation_score(&self, query: &ContinuationQuery) -> Option<TerminalScoreDifference> {
        let score = (0..=CONTINUATION_STEPS).find_map(|used| self.continuations.get(&query.consumed(used)).copied());
        #[cfg(test)]
        if score.is_some() {
            self.continuation_hits.set(self.continuation_hits.get() + 1);
        }
        score
    }

    /// Stores a trajectory that consumed the first `used` steps of `query`'s plan.
    pub fn completed_continuation(&mut self, query: ContinuationQuery, used: usize, score: TerminalScoreDifference) {
        self.continuations.insert(query.consumed(used), score);
    }
}

#[cfg(test)]
impl Reads {
    pub(crate) fn continuation_counts(&self) -> (usize, usize) {
        (self.continuations.len(), self.continuation_hits.get())
    }

    pub(crate) fn cap_for_test(&mut self, remaining: usize) {
        self.remaining = self.remaining.min(remaining);
    }

    pub(crate) fn snapshot(&self) -> (usize, Vec<(ConceptQuery, Option<u32>)>) {
        (
            self.remaining,
            self.results.iter().map(|(q, p)| (q.clone(), p.map(f32::to_bits))).collect(),
        )
    }
}

impl Ctx<'_> {
    pub(crate) fn joint_probability(&self, query: &ConceptQuery) -> Option<f32> {
        let _timing = beliefs::profile::scope("concept.joint");
        let mut reads = self.concepts.borrow_mut();
        if let Some((_, result)) = reads.results.iter().find(|(q, _)| q == query) {
            return *result;
        }
        let allowance = reads.remaining.min(512);
        let estimate = self.mind.concept(query, ConceptBudget { max_nodes: allowance });
        let result = match estimate {
            Ok(estimate) => {
                reads.remaining = reads.remaining.saturating_sub(estimate.nodes);
                if estimate.complete {
                    estimate.bounds.map(|b| b.lower as f32)
                } else {
                    None
                }
            }
            Err(_) => {
                // Errors do not report work already spent. Charge the full
                // requested allowance rather than silently replenishing it.
                reads.remaining = reads.remaining.saturating_sub(allowance);
                None
            }
        };
        // A decision has a small finite card/query vocabulary; this also bounds
        // repeated queries after the shared traversal allowance is exhausted.
        if reads.results.len() < 128 {
            reads.results.push((query.clone(), result));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use beliefs::{Concept, Mind};
    use engine::CardSet;
    use std::cell::RefCell;

    #[test]
    fn errors_consume_allowance_and_repeated_reads_do_not_repeat_them() {
        let hand = (0..9).fold(CardSet::EMPTY, |h, c| h | CardSet::single(c));
        let params = crate::Params::default();
        let mind = Mind::new(0, hand, [crate::SeatKind::Unknown; 4], params.beliefs.clone());
        let ctx = Ctx {
            me: 0,
            k: mind.knowledge(),
            mind: &mind,
            params: &params,
            solver: &RefCell::new(None),
            seed: 1,
            deadline: None,
            deadline_hit: Default::default(),
            series: Default::default(),
            class: Default::default(),
            routes: RefCell::new(None),
            concepts: RefCell::new(Reads {
                remaining: 700,
                ..Reads::default()
            }),
        };
        let invalid = ConceptQuery::new(Concept::Card { seat: 4, card: 0 });
        assert_eq!(ctx.joint_probability(&invalid), None);
        assert_eq!(ctx.concepts.borrow().remaining, 188);
        assert_eq!(ctx.joint_probability(&invalid), None);
        assert_eq!(ctx.concepts.borrow().remaining, 188);
        assert_eq!(
            ctx.joint_probability(&ConceptQuery::new(Concept::Card { seat: 4, card: 1 })),
            None
        );
        assert_eq!(ctx.concepts.borrow().remaining, 0);
        let known = ConceptQuery::new(Concept::Card { seat: 0, card: 0 });
        assert_eq!(
            ctx.joint_probability(&known),
            Some(1.0),
            "facts need no remaining traversal allowance"
        );
    }

    #[test]
    fn continuation_identity_separates_world_phase_action_and_actor_plan() {
        let mut state = FastState::new(
            std::array::from_fn(|seat| CardSet::single(seat as u8)),
            0,
        );
        assert!(TerminalScoreDifference::from_terminal(&state, Some((0, 120)), 0).is_none());
        let query = ContinuationQuery {
            world: std::array::from_fn(|seat| state.hand(seat as u8)),
            phase: PlayPhase::Cards,
            action: Move::Card(0),
            plan: ContinuationPlan {
                actor: 0,
                steps: [ActivationStep::Own, ActivationStep::Wait],
                next: 0,
            },
        };
        while !state.is_over() {
            state.play(state.legal_cards().iter().next().unwrap());
        }
        let score = TerminalScoreDifference::from_terminal(&state, Some((0, 120)), 0).unwrap();
        let mut reads = Reads::default();
        reads.completed_continuation(query, CONTINUATION_STEPS, score);
        assert_eq!(reads.continuation_score(&query), Some(score));
        assert_eq!(Reads::default().continuation_score(&query), None);
        let mut variants = [query; 6];
        variants[0].world.swap(1, 3);
        variants[1].phase = PlayPhase::Start;
        variants[2].action = Move::Card(1);
        variants[3].plan.actor = 2;
        variants[4].plan.steps[1] = ActivationStep::Ask;
        // A query starting mid-plan keys on its exact plan.
        variants[5].plan.next = 1;
        for other in variants {
            assert_eq!(reads.continuation_score(&other), None);
        }
    }

    #[test]
    fn a_trajectory_answers_every_plan_sharing_its_consumed_steps() {
        let mut state = FastState::new(std::array::from_fn(|seat| CardSet::single(seat as u8)), 0);
        let plan = |steps| ContinuationPlan { actor: 0, steps, next: 0 };
        let query = |steps| ContinuationQuery {
            world: std::array::from_fn(|seat| state.hand(seat as u8)),
            phase: PlayPhase::Cards,
            action: Move::Card(0),
            plan: plan(steps),
        };
        let (own_wait, own_ask, ask_own) = (
            query([ActivationStep::Own, ActivationStep::Wait]),
            query([ActivationStep::Own, ActivationStep::Ask]),
            query([ActivationStep::Ask, ActivationStep::Own]),
        );
        while !state.is_over() {
            state.play(state.legal_cards().iter().next().unwrap());
        }
        let score = TerminalScoreDifference::from_terminal(&state, Some((0, 120)), 0).unwrap();
        let mut reads = Reads::default();
        // The trajectory consumed only the first step (Own): the second never came up.
        reads.completed_continuation(own_wait, 1, score);
        assert_eq!(reads.continuation_score(&own_ask), Some(score));
        assert_eq!(reads.continuation_score(&ask_own), None);
        // With no step consumed, every plan shares the trajectory.
        let mut none = Reads::default();
        none.completed_continuation(own_wait, 0, score);
        assert_eq!(none.continuation_score(&ask_own), Some(score));
    }
}

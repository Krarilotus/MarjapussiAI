//! Heuristic Player v1 (HP1): a rule-based, explainable benchmark agent (layer 5, policy).
//! Spec: `docs/design/heuristic-player.md`.
//!
//! HP1 decides from one seat's view only: its `beliefs::Mind`, built from the seat's start hand
//! and its own `SeatEvent`s (the facts K1–K6 on `Knowledge`, and the believed forms), and the
//! legal actions. It never sees the full game state of the engine. Every decision names the
//! rule (`rules.rs`) that produced it and the facts that rule used.

mod bidding;
mod concepts;
mod endgame;
mod hand;
mod leads;
mod params;
pub mod passing;
mod planning;
mod play;
mod raising;
mod response;
mod routes;
mod rules;
mod stakes;
mod trump_plan;
mod valuation;

pub use agent::{Agent, Explanation, Fact, SeatKind, SeatView, SeriesScore};
use std::cell::{Cell, RefCell};

use beliefs::Mind;
use engine::{ActionType, CardSet, GameAction};
use knowledge::Knowledge;
use observation::{Seat, SeatEvent};

pub use concepts::sequence as sequence_concepts;
pub use concepts::value as value_concepts;
pub use params::{Params, Selection};
pub use planning::retained::{PlanMode, SequenceTrace};
pub use planning::sequence as sequence_evaluation;
pub use planning::{BeliefScope, PlanningAssessment, PlanningContext};
pub use response::{CompletedResponse, ResponseFailure, ResponseFailureReason};
pub use rules::{Phase, Rule, RuleId, RULES};

pub(crate) fn fact(key: &'static str, value: impl Into<f64>) -> Fact {
    Fact {
        key,
        value: value.into() as f32,
    }
}

/// HP1's decision with its typed rule; `Agent::act` turns it into the generic `agent::Decision`.
#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub action: GameAction,
    pub rule: RuleId,
    pub why: Vec<Fact>,
}

impl From<Decision> for agent::Decision {
    fn from(d: Decision) -> agent::Decision {
        agent::Decision {
            action: d.action,
            explanation: Explanation {
                label: d.rule.code(),
                facts: d.why,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum HeuristicError {
    Params(String),
}

impl std::fmt::Display for HeuristicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HeuristicError::Params(msg) => write!(f, "params: {msg}"),
        }
    }
}

impl std::error::Error for HeuristicError {}

/// Everything a rule module needs for one decision.
pub(crate) struct Ctx<'a> {
    pub me: Seat,
    /// The facts (K1–K6): `mind.knowledge()`.
    pub k: &'a Knowledge,
    /// The believed forms and the signal record (`beliefs`).
    pub mind: &'a Mind,
    pub params: &'a Params,
    /// The endgame solver, kept across decisions (its table is reused).
    pub solver: &'a RefCell<Option<solver::Solver>>,
    /// Seed for `Params.sample_temperature > 0` (deterministic per seat and event count).
    pub seed: u64,
    /// A caller-owned soft deadline, propagated to bounded primitive searches.
    pub deadline: Option<std::time::Instant>,
    /// Received history including undo markers; semantic queries use Mind::events().
    pub raw_events: &'a [observation::SeatEvent],
    pub routes: RefCell<Option<routes::Routes>>,
    pub concepts: RefCell<concepts::Reads>,
    pub intent: &'a RefCell<Option<planning::retained::RetainedPlan>>,
    pub completed: &'a RefCell<planning::sequence::CompletedCache>,
    pub formation_attempted: &'a Cell<bool>,
    /// A continuation response may publish only a completed primitive result.
    /// Actual play keeps its existing bounded fallback policy.
    pub response_completion: Option<&'a response::ResponseState>,
    pub series: SeriesScore,
}

impl Ctx<'_> {
    /// Argmax, or a softmax sample at `sample_temperature` (C10/C11), over `scores` (higher is better).
    pub fn select(&self, scores: &[f32]) -> usize {
        let best = (0..scores.len())
            .max_by(|&a, &b| scores[a].total_cmp(&scores[b]).then(b.cmp(&a)))
            .unwrap_or(0);
        let Selection::Sample { temperature: t } = self.params.selection() else {
            return best;
        };
        if scores.len() < 2 {
            return best;
        }
        let max = scores[best];
        let weights: Vec<f32> = scores.iter().map(|&s| ((s - max) / t).exp()).collect();
        let total: f32 = weights.iter().sum();
        let mut rng = Rng::new(self.seed);
        let mut u = (rng.next() >> 40) as f32 / (1u64 << 24) as f32 * total;
        for (i, w) in weights.iter().enumerate() {
            if u < *w {
                return i;
            }
            u -= w;
        }
        best
    }
}

/// Deterministic SplitMix64 for sampled choices (`Ctx::select`) and the endgame's worlds.
pub(crate) struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

impl rand::RngCore for Rng {
    fn next_u32(&mut self) -> u32 {
        (Rng::next(self) >> 32) as u32
    }

    fn next_u64(&mut self) -> u64 {
        Rng::next(self)
    }

    fn fill_bytes(&mut self, dst: &mut [u8]) {
        for chunk in dst.chunks_mut(8) {
            chunk.copy_from_slice(&Rng::next(self).to_le_bytes()[..chunk.len()]);
        }
    }
}

/// The `Mind` of the seat HP1 plays, and how far it has read the view's events.
struct Synced {
    start: CardSet,
    /// Raw input prefix, including undo events (unlike `Mind::events`).
    history: Vec<SeatEvent>,
    mind: Mind,
}

impl Synced {
    fn continues(&self, params: &Params, view: &SeatView<'_>) -> bool {
        self.mind.seat() == view.seat
            && self.start == view.knowledge.dealt_certain(view.seat)
            && self.mind.kinds() == view.kinds
            && self.mind.params() == &params.beliefs
            && view.events.starts_with(&self.history)
    }
}

/// Transaction retry state, distinct from a strategic plan step. A fallback
/// can be retried without advancing or certifying the retained program.
#[derive(Clone)]
struct PendingDecision {
    information: concepts::sequence::Information,
    legal: Vec<GameAction>,
    parameters: Params,
    decision: Decision,
    assessment: Option<std::sync::Arc<PlanningAssessment>>,
    sequence: Option<std::sync::Arc<SequenceTrace>>,
}
impl PendingDecision {
    fn matches(&self, view: &SeatView<'_>, legal: &[GameAction], parameters: &Params) -> bool {
        self.information.matches(view) && self.legal == legal && &self.parameters == parameters
    }
}

/// HP1.
#[derive(Default)]
pub struct HeuristicAgent {
    pub params: Params,
    mind: Option<Synced>,
    solver: RefCell<Option<solver::Solver>>,
    assessment: Option<std::sync::Arc<PlanningAssessment>>,
    sequence: Option<std::sync::Arc<SequenceTrace>>,
    pending_decision: Option<PendingDecision>,
    intent: RefCell<Option<planning::retained::RetainedPlan>>,
    completed: RefCell<planning::sequence::CompletedCache>,
    formation_attempted: Cell<bool>,
}

impl Clone for HeuristicAgent {
    /// Preserve private semantic intent across an explicit policy fork. The
    /// clone reconstructs its own Mind and disposable solver scratch. A different
    /// seat/history/model invalidates the copied intent before it can be used.
    fn clone(&self) -> Self {
        let mut cloned = HeuristicAgent::new(self.params.clone());
        cloned.intent = RefCell::new(self.intent.borrow().clone());
        cloned.pending_decision = self.pending_decision.clone();
        cloned.formation_attempted = self.formation_attempted.clone();
        cloned
    }
}

impl std::fmt::Debug for HeuristicAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeuristicAgent")
            .field("params", &self.params)
            .finish_non_exhaustive()
    }
}

impl HeuristicAgent {
    /// Construct from trusted parameters; use `Params::from_toml` for external
    /// configuration, or `Params::validate` after setting fields directly.
    pub fn new(params: Params) -> Self {
        HeuristicAgent {
            params,
            mind: None,
            solver: RefCell::new(None),
            assessment: None,
            sequence: None,
            pending_decision: None,
            intent: RefCell::new(None),
            completed: RefCell::default(),
            formation_attempted: Cell::new(false),
        }
    }

    /// My `Mind` for this view: the seat's start hand and its own events, read on from where
    /// the last view stopped. Changed input or belief parameters start a fresh one.
    pub fn mind(&mut self, view: &SeatView) -> &Mind {
        sync(&mut self.mind, &self.params, view)
    }

    /// The completed forecast assessment used by the last decision, if any.
    /// Uses the original decision context, not hindsight or another evaluation.
    pub fn last_sequence(&self) -> Option<&SequenceTrace> {
        self.sequence.as_deref()
    }

    pub fn last_assessment(&self) -> Option<&PlanningAssessment> {
        self.assessment.as_deref()
    }
}

fn sync<'a>(slot: &'a mut Option<Synced>, params: &Params, view: &SeatView) -> &'a Mind {
    let _timing = beliefs::profile::scope("policy.sync");
    let start = view.knowledge.dealt_certain(view.seat);
    let continues = slot.as_ref().is_some_and(|m| m.continues(params, view));
    if !continues {
        *slot = Some(Synced {
            start,
            history: Vec::new(),
            mind: Mind::new(view.seat, start, view.kinds, params.beliefs.clone()),
        });
    }
    let m = slot.as_mut().expect("set above");
    let tail = &view.events[m.history.len()..];
    for e in tail {
        m.mind.apply(e).expect("a seat's own events are consistent");
    }
    m.history.extend_from_slice(tail);
    &m.mind
}

fn decision(action: &GameAction, rule: RuleId, why: Vec<Fact>) -> Decision {
    Decision {
        action: *action,
        rule,
        why,
    }
}

impl Agent for HeuristicAgent {
    fn act(&mut self, view: &SeatView, legal: &[GameAction]) -> agent::Decision {
        self.decide(view, legal).into()
    }

    fn kind(&self) -> SeatKind {
        SeatKind::Heuristic
    }
}

impl HeuristicAgent {
    /// The decision with its typed rule ID (`rules.rs`).
    pub fn decide(&mut self, view: &SeatView, legal: &[GameAction]) -> Decision {
        self.decide_scoped(view, legal, None, true, None)
    }

    pub(crate) fn decide_before(
        &mut self,
        view: &SeatView,
        legal: &[GameAction],
        deadline: Option<std::time::Instant>,
    ) -> Decision {
        self.decide_scoped(view, legal, deadline, false, None)
    }

    pub(crate) fn completed_response_before(
        &mut self,
        view: &SeatView,
        legal: &[GameAction],
        deadline: Option<std::time::Instant>,
    ) -> Option<Decision> {
        self.completed_response_with_effort_before(view, legal, deadline, None)
            .ok()
            .map(|completed| completed.decision)
    }

    /// A completed response plus canonical endgame search effort, or failure
    /// with the effort already consumed. The optional caller-owned node budget
    /// applies across eligible solver calls; no default cap is introduced.
    /// Other belief/valuation kernels are not counted by this node metric.
    /// This is a soft-deadline API, not a total CPU-work/preemption guarantee.
    /// Requires `plan_worlds == 0`, `return_worlds == 0` and no retained intent,
    /// as supplied by Forecast's private actors. Otherwise returns NonIsolated
    /// before touching evidence, diagnostics or intent; configuration is unchanged.
    pub fn completed_response_with_effort_before(
        &mut self,
        view: &SeatView,
        legal: &[GameAction],
        deadline: Option<std::time::Instant>,
        remaining_solver_nodes: Option<u64>,
    ) -> Result<CompletedResponse, ResponseFailure> {
        let retained_intent = self.intent.borrow().is_some();
        if self.params.plan_worlds != 0 || self.params.return_worlds != 0 || retained_intent {
            return Err(ResponseFailure {
                reason: ResponseFailureReason::NonIsolated {
                    plan_worlds: self.params.plan_worlds,
                    return_worlds: self.params.return_worlds,
                    retained_intent,
                },
                canonical_solver_nodes: 0,
            });
        }
        let state = response::ResponseState::new(remaining_solver_nodes);
        if deadline.is_some_and(|until| std::time::Instant::now() >= until) {
            state.fail(ResponseFailureReason::DeadlineElapsed);
            return Err(state.failure().expect("expired deadline recorded"));
        }
        let decision = self.decide_scoped(view, legal, deadline, false, Some(&state));
        state.finish(decision, deadline)
    }

    fn decide_scoped(
        &mut self,
        view: &SeatView,
        legal: &[GameAction],
        deadline: Option<std::time::Instant>,
        retain_retry: bool,
        response_completion: Option<&response::ResponseState>,
    ) -> Decision {
        let _timing = beliefs::profile::scope("policy.decide");
        let started = std::time::Instant::now();
        self.assessment = None;
        self.sequence = None;
        let legal: Vec<GameAction> = legal
            .iter()
            .filter(|a| a.action_type != ActionType::UndoRequest)
            .cloned()
            .collect();
        assert!(!legal.is_empty(), "no legal action for seat {}", view.seat);
        // Forced actions also accept the current observation prefix. Intent and
        // cached evidence must never lag merely because this turn has no choice.
        let extends_cache_scope = self.mind.as_ref().is_some_and(|reader| {
            reader.continues(&self.params, view) && !view.events[reader.history.len()..].contains(&SeatEvent::Undone)
        });
        if !extends_cache_scope {
            self.completed.get_mut().clear();
            // A clone may reconstruct its reader while preserving this private
            // transaction's planning allowance. Genuine forks reset it below.
            if self.mind.is_some() {
                self.formation_attempted.set(false);
            }
        }
        let mind = sync(&mut self.mind, &self.params, view);
        let canonical_view = SeatView {
            seat: view.seat,
            knowledge: mind.knowledge(),
            events: mind.events(),
            series: view.series,
            kinds: view.kinds,
        };
        let intent_reset = self
            .intent
            .get_mut()
            .as_mut()
            .and_then(|plan| plan.synchronize(view, &canonical_view, &self.params).err());
        if intent_reset.is_some() {
            // Rewind, rejected/forked history or changed configuration requires
            // explicit recovery. Previous values cannot justify today's move.
            *self.intent.get_mut() = None;
            self.completed.get_mut().clear();
        }
        if let Some(pending) = self
            .pending_decision
            .as_ref()
            .filter(|p| retain_retry && p.matches(view, &legal, &self.params))
        {
            self.assessment = pending.assessment.clone();
            self.sequence = pending.sequence.clone();
            return pending.decision.clone();
        }
        let seed = self.params.seed ^ (view.seat as u64) << 56 ^ view.events.len() as u64;
        // Include synchronization in the same ordinary-play allowance. Decide
        // the lane after detecting a new game/fork, so stale work does not turn
        // a new opening into a repair or grant a repeated opening allowance.
        let deadline = deadline.or_else(|| {
            (!view.knowledge.played().is_empty() || self.intent.borrow().is_some() || self.formation_attempted.get())
                .then(|| started + std::time::Duration::from_millis(50))
        });
        let ctx = Ctx {
            me: view.seat,
            k: mind.knowledge(),
            mind,
            params: &self.params,
            solver: &self.solver,
            seed,
            deadline,
            raw_events: view.events,
            routes: RefCell::new(None),
            concepts: RefCell::new(concepts::Reads::default()),
            intent: &self.intent,
            completed: &self.completed,
            formation_attempted: &self.formation_attempted,
            response_completion,
            series: view.series,
        };
        let mut decision = (|| {
            if let Some(a) = legal.iter().find(|a| a.action_type == ActionType::UndoDecline) {
                return decision(a, RuleId::HX2, vec![]);
            }
            if legal.len() == 1 {
                planning::retained::forced(&ctx, &legal);
                return decision(&legal[0], RuleId::HX1, vec![]);
            }
            if let Some(decision) = planning::retained::prepared(&ctx, &legal) {
                return decision;
            }
            let has = |f: fn(&ActionType) -> bool| legal.iter().any(|a| f(&a.action_type));
            if has(|t| matches!(t, ActionType::Pass(_))) {
                return passing::decide(&ctx, &legal);
            }
            if let Some(d) = raising::follow(&ctx, &legal) {
                return planning::choose(&ctx, &legal, d);
            }
            if has(|t| matches!(t, ActionType::NewBid(_))) {
                if has(|t| matches!(t, ActionType::CardPlayed(_))) {
                    let cards: Vec<GameAction> = legal
                        .iter()
                        .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
                        .cloned()
                        .collect();
                    let baseline = raising::decide(&ctx, &legal, &cards);
                    return planning::choose(&ctx, &legal, baseline);
                }
                let baseline = bidding::decide(&ctx, &legal);
                return planning::choose(&ctx, &legal, baseline);
            }
            if has(|t| matches!(t, ActionType::Answer(_))) {
                let baseline = play::answer(&ctx, &legal);
                return planning::choose(&ctx, &legal, baseline);
            }
            let baseline = play::decide(&ctx, &legal);
            planning::choose(&ctx, &legal, baseline)
        })();
        if intent_reset.is_some() {
            decision.why.push(fact("sequence_context_reset", 1));
        }
        self.assessment = ctx.concepts.borrow_mut().assessment.take().map(std::sync::Arc::new);
        self.sequence = ctx.concepts.borrow_mut().sequence.take().map(std::sync::Arc::new);
        if retain_retry {
            self.pending_decision = Some(PendingDecision {
                information: concepts::sequence::Information::of(view),
                legal,
                parameters: self.params.clone(),
                decision: decision.clone(),
                assessment: self.assessment.clone(),
                sequence: self.sequence.clone(),
            });
        }
        decision
    }
}

#[cfg(test)]
mod evidence_tests {
    use super::*;

    #[test]
    fn forced_decisions_synchronize_the_accepted_observation_prefix() {
        let hands = std::array::from_fn(|s| (0..9).map(|c| engine::Card::from_index((s * 9 + c) as u8)).collect());
        let mut table = agent::Table::new(&hands);
        for kind in [
            ActionType::NewBid(120),
            ActionType::StopBidding,
            ActionType::StopBidding,
            ActionType::StopBidding,
        ] {
            let action = *table
                .to_act()
                .unwrap()
                .1
                .iter()
                .find(|a| a.action_type == kind)
                .unwrap();
            table.apply(action);
        }
        let (seat, legal) = table.to_act().unwrap();
        let mut policy = HeuristicAgent::default();
        let view = table.view(seat);
        let selected = policy.decide(&view, &legal[..1]);
        assert_eq!(selected.rule, RuleId::HX1);
        let synced = policy.mind.as_ref().expect("forced decision synchronized evidence");
        assert_eq!(synced.history, view.events);
        assert_eq!(synced.mind.knowledge(), view.knowledge);
        assert!(policy.last_assessment().is_none());

        let action = legal[0];
        // Keep the live-policy synchronization assertion above; strict modeled
        // calls use an explicitly isolated clone, just like Forecast actors.
        let mut response = policy.clone();
        response.params.plan_worlds = 0;
        response.params.return_worlds = 0;
        let expired = std::time::Instant::now() - std::time::Duration::from_millis(1);
        assert!(response
            .completed_response_before(&view, &[action], Some(expired))
            .is_none());
        assert_eq!(
            response
                .completed_response_before(&view, &[action], None)
                .unwrap()
                .action,
            action,
            "an applicable completed forced response needs no solver approximation"
        );

        policy.formation_attempted.set(true);
        let mut cloned = policy.clone();
        cloned.decide(&view, &[action]);
        assert!(
            cloned.formation_attempted.get(),
            "cloning the same observation cannot grant a new formation"
        );
        table.apply(action);
        let (other_seat, other_legal) = table.to_act().unwrap();
        assert_ne!(other_seat, seat);
        cloned.decide(&table.view(other_seat), &other_legal[..1]);
        assert!(
            !cloned.formation_attempted.get(),
            "a genuine seat/context change starts a new planning scope"
        );
    }
}

#[cfg(test)]
#[path = "../tests/internal/sequence_policy.rs"]
mod sequence_policy_tests;

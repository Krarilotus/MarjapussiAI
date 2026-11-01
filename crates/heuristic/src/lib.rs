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
mod outlook;
mod legal_actions;
mod params;
pub mod diagnostics;
pub mod passing;
mod play;
mod raising;
mod routes;
mod rules;
mod series;
mod stakes;
mod trump_plan;
mod valuation;

pub use agent::{Agent, Explanation, Fact, SeatKind, SeatView, SeriesScore};
use std::cell::RefCell;

use beliefs::Mind;
use engine::{ActionType, CardSet, GameAction};
use knowledge::Knowledge;
use observation::{Seat, SeatEvent};

pub use params::{Params, Selection};
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

/// The owner's decision classes by their hard time limits (2026-10-31): each exchange step
/// under 5 s, each bid under 200 ms, each card play, question or answer under 100 ms. The
/// raise-or-lead choice after the exchange is both a bid and a card play: the stricter class.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DecisionClass {
    #[default]
    Move,
    Bid,
    Exchange,
}

impl DecisionClass {
    pub fn of(legal: &[GameAction]) -> Self {
        let has = |f: fn(&ActionType) -> bool| legal.iter().any(|a| f(&a.action_type));
        if has(|t| matches!(t, ActionType::Pass(_))) {
            DecisionClass::Exchange
        } else if has(|t| matches!(t, ActionType::CardPlayed(_))) {
            DecisionClass::Move
        } else if has(|t| matches!(t, ActionType::NewBid(_))) {
            DecisionClass::Bid
        } else {
            DecisionClass::Move
        }
    }

    /// The class's limit. Bounded evaluations stop at four fifths of it; the last fifth is left
    /// for the work after them.
    fn budget(self, params: &Params) -> std::time::Duration {
        let ms = match self {
            DecisionClass::Exchange => params.planning_budget_ms,
            DecisionClass::Bid => params.bidding_budget_ms,
            DecisionClass::Move => params.play_budget_ms,
        };
        std::time::Duration::from_millis(ms as u64 * 4 / 5)
    }

    /// The believed worlds the decision's routes sample with `tricks_left` tricks to play: more
    /// for an exchange step's limit, and for a move before the exact endgame's scope
    /// (`endgame_tricks`), whose routes get the time the search takes later.
    pub fn worlds(self, params: &Params, tricks_left: u32) -> usize {
        let worlds = match self {
            DecisionClass::Exchange => params.exchange_worlds,
            DecisionClass::Move if tricks_left > params.endgame_tricks => params.early_worlds,
            _ => params.outlook_worlds,
        };
        worlds as usize
    }

    /// The logical work limit of one route evaluation, in rollouts (believed worlds × options):
    /// a move's (`move_route_rollouts`; 0: none); a bid's and an exchange step's evaluations
    /// are bounded by their worlds and limits alone.
    pub fn route_rollouts(self, params: &Params) -> Option<usize> {
        match self {
            DecisionClass::Move if params.move_route_rollouts > 0 => Some(params.move_route_rollouts as usize),
            _ => None,
        }
    }
}

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
    /// A caller-owned soft deadline, propagated to bounded primitive searches; `None` without
    /// `Params::deadlines`.
    pub deadline: Option<std::time::Instant>,
    /// Some evaluation of this decision was cut short by the deadline (reported as the
    /// `deadline_hit` fact: the decision may then depend on the machine's load).
    pub deadline_hit: std::cell::Cell<bool>,
    /// What this game's outcomes are worth in the series (HS-1).
    pub series: series::SeriesStakes,
    /// The decision's class by the owner's time limits.
    pub class: DecisionClass,
    pub routes: RefCell<Option<routes::Routes>>,
    pub concepts: RefCell<concepts::Reads>,
}

impl Ctx<'_> {
    /// The tricks I still play (my hand's cards): what the endgame's scope and the routes' world
    /// pool are measured by.
    pub(crate) fn tricks_left(&self) -> u32 {
        self.k.hand_len(self.me) as u32
    }

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

/// HP1.
#[derive(Default)]
pub struct HeuristicAgent {
    pub params: Params,
    mind: Option<Synced>,
    solver: RefCell<Option<solver::Solver>>,
}

impl Clone for HeuristicAgent {
    /// The parameters only: the clone builds its own `Mind` from its first view.
    fn clone(&self) -> Self {
        HeuristicAgent::new(self.params.clone())
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
        series::prepare();
        HeuristicAgent {
            params,
            mind: None,
            solver: RefCell::new(None),
        }
    }

    /// My `Mind` for this view: the seat's start hand and its own events, read on from where
    /// the last view stopped. Changed input or belief parameters start a fresh one.
    pub fn mind(&mut self, view: &SeatView) -> &Mind {
        sync(&mut self.mind, &self.params, view)
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
        let _timing = beliefs::profile::scope("policy.decide");
        let legal = legal_actions::eligible(legal);
        assert!(!legal.is_empty(), "no legal action for seat {}", view.seat);
        if let Some(a) = legal.iter().find(|a| a.action_type == ActionType::UndoDecline) {
            return decision(a, RuleId::HX2, vec![]);
        }
        if legal.len() == 1 {
            sync(&mut self.mind, &self.params, view);
            return decision(&legal[0], RuleId::HX1, vec![]);
        }
        let seed = self.params.seed ^ (view.seat as u64) << 56 ^ view.events.len() as u64;
        let started = std::time::Instant::now();
        let mind = sync(&mut self.mind, &self.params, view);
        let class = DecisionClass::of(&legal);
        let ctx = Ctx {
            me: view.seat,
            k: mind.knowledge(),
            mind,
            params: &self.params,
            solver: &self.solver,
            seed,
            deadline: self.params.deadlines.then(|| started + class.budget(&self.params)),
            deadline_hit: Default::default(),
            series: series::SeriesStakes::new(view.series, view.seat),
            class,
            routes: RefCell::new(None),
            concepts: RefCell::new(concepts::Reads::default()),
        };
        let mut d = decide_in(&ctx, &legal);
        if ctx.deadline_hit.get() {
            d.why.push(fact("deadline_hit", 1.0));
        }
        d
    }
}

/// The decision of one non-forced position, by phase.
fn decide_in(ctx: &Ctx, legal: &[GameAction]) -> Decision {
    let has = |f: fn(&ActionType) -> bool| legal.iter().any(|a| f(&a.action_type));
    if ctx.class == DecisionClass::Exchange {
        return passing::decide(ctx, legal);
    }
    if let Some(d) = raising::follow(ctx, legal) {
        return d;
    }
    if has(|t| matches!(t, ActionType::NewBid(_))) {
        if has(|t| matches!(t, ActionType::CardPlayed(_))) {
            let cards: Vec<GameAction> = legal
                .iter()
                .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
                .cloned()
                .collect();
            return raising::decide(ctx, legal, &cards);
        }
        return bidding::decide(ctx, legal);
    }
    if has(|t| matches!(t, ActionType::Answer(_))) {
        return play::answer(ctx, legal);
    }
    play::decide(ctx, legal)
}

#[cfg(test)]
#[path = "../tests/internal/budget.rs"]
mod budget_tests;

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
mod legal_actions;
mod params;
pub mod passing;
mod play;
mod raising;
mod routes;
mod rules;
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
    pub routes: RefCell<Option<routes::Routes>>,
    pub concepts: RefCell<concepts::Reads>,
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
        let mind = sync(&mut self.mind, &self.params, view);
        let ctx = Ctx {
            me: view.seat,
            k: mind.knowledge(),
            mind,
            params: &self.params,
            solver: &self.solver,
            seed,
            deadline: None,
            routes: RefCell::new(None),
            concepts: RefCell::new(concepts::Reads::default()),
        };
        let has = |f: fn(&ActionType) -> bool| legal.iter().any(|a| f(&a.action_type));
        if has(|t| matches!(t, ActionType::Pass(_))) {
            return passing::decide(&ctx, &legal);
        }
        if let Some(d) = raising::follow(&ctx, &legal) {
            return d;
        }
        if has(|t| matches!(t, ActionType::NewBid(_))) {
            if has(|t| matches!(t, ActionType::CardPlayed(_))) {
                let cards: Vec<GameAction> = legal
                    .iter()
                    .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
                    .cloned()
                    .collect();
                return raising::decide(&ctx, &legal, &cards);
            }
            return bidding::decide(&ctx, &legal);
        }
        if has(|t| matches!(t, ActionType::Answer(_))) {
            return play::answer(&ctx, &legal);
        }
        play::decide(&ctx, &legal)
    }
}

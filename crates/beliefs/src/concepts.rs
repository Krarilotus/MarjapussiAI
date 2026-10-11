//! Joint conditional concepts. Hard proof and soft model mass are separate.

use engine::{CardSet, Suit, CARDS};
use knowledge::{Belief, Hands, Knowledge};
use observation::Seat;

use crate::Mind;

pub(crate) const CACHE_ENTRIES: usize = 32;
pub(crate) const MAX_NODES: usize = 1_000_000;
const MAX_LITERALS: usize = 16;

/// Predicates on current holdings, not intentions or future optimal play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Concept {
    Card {
        seat: Seat,
        card: u8,
    },
    Void {
        seat: Seat,
        suit: Suit,
    },
    Pair {
        seat: Seat,
        suit: Suit,
    },
    Count {
        seat: Seat,
        cards: CardSet,
        min: u8,
        max: u8,
    },
}

impl Concept {
    pub(crate) fn parts(self) -> (Seat, CardSet, u8, u8) {
        match self {
            Self::Card { seat, card } => (seat, CardSet::single(card), 1, 1),
            Self::Void { seat, suit } => (seat, CardSet::suit(suit), 0, 0),
            Self::Pair { seat, suit } => (seat, CardSet::halves(suit), 2, 2),
            Self::Count { seat, cards, min, max } => (seat, cards, min, max),
        }
    }

    fn validate(self) -> Result<(), ConceptError> {
        if matches!(self, Self::Card { card, .. } if card >= CARDS as u8) {
            return Err(ConceptError::InvalidConcept);
        }
        let (seat, cards, min, max) = self.parts();
        if seat >= 4 || min > max || max > CARDS as u8 || (cards.0 & !CardSet::ALL.0) != 0 {
            return Err(ConceptError::InvalidConcept);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Literal {
    pub concept: Concept,
    pub value: bool,
}

impl From<Concept> for Literal {
    fn from(concept: Concept) -> Self {
        Self { concept, value: true }
    }
}

/// P(all event literals | all given literals). Conditions are hypotheses; they
/// do not modify the Mind's facts or add the same bidding evidence a second time.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConceptQuery {
    pub event: Vec<Literal>,
    pub given: Vec<Literal>,
}

impl ConceptQuery {
    pub fn new(concept: Concept) -> Self {
        Self {
            event: vec![concept.into()],
            given: vec![],
        }
    }

    /// At most 32 input literals and 16 unique literals across both conjunctions.
    /// Equivalent typed predicates share a cache key; duplicates are removed.
    pub(crate) fn canonical(&self) -> Result<Self, ConceptError> {
        if self.event.len().saturating_add(self.given.len()) > 2 * MAX_LITERALS {
            return Err(ConceptError::QueryLimit);
        }
        for literal in self.event.iter().chain(&self.given) {
            literal.concept.validate()?;
        }
        let normalise = |literals: &[Literal]| {
            let mut literals: Vec<_> = literals
                .iter()
                .map(|literal| {
                    let (seat, cards, min, max) = literal.concept.parts();
                    Literal {
                        concept: Concept::Count { seat, cards, min, max },
                        value: literal.value,
                    }
                })
                .collect();
            literals.sort_by_key(|literal| {
                let (seat, cards, min, max) = literal.concept.parts();
                (seat, cards.0, min, max, literal.value)
            });
            literals.dedup();
            literals
        };
        let query = Self {
            event: normalise(&self.event),
            given: normalise(&self.given),
        };
        if query.event.len() + query.given.len() > MAX_LITERALS {
            return Err(ConceptError::QueryLimit);
        }
        Ok(query)
    }

    pub(crate) fn masks(&self) -> Vec<CardSet> {
        self.event
            .iter()
            .chain(&self.given)
            .map(|l| l.concept.parts().1)
            .collect()
    }
}

/// Deterministic traversal-node bound (0..=1,000,000), independent of scheduling.
/// Zero permits hard deductions only, without constructing a fact DP or model.
/// Positive budgets do not bound fact DP construction, exchange-kernel work,
/// memory or wall time. Query/cache limits bound retained data, not the fact DP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConceptBudget {
    pub max_nodes: usize,
}

impl Default for ConceptBudget {
    fn default() -> Self {
        Self { max_nodes: 100_000 }
    }
}

impl ConceptBudget {
    pub fn validate(self) -> Result<(), ConceptError> {
        if self.max_nodes > MAX_NODES {
            Err(ConceptError::InvalidBudget)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbabilityBounds {
    pub lower: f64,
    pub upper: f64,
}

/// Model probabilities are not calibration claims. `known` is unconditional
/// hard proof; `conditional_fact` is hard truth only under the supplied condition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConceptEstimate {
    pub bounds: Option<ProbabilityBounds>,
    pub known: Option<bool>,
    pub conditional_fact: Option<bool>,
    pub complete: bool,
    pub nodes: usize,
    pub classes: usize,
    pub covered_deals: u64,
    /// Not computed by hard-proof or unresolved zero-budget shortcuts (0).
    pub total_deals: u64,
    pub resolved_condition_weight: f64,
    /// Infinity when unresolved model mass has not been constructed or bounded.
    pub unresolved_weight_upper: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConceptError {
    InvalidConcept,
    QueryLimit,
    InvalidBudget,
    InvalidWeights,
    InconsistentHistory,
    UnsupportedPrivateHistory,
}

impl std::fmt::Display for ConceptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidConcept => "invalid concept seat, card or count range",
            Self::QueryLimit => "concept queries allow 32 input and 16 unique literals",
            Self::InvalidBudget => "concept budgets allow at most 1000000 nodes and 32 private hypotheses",
            Self::InvalidWeights => "nonfinite or negative model weight",
            Self::InconsistentHistory => "private hypothesis contradicts the observed history",
            Self::UnsupportedPrivateHistory => "unobserved private transfers cannot be reconstructed",
        })
    }
}

impl std::error::Error for ConceptError {}

pub(crate) struct ConceptStore {
    queries: Vec<(ConceptQuery, ConceptBudget, ConceptEstimate)>,
    pub perspectives: Vec<(Seat, ConceptQuery, crate::PerspectiveBudget, crate::PerspectiveEstimate)>,
    /// Same epoch, viewer, private starting hand, params/kinds and filtered history.
    /// Bounded independently of a caller's requested traversal budget.
    pub hypothesis_minds: Vec<(Seat, CardSet, Mind)>,
}

impl Mind {
    pub(crate) fn ensure_concepts(&self) {
        let key = (self.epochs(), self.public.fact_epoch());
        if self
            .cache
            .borrow()
            .concepts
            .as_ref()
            .is_some_and(|(e, p, _)| (*e, *p) == key)
        {
            return;
        }
        let store = ConceptStore {
            queries: vec![],
            perspectives: vec![],
            hypothesis_minds: vec![],
        };
        self.cache.borrow_mut().concepts = Some((key.0, key.1, store));
    }

    /// Joint current-holding probability under the existing DealWeights model.
    /// Exact summation when complete, conservative residual bounds otherwise.
    /// Cheap hard deductions do not construct or traverse a deal space.
    pub fn concept(&self, query: &ConceptQuery, budget: ConceptBudget) -> Result<ConceptEstimate, ConceptError> {
        budget.validate()?;
        let canonical = query.canonical()?;
        let query = &canonical;
        let known = hard_conjunction(self.knowledge(), &query.event);
        let condition = hard_conjunction(self.knowledge(), &query.given);
        if condition == Some(false) || (condition == Some(true) && known.is_some()) {
            return Ok(ConceptEstimate::proof(known, condition, !query.given.is_empty()));
        }
        if budget.max_nodes == 0 {
            return Ok(ConceptEstimate::unresolved(known));
        }
        let model = self.deal_model();
        self.ensure_concepts();
        let mut cache = self.cache.borrow_mut();
        let store = &mut cache.concepts.as_mut().expect("initialised").2;
        if let Some((_, _, result)) = store.queries.iter().find(|(q, b, _)| q == query && *b == budget) {
            return Ok(*result);
        }
        let labels = model.weights().allocation_labels(&query.masks());
        let ceiling = model.weights().weight_ceiling(!self.knowledge().played());
        if !ceiling.is_finite() || ceiling < 0.0 {
            return Err(ConceptError::InvalidWeights);
        }
        let (mut numerator, mut denominator) = (0.0, 0.0);
        let (mut condition_deals, mut event_deals) = (0, 0);
        let mut invalid = false;
        let visit = model
            .space()
            .visit_allocations(&labels, budget.max_nodes, |hands, ways| {
                if holds(&query.given, hands) {
                    condition_deals += ways;
                    let event = holds(&query.event, hands);
                    event_deals += if event { ways } else { 0 };
                    let w = model.weights().weight(hands) * ways as f64;
                    invalid |= !w.is_finite() || w < 0.0;
                    denominator += w;
                    numerator += if event { w } else { 0.0 };
                }
                invalid
            });
        if invalid || !denominator.is_finite() || !numerator.is_finite() {
            return Err(ConceptError::InvalidWeights);
        }
        let residual = visit.unresolved_deals() as f64 * ceiling;
        // This proof counts ALL hard-consistent deals, including zero-weight ones.
        let fact = if visit.complete() && condition_deals > 0 {
            if event_deals == 0 {
                Some(false)
            } else if event_deals == condition_deals {
                Some(true)
            } else {
                None
            }
        } else {
            None
        };
        let result = ConceptEstimate {
            bounds: probability_bounds(numerator, denominator, residual),
            known: if query.given.is_empty() { fact.or(known) } else { known },
            conditional_fact: if query.given.is_empty() { None } else { fact },
            complete: visit.complete(),
            nodes: visit.nodes,
            classes: visit.classes,
            covered_deals: visit.covered_deals,
            total_deals: visit.total_deals,
            resolved_condition_weight: denominator,
            unresolved_weight_upper: residual,
        };
        if store.queries.len() == CACHE_ENTRIES {
            store.queries.remove(0);
        }
        store.queries.push((query.clone(), budget, result));
        Ok(result)
    }
}

impl ConceptEstimate {
    fn unresolved(known: Option<bool>) -> Self {
        Self {
            bounds: Some(ProbabilityBounds { lower: 0.0, upper: 1.0 }),
            known,
            conditional_fact: None,
            complete: false,
            nodes: 0,
            classes: 0,
            covered_deals: 0,
            total_deals: 0,
            resolved_condition_weight: 0.0,
            unresolved_weight_upper: f64::INFINITY,
        }
    }

    fn proof(known: Option<bool>, condition: Option<bool>, conditional: bool) -> Self {
        Self {
            bounds: if condition == Some(false) {
                None
            } else {
                known.map(|v| ProbabilityBounds {
                    lower: f64::from(v),
                    upper: f64::from(v),
                })
            },
            known,
            conditional_fact: if conditional && condition == Some(true) {
                known
            } else {
                None
            },
            complete: true,
            nodes: 0,
            classes: 0,
            covered_deals: 0,
            total_deals: 0,
            resolved_condition_weight: 0.0,
            unresolved_weight_upper: 0.0,
        }
    }
}

pub(crate) fn probability_bounds(numerator: f64, denominator: f64, residual: f64) -> Option<ProbabilityBounds> {
    if denominator == 0.0 && residual == 0.0 {
        return None; // impossible/zero-model-mass condition, never P=0
    }
    let total = denominator + residual;
    if !total.is_finite() {
        return Some(ProbabilityBounds { lower: 0.0, upper: 1.0 });
    }
    Some(ProbabilityBounds {
        lower: (numerator / total).clamp(0.0, 1.0),
        upper: ((numerator + residual) / total).clamp(0.0, 1.0),
    })
}

pub(crate) fn holds(literals: &[Literal], hands: &Hands) -> bool {
    literals.iter().all(|l| {
        let (seat, mask, min, max) = l.concept.parts();
        let n = (hands[seat as usize] & mask).len() as u8;
        (min <= n && n <= max) == l.value
    })
}

pub(crate) fn hard_literal(k: &Knowledge, literal: &Literal) -> Option<bool> {
    let (seat, mask, min, max) = literal.concept.parts();
    let mut lo = (k.certain(seat) & mask).len() as u8;
    let mut hi = (k.may_hold(seat) & mask).len().min(k.hand_len(seat) as u32) as u8;
    lo = lo.max(k.hand_len(seat).saturating_sub((k.may_hold(seat) - mask).len() as u8));
    for constraint in k.constraints().iter().filter(|c| c.seat == seat) {
        if constraint.mask.is_subset(mask) {
            lo = lo.max(constraint.lo);
        }
        if mask.is_subset(constraint.mask) {
            hi = hi.min(constraint.hi);
        }
    }
    let truth = if hi < min || lo > max {
        Some(false)
    } else if min <= lo && hi <= max {
        Some(true)
    } else {
        None
    };
    truth.map(|v| v == literal.value)
}

pub(crate) fn hard_conjunction(k: &Knowledge, literals: &[Literal]) -> Option<bool> {
    if literals
        .windows(2)
        .any(|pair| pair[0].concept == pair[1].concept && pair[0].value != pair[1].value)
    {
        return Some(false);
    }
    let mut unknown = false;
    for literal in literals {
        match hard_literal(k, literal) {
            Some(false) => return Some(false),
            None => unknown = true,
            Some(true) => {}
        }
    }
    if unknown {
        None
    } else {
        Some(true)
    }
}

#[cfg(test)]
#[path = "../tests/internal/zero_budget.rs"]
mod zero_budget_tests;

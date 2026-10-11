//! What root proves a viewer knows, and root's expected viewer-model estimate.
//! Private hypotheses are integrated, never read from an actual hidden game.

use engine::CardSet;
use knowledge::{Belief, Knowledge, Passed, Tracker};
use observation::{Seat, SeatEvent};

use crate::concepts::{hard_conjunction, hard_literal, holds, probability_bounds, CACHE_ENTRIES, MAX_NODES};
use crate::{ConceptBudget, ConceptError, ConceptQuery, Mind, ProbabilityBounds};

/// Bounds outer subset/group traversal and at most 32 inner concept queries.
/// Fact/conditional DP construction and exchange-kernel computations are outside
/// these node counts. This is not a wall-clock or aggregate-memory guarantee.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PerspectiveBudget {
    pub outer_nodes: usize,
    pub inner: ConceptBudget,
    pub max_hypotheses: usize,
}

impl Default for PerspectiveBudget {
    fn default() -> Self {
        Self {
            outer_nodes: 10_000,
            inner: ConceptBudget { max_nodes: 10_000 },
            max_hypotheses: 32,
        }
    }
}

impl PerspectiveBudget {
    pub fn validate(self) -> Result<(), ConceptError> {
        self.inner.validate()?;
        if self.outer_nodes > MAX_NODES || self.max_hypotheses > CACHE_ENTRIES {
            Err(ConceptError::InvalidBudget)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerspectiveEstimate {
    /// Root proves the viewer knows this unconditional truth. All hard private
    /// hypotheses must support it, even if the model gives a hypothesis weight 0.
    pub known_by_viewer: Option<bool>,
    /// Proof under `query.given`, not observed viewer knowledge.
    pub conditional_fact: Option<bool>,
    /// E_root[P_viewer(event | given) | given], not a player's reported thought.
    pub expected_belief: Option<ProbabilityBounds>,
    pub complete: bool,
    pub outer_nodes: usize,
    pub inner_nodes: usize,
    pub hypotheses: usize,
    /// Root-model weight that may still contain unresolved outer or inner work.
    pub unresolved_weight_upper: f64,
}

#[derive(Clone)]
pub(crate) struct ViewerFacts {
    knowledge: Knowledge,
    events: Vec<SeatEvent>,
}

impl Mind {
    fn viewer_facts(&self, viewer: Seat) -> Result<ViewerFacts, ConceptError> {
        let key = self.epochs();
        if let Some((epoch, facts)) = &self.cache.borrow().perspective_facts[viewer as usize] {
            if *epoch == key {
                return Ok(facts.clone());
            }
        }
        let events: Vec<_> = self
            .events()
            .iter()
            .map(|e| match *e {
                SeatEvent::Pass { from, to, cards } => SeatEvent::Pass {
                    from,
                    to,
                    cards: if viewer == from || viewer == to { cards } else { None },
                },
                ref other => other.clone(),
            })
            .collect();
        let mut shared = Tracker::public();
        for event in &events {
            shared.apply(event).map_err(|_| ConceptError::InconsistentHistory)?;
        }
        let facts = ViewerFacts {
            knowledge: *shared.knowledge(),
            events,
        };
        self.cache.borrow_mut().perspective_facts[viewer as usize] = Some((key, facts.clone()));
        Ok(facts)
    }

    /// A viewer's conditional concept model, marginalized over root-consistent
    /// private hands. Only the viewer's own hand and filtered events reach its Mind.
    /// Unknown private transfer trajectories are explicitly unsupported; cheap
    /// public/shared proofs still work in those histories without enumeration.
    pub fn perspective_concept(
        &self,
        viewer: Seat,
        query: &ConceptQuery,
        budget: PerspectiveBudget,
    ) -> Result<PerspectiveEstimate, ConceptError> {
        budget.validate()?;
        let canonical = query.canonical()?;
        let query = &canonical;
        if viewer >= 4 {
            return Err(ConceptError::InvalidConcept);
        }
        if viewer == self.seat() {
            let result = self.concept(query, budget.inner)?;
            return Ok(PerspectiveEstimate {
                known_by_viewer: result.known,
                conditional_fact: result.conditional_fact,
                expected_belief: result.bounds,
                complete: result.complete,
                outer_nodes: 0,
                inner_nodes: result.nodes,
                hypotheses: 1,
                unresolved_weight_upper: result.unresolved_weight_upper,
            });
        }
        let facts = self.viewer_facts(viewer)?;
        let viewer_truth = |l: &crate::Literal| {
            if l.concept.parts().0 == viewer {
                // The viewer knows its own current hand, so a root hard proof
                // about it is also a proof of what that viewer knows.
                hard_literal(self.knowledge(), l)
            } else {
                hard_literal(&facts.knowledge, l)
            }
        };
        let cheap = conjunction(query.event.iter().map(viewer_truth));
        if hard_conjunction(self.knowledge(), &query.given) == Some(false) {
            return Ok(PerspectiveEstimate {
                known_by_viewer: cheap,
                conditional_fact: None,
                expected_belief: None,
                complete: true,
                outer_nodes: 0,
                inner_nodes: 0,
                hypotheses: 0,
                unresolved_weight_upper: 0.0,
            });
        }
        if conjunction(query.given.iter().map(viewer_truth)) == Some(true) {
            if let Some(truth) = cheap {
                return Ok(PerspectiveEstimate {
                    known_by_viewer: Some(truth),
                    conditional_fact: if query.given.is_empty() { None } else { Some(truth) },
                    expected_belief: Some(ProbabilityBounds {
                        lower: f64::from(truth),
                        upper: f64::from(truth),
                    }),
                    complete: true,
                    outer_nodes: 0,
                    inner_nodes: 0,
                    hypotheses: 0,
                    unresolved_weight_upper: 0.0,
                });
            }
        }
        if matches!(self.knowledge().given(viewer), Passed::Hidden)
            || matches!(self.knowledge().received(viewer), Passed::Hidden)
        {
            return Err(ConceptError::UnsupportedPrivateHistory);
        }
        self.ensure_concepts();
        let mut cache = self.cache.borrow_mut();
        let store = &mut cache.concepts.as_mut().expect("initialised").2;
        if let Some((_, _, _, result)) = store
            .perspectives
            .iter()
            .find(|(v, q, b, _)| *v == viewer && q == query && *b == budget)
        {
            return Ok(*result);
        }
        let labels = store.weights.allocation_labels(&query.masks());
        let ceiling = store.weights.weight_ceiling(!self.knowledge().played());
        if !ceiling.is_finite() || ceiling < 0.0 {
            return Err(ConceptError::InvalidWeights);
        }
        let mut hypotheses: Vec<(CardSet, crate::ConceptEstimate)> = Vec::new();
        let (mut lower_sum, mut upper_sum, mut mass) = (0.0, 0.0, 0.0);
        let mut inner_unresolved_weight = 0.0;
        let (mut all_true, mut all_false, mut inner_complete) = (true, true, true);
        let mut inner_nodes = 0;
        let mut error = None;
        let space = &store.space;
        let weights = &store.weights;
        let model_minds = &mut store.hypothesis_minds;
        let visit = space.visit_private_allocations(
            self.knowledge(),
            viewer,
            &labels,
            budget.outer_nodes,
            budget.max_hypotheses,
            |hands, ways| {
                if !holds(&query.given, hands) {
                    return false;
                }
                let Some(start) = self.knowledge().dealt_from_current(viewer, hands[viewer as usize]) else {
                    error = Some(ConceptError::InconsistentHistory);
                    return true;
                };
                let result = if let Some((_, result)) = hypotheses.iter().find(|(h, _)| *h == start) {
                    *result
                } else {
                    if start.len() != engine::HAND_SIZE as u32 {
                        error = Some(ConceptError::InconsistentHistory);
                        return true;
                    }
                    let model_index =
                        if let Some(i) = model_minds.iter().position(|(v, h, _)| *v == viewer && *h == start) {
                            i
                        } else {
                            let mut mind = Mind::new(viewer, start, self.kinds(), self.params().clone());
                            for event in &facts.events {
                                if mind.apply(event).is_err() {
                                    error = Some(ConceptError::InconsistentHistory);
                                    return true;
                                }
                            }
                            if model_minds.len() == CACHE_ENTRIES {
                                model_minds.remove(0);
                            }
                            model_minds.push((viewer, start, mind));
                            model_minds.len() - 1
                        };
                    let result = match model_minds[model_index].2.concept(query, budget.inner) {
                        Ok(result) => result,
                        Err(e) => {
                            error = Some(e);
                            return true;
                        }
                    };
                    inner_nodes += result.nodes;
                    hypotheses.push((start, result));
                    result
                };
                // Proof examines zero-model-weight hypotheses too.
                let truth = if query.given.is_empty() {
                    result.known
                } else {
                    result.conditional_fact.or(result.known)
                };
                all_true &= truth == Some(true);
                all_false &= truth == Some(false);
                inner_complete &= result.complete;
                let w = weights.weight(hands) * ways as f64;
                if !w.is_finite() || w < 0.0 {
                    error = Some(ConceptError::InvalidWeights);
                    return true;
                }
                let bounds = result.bounds.unwrap_or(ProbabilityBounds { lower: 0.0, upper: 1.0 });
                if result.bounds.is_none() && w > 0.0 {
                    inner_complete = false;
                }
                if !result.complete || result.bounds.is_none() {
                    inner_unresolved_weight += w;
                }
                lower_sum += w * bounds.lower;
                upper_sum += w * bounds.upper;
                mass += w;
                false
            },
        );
        if let Some(error) = error {
            return Err(error);
        }
        if !mass.is_finite() || !lower_sum.is_finite() || !upper_sum.is_finite() {
            return Err(ConceptError::InvalidWeights);
        }
        let residual = visit.unresolved_deals() as f64 * ceiling;
        let expected = probability_bounds(lower_sum, mass, residual)
            .zip(probability_bounds(upper_sum, mass, residual))
            .map(|(lo, hi)| ProbabilityBounds {
                lower: lo.lower,
                upper: hi.upper,
            });
        let proof = if visit.complete() && !hypotheses.is_empty() {
            if all_true {
                Some(true)
            } else if all_false {
                Some(false)
            } else {
                None
            }
        } else {
            None
        };
        let result = PerspectiveEstimate {
            known_by_viewer: if query.given.is_empty() { proof.or(cheap) } else { cheap },
            conditional_fact: if query.given.is_empty() { None } else { proof },
            expected_belief: expected,
            complete: visit.complete() && inner_complete,
            outer_nodes: visit.nodes,
            inner_nodes,
            hypotheses: hypotheses.len(),
            unresolved_weight_upper: residual + inner_unresolved_weight,
        };
        if store.perspectives.len() == CACHE_ENTRIES {
            store.perspectives.remove(0);
        }
        store.perspectives.push((viewer, query.clone(), budget, result));
        Ok(result)
    }
}

fn conjunction(values: impl Iterator<Item = Option<bool>>) -> Option<bool> {
    let mut unknown = false;
    for value in values {
        match value {
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

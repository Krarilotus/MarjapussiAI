//! Bounded joint holding queries shared by policy terms.
//! Unresolved probabilities leave the existing estimate in charge; a visited
//! fraction is never renormalized and presented as a complete calculation.

use beliefs::{ConceptBudget, ConceptQuery};

use crate::Ctx;

pub(crate) struct Reads {
    remaining: usize,
    results: Vec<(ConceptQuery, Option<f32>)>,
}

impl Default for Reads {
    fn default() -> Self {
        Self {
            remaining: 2048,
            results: vec![],
        }
    }
}

impl Ctx<'_> {
    pub(crate) fn joint_probability(&self, query: &ConceptQuery) -> Option<f32> {
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
            routes: RefCell::new(None),
            concepts: RefCell::new(Reads {
                remaining: 700,
                results: vec![],
            }),
            series: crate::SeriesScore::default(),
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
}

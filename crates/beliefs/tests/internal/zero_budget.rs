use super::*;
use crate::{BeliefParams, PerspectiveBudget};
use agent::SeatKind;
use observation::SeatEvent;

const ZERO: ConceptBudget = ConceptBudget { max_nodes: 0 };
const NO_PERSPECTIVE_WORK: PerspectiveBudget = PerspectiveBudget {
    outer_nodes: 0,
    inner: ZERO,
    max_hypotheses: 0,
};

fn cards(text: &str) -> CardSet {
    text.split_whitespace().fold(CardSet::EMPTY, |h, c| {
        h | CardSet::single(c.parse::<engine::Card>().unwrap().index())
    })
}

fn card(text: &str) -> u8 {
    text.parse::<engine::Card>().unwrap().index()
}

fn mind() -> Mind {
    Mind::new(
        0,
        cards("g-6 g-7 e-6 e-7 s-6 s-7 r-6 r-7 r-8"),
        [SeatKind::Human; 4],
        BeliefParams::default(),
    )
}

// Inspect owning caches: assertions on returned node counts alone would miss
// eager DP, likelihood and exchange construction before traversal starts.
fn assert_no_model(mind: &Mind) {
    assert!(mind.model.borrow().is_none());
    assert!(mind.fact_space.borrow().is_none());
    let cache = mind.cache.borrow();
    assert!(cache.concepts.is_none());
    assert!(cache.base.is_none());
    assert!(cache.signals.is_none());
    assert!(cache.exchange.is_none());
    assert!(cache.dealt.is_none());
    assert!(cache.leaves.is_none());
    assert!(cache.derived.is_none());
}

#[test]
fn direct_hard_reads_preserve_constraints_unknowns_and_conditional_semantics() {
    let mut mind = mind();
    mind.apply(&SeatEvent::Bid { seat: 2, value: 130 }).unwrap();
    mind.apply(&SeatEvent::NoPair { seat: 2 }).unwrap();
    let own = ConceptQuery::new(Concept::Card {
        seat: 0,
        card: card("g-6"),
    });
    let unknown = ConceptQuery::new(Concept::Card {
        seat: 1,
        card: card("g-A"),
    });
    let pair = ConceptQuery::new(Concept::Pair {
        seat: 2,
        suit: Suit::Green,
    });
    let absent = ConceptQuery::new(Concept::Card {
        seat: 0,
        card: card("g-A"),
    });
    assert_eq!(own.hard(mind.knowledge()).unwrap().known, Some(true));
    assert_eq!(pair.hard(mind.knowledge()).unwrap().known, Some(false));
    assert_eq!(unknown.hard(mind.knowledge()).unwrap().known, None);
    let contradiction = ConceptQuery {
        event: vec![
            unknown.event[0],
            Literal {
                value: false,
                ..unknown.event[0]
            },
        ],
        given: vec![],
    };
    assert_eq!(contradiction.hard(mind.knowledge()).unwrap().known, Some(false));
    let impossible = ConceptQuery {
        given: absent.event.clone(),
        ..own.clone()
    };
    assert_eq!(impossible.hard(mind.knowledge()).unwrap().bounds, None);
    let conditional = ConceptQuery {
        given: unknown.event.clone(),
        ..own.clone()
    };
    let unresolved = conditional.hard(mind.knowledge()).unwrap();
    assert_eq!(unresolved.known, Some(true));
    assert_eq!(unresolved.conditional_fact, None);
    assert!(!unresolved.complete);
    for query in [own, unknown, pair, absent, contradiction, impossible, conditional] {
        assert_eq!(query.hard(mind.knowledge()), mind.concept(&query, ZERO));
        assert_no_model(&mind);
    }
    let invalid = ConceptQuery::new(Concept::Card { seat: 4, card: 0 });
    assert_eq!(invalid.hard(mind.knowledge()), Err(ConceptError::InvalidConcept));
    let oversized = ConceptQuery {
        event: vec![Concept::Card { seat: 0, card: 0 }.into(); 33],
        given: vec![],
    };
    assert_eq!(oversized.hard(mind.knowledge()), Err(ConceptError::QueryLimit));
    assert_no_model(&mind);
}

#[test]
fn direct_hard_reads_use_only_the_supplied_seat_knowledge() {
    let root = mind();
    let partner = Mind::new(
        2,
        cards("g-A g-K g-O e-A e-K e-O s-A s-K s-O"),
        [SeatKind::Human; 4],
        BeliefParams::default(),
    );
    let private = ConceptQuery::new(Concept::Card {
        seat: 0,
        card: card("g-6"),
    });
    assert_eq!(private.hard(root.knowledge()).unwrap().known, Some(true));
    assert_eq!(private.hard(partner.knowledge()).unwrap().known, None);
    assert_eq!(private.hard(partner.knowledge()), partner.concept(&private, ZERO));
    assert_no_model(&root);
    assert_no_model(&partner);
}

#[test]
fn zero_concept_keeps_hard_truth_separate_from_unresolved_conditions() {
    let mut mind = mind();
    mind.apply(&SeatEvent::Bid { seat: 2, value: 130 }).unwrap();
    let unknown = ConceptQuery::new(Concept::Card {
        seat: 1,
        card: card("g-A"),
    });
    let private = ConceptQuery::new(Concept::Card {
        seat: 0,
        card: card("g-6"),
    });
    let fact = mind.concept(&private, ZERO).unwrap();
    assert!(fact.complete);
    assert_eq!(fact.known, Some(true));
    assert_eq!(fact.conditional_fact, None);
    assert_eq!(fact.bounds, Some(ProbabilityBounds { lower: 1.0, upper: 1.0 }));
    let unresolved = mind.concept(&unknown, ZERO).unwrap();
    assert!(!unresolved.complete);
    assert_eq!(unresolved.known, None);
    assert_eq!(unresolved.bounds, Some(ProbabilityBounds { lower: 0.0, upper: 1.0 }));
    assert_eq!(unresolved.nodes, 0);
    assert_eq!(unresolved.classes, 0);
    assert_eq!(unresolved.covered_deals, 0);
    assert_eq!(unresolved.total_deals, 0); // not constructed/counted
    assert_eq!(unresolved.unresolved_weight_upper, f64::INFINITY);
    let conditional = ConceptQuery {
        given: unknown.event.clone(),
        ..private.clone()
    };
    let unresolved = mind.concept(&conditional, ZERO).unwrap();
    assert_eq!(unresolved.known, Some(true));
    assert_eq!(unresolved.conditional_fact, None);
    assert!(!unresolved.complete);
    assert_eq!(unresolved.bounds, Some(ProbabilityBounds { lower: 0.0, upper: 1.0 }));
    let impossible = ConceptQuery {
        given: vec![Concept::Card {
            seat: 0,
            card: card("g-A"),
        }
        .into()],
        ..private
    };
    let result = mind.concept(&impossible, ZERO).unwrap();
    assert!(result.complete);
    assert_eq!(result.known, Some(true));
    assert_eq!(result.conditional_fact, None);
    assert_eq!(result.bounds, None);
    assert_no_model(&mind);
}

#[test]
fn zero_perspective_never_turns_root_private_truth_into_viewer_proof() {
    let mut mind = mind();
    mind.apply(&SeatEvent::Bid { seat: 2, value: 130 }).unwrap();
    let private = ConceptQuery::new(Concept::Card {
        seat: 0,
        card: card("g-6"),
    });
    for budget in [
        NO_PERSPECTIVE_WORK,
        PerspectiveBudget {
            outer_nodes: 0,
            ..PerspectiveBudget::default()
        },
        PerspectiveBudget {
            max_hypotheses: 0,
            ..PerspectiveBudget::default()
        },
    ] {
        let result = mind.perspective_concept(2, &private, budget).unwrap();
        assert!(!result.complete);
        assert_eq!(result.known_by_viewer, None);
        assert_eq!(result.conditional_fact, None);
        assert_eq!(
            result.expected_belief,
            Some(ProbabilityBounds { lower: 0.0, upper: 1.0 })
        );
        assert_eq!(result.outer_nodes, 0);
        assert_eq!(result.inner_nodes, 0);
        assert_eq!(result.hypotheses, 0);
        assert_eq!(result.unresolved_weight_upper, f64::INFINITY);
        assert_no_model(&mind);
    }
    let own = mind.perspective_concept(0, &private, NO_PERSPECTIVE_WORK).unwrap();
    assert!(own.complete);
    assert_eq!(own.known_by_viewer, Some(true));
    let own_unknown = ConceptQuery::new(Concept::Card {
        seat: 1,
        card: card("g-A"),
    });
    let own = mind.perspective_concept(0, &own_unknown, NO_PERSPECTIVE_WORK).unwrap();
    assert!(!own.complete);
    assert_eq!(own.known_by_viewer, None);
    assert_no_model(&mind);
}

#[test]
fn zero_perspective_preserves_shared_proofs_and_rejects_impossible_givens() {
    let mut mind = mind();
    mind.apply(&SeatEvent::Pass {
        from: 2,
        to: 0,
        cards: Some(cards("g-A g-K g-O e-A")),
    })
    .unwrap();
    let connected = ConceptQuery {
        event: vec![
            Concept::Pair {
                seat: 0,
                suit: Suit::Green,
            }
            .into(),
            Concept::Card {
                seat: 0,
                card: card("e-A"),
            }
            .into(),
        ],
        given: vec![],
    };
    let result = mind.perspective_concept(2, &connected, NO_PERSPECTIVE_WORK).unwrap();
    assert!(result.complete);
    assert_eq!(result.known_by_viewer, Some(true));
    assert_eq!(result.conditional_fact, None);
    assert_eq!(
        result.expected_belief,
        Some(ProbabilityBounds { lower: 1.0, upper: 1.0 })
    );
    // Root knows this private condition, but cannot claim the viewer knows it.
    let private_condition = ConceptQuery {
        given: vec![Concept::Card {
            seat: 0,
            card: card("g-6"),
        }
        .into()],
        ..connected.clone()
    };
    let result = mind
        .perspective_concept(2, &private_condition, NO_PERSPECTIVE_WORK)
        .unwrap();
    assert!(!result.complete);
    assert_eq!(result.known_by_viewer, Some(true));
    assert_eq!(result.conditional_fact, None);
    assert_eq!(
        result.expected_belief,
        Some(ProbabilityBounds { lower: 0.0, upper: 1.0 })
    );
    let conditional = ConceptQuery {
        given: vec![Concept::Card {
            seat: 0,
            card: card("g-A"),
        }
        .into()],
        ..connected.clone()
    };
    let result = mind.perspective_concept(2, &conditional, NO_PERSPECTIVE_WORK).unwrap();
    assert!(result.complete);
    assert_eq!(result.known_by_viewer, Some(true));
    assert_eq!(result.conditional_fact, Some(true));
    let impossible = ConceptQuery {
        given: vec![Literal {
            concept: Concept::Card {
                seat: 0,
                card: card("g-A"),
            },
            value: false,
        }],
        ..connected
    };
    let result = mind.perspective_concept(2, &impossible, NO_PERSPECTIVE_WORK).unwrap();
    assert!(result.complete);
    assert_eq!(result.known_by_viewer, Some(true));
    assert_eq!(result.conditional_fact, None);
    assert_eq!(result.expected_belief, None);
    assert_no_model(&mind);
}

#[test]
fn contradictory_literals_are_hard_false_without_private_hypotheses() {
    let mind = mind();
    let concept = Concept::Card {
        seat: 1,
        card: card("g-A"),
    };
    let contradiction = vec![concept.into(), Literal { concept, value: false }];
    let event = ConceptQuery {
        event: contradiction.clone(),
        given: vec![],
    };
    let result = mind.perspective_concept(2, &event, NO_PERSPECTIVE_WORK).unwrap();
    assert!(result.complete);
    assert_eq!(result.known_by_viewer, Some(false));
    assert_eq!(
        result.expected_belief,
        Some(ProbabilityBounds { lower: 0.0, upper: 0.0 })
    );
    let conditional = ConceptQuery {
        event: vec![Concept::Card {
            seat: 0,
            card: card("g-6"),
        }
        .into()],
        given: contradiction,
    };
    let result = mind.concept(&conditional, ZERO).unwrap();
    assert!(result.complete);
    assert_eq!(result.known, Some(true));
    assert_eq!(result.conditional_fact, None);
    assert_eq!(result.bounds, None);
    let result = mind.perspective_concept(2, &conditional, NO_PERSPECTIVE_WORK).unwrap();
    assert!(result.complete);
    assert_eq!(result.known_by_viewer, None);
    assert_eq!(result.conditional_fact, None);
    assert_eq!(result.expected_belief, None);
    assert_no_model(&mind);
}

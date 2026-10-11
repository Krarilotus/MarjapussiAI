//! IEEE enclosure oracle tests for the real ValueStore bound operators.
use super::*;
use knowledge::IntegerInterval;

fn provenance() -> Provenance {
    Provenance {
        belief_model: "private-declared-fixture-cohort",
        response_model: "numeric-fixture",
        cohort: CohortMethod::Enumerated,
    }
}
fn outcome(points: [i32; 2]) -> GameOutcome {
    GameOutcome {
        playing_team: None,
        game_value: 0,
        team_points: points,
        team_tricks: [5, 4],
    }
}
fn scores(ours: [i64; 2], other: [i64; 2]) -> ScoreIntervals {
    ScoreIntervals {
        teams: [
            IntegerInterval {
                lower: ours[0],
                upper: ours[1],
            },
            IntegerInterval {
                lower: other[0],
                upper: other[1],
            },
        ],
    }
}
fn evaluated_interval(store: &mut ValueStore<'_>, slots: Vec<ScoreIntervals>) -> (IntervalRef, BoundValue) {
    let reference = store.interval_nodes(slots, BoundSource::NumericalFixture);
    store.evaluate(reference.0).unwrap();
    (reference, store.bound(reference.0).unwrap().clone())
}

#[test]
fn singleton_intervals_match_value_store_bits_with_signed_zero_and_subnormals() {
    let tiny = f64::from_bits(1);
    for alpha in [-2.0, -1.0, -0.0, 0.0, tiny, -tiny, 0.5, 1.0, 2.0] {
        for weights in [
            [1.0, tiny],
            [tiny, tiny],
            [-0.0, 1.0],
            [0.0, 1.0],
            [1.0 / 3.0, 2.0 / 3.0],
        ] {
            let terminals = [(outcome([0, 5]), weights[0]), (outcome([10, 0]), weights[1])];
            let inputs = [terminals.as_slice()];
            let mut store =
                ValueStore::with_definition(&inputs, 0, provenance(), Definition { opponent_weight: alpha }).unwrap();
            let computed = store.assess().unwrap().candidates[0].difference;
            let exact = terminals
                .iter()
                .map(|(t, _)| {
                    let [a, b] = engine::game_scores(t, SCHWARZ_FACTOR_FIFTHS);
                    scores([i64::from(a); 2], [i64::from(b); 2])
                })
                .collect();
            let (_, bounded) = evaluated_interval(&mut store, exact);
            let BoundValue::Interval(interval) = bounded else {
                panic!("{bounded:?}")
            };
            assert_eq!(
                interval.lower.to_bits(),
                computed.to_bits(),
                "alpha={alpha:?} weights={weights:?}"
            );
            assert_eq!(interval.upper.to_bits(), computed.to_bits());
            assert!(interval.contains(computed));
            // Complete Assessment cannot conceal bound nodes, even unconsumed ones.
            assert_eq!(store.assess(), Err(ValueError::InvalidContext));
        }
    }
    // Underflowed negative products plus the -0 sum seed produce a genuine -0.
    let terminals = [(outcome([0, 5]), tiny)];
    let inputs = [terminals.as_slice()];
    let mut store =
        ValueStore::with_definition(&inputs, 0, provenance(), Definition { opponent_weight: tiny }).unwrap();
    let value = store.assess().unwrap().candidates[0].difference;
    assert_eq!(value.to_bits(), (-0.0_f64).to_bits());
    let (_, bound) = evaluated_interval(&mut store, vec![scores([0, 0], [5, 5])]);
    assert!(
        matches!(bound, BoundValue::Interval(i) if i.lower.to_bits() == value.to_bits() && i.upper.to_bits() == value.to_bits())
    );
}

#[test]
fn tiny_boxes_contain_every_computed_score_combination_and_canonical_zero_weight() {
    let tiny = f64::from_bits(1);
    for alpha in [-2.0, -0.0, 0.0, 0.5, 1.0, 2.0] {
        for weights in [[1.0 / 3.0, 2.0 / 3.0], [tiny, tiny], [-0.0, 1.0], [0.0, 1.0]] {
            let representative = [(outcome([0, 0]), weights[0]), (outcome([0, 0]), weights[1])];
            let inputs = [representative.as_slice()];
            let mut store =
                ValueStore::with_definition(&inputs, 0, provenance(), Definition { opponent_weight: alpha }).unwrap();
            let (_, bounded) = evaluated_interval(&mut store, vec![scores([0, 10], [0, 10]); 2]);
            let BoundValue::Interval(interval) = bounded else {
                panic!("{bounded:?}")
            };
            for a in [0, 5, 10] {
                for b in [0, 5, 10] {
                    for c in [0, 5, 10] {
                        for d in [0, 5, 10] {
                            let terminals = [(outcome([a, b]), weights[0]), (outcome([c, d]), weights[1])];
                            let inputs = [terminals.as_slice()];
                            let actual = ValueStore::with_definition(
                                &inputs,
                                0,
                                provenance(),
                                Definition { opponent_weight: alpha },
                            )
                            .unwrap()
                            .assess()
                            .unwrap()
                            .candidates[0]
                                .difference;
                            assert!(interval.contains(actual), "{interval:?} misses {actual:?}");
                        }
                    }
                }
            }
        }
    }
    // Current ValueStore accepts differing ±0 signs in candidate-local weights;
    // its SHARED first-cohort weights (not those local bits) drive all arithmetic.
    let first = [(outcome([0, 5]), -0.0), (outcome([0, 0]), 1.0)];
    let second = [(outcome([0, 5]), 0.0), (outcome([0, 0]), 1.0)];
    let inputs = [first.as_slice(), second.as_slice()];
    let actual = ValueStore::new(&inputs, 0, provenance()).unwrap().assess().unwrap();
    assert_eq!(
        actual.candidates[0].difference.to_bits(),
        actual.candidates[1].difference.to_bits()
    );
}

#[test]
fn ordered_cancellation_seed_and_nonfinite_prefixes_are_not_reassociated() {
    let terminals = [
        (outcome([10, 0]), 1.0),
        (outcome([0, 10]), 1.0),
        (outcome([0, 0]), -0.0),
    ];
    let inputs = [terminals.as_slice()];
    let mut store = ValueStore::new(&inputs, 0, provenance()).unwrap();
    let actual = store.assess().unwrap().candidates[0].difference;
    let (_, bound) = evaluated_interval(
        &mut store,
        vec![
            scores([10, 10], [0, 0]),
            scores([0, 0], [10, 10]),
            scores([0, 0], [0, 0]),
        ],
    );
    assert!(
        matches!(bound, BoundValue::Interval(i) if i.lower.to_bits() == actual.to_bits() && i.upper.to_bits() == actual.to_bits())
    );
    let terminals = [(outcome([0, 0]), 1.0), (outcome([0, 0]), 1.0)];
    let inputs = [terminals.as_slice()];
    let mut store = ValueStore::with_definition(
        &inputs,
        0,
        provenance(),
        Definition {
            opponent_weight: f64::MAX,
        },
    )
    .unwrap();
    // Each lower branch endpoint is finite -MAX; their ORDERED sum is -infinity.
    let (_, bound) = evaluated_interval(&mut store, vec![scores([0, 0], [0, 1]); 2]);
    assert_eq!(bound, BoundValue::Unresolved(BoundFailure::NonfiniteArithmetic));
    assert!(FloatInterval::new(f64::MAX, f64::MAX)
        .unwrap()
        .divide(f64::from_bits(1))
        .is_err());
    assert!(FloatInterval::new(-f64::MAX, f64::MAX)
        .unwrap()
        .subtract(FloatInterval::new(-f64::MAX, f64::MAX).unwrap())
        .is_err());
}

#[test]
fn overflowing_endpoint_stays_unresolved_even_with_complete_finite_interior() {
    for (weight, alpha, box_scores) in [
        (1.0, f64::MAX, scores([0, 0], [0, 2])),
        (f64::MAX, 1.0, scores([0, 5], [0, 0])),
    ] {
        let terminals = [(outcome([0, 0]), weight)];
        let inputs = [terminals.as_slice()];
        let mut store =
            ValueStore::with_definition(&inputs, 0, provenance(), Definition { opponent_weight: alpha }).unwrap();
        assert_eq!(store.assess().unwrap().candidates[0].difference, 0.0);
        let (_, bound) = evaluated_interval(&mut store, vec![box_scores]);
        assert_eq!(bound, BoundValue::Unresolved(BoundFailure::NonfiniteArithmetic));
    }
    let terminals = [(outcome([0, 0]), 1.0)];
    let inputs = [terminals.as_slice()];
    let mut store = ValueStore::new(&inputs, 0, provenance()).unwrap();
    let (_, bound) = evaluated_interval(&mut store, vec![scores([1, 0], [0, 0])]);
    assert_eq!(bound, BoundValue::Unresolved(BoundFailure::InvalidIntervals));
    let (_, bound) = evaluated_interval(&mut store, Vec::new());
    assert_eq!(bound, BoundValue::Unresolved(BoundFailure::InvalidIntervals));
    let (_, bound) = evaluated_interval(&mut store, vec![scores([0, 0], [0, 0]); 2]);
    assert_eq!(bound, BoundValue::Unresolved(BoundFailure::InvalidIntervals));
}

#[test]
fn strict_earlier_ties_total_cmp_signed_zeros_and_shared_trace_memo() {
    let terminals = [(outcome([0, 0]), 1.0)];
    let inputs = [terminals.as_slice()];
    let mut store = ValueStore::new(&inputs, 0, provenance()).unwrap();
    let interval = store.interval_nodes(vec![scores([0, 0], [0, 0])], BoundSource::NumericalFixture);
    // Same interval dependency shared by BOTH later original competitors.
    let certificate = store.add_bound(
        "certificate",
        BoundOperator::CertifiedBest {
            selected: 0,
            incumbent: store.candidates[0].difference,
            competitors: vec![(1, interval), (2, interval)],
        },
    );
    store.evaluate(certificate).unwrap();
    assert!(
        matches!(store.bound(certificate).unwrap(), BoundValue::Certified { selected: 0, excluded } if excluded.len() == 2)
    );
    let trace = store.bound_trace();
    assert_eq!(
        trace.len(),
        trace
            .iter()
            .map(|n| n.reference)
            .collect::<std::collections::HashSet<_>>()
            .len()
    );
    for (i, node) in trace.iter().enumerate() {
        assert!(node
            .dependencies
            .iter()
            .all(|r| trace[..i].iter().any(|n| n.reference == *r)));
    }
    let hits = store.hits;
    store.evaluate(certificate).unwrap();
    assert_eq!(store.bound_trace(), trace);
    assert!(store.hits > hits);
    assert_eq!(
        trace
            .iter()
            .filter(|n| matches!(n.operator, RuntimeOperator::Value(Operator::CohortWeights)))
            .count(),
        1
    );
    let earlier = store.add_bound(
        "earlier",
        BoundOperator::CertifiedBest {
            selected: 1,
            incumbent: store.candidates[0].difference,
            competitors: vec![(0, interval)],
        },
    );
    store.evaluate(earlier).unwrap();
    assert_eq!(
        store.bound(earlier).unwrap(),
        &BoundValue::Unresolved(BoundFailure::NoSeparation)
    );
    // An ACTUAL -0 computed incumbent loses to a +0 upper, even for a later index.
    let negative = NumberRef(store.add("negative_zero_fixture", Operator::Constant(-0.0)));
    let signed = store.add_bound(
        "signed",
        BoundOperator::CertifiedBest {
            selected: 0,
            incumbent: negative,
            competitors: vec![(1, interval)],
        },
    );
    store.evaluate(signed).unwrap();
    assert_eq!(
        store.bound(signed).unwrap(),
        &BoundValue::Unresolved(BoundFailure::NoSeparation)
    );
}

#[test]
fn authored_coefficient_changes_certification_like_complete_original_domain() {
    let incumbent = [(outcome([100, 80]), 1.0)];
    let other = [(outcome([90, 0]), 1.0)];
    for (alpha, expected) in [(1.0, 1), (0.0, 0)] {
        let definition = Definition { opponent_weight: alpha };
        let full = [incumbent.as_slice(), other.as_slice()];
        let oracle = ValueStore::with_definition(&full, 0, provenance(), definition)
            .unwrap()
            .assess()
            .unwrap();
        assert_eq!(oracle.selected, expected);
        let one = [incumbent.as_slice()];
        let mut store = ValueStore::with_definition(&one, 0, provenance(), definition).unwrap();
        let interval = store.interval_nodes(vec![scores([90, 90], [0, 0])], BoundSource::NumericalFixture);
        let proof = store.add_bound(
            "proof",
            BoundOperator::CertifiedBest {
                selected: 0,
                incumbent: store.candidates[0].difference,
                competitors: vec![(1, interval)],
            },
        );
        store.evaluate(proof).unwrap();
        assert_eq!(
            matches!(store.bound(proof).unwrap(), BoundValue::Certified { selected: 0, .. }),
            expected == 0
        );
        let trace = store.bound_trace();
        let authored = trace
            .iter()
            .find(|n| matches!(n.operator, RuntimeOperator::Value(Operator::Constant(_))))
            .unwrap();
        assert!(trace
            .iter()
            .filter(|n| matches!(
                n.operator,
                RuntimeOperator::Value(Operator::BranchDifference { .. })
                    | RuntimeOperator::Bound(BoundOperator::BranchDifference { .. })
            ))
            .all(|n| n.dependencies.contains(&authored.reference)));
    }
}

#[test]
fn complete_legacy_trace_preserves_frozen_dependency_order_and_values() {
    let terminals = [(outcome([140, 0]), 1.0)];
    let inputs = [terminals.as_slice()];
    let a = ValueStore::new(&inputs, 0, provenance()).unwrap().assess().unwrap();
    // Frozen 3eb23b2 single-candidate public trace: bound wrappers must not alter it.
    assert_eq!(
        a.trace.iter().map(|n| n.reference.0).collect::<Vec<_>>(),
        vec![3, 4, 2, 7, 0, 1, 8, 14, 5, 6, 9, 11, 10, 12, 13, 15]
    );
    assert_eq!((a.root, a.selected, a.alternative), (NodeId(15), 0, None));
    assert!(a.cohort_complete);
    assert_eq!(
        (
            a.candidates[0].our_score,
            a.candidates[0].opponent_score,
            a.candidates[0].difference
        ),
        (140.0, 0.0, 140.0)
    );
    assert!(a.trace.iter().all(|n| n.meaning == n.operator.meaning()));
}

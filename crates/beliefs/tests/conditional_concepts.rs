use agent::{SeatKind, Table};
use beliefs::{BeliefParams, Concept, ConceptBudget, ConceptError, ConceptQuery, Literal, Mind, PerspectiveBudget};
use engine::{ActionType, Card, CardSet, Suit, SUITS};
use knowledge::{Belief, Hands};
use observation::SeatEvent;
use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};

const COMPLETE: ConceptBudget = ConceptBudget { max_nodes: 1_000_000 };
const ZERO: ConceptBudget = ConceptBudget { max_nodes: 0 };
const NO_PERSPECTIVE_WORK: PerspectiveBudget = PerspectiveBudget {
    outer_nodes: 0,
    inner: ZERO,
    max_hypotheses: 0,
};

fn cards(text: &str) -> CardSet {
    text.split_whitespace().fold(CardSet::EMPTY, |set, card| {
        set | CardSet::single(card.parse::<Card>().unwrap().index())
    })
}

fn c(text: &str) -> u8 {
    text.parse::<Card>().unwrap().index()
}

fn low_mind() -> Mind {
    Mind::new(
        0,
        cards("g-6 g-7 e-6 e-7 s-6 s-7 r-6 r-7 r-8"),
        [SeatKind::Human; 4],
        BeliefParams::base_only(),
    )
}

fn matches(literals: &[Literal], hands: &Hands) -> bool {
    literals.iter().all(|literal| {
        let truth = match literal.concept {
            Concept::Card { seat, card } => hands[seat as usize].contains(card),
            Concept::Void { seat, suit } => (hands[seat as usize] & CardSet::suit(suit)).is_empty(),
            Concept::Pair { seat, suit } => CardSet::halves(suit).is_subset(hands[seat as usize]),
            Concept::Count { seat, cards, min, max } => {
                (min as u32..=max as u32).contains(&(hands[seat as usize] & cards).len())
            }
        };
        truth == literal.value
    })
}

fn exact(mind: &Mind, query: &ConceptQuery) -> (Option<f64>, Option<bool>) {
    let (mut numerator, mut mass) = (0.0, 0.0);
    let (mut all, mut event) = (0, 0);
    let weights = mind.deal_weights();
    mind.knowledge().deals().for_each_deal(|hands| {
        if matches(&query.given, hands) {
            all += 1;
            let truth = matches(&query.event, hands);
            event += usize::from(truth);
            let w = weights.weight(hands);
            mass += w;
            numerator += if truth { w } else { 0.0 };
        }
    });
    let proof = if all == 0 {
        None
    } else if event == 0 {
        Some(false)
    } else if event == all {
        Some(true)
    } else {
        None
    };
    ((mass > 0.0).then(|| numerator / mass), proof)
}

fn small_table(seed: u64) -> Table {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut deck = engine::get_all_cards();
    deck.shuffle(&mut rng);
    let mut table = Table::new(&engine::hands_from_deck(&deck));
    let mut bid = false;
    for _ in 0..160 {
        let (_, legal) = table.to_act().expect("fixture ends before the game");
        let action = if !bid {
            bid = true;
            *legal.iter().find(|a| a.action_type == ActionType::NewBid(130)).unwrap()
        } else if let Some(stop) = legal.iter().find(|a| a.action_type == ActionType::StopBidding) {
            *stop
        } else {
            let plays: Vec<_> = legal
                .iter()
                .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
                .collect();
            if plays.is_empty() {
                legal[0]
            } else {
                *plays[rng.random_range(0..plays.len())]
            }
        };
        table.apply(action);
        let k = table.view(0).knowledge;
        if (!k.played()).len() <= 8 && (!k.played()).len() >= 4 && k.deals().count() <= 2_000 {
            return table;
        }
    }
    panic!("fixture did not reach a small live state");
}

fn from_table(table: &Table, seat: u8, params: BeliefParams) -> Mind {
    let view = table.view(seat);
    let mut mind = Mind::new(seat, view.knowledge.dealt_certain(seat), view.kinds, params);
    for event in view.events {
        mind.apply(event).unwrap();
    }
    mind
}

#[test]
fn joint_queries_preserve_card_ownership_and_pair_conditioning() {
    let mind = low_mind();
    let king = Concept::Card {
        seat: 1,
        card: c("r-K"),
    };
    let impossible = ConceptQuery {
        event: vec![
            king.into(),
            Concept::Card {
                seat: 2,
                card: c("r-K"),
            }
            .into(),
        ],
        given: vec![],
    };
    let result = mind.concept(&impossible, COMPLETE).unwrap();
    assert_eq!(result.known, Some(false));
    assert_eq!(result.bounds.unwrap().upper, 0.0);
    let pair = ConceptQuery::new(Concept::Pair {
        seat: 1,
        suit: Suit::Red,
    });
    let prior = mind.concept(&pair, COMPLETE).unwrap().bounds.unwrap().lower;
    assert!((prior - (9.0 / 27.0) * (8.0 / 26.0)).abs() < 1e-12);
    let conditioned = ConceptQuery {
        given: vec![king.into()],
        ..pair
    };
    let result = mind.concept(&conditioned, COMPLETE).unwrap();
    assert!((result.bounds.unwrap().lower - 8.0 / 26.0).abs() < 1e-12);
    assert_eq!(result.known, None);
    assert_eq!(result.conditional_fact, None);
    let exclusive = ConceptQuery {
        event: vec![Concept::Card {
            seat: 2,
            card: c("r-K"),
        }
        .into()],
        given: vec![king.into()],
    };
    let result = mind.concept(&exclusive, COMPLETE).unwrap();
    assert_eq!(result.conditional_fact, Some(false));
    assert_eq!(result.known, None, "a hypothesis is not an unconditional fact");
}

#[test]
fn grouped_model_matches_exhaustive_small_states_including_hidden_exchange() {
    for seed in [73, 97] {
        let table = small_table(seed);
        for observer in [0, 1] {
            let mut params = BeliefParams::default();
            // Exercise unequal per-card factors as well as joint bid likelihoods.
            params.factors.human.lead = [1.3, 0.7, 1.1, 0.9];
            let mind = from_table(&table, observer, params);
            assert!(mind.knowledge().deals().count() <= 5_000);
            for seat in 0..4 {
                for suit in SUITS {
                    for concept in [
                        Concept::Void { seat, suit },
                        Concept::Pair { seat, suit },
                        Concept::Count {
                            seat,
                            cards: CardSet::suit(suit),
                            min: 1,
                            max: 2,
                        },
                    ] {
                        let query = ConceptQuery::new(concept);
                        let (expected, proof) = exact(&mind, &query);
                        let result = mind.concept(&query, COMPLETE).unwrap();
                        assert!(result.complete);
                        assert_eq!(result.known, proof);
                        match (expected, result.bounds) {
                            (Some(p), Some(b)) => {
                                assert!((p - b.lower).abs() < 1e-10);
                                assert!((p - b.upper).abs() < 1e-10);
                            }
                            (None, None) => {}
                            other => panic!("incompatible mass {other:?}"),
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn bid_interpretation_and_undo_invalidate_joint_query_cache() {
    let mut mind = Mind::new(
        0,
        cards("g-A e-A s-A r-A g-6 e-6 s-6 r-6 r-7"),
        [SeatKind::Human; 4],
        BeliefParams::default(),
    );
    mind.apply(&SeatEvent::Bid { seat: 0, value: 120 }).unwrap();
    let query = ConceptQuery::new(Concept::Pair {
        seat: 1,
        suit: Suit::Bells,
    });
    let before = mind.concept(&query, COMPLETE).unwrap();
    mind.apply(&SeatEvent::Bid { seat: 1, value: 135 }).unwrap();
    let after = mind.concept(&query, COMPLETE).unwrap();
    assert!(after.complete && before.complete);
    assert!(after.bounds.unwrap().lower > before.bounds.unwrap().lower);
    assert_eq!(after.known, None, "a trusted pair message is still uncertain");
    assert!((after.bounds.unwrap().lower - mind.has_pair(1, Suit::Bells) as f64).abs() < 1e-6);
    assert_eq!(after, mind.concept(&query, COMPLETE).unwrap());
    mind.apply(&SeatEvent::Undone).unwrap();
    assert_eq!(before, mind.concept(&query, COMPLETE).unwrap());
}

#[test]
fn budget_exhaustion_and_impossible_conditions_are_not_false_certainty() {
    let mind = low_mind();
    let query = ConceptQuery::new(Concept::Pair {
        seat: 1,
        suit: Suit::Red,
    });
    let stopped = mind.concept(&query, ZERO).unwrap();
    assert!(!stopped.complete);
    assert_eq!(stopped.known, None);
    assert_eq!(stopped.bounds.unwrap().lower, 0.0);
    assert_eq!(stopped.bounds.unwrap().upper, 1.0);
    assert!(stopped.unresolved_weight_upper > 0.0);
    let invalid_condition = ConceptQuery {
        given: vec![Concept::Card {
            seat: 1,
            card: c("g-6"),
        }
        .into()],
        ..query
    };
    let result = mind.concept(&invalid_condition, COMPLETE).unwrap();
    assert!(result.complete);
    assert_eq!(result.bounds, None);
    assert_eq!(result.conditional_fact, None);
    assert_eq!(result.nodes, 0);
}

#[test]
fn public_voids_and_trump_cards_are_viewer_proofs_without_enumeration() {
    let mut mind = Mind::new(
        0,
        cards("e-6 e-7 e-8 s-6 s-7 s-8 r-6 r-7 r-8"),
        [SeatKind::Human; 4],
        BeliefParams::default(),
    );
    for event in [
        SeatEvent::CardPlayed {
            seat: 1,
            card: c("e-A"),
        },
        SeatEvent::CardPlayed {
            seat: 2,
            card: c("e-Z"),
        },
        SeatEvent::CardPlayed {
            seat: 3,
            card: c("e-K"),
        },
        SeatEvent::CardPlayed {
            seat: 0,
            card: c("e-6"),
        },
        SeatEvent::AnnounceTrump {
            seat: 1,
            suit: Suit::Red,
        },
        SeatEvent::CardPlayed {
            seat: 1,
            card: c("g-A"),
        },
        SeatEvent::CardPlayed {
            seat: 2,
            card: c("g-Z"),
        },
        SeatEvent::CardPlayed {
            seat: 3,
            card: c("g-K"),
        },
        SeatEvent::CardPlayed {
            seat: 0,
            card: c("r-8"),
        },
    ] {
        mind.apply(&event).unwrap();
    }
    let query = ConceptQuery {
        event: vec![
            Concept::Void {
                seat: 0,
                suit: Suit::Green,
            }
            .into(),
            Concept::Card {
                seat: 1,
                card: c("r-K"),
            }
            .into(),
        ],
        given: vec![],
    };
    for viewer in [2, 3] {
        let result = mind.perspective_concept(viewer, &query, NO_PERSPECTIVE_WORK).unwrap();
        assert_eq!(result.known_by_viewer, Some(true));
        assert_eq!(result.outer_nodes + result.inner_nodes, 0);
    }
    let ruff = ConceptQuery {
        event: vec![
            Concept::Void {
                seat: 0,
                suit: Suit::Green,
            }
            .into(),
            Concept::Count {
                seat: 0,
                cards: CardSet::suit(Suit::Red),
                min: 1,
                max: 36,
            }
            .into(),
        ],
        given: vec![],
    };
    assert_eq!(mind.concept(&ruff, ZERO).unwrap().known, Some(true));
    let partner = mind.perspective_concept(2, &ruff, NO_PERSPECTIVE_WORK).unwrap();
    assert_eq!(partner.known_by_viewer, None);
    assert!(
        !partner.complete,
        "known void does not make uncertain remaining trump known"
    );
}

#[test]
fn shared_transfer_and_public_consumption_update_proofs_but_opponents_do_not_see_passes() {
    let mut mind = Mind::new(
        0,
        cards("r-A g-6 g-7 e-6 e-7 s-6 s-7 r-6 r-7"),
        [SeatKind::Human; 4],
        BeliefParams::base_only(),
    );
    mind.apply(&SeatEvent::Pass {
        from: 0,
        to: 2,
        cards: Some(cards("r-A g-6 e-6 r-6")),
    })
    .unwrap();
    let held = ConceptQuery::new(Concept::Card {
        seat: 2,
        card: c("r-A"),
    });
    let partner = mind.perspective_concept(2, &held, NO_PERSPECTIVE_WORK).unwrap();
    assert_eq!(partner.known_by_viewer, Some(true));
    assert_eq!(partner.outer_nodes, 0);
    let opponent = mind.perspective_concept(1, &held, NO_PERSPECTIVE_WORK).unwrap();
    assert_eq!(opponent.known_by_viewer, None);
    assert!(!opponent.complete);
    mind.apply(&SeatEvent::Pass {
        from: 2,
        to: 0,
        cards: Some(cards("r-A g-A e-A s-A")),
    })
    .unwrap();
    assert_eq!(
        mind.perspective_concept(2, &held, NO_PERSPECTIVE_WORK)
            .unwrap()
            .known_by_viewer,
        Some(false)
    );
    let returned = ConceptQuery::new(Concept::Card {
        seat: 0,
        card: c("r-A"),
    });
    assert_eq!(
        mind.perspective_concept(2, &returned, NO_PERSPECTIVE_WORK)
            .unwrap()
            .known_by_viewer,
        Some(true)
    );
    mind.apply(&SeatEvent::CardPlayed {
        seat: 0,
        card: c("r-A"),
    })
    .unwrap();
    for viewer in [1, 2] {
        let consumed = mind
            .perspective_concept(viewer, &returned, NO_PERSPECTIVE_WORK)
            .unwrap();
        assert_eq!(consumed.known_by_viewer, Some(false));
        assert_eq!(consumed.outer_nodes, 0);
    }
    mind.apply(&SeatEvent::Undone).unwrap();
    assert_eq!(
        mind.perspective_concept(2, &returned, NO_PERSPECTIVE_WORK)
            .unwrap()
            .known_by_viewer,
        Some(true)
    );
}

#[test]
fn root_private_hand_is_not_a_viewer_fact_and_hidden_private_pass_is_unsupported() {
    let mind = low_mind();
    let private = ConceptQuery::new(Concept::Card {
        seat: 0,
        card: c("g-6"),
    });
    assert_eq!(mind.concept(&private, ZERO).unwrap().known, Some(true));
    let estimate = mind
        .perspective_concept(
            1,
            &private,
            PerspectiveBudget {
                outer_nodes: 1_000,
                inner: COMPLETE,
                max_hypotheses: 2,
            },
        )
        .unwrap();
    assert_eq!(estimate.known_by_viewer, None);
    assert!(estimate.hypotheses > 0);
    let mut hidden = low_mind();
    hidden
        .apply(&SeatEvent::Pass {
            from: 1,
            to: 3,
            cards: None,
        })
        .unwrap();
    let unresolved = hidden.perspective_concept(1, &private, NO_PERSPECTIVE_WORK).unwrap();
    assert!(!unresolved.complete);
    assert_eq!(unresolved.known_by_viewer, None);
    assert_eq!(unresolved.expected_belief.unwrap().lower, 0.0);
    assert_eq!(unresolved.expected_belief.unwrap().upper, 1.0);
    assert_eq!(
        hidden.perspective_concept(1, &private, PerspectiveBudget::default()),
        Err(ConceptError::UnsupportedPrivateHistory)
    );
}

#[test]
fn hidden_hand_swaps_leave_root_and_nested_inference_unchanged() {
    let mut hands = engine::hands_from_deck(&engine::get_all_cards());
    let first = Table::new(&hands);
    let card = first.view(0).knowledge.certain(0).iter().next().unwrap();
    let swapped = hands[1][0];
    hands[1][0] = hands[3][0];
    hands[3][0] = swapped;
    let second = Table::new(&hands);
    let left = from_table(&first, 0, BeliefParams::base_only());
    let right = from_table(&second, 0, BeliefParams::base_only());
    let query = ConceptQuery::new(Concept::Card { seat: 0, card });
    let budget = PerspectiveBudget {
        outer_nodes: 1_000,
        inner: COMPLETE,
        max_hypotheses: 2,
    };
    assert_eq!(
        left.perspective_concept(1, &query, budget),
        right.perspective_concept(1, &query, budget)
    );
}

#[test]
fn nested_estimate_matches_independent_small_private_hand_mixture() {
    let table = small_table(73);
    let root = from_table(&table, 0, BeliefParams::default());
    let viewer = 2;
    let budget = PerspectiveBudget {
        outer_nodes: 1_000_000,
        inner: COMPLETE,
        max_hypotheses: 32,
    };
    let card = root.knowledge().may_hold(1).iter().next().unwrap();
    let query = ConceptQuery::new(Concept::Card { seat: 1, card });
    let result = root.perspective_concept(viewer, &query, budget).unwrap();
    let weights = root.deal_weights();
    let (mut numerator, mut mass) = (0.0, 0.0);
    let (mut all_true, mut all_false) = (true, true);
    root.knowledge().deals().for_each_deal(|hands| {
        let start = root
            .knowledge()
            .dealt_from_current(viewer, hands[viewer as usize])
            .unwrap();
        let mut model = Mind::new(viewer, start, root.kinds(), root.params().clone());
        for event in root.events() {
            model.apply(event).unwrap();
        } // all this fixture's passes are shared by 0/2
        assert!(model.knowledge().deals().count() <= 5_000);
        let (p, proof) = exact(&model, &query);
        all_true &= proof == Some(true);
        all_false &= proof == Some(false);
        let w = weights.weight(hands);
        mass += w;
        numerator += w * p.unwrap();
    });
    assert!(result.complete);
    let expected = numerator / mass;
    let bounds = result.expected_belief.unwrap();
    assert!((expected - bounds.lower).abs() < 1e-10);
    assert!((expected - bounds.upper).abs() < 1e-10);
    assert_eq!(
        result.known_by_viewer,
        if all_true {
            Some(true)
        } else if all_false {
            Some(false)
        } else {
            None
        }
    );
}

#[test]
fn zero_model_weight_counterexamples_do_not_become_hard_proofs() {
    let mut params = BeliefParams::default();
    // Valid extreme factors: two f32 factors underflow to zero. The supported
    // hard deals must still participate in proof, independently of their weight.
    params.conventions.convention_floor = f32::MIN_POSITIVE;
    params.factors.human.pass = [[0.0; 3]; 4];
    params.factors.human.lead = [0.0; 4];
    params.validate().unwrap();
    let mut mind = Mind::new(
        0,
        cards("g-6 g-7 e-6 e-7 s-6 s-7 r-6 r-7 r-8"),
        [SeatKind::Human; 4],
        params,
    );
    let transfer = cards("g-K e-K s-K r-K");
    for event in [
        SeatEvent::Pass {
            from: 2,
            to: 0,
            cards: Some(transfer),
        },
        SeatEvent::Pass {
            from: 0,
            to: 2,
            cards: Some(transfer),
        },
        SeatEvent::CardPlayed {
            seat: 2,
            card: c("e-A"),
        },
    ] {
        mind.apply(&event).unwrap();
    }
    let query = ConceptQuery::new(Concept::Card {
        seat: 2,
        card: c("e-8"),
    });
    let result = mind.concept(&query, COMPLETE).unwrap();
    assert!(result.complete);
    assert!(result.resolved_condition_weight > 0.0);
    assert_eq!(result.bounds.unwrap().upper, 0.0);
    assert_eq!(
        result.known, None,
        "zero-weight true worlds are hard counterexamples to known-false"
    );
    let conditional = ConceptQuery {
        given: vec![Concept::Card {
            seat: 2,
            card: c("e-8"),
        }
        .into()],
        ..query
    };
    let assumed = mind.concept(&conditional, COMPLETE).unwrap();
    assert_eq!(
        assumed.bounds, None,
        "zero model mass is undefined, not probability zero"
    );
    assert_eq!(
        assumed.conditional_fact,
        Some(true),
        "hard implication survives zero model mass"
    );
    assert_eq!(assumed.known, None);
}

#[test]
fn canonical_queries_and_validated_limits_bound_cache_inputs() {
    let mind = low_mind();
    let query = ConceptQuery::new(Concept::Card {
        seat: 1,
        card: c("r-K"),
    });
    let single = mind.concept(&query, COMPLETE).unwrap();
    let duplicate = ConceptQuery {
        event: vec![query.event[0]; 32],
        given: vec![],
    };
    assert_eq!(single, mind.concept(&duplicate, COMPLETE).unwrap());
    let too_many = ConceptQuery {
        event: vec![query.event[0]; 33],
        given: vec![],
    };
    assert_eq!(mind.concept(&too_many, ZERO), Err(ConceptError::QueryLimit));
    assert_eq!(
        mind.concept(&query, ConceptBudget { max_nodes: 1_000_001 }),
        Err(ConceptError::InvalidBudget)
    );
    assert_eq!(
        mind.perspective_concept(
            1,
            &query,
            PerspectiveBudget {
                max_hypotheses: 33,
                ..NO_PERSPECTIVE_WORK
            }
        ),
        Err(ConceptError::InvalidBudget)
    );
    let contradictory = ConceptQuery {
        given: vec![
            query.event[0],
            Literal {
                value: false,
                ..query.event[0]
            },
        ],
        ..query
    };
    let result = mind.concept(&contradictory, ZERO).unwrap();
    assert!(result.complete);
    assert_eq!(result.nodes, 0);
    assert_eq!(result.bounds, None);
}

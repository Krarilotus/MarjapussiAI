use super::*;
use agent::Table;
use beliefs::{BeliefParams, Concept};
use engine::{Card, PlaceAtTable, QuestionType};

fn table() -> Table {
    let hands = std::array::from_fn(|seat| (0..9).map(|i| Card::from_index((4 * i + seat) as u8)).collect());
    let mut table = Table::new(&hands);
    while table
        .to_act()
        .unwrap()
        .1
        .iter()
        .any(|action| matches!(action.action_type, ActionType::StopBidding))
    {
        let action = *table
            .to_act()
            .unwrap()
            .1
            .iter()
            .find(|a| a.action_type == ActionType::StopBidding)
            .unwrap();
        table.apply(action);
    }
    table
}

#[test]
fn canonical_suffix_ignores_names_indices_and_unreachable_nodes_but_not_followthrough() {
    let kind = ActionType::Question(QuestionType::Yours);
    let a = Program {
        name: "one",
        version: 1,
        start: Stage(1),
        nodes: vec![
            Node::Tail,
            Node::Action {
                actions: Actions::Exact(kind),
                next: Stage(0),
            },
        ],
    };
    let b = Program {
        name: "two",
        version: 2,
        start: Stage(0),
        nodes: vec![
            Node::Action {
                actions: Actions::Exact(kind),
                next: Stage(2),
            },
            Node::Tail,
            Node::Tail,
        ],
    };
    assert_eq!(a.suffix(a.start), b.suffix(b.start));
    let mut c = a.clone();
    c.nodes.push(Node::Guard {
        predicate: Guard::Trump(Suit::Bells),
        yes: Stage(0),
        no: Stage(0),
        unknown: Stage(0),
    });
    c.nodes[1] = Node::Action {
        actions: Actions::Exact(kind),
        next: Stage(2),
    };
    assert_ne!(
        a.suffix(a.start),
        c.suffix(c.start),
        "consumed guard semantics are part of the suffix"
    );
    c.nodes[2] = Node::Choice(vec![Stage(2)]);
    assert_eq!(c.validate(), Err(SequenceError::InvalidProgram));
}

#[test]
fn conditional_fact_cannot_authorize_an_observed_state_guard() {
    let query = ConceptQuery {
        event: vec![Concept::Pair {
            seat: 2,
            suit: Suit::Bells,
        }
        .into()],
        given: vec![Concept::Pair {
            seat: 2,
            suit: Suit::Bells,
        }
        .into()],
    };
    let program = Program {
        name: "invalid-hypothesis",
        version: 1,
        start: Stage(1),
        nodes: vec![
            Node::Tail,
            Node::Guard {
                predicate: Guard::Known(query),
                yes: Stage(0),
                no: Stage(0),
                unknown: Stage(0),
            },
        ],
    };
    assert_eq!(program.validate(), Err(SequenceError::InvalidProgram));
}

#[test]
fn unknown_hard_query_takes_its_explicit_edge_and_consumed_trace_is_actual() {
    let table = table();
    let (seat, legal) = table.to_act().unwrap();
    let view = table.view(seat);
    let mind = Mind::new(
        seat,
        view.knowledge.dealt_certain(seat),
        view.kinds,
        BeliefParams::base_only(),
    );
    // Reconstruct the observed auction so the store is exactly this view.
    let mut mind = mind;
    for event in view.events {
        mind.apply(event).unwrap();
    }
    let query = SUITS
        .into_iter()
        .map(|suit| {
            ConceptQuery::new(Concept::Pair {
                seat: engine::partner(seat),
                suit,
            })
        })
        .find(|query| {
            mind.concept(query, ConceptBudget { max_nodes: 0 })
                .unwrap()
                .known
                .is_none()
        })
        .unwrap();
    let program = Program {
        name: "unknown",
        version: 1,
        start: Stage(3),
        nodes: vec![
            Node::Tail,
            Node::Action {
                actions: Actions::Exact(legal[0].action_type),
                next: Stage(0),
            },
            Node::Choice(vec![Stage(0)]),
            Node::Guard {
                predicate: Guard::Known(query.clone()),
                yes: Stage(1),
                no: Stage(0),
                unknown: Stage(2),
            },
        ],
    };
    let reading = inspect(&program, program.start, &view, legal, &mind).unwrap();
    assert_eq!(reading.frontier, Frontier::Choice(vec![Stage(0)]));
    assert!(matches!(
        &reading.consumed[0].meaning,
        ReadMeaning::Guard { truth: None, .. }
    ));
    assert_eq!(reading.consumed[0].event_count, view.events.len());
    assert_eq!(
        mind.concept(&query, ConceptBudget { max_nodes: 0 }).unwrap().known,
        None
    );
}

#[test]
fn pending_resources_advance_only_after_the_matching_accepted_action() {
    let mut table = table();
    let (seat, legal) = table.to_act().unwrap();
    let action = legal[0];
    let program = Program {
        name: "pending",
        version: 1,
        start: Stage(1),
        nodes: vec![
            Node::Tail,
            Node::Action {
                actions: Actions::Exact(action.action_type),
                next: Stage(0),
            },
        ],
    };
    let mut receipt = Receipt::new(program, &table.view(seat)).unwrap();
    receipt
        .propose(Offer { action, next: Stage(0) }, &table.view(seat), legal)
        .unwrap();
    assert_eq!(receipt.stage, Stage(1));
    receipt.synchronize(&table.view(seat)).unwrap();
    assert_eq!(receipt.stage, Stage(1));
    table.apply(action);
    receipt.synchronize(&table.view(seat)).unwrap();
    assert_eq!(receipt.stage, Stage(0));
    assert!(receipt.pending.is_none());
    let mut events = table.view(seat).events.to_vec();
    events.push(SeatEvent::Undone);
    let mut view = table.view(seat);
    view.events = &events;
    assert_eq!(receipt.synchronize(&view), Err(SequenceError::HistoryChanged));
    // Changing public player metadata also invalidates private continuation state.
    let mut view = table.view(seat);
    view.kinds[(seat as usize + 1) % 4] = SeatKind::Human;
    assert_eq!(receipt.synchronize(&view), Err(SequenceError::HistoryChanged));
}

#[test]
fn split_pair_has_the_same_activation_chain_and_distinct_success_suffix() {
    let card = |code: &str| code.parse::<Card>().unwrap().index();
    let own = ["s-K", "e-A", "g-6", "g-7", "g-8", "g-9", "e-6", "e-7", "r-6"]
        .into_iter()
        .fold(CardSet::EMPTY, |cards, code| cards | CardSet::single(card(code)));
    let transfer = ["s-O", "r-7", "r-8", "r-9"]
        .into_iter()
        .fold(CardSet::EMPTY, |cards, code| cards | CardSet::single(card(code)));
    let mut mind = Mind::new(0, own, [SeatKind::Unknown; 4], BeliefParams::base_only());
    mind.apply(&SeatEvent::Pass {
        from: 2,
        to: 0,
        cards: Some(transfer),
    })
    .unwrap();
    // Return the received half: both players now know the live split ownership.
    mind.apply(&SeatEvent::Pass {
        from: 0,
        to: 2,
        cards: Some(transfer),
    })
    .unwrap();
    let view = SeatView {
        seat: 0,
        knowledge: mind.knowledge(),
        events: &[],
        kinds: [SeatKind::Unknown; 4],
        series: SeriesScore::default(),
    };
    let legal = [GameAction {
        player: PlaceAtTable(0),
        action_type: ActionType::Question(QuestionType::YourHalf(Suit::Bells)),
    }];
    let programs = authored_programs(&view, &legal);
    let connected = programs
        .iter()
        .filter(|p| {
            p.name == "cash-activation-control-transfer-recovery"
                && p.nodes.iter().any(|node| {
                    matches!(
                        node,
                        Node::Action {
                            actions: Actions::Exact(ActionType::Question(QuestionType::YourHalf(Suit::Bells))),
                            ..
                        }
                    )
                })
        })
        .collect::<Vec<_>>();
    assert!(!connected.is_empty());
    assert!(connected.iter().any(|p| p.nodes.iter().any(|node| matches!(
        node,
        Node::Action {
            actions: Actions::Exact(ActionType::Question(QuestionType::YourHalf(Suit::Bells))),
            ..
        }
    ))));
    for program in connected {
        program.validate().unwrap();
        assert!(program.nodes.iter().any(|node| matches!(node,
            Node::Guard { predicate: Guard::Trump(Suit::Bells), yes, no, .. } if yes != no)));
        assert!(program.nodes.len() < MAX_STAGES);
        assert!(program.nodes.iter().any(|node| matches!(node,
            Node::Guard { predicate: Guard::Leader(_), yes, no, .. } if yes != no)));
    }
}

fn raising_table() -> Table {
    let hands = std::array::from_fn(|seat| (0..9).map(|i| Card::from_index((4 * i + seat) as u8)).collect());
    let mut table = Table::new(&hands);
    let bid = *table
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| a.action_type == ActionType::NewBid(120))
        .unwrap();
    table.apply(bid);
    while table.game.state.phase == engine::GamePhase::Bidding {
        let pass = *table
            .to_act()
            .unwrap()
            .1
            .iter()
            .find(|a| a.action_type == ActionType::StopBidding)
            .unwrap();
        table.apply(pass);
    }
    for _ in 0..2 {
        let pass = *table
            .to_act()
            .unwrap()
            .1
            .iter()
            .find(|a| matches!(a.action_type, ActionType::Pass(_)))
            .unwrap();
        table.apply(pass);
    }
    assert_eq!(table.game.state.phase, engine::GamePhase::Raising);
    table
}

fn reader(view: &SeatView<'_>) -> Mind {
    let mut mind = Mind::new(
        view.seat,
        view.knowledge.dealt_certain(view.seat),
        view.kinds,
        BeliefParams::base_only(),
    );
    for event in view.events {
        mind.apply(event).unwrap();
    }
    assert_eq!(mind.knowledge(), view.knowledge);
    mind
}

#[test]
fn optional_raises_preserve_every_legal_value_and_bid_receipts_install_composed_programs() {
    let mut table = raising_table();
    let (seat, legal) = table.to_act().unwrap();
    let view = table.view(seat);
    let mind = reader(&view);
    let programs = authored_programs_with_reconsideration(&view, legal, 1).unwrap();
    let raising = programs
        .iter()
        .find(|p| p.name == "optional-raise-then-opening")
        .unwrap();
    let opening = programs
        .iter()
        .find(|p| p.name == "cash-activation-control-transfer-recovery")
        .unwrap();
    let Node::Action {
        actions: Actions::Raises,
        next,
    } = &raising.nodes[raising.start.0]
    else {
        panic!("shared raising action family expected")
    };
    assert_eq!(raising.suffix(*next).unwrap(), opening.suffix(opening.start).unwrap());
    let inspection = inspect(raising, raising.start, &view, legal, &mind).unwrap();
    let Frontier::Actions(offers) = inspection.frontier else {
        panic!("raise binding expected")
    };
    let expected: Vec<_> = legal
        .iter()
        .filter(|a| matches!(a.action_type, ActionType::NewBid(_)))
        .copied()
        .collect();
    assert_eq!(offers.iter().map(|offer| offer.action).collect::<Vec<_>>(), expected);
    assert!(offers.len() > 1);
    assert!(offers.iter().all(|offer| offer.next == offers[0].next));
    assert!(matches!(&inspection.consumed[0].meaning,
        ReadMeaning::Binding { actions: Actions::Raises, legal_members } if *legal_members == expected));
    // Engine limits, not a chosen hand or a heuristic priority, determine the
    // needed capacity. Combined bid/card domains may exceed the old cap of 64.
    assert!(MAX_ALTERNATIVES >= engine::bid_range(engine::NO_BID).count() + engine::HAND_SIZE + 8);
    for card in legal
        .iter()
        .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
    {
        assert!(
            programs.iter().any(|p| matches!(&p.nodes[p.start.0],
            Node::Action { actions: Actions::Exact(kind), .. } if *kind == card.action_type)),
            "no-raise openings must remain explicit alternatives"
        );
    }

    // Isolate the receipt/binder contract from where a catalogue schedules its
    // discretionary boundary. Production raises now share the explicit opening
    // and retain its play reconsideration; this fixture deliberately puts the
    // boundary immediately after a bid to test accepted NewBid installation.
    let raising = Program {
        name: "bid-receipt-boundary-fixture",
        version: 1,
        start: Stage(2),
        nodes: vec![
            Node::Tail,
            Node::Reconsider {
                remaining: 1,
                retained: Stage(0),
            },
            Node::Action {
                actions: Actions::Raises,
                next: Stage(1),
            },
        ],
    };
    let Frontier::Actions(offers) = inspect(&raising, raising.start, &view, legal, &mind).unwrap().frontier else {
        panic!("raise fixture binding expected")
    };

    let selected = offers.last().unwrap().clone();
    let mut receipt = Receipt::new(raising.clone(), &view).unwrap();
    receipt.propose(selected.clone(), &view, legal).unwrap();
    let pending = receipt.clone();
    receipt.synchronize(&view).unwrap();
    assert_eq!(receipt, pending, "unaccepted retry cannot spend the raise");
    let mut wrong_table = table.clone();
    wrong_table.apply(offers[0].action);
    assert_eq!(
        receipt.synchronize(&wrong_table.view(seat)),
        Err(SequenceError::HistoryChanged)
    );
    assert_eq!(receipt, pending);

    table.apply(selected.action);
    assert_eq!(table.game.state.value.0, engine::MAX_BID);
    assert_eq!(table.game.state.phase, engine::GamePhase::Trick);
    receipt.synchronize(&table.view(seat)).unwrap();
    assert_eq!(receipt.stage, selected.next);
    assert!(receipt.pending.is_none());
    let after_raise = receipt.clone();
    receipt.synchronize(&table.view(seat)).unwrap();
    assert_eq!(receipt, after_raise, "accepted raise advances exactly once");

    let legal = table.to_act().unwrap().1;
    let view = table.view(seat);
    let mind = reader(&view);
    let reading = inspect(&receipt.program, receipt.stage, &view, legal, &mind).unwrap();
    let Frontier::Reconsider {
        stage, remaining: 1, ..
    } = reading.frontier
    else {
        panic!("future boundary expected")
    };
    assert!(matches!(
        &reading.consumed[0].meaning,
        ReadMeaning::Reconsider { remaining: 1, .. }
    ));
    let binding = bind_reconsideration(&receipt.program, stage, &view, legal).unwrap();
    assert!(
        binding
            .program
            .nodes
            .iter()
            .all(|node| !matches!(node, Node::Reconsider { .. })),
        "all successors exhaust the one permitted global expansion"
    );
    assert!(
        binding.program.nodes.iter().all(|node| !matches!(
            node,
            Node::Action {
                actions: Actions::Raises,
                ..
            }
        )),
        "engine already consumed the raise right"
    );
    let actual = binding
        .alternatives
        .iter()
        .map(|stage| binding.program.suffix(*stage).unwrap())
        .collect::<Vec<_>>();
    for program in authored_programs_with_reconsideration(&view, legal, 0).unwrap() {
        assert!(actual.contains(&program.suffix(program.start).unwrap()));
    }
    assert!(binding.alternatives.contains(&binding.retained));
    for (index, node) in binding.program.nodes.iter().enumerate() {
        assert!(
            !binding.program.nodes[..index].contains(node),
            "identical semantic nodes share storage"
        );
    }
    let mut stale = binding.clone();
    stale.legal.clear();
    assert_eq!(
        receipt.install_reconsideration(&stale, &view, legal, &mind),
        Err(SequenceError::WrongContext)
    );
    let mut stale = binding.clone();
    stale.information.series = SeriesScore::default();
    stale.information.events.clear();
    assert_eq!(
        receipt.install_reconsideration(&stale, &view, legal, &mind),
        Err(SequenceError::WrongContext)
    );
    assert_eq!(receipt, after_raise);
    receipt.install_reconsideration(&binding, &view, legal, &mind).unwrap();
    assert_eq!(receipt.program, binding.program);
    assert_eq!(receipt.stage, binding.program.start);
    assert!(
        receipt.pending.is_none(),
        "installation is not a strategic choice or accepted action"
    );
    // Simulate the evaluator selecting an explicit no-raise card branch from
    // this new program. Its Offer.next must refer to the installed DAG.
    let stage = *binding
        .alternatives
        .iter()
        .find(|stage| {
            matches!(
                &receipt.program.nodes[stage.0],
                Node::Action {
                    actions: Actions::Exact(ActionType::CardPlayed(_)),
                    ..
                }
            )
        })
        .unwrap();
    receipt.stage = stage;
    let Frontier::Actions(next) = inspect(&receipt.program, stage, &view, legal, &mind).unwrap().frontier else {
        panic!("selected opening must bind")
    };
    let next = next[0].clone();
    receipt.propose(next.clone(), &view, legal).unwrap();
    table.apply(next.action);
    receipt.synchronize(&table.view(seat)).unwrap();
    assert_eq!(receipt.stage, next.next);
    assert!(matches!(receipt.program.nodes[receipt.stage.0], Node::Tail));
}

#[test]
fn reconsideration_identity_retains_resource_edges_and_zero_removes_global_expansion() {
    let plain = Program {
        name: "two-actions",
        version: 1,
        start: Stage(2),
        nodes: vec![
            Node::Tail,
            Node::Action {
                actions: Actions::Exact(ActionType::CardPlayed(Card::from_index(1))),
                next: Stage(0),
            },
            Node::Action {
                actions: Actions::Exact(ActionType::CardPlayed(Card::from_index(2))),
                next: Stage(1),
            },
        ],
    };
    let bounded = with_reconsideration(&plain, 1).unwrap();
    assert_ne!(
        bounded.suffix(bounded.start).unwrap(),
        plain.suffix(plain.start).unwrap()
    );
    let stripped = with_reconsideration(&bounded, 0).unwrap();
    assert_eq!(
        stripped.suffix(stripped.start).unwrap(),
        plain.suffix(plain.start).unwrap()
    );
    let again = with_reconsideration(&bounded, 1).unwrap();
    assert_eq!(
        again.suffix(again.start).unwrap(),
        bounded.suffix(bounded.start).unwrap()
    );
    let stage = bounded
        .nodes
        .iter()
        .position(|node| {
            matches!(node,
        Node::Reconsider { retained, .. } if matches!(bounded.nodes[retained.0], Node::Action { .. }))
        })
        .unwrap();
    let mut changed = bounded.clone();
    let tail = Stage(
        changed
            .nodes
            .iter()
            .position(|node| matches!(node, Node::Tail))
            .unwrap(),
    );
    changed.nodes[stage] = Node::Reconsider {
        remaining: 1,
        retained: tail,
    };
    assert_ne!(
        changed.suffix(Stage(stage)).unwrap(),
        bounded.suffix(Stage(stage)).unwrap()
    );
    changed.nodes[stage] = Node::Reconsider {
        remaining: 0,
        retained: tail,
    };
    let table = table();
    let (seat, legal) = table.to_act().unwrap();
    assert_eq!(
        bind_reconsideration(&changed, Stage(stage), &table.view(seat), legal),
        Err(SequenceError::Budget)
    );
    assert_eq!(
        with_reconsideration(&plain, MAX_RECONSIDERATIONS + 1),
        Err(SequenceError::InvalidProgram)
    );
}

#[test]
fn finite_composition_exhaustion_defers_without_returning_a_partial_catalogue() {
    // A broad, shallow syntactically valid DAG avoids a recursion-depth test.
    // Distinct valid card masks prevent folding of these resource paths. This
    // exercises structural admission, not a claim that every route is playable.
    let mut nodes = vec![Node::Tail];
    let mut roots = Vec::new();
    for route in 0..MAX_ALTERNATIVES {
        let mut next = Stage(0);
        for _ in 0..31 {
            let stage = Stage(nodes.len());
            nodes.push(Node::Action {
                actions: Actions::Cards(CardSet(route as u64 + 1)),
                next,
            });
            next = stage;
        }
        roots.push(next);
    }
    let start = Stage(nodes.len());
    nodes.push(Node::Choice(roots));
    let program = Program {
        name: "structural-bound-fixture",
        version: 1,
        start,
        nodes,
    };
    program.validate().unwrap();
    assert_eq!(with_reconsideration(&program, 1), Err(SequenceError::Budget));
}

#[test]
fn current_catalogue_binding_preserves_a_still_legal_non_catalogue_retained_route() {
    let table = table();
    let (seat, legal) = table.to_act().unwrap();
    let view = table.view(seat);
    let mind = reader(&view);
    let first = *legal
        .iter()
        .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
        .unwrap();
    let program = Program {
        name: "retained-resource-proof",
        version: 1,
        start: Stage(3),
        nodes: vec![
            Node::Tail,
            Node::Action {
                actions: Actions::Exact(first.action_type),
                next: Stage(0),
            },
            Node::Guard {
                predicate: Guard::OwnCards(view.knowledge.certain(seat)),
                yes: Stage(1),
                no: Stage(0),
                unknown: Stage(0),
            },
            Node::Reconsider {
                remaining: 1,
                retained: Stage(2),
            },
        ],
    };
    let retained = program.suffix(Stage(2)).unwrap();
    let catalogue = authored_programs_with_reconsideration(&view, legal, 0).unwrap();
    assert!(!catalogue.iter().any(|p| p.suffix(p.start).unwrap() == retained));
    let binding = bind_reconsideration(&program, program.start, &view, legal).unwrap();
    assert_eq!(binding.program.suffix(binding.retained).unwrap(), retained);
    let actual = binding
        .alternatives
        .iter()
        .map(|stage| binding.program.suffix(*stage).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(actual.len(), catalogue.len() + 1);
    for candidate in catalogue {
        assert!(actual.contains(&candidate.suffix(candidate.start).unwrap()));
    }
    let reading = inspect(&binding.program, binding.retained, &view, legal, &mind).unwrap();
    assert!(matches!(reading.frontier, Frontier::Actions(ref offers) if offers.iter().any(|o| o.action == first)));
    assert!(reading.consumed.iter().any(|read| matches!(
        &read.meaning,
        ReadMeaning::Guard {
            predicate: Guard::OwnCards(_),
            truth: Some(true)
        }
    )));
}

#[test]
fn reached_frontier_install_matches_guarded_install_and_rejects_stale_bindings_atomically() {
    let table = table();
    let (seat, legal) = table.to_act().unwrap();
    let view = table.view(seat);
    let mind = reader(&view);
    let program = Program {
        name: "guarded-reconsideration",
        version: 1,
        start: Stage(2),
        nodes: vec![
            Node::Tail,
            Node::Reconsider {
                remaining: 1,
                retained: Stage(0),
            },
            Node::Guard {
                predicate: Guard::OwnCards(view.knowledge.certain(seat)),
                yes: Stage(1),
                no: Stage(0),
                unknown: Stage(0),
            },
        ],
    };
    let binding = bind_reconsideration(&program, Stage(1), &view, legal).unwrap();
    let mut guarded = Receipt::new(program, &view).unwrap();
    let original = guarded.clone();
    assert_eq!(
        guarded.install_reconsideration_at_frontier(&binding, &view, legal),
        Err(SequenceError::WrongContext)
    );
    assert_eq!(guarded, original, "direct installation cannot skip a guard");
    let mut direct = guarded.clone();
    direct.stage = Stage(1); // Evaluator already consumed the guard.
    let reached = direct.clone();
    let mut stale = binding.clone();
    stale.source = binding.program.suffix(binding.program.start).unwrap();
    assert_eq!(
        guarded.install_reconsideration(&stale, &view, legal, &mind),
        Err(SequenceError::WrongContext)
    );
    assert_eq!(guarded, original, "failed public installation restores its cursor");
    for stale in [
        stale,
        {
            let mut b = binding.clone();
            b.legal.clear();
            b
        },
        {
            let mut b = binding.clone();
            b.alternatives.clear();
            b
        },
        {
            let mut b = binding.clone();
            b.information.events.clear();
            b
        },
    ] {
        assert_eq!(
            direct.install_reconsideration_at_frontier(&stale, &view, legal),
            Err(SequenceError::WrongContext)
        );
        assert_eq!(direct, reached);
    }
    direct
        .install_reconsideration_at_frontier(&binding, &view, legal)
        .unwrap();
    guarded.install_reconsideration(&binding, &view, legal, &mind).unwrap();
    assert_eq!(direct, guarded);
    assert_eq!(direct.stage, binding.program.start);
    assert_eq!(direct.program, binding.program);
    assert!(direct.pending.is_none());
}

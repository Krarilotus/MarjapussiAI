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
            p.name == "standing-lead-activation-bound-side"
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
        assert!(
            !program.nodes.iter().any(|node| matches!(
                node,
                Node::Action {
                    actions: Actions::Cards(_),
                    ..
                }
            )),
            "no unrestricted future card-choice product"
        );
    }
}

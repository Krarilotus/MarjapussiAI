use super::*;
use agent::Table;
use beliefs::BeliefParams;
use engine::Card;

fn table() -> Table {
    let hands = std::array::from_fn(|seat| {
        (0..engine::HAND_SIZE)
            .map(|index| Card::from_index((index * 4 + seat) as u8))
            .collect()
    });
    Table::new(&hands)
}

fn reader(table: &Table, seat: u8) -> Mind {
    let view = table.view(seat);
    let mut mind = Mind::new(
        seat,
        view.knowledge.dealt_certain(seat),
        view.kinds,
        BeliefParams::base_only(),
    );
    for event in view.events {
        mind.apply(event).unwrap();
    }
    assert_eq!(mind.knowledge(), view.knowledge);
    mind
}

fn exact(kind: ActionType) -> Program {
    Program {
        name: "preplay-receipt",
        version: 1,
        start: Stage(1),
        nodes: vec![
            Node::Deferred,
            Node::Action {
                actions: Actions::Exact(kind),
                next: Stage(0),
            },
        ],
    }
}

#[test]
fn play_gate_uses_only_owned_legal_actions_and_expiration_is_an_explicit_frontier() {
    let table = table();
    let (seat, legal) = table.to_act().unwrap();
    let view = table.view(seat);
    let mind = reader(&table, seat);
    // Shape witness only: the evaluator owns committing the modeled response
    // before reaching each successor. Sixteen is the approved bridge bound.
    let mut nodes = vec![Node::Deferred, Node::Tail];
    let mut next = Stage(nodes.len());
    nodes.push(Node::Guard {
        predicate: Guard::PlayReady,
        yes: Stage(1),
        no: Stage(0),
        unknown: Stage(0),
    });
    for _ in 0..16 {
        let response = Stage(nodes.len());
        nodes.push(Node::Response { next });
        next = Stage(nodes.len());
        nodes.push(Node::Guard {
            predicate: Guard::PlayReady,
            yes: Stage(1),
            no: response,
            unknown: Stage(0),
        });
    }
    let program = Program {
        name: "finite-phase-gate-fixture",
        version: 1,
        start: next,
        nodes,
    };
    program.validate().unwrap();
    let mut stage = program.start;
    for _ in 0..16 {
        let read = inspect(&program, stage, &view, legal, &mind).unwrap();
        assert!(read.consumed.iter().any(|r| matches!(
            r.meaning,
            ReadMeaning::Guard {
                predicate: Guard::PlayReady,
                truth: Some(false)
            }
        )));
        let Frontier::Response { next } = read.frontier else {
            panic!("explicit modeled response required")
        };
        assert!(matches!(read.consumed.last().unwrap().meaning, ReadMeaning::Response { next: edge } if edge == next));
        stage = next;
    }
    let exhausted = inspect(&program, stage, &view, legal, &mind).unwrap();
    assert_eq!(exhausted.frontier, Frontier::Deferred);
    assert_eq!(exhausted.consumed.last().unwrap().meaning, ReadMeaning::Deferred);
    assert!(!exhausted
        .consumed
        .iter()
        .any(|r| r.meaning == ReadMeaning::ResponseLeaf));

    let mut foreign = legal.to_vec();
    foreign.push(GameAction {
        player: engine::PlaceAtTable(engine::partner(seat)),
        action_type: ActionType::CardPlayed(Card::from_index(0)),
    });
    assert!(matches!(
        inspect(&program, program.start, &view, &foreign, &mind)
            .unwrap()
            .frontier,
        Frontier::Response { .. }
    ));

    let mut playing = table.clone();
    while playing.game.state.phase == engine::GamePhase::Bidding {
        let action = *playing
            .to_act()
            .unwrap()
            .1
            .iter()
            .find(|a| a.action_type == ActionType::StopBidding)
            .unwrap();
        playing.apply(action);
    }
    let (seat, legal) = playing.to_act().unwrap();
    assert!(legal.iter().any(|a| matches!(a.action_type, ActionType::CardPlayed(_))));
    let ready = inspect(
        &program,
        program.start,
        &playing.view(seat),
        legal,
        &reader(&playing, seat),
    )
    .unwrap();
    assert_eq!(ready.frontier, Frontier::Tail);
    assert!(matches!(
        ready.consumed[0].meaning,
        ReadMeaning::Guard {
            predicate: Guard::PlayReady,
            truth: Some(true)
        }
    ));
}

#[test]
fn response_edges_are_canonical_and_do_not_replenish_play_reconsideration() {
    let program = Program {
        name: "response",
        version: 1,
        start: Stage(2),
        nodes: vec![
            Node::Deferred,
            Node::Action {
                actions: Actions::Exact(ActionType::StopBidding),
                next: Stage(0),
            },
            Node::Response { next: Stage(1) },
        ],
    };
    program.validate().unwrap();
    let bounded = with_reconsideration(&program, 1).unwrap();
    assert_eq!(
        bounded.suffix(bounded.start).unwrap(),
        program.suffix(program.start).unwrap()
    );
    let renamed = Program {
        name: "same-response",
        version: 2,
        start: Stage(0),
        nodes: vec![
            Node::Response { next: Stage(2) },
            Node::Deferred,
            Node::Action {
                actions: Actions::Exact(ActionType::StopBidding),
                next: Stage(1),
            },
        ],
    };
    assert_eq!(
        program.suffix(program.start).unwrap(),
        renamed.suffix(renamed.start).unwrap()
    );
    let mut changed = program.clone();
    changed.nodes[2] = Node::Response { next: Stage(0) };
    assert_ne!(
        program.suffix(program.start).unwrap(),
        changed.suffix(changed.start).unwrap()
    );
    changed.nodes[2] = Node::Response { next: Stage(2) };
    assert_eq!(changed.validate(), Err(SequenceError::InvalidProgram));
    changed.nodes[2] = Node::Response { next: Stage(3) };
    assert_eq!(changed.validate(), Err(SequenceError::InvalidProgram));
    changed.nodes[2] = Node::Response { next: Stage(0) };
    changed.nodes[0] = Node::Tail;
    assert_ne!(
        program.suffix(Stage(0)).unwrap(),
        changed.suffix(Stage(0)).unwrap(),
        "exhaustion is not a legacy leaf"
    );
}

#[test]
fn stop_bidding_receipt_accepts_only_own_matching_event_once() {
    let mut table = table();
    let (seat, legal) = table.to_act().unwrap();
    let action = *legal.iter().find(|a| a.action_type == ActionType::StopBidding).unwrap();
    let mut receipt = Receipt::new(exact(ActionType::StopBidding), &table.view(seat)).unwrap();
    receipt
        .propose(Offer { action, next: Stage(0) }, &table.view(seat), legal)
        .unwrap();
    let pending = receipt.clone();
    receipt.synchronize(&table.view(seat)).unwrap();
    assert_eq!(receipt, pending);
    table.apply(action);
    let mut events = table.view(seat).events.to_vec();
    events[pending.accepted.events.len()] = SeatEvent::StopBidding {
        seat: engine::partner(seat),
    };
    let mut wrong = table.view(seat);
    wrong.events = &events;
    assert_eq!(receipt.synchronize(&wrong), Err(SequenceError::HistoryChanged));
    assert_eq!(receipt, pending);
    receipt.synchronize(&table.view(seat)).unwrap();
    assert_eq!(receipt.stage, Stage(0));
    assert!(receipt.pending.is_none());
    let accepted = receipt.clone();
    receipt.synchronize(&table.view(seat)).unwrap();
    assert_eq!(receipt, accepted);
}

#[test]
fn first_and_return_pass_receipts_require_own_unredacted_exact_card_set() {
    let mut table = table();
    let bid = *table
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| a.action_type == ActionType::NewBid(120))
        .unwrap();
    table.apply(bid);
    while table.game.state.phase == engine::GamePhase::Bidding {
        let action = *table
            .to_act()
            .unwrap()
            .1
            .iter()
            .find(|a| a.action_type == ActionType::StopBidding)
            .unwrap();
        table.apply(action);
    }
    for _ in 0..2 {
        let (seat, legal) = table.to_act().unwrap();
        let action = *legal
            .iter()
            .find(|a| matches!(a.action_type, ActionType::Pass(_)))
            .unwrap();
        let ActionType::Pass(cards) = action.action_type else {
            unreachable!()
        };
        let passed = CardSet::from_cards(&cards);
        let mut receipt = Receipt::new(exact(action.action_type), &table.view(seat)).unwrap();
        receipt
            .propose(Offer { action, next: Stage(0) }, &table.view(seat), legal)
            .unwrap();
        let pending = receipt.clone();
        receipt.synchronize(&table.view(seat)).unwrap();
        assert_eq!(receipt, pending, "rejected/unaccepted proposal spends nothing");
        table.apply(action);
        let wrong_card = CardSet::single((CardSet::ALL - passed).iter().next().unwrap());
        let wrong_cards = (passed - CardSet::single(passed.iter().next().unwrap())) | wrong_card;
        for event in [
            SeatEvent::Pass {
                from: seat,
                to: engine::partner(seat),
                cards: None,
            },
            SeatEvent::Pass {
                from: engine::partner(seat),
                to: seat,
                cards: Some(passed),
            },
            SeatEvent::Pass {
                from: seat,
                to: (seat + 1) % 4,
                cards: Some(passed),
            },
            SeatEvent::Pass {
                from: seat,
                to: engine::partner(seat),
                cards: Some(wrong_cards),
            },
        ] {
            let mut events = table.view(seat).events.to_vec();
            events[pending.accepted.events.len()] = event;
            let mut wrong = table.view(seat);
            wrong.events = &events;
            assert_eq!(receipt.synchronize(&wrong), Err(SequenceError::HistoryChanged));
            assert_eq!(receipt, pending);
        }
        receipt.synchronize(&table.view(seat)).unwrap();
        assert_eq!(receipt.stage, Stage(0));
        assert!(receipt.pending.is_none());
        let accepted = receipt.clone();
        receipt.synchronize(&table.view(seat)).unwrap();
        assert_eq!(receipt, accepted);
    }
}

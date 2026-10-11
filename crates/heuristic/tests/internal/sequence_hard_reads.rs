use super::*;
use crate::concepts::sequence::{Guard, Node};
use crate::concepts::value::CohortMethod;
use beliefs::{BeliefParams, Concept, ConceptBudget, ConceptQuery};
use engine::Card;

fn branch(table: Table, query: ConceptQuery) -> Branch {
    let program = Program {
        name: "hard-proof-route",
        version: 1,
        start: Stage(3),
        nodes: vec![
            Node::Tail,
            Node::Deferred,
            Node::Response { next: Stage(0) },
            Node::Guard {
                predicate: Guard::Known(query),
                yes: Stage(0),
                no: Stage(1),
                unknown: Stage(2),
            },
        ],
    };
    let receipt = Receipt::new(program, &table.view(0)).unwrap();
    Branch {
        table,
        origin: 0,
        weight: 1.0,
        receipt,
        first: None,
        first_program: None,
        tricks: Vec::new(),
        consumed: Vec::new(),
        interrupted: false,
    }
}

#[test]
fn evaluator_hard_guard_preserves_hidden_swap_unknown_and_truth_routes() {
    let mut hands: [Vec<Card>; 4] =
        std::array::from_fn(|seat| (0..9).map(|i| Card::from_index((9 * seat + i) as u8)).collect());
    let original = Table::new(&hands);
    let card = hands[1][0];
    let other = hands[2][0];
    hands[1][0] = other;
    hands[2][0] = card;
    let swapped = Table::new(&hands);
    assert_eq!(Information::of(&original.view(0)), Information::of(&swapped.view(0)));
    let query = ConceptQuery::new(Concept::Card {
        seat: 1,
        card: card.index(),
    });
    let mut a = branch(original, query.clone());
    let b = branch(swapped, query);
    let provenance = Provenance {
        belief_model: "hard-read-fixture",
        response_model: "must-not-run",
        cohort: CohortMethod::Enumerated,
    };
    let context = Context {
        params: crate::Params::default(),
        seed: 1,
        posterior_revision: 0,
        provenance,
    };
    let mut leaf = |_: &SeatView<'_>, _: &[GameAction], _: LeafBudget| -> Result<LeafResponse, LeafFailure> {
        panic!("proof inspection must not invoke response policy")
    };
    let mut session = Session {
        root: 0,
        provenance,
        context: &context,
        limits: Limits::default(),
        work: Work::default(),
        leaf: &mut leaf,
        responses: Vec::new(),
        hard_reads: Vec::new(),
        memo: Vec::new(),
        tails: Vec::new(),
        canonical: Vec::new(),
        memberships: Vec::new(),
        reusable_bytes: 0,
        response_bytes: 0,
        limit: None,
    };
    // Compare the real evaluator dispatcher with the old filtered reader route.
    let view = a.table.view(0);
    let mut reader = Mind::new(
        0,
        view.knowledge.dealt_certain(0),
        view.kinds,
        BeliefParams::base_only(),
    );
    for event in view.events {
        reader.apply(event).unwrap();
    }
    let expected = inspect(
        &a.receipt.program,
        a.receipt.stage,
        &view,
        a.table.to_act().unwrap().1,
        &reader,
    )
    .unwrap();
    assert_eq!(session.inspect(&a).unwrap(), expected);
    assert_eq!(expected.frontier, Frontier::Response { next: Stage(0) });
    assert_eq!(session.inspect(&b).unwrap(), expected);
    assert_eq!(session.hard_reads.len(), 1);
    assert_eq!(session.hard_reads[0].2, None);
    for (seat, expected) in [(0, Frontier::Tail), (1, Frontier::Deferred)] {
        let query = ConceptQuery::new(Concept::Card { seat, card: 0 });
        assert_eq!(
            query.hard(a.table.view(0).knowledge),
            reader.concept(&query, ConceptBudget { max_nodes: 0 })
        );
        a.receipt.program.nodes[3] = Node::Guard {
            predicate: Guard::Known(query),
            yes: Stage(0),
            no: Stage(1),
            unknown: Stage(2),
        };
        assert_eq!(session.inspect(&a).unwrap().frontier, expected);
    }
    let mut conditional = ConceptQuery::new(Concept::Card {
        seat: 1,
        card: card.index(),
    });
    conditional.given = conditional.event.clone();
    a.receipt.program.nodes[3] = Node::Guard {
        predicate: Guard::Known(conditional),
        yes: Stage(0),
        no: Stage(1),
        unknown: Stage(2),
    };
    assert_eq!(session.inspect(&a), Err(SequenceError::InvalidProgram));
    assert_eq!(session.work.response_calls, 0);
    assert_eq!(session.work.transitions, 0);
}

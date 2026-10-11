//! Structural enclosure and ordinary executor checks; the authentic composed
//! final_cash descendant fixture is owned separately, not substituted here.
use super::*;

fn tail() -> Suffix {
    Suffix {
        start: Stage(0),
        nodes: vec![Node::Tail],
    }
}

fn cash_suffix(cards: CardSet) -> Suffix {
    Suffix {
        start: Stage(3),
        nodes: vec![
            Node::Tail,
            Node::Action {
                actions: Actions::Standing(cards),
                next: Stage(0),
            },
            Node::Choice(vec![Stage(0), Stage(1), Stage(1)]),
            Node::Guard {
                predicate: Guard::OwnLead,
                yes: Stage(2),
                no: Stage(0),
                unknown: Stage(0),
            },
        ],
    }
}

#[test]
fn scans_shared_card_tail_dag_without_widening_incumbent() {
    let table = fixture();
    let legal = table.to_act().unwrap().1;
    let held = table.hand(0);
    let suffix = cash_suffix(held);
    assert!(card_tail_suffix(&suffix));
    assert!(competitor_admitted(&Selection::Stage(suffix.clone()), 0, legal));
    assert!(!incumbent_admitted(&Selection::Stage(suffix), 0, legal));
    assert!(incumbent_admitted(&Selection::Stage(tail()), 0, legal));
    for actions in [
        Actions::Cards(held),
        Actions::Standing(held),
        Actions::Exact(cards(&table)[0]),
    ] {
        let suffix = Suffix {
            start: Stage(1),
            nodes: vec![
                Node::Tail,
                Node::Action {
                    actions,
                    next: Stage(0),
                },
            ],
        };
        assert!(card_tail_suffix(&suffix));
    }
    // Only reachable semantic operators are enclosed; normal validation still
    // checks all nodes, even when outside the compared suffix's reachable DAG.
    let mut unused_gate = tail();
    unused_gate.nodes.push(Node::Deferred);
    assert!(card_tail_suffix(&unused_gate));
}

#[test]
fn every_guard_arm_and_choice_entry_including_aliases_must_be_enclosed() {
    let table = fixture();
    for arm in 0..3 {
        let mut suffix = cash_suffix(table.hand(0));
        suffix.nodes.push(Node::Deferred);
        let Node::Guard { yes, no, unknown, .. } = &mut suffix.nodes[3] else {
            unreachable!()
        };
        match arm {
            0 => *yes = Stage(4),
            1 => *no = Stage(4),
            _ => *unknown = Stage(4),
        }
        assert!(!card_tail_suffix(&suffix), "no current truth can hide arm {arm}");
    }
    let mut suffix = cash_suffix(table.hand(0));
    suffix.nodes.push(Node::Deferred);
    let Node::Choice(children) = &mut suffix.nodes[2] else {
        unreachable!()
    };
    children.push(Stage(4));
    assert!(!card_tail_suffix(&suffix));
}

#[test]
fn unsupported_operators_and_malformed_or_cyclic_programs_refuse_enclosure() {
    let table = fixture();
    let card = table.hand(0).cards()[0];
    for node in [
        Node::Deferred,
        Node::Response { next: Stage(0) },
        Node::Reconsider {
            remaining: 1,
            retained: Stage(0),
        },
        Node::Action {
            actions: Actions::Raises,
            next: Stage(0),
        },
        Node::Action {
            actions: Actions::Exact(ActionType::StopBidding),
            next: Stage(0),
        },
        Node::Action {
            actions: Actions::Exact(ActionType::Question(engine::QuestionType::Yours)),
            next: Stage(0),
        },
        Node::Action {
            actions: Actions::Exact(ActionType::UndoRequest),
            next: Stage(0),
        },
        Node::Action {
            actions: Actions::Cards(CardSet::EMPTY),
            next: Stage(0),
        },
        Node::Choice(vec![]),
        Node::Choice(vec![Stage(9)]),
        Node::Choice(vec![Stage(1)]),
        Node::Action {
            actions: Actions::Exact(ActionType::CardPlayed(card)),
            next: Stage(1),
        },
    ] {
        assert!(!card_tail_suffix(&Suffix {
            start: Stage(1),
            nodes: vec![Node::Tail, node]
        }));
    }
    assert!(!card_tail_suffix(&Suffix {
        start: Stage(1),
        nodes: vec![Node::Tail]
    }));
    // A Tail elsewhere is not sufficient if the reachable component cycles.
    assert!(!card_tail_suffix(&Suffix {
        start: Stage(1),
        nodes: vec![Node::Tail, Node::Choice(vec![Stage(2)]), Node::Choice(vec![Stage(1)])]
    }));
    let mut unreachable_cycle = tail();
    unreachable_cycle.nodes.push(Node::Choice(vec![Stage(1)]));
    assert!(!card_tail_suffix(&unreachable_cycle));
}

#[test]
fn offers_require_current_legal_root_card_and_literal_tail_stays_eligible() {
    let table = fixture();
    let legal = table.to_act().unwrap().1;
    let action = *legal
        .iter()
        .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
        .unwrap();
    let next = cash_suffix(table.hand(0));
    assert!(competitor_admitted(
        &Selection::Offer {
            action,
            next: next.clone()
        },
        0,
        legal
    ));
    assert!(!incumbent_admitted(
        &Selection::Offer {
            action,
            next: next.clone()
        },
        0,
        legal
    ));
    assert!(!competitor_admitted(&Selection::Offer { action, next }, 1, legal));
    // Tail is an existing response boundary; the authored card restriction
    // must not apply to its legal Question/Announce/Answer leaf policy.
    assert!(card_tail_suffix(&tail()));
}

#[test]
fn literal_tail_incumbent_is_actual_completion_and_competitors_remain_bounds_only() {
    let table = fixture();
    let mut candidate = local(&cards(&table));
    let Node::Guard { yes, .. } = candidate.program.nodes[candidate.stage.0] else {
        panic!("wrapper")
    };
    let guarded = Stage(candidate.program.nodes.len());
    candidate.program.nodes.push(Node::Guard {
        predicate: Guard::OwnLead,
        yes: Stage(1),
        no: Stage(0),
        unknown: Stage(0),
    });
    let standing = Stage(candidate.program.nodes.len());
    candidate.program.nodes.push(Node::Action {
        actions: Actions::Standing(table.hand(0)),
        next: Stage(0),
    });
    let shared = Stage(candidate.program.nodes.len());
    candidate
        .program
        .nodes
        .push(Node::Choice(vec![standing, Stage(2), standing]));
    let Node::Choice(children) = &mut candidate.program.nodes[yes.0] else {
        panic!("choice")
    };
    *children = vec![Stage(0), guarded, shared];
    let context = context();
    let actual = run(&candidate, &[(table.clone(), 1.0)], &context, false);
    let choice = actual
        .prepared()
        .iter()
        .find(|c| matches!(c.proof, Some(SelectionProof::CompletionDominance(_))))
        .unwrap();
    assert_eq!(choice.selected, 0);
    assert_eq!(choice.alternatives.len(), 3);
    assert!(matches!(&choice.alternatives[0], Selection::Stage(s) if literal_tail(s)));
    assert!(choice.assessment().is_none());
    assert!(proof(choice).validates(choice));
    assert_eq!(
        proof(choice).values().selected_value.difference.to_bits(),
        140.0_f64.to_bits()
    );
    assert!(actual.selection.root().assessment().unwrap().cohort_complete);
    assert_eq!(actual.executions[0].terminals.len(), 1);
    assert!(!actual.executions[0].terminals[0].interrupted);
    assert_eq!(actual.executions[0].terminals[0].outcome.team_points, [140, 0]);
    assert!(actual.prepared().iter().all(|c| c.scope != ChoiceScope::FixedAction));
}

#[test]
fn empty_binding_uses_real_interrupted_tail_not_fabricated_terminal_or_offer() {
    let table = fixture();
    let missing = CardSet::ALL - table.hand(0);
    assert!(!missing.is_empty());
    let program = Program {
        name: "empty-binding-enclosure",
        version: 1,
        start: Stage(1),
        nodes: vec![
            Node::Tail,
            Node::Action {
                actions: Actions::Cards(missing),
                next: Stage(0),
            },
        ],
    };
    assert!(card_tail_suffix(&program.suffix(program.start).unwrap()));
    let candidate = Candidate::new(program);
    let context = context();
    let mut leaf = leaf;
    let mut session = session(&context, &mut leaf);
    let branch = Branch {
        receipt: Receipt::new(candidate.program, &table.view(0)).unwrap(),
        table,
        origin: 7,
        weight: 0.75,
        first: None,
        first_program: None,
        tricks: vec![],
        consumed: vec![],
        interrupted: false,
    };
    let mut prepared = vec![];
    let terminals = session.finish(vec![branch], &mut prepared).unwrap();
    assert_eq!(terminals.len(), 1);
    assert!(terminals[0].interrupted);
    assert_eq!(terminals[0].origin, 7);
    assert_eq!(terminals[0].weight.to_bits(), 0.75_f64.to_bits());
    assert_eq!(terminals[0].outcome.team_points, [140, 0]);
    assert!(terminals[0].consumed.iter().any(|read| matches!(
        &read.reading,
        ReadMeaning::Binding { actions: Actions::Cards(cards), legal_members }
            if *cards == missing && legal_members.is_empty()
    )));
    assert!(terminals[0]
        .consumed
        .iter()
        .any(|read| read.reading == ReadMeaning::ResponseLeaf));
    assert!(prepared.is_empty());
}

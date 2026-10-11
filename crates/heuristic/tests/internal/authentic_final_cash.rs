//! Scripted reachability plus a real Local comparison, NOT completion of the
//! initial Parent/Global gate. Uses the original two-world fixture unchanged.
use super::*;
use crate::concepts::sequence::{
    authored_programs_with_reconsideration, bind_reconsideration, Actions, Guard, Program,
};
use sequence::{descendant_probe, ChoiceScope, Selection, SelectionProof};

fn original_limits() -> Limits {
    Limits {
        transitions: 500_000,
        comparisons: 32_768,
        memo_bytes: 64 * 1024 * 1024,
        ..Limits::default()
    }
}

// Traverse only current pure dependencies and an existing ordered Choice edge.
// This is a fixture script, not a value-based selection or minted certificate.
fn scripted_choice(table: &Table, receipt: &mut Receipt, index: usize) -> Vec<Stage> {
    let view = table.view(0);
    let legal = crate::legal_actions::eligible(table.to_act().unwrap().1);
    let inspected = inspect(&receipt.program, receipt.stage, &view, &legal, &reader(&view)).unwrap();
    let Frontier::Choice(alternatives) = inspected.frontier else {
        panic!("script must reach an actual authored Choice")
    };
    receipt.stage = alternatives[index];
    alternatives
}

fn scripted_ace_trick(table: &mut Table, receipt: &mut Receipt, expected: [u8; 4], own: CardSet) {
    let original_program = receipt.program.clone();
    let view = table.view(0);
    let legal = crate::legal_actions::eligible(table.to_act().unwrap().1);
    let inspected = inspect(&receipt.program, receipt.stage, &view, &legal, &reader(&view)).unwrap();
    assert!(matches!(inspected.consumed.last().unwrap().meaning,
        crate::concepts::sequence::ReadMeaning::Binding { actions: Actions::Standing(cards), .. } if cards == own));
    let Frontier::Actions(offers) = inspected.frontier else {
        panic!("script must bind the authored Standing domain")
    };
    let offer = offers
        .into_iter()
        .find(|offer| offer.action.action_type == ActionType::CardPlayed(Card::from_index(expected[0])))
        .unwrap();
    receipt.propose(offer.clone(), &view, &legal).unwrap();
    assert!(table.game.is_legal(&offer.action));
    table.apply(offer.action);
    for card in expected.into_iter().skip(1) {
        let (actor, legal) = table.to_act().unwrap();
        assert_ne!(actor, 0);
        let legal = crate::legal_actions::eligible(legal);
        let action = scoped_response(
            &table.view(actor),
            &legal,
            sequence::LeafBudget {
                deadline: None,
                remaining_transitions: 1000,
                remaining_solver_nodes: None,
            },
        )
        .unwrap()
        .action;
        assert_eq!(action.action_type, ActionType::CardPlayed(Card::from_index(card)));
        assert!(table.game.is_legal(&action));
        table.apply(action);
    }
    assert_eq!(table.to_act().unwrap().0, 0);
    receipt.synchronize(&table.view(0)).unwrap();
    assert!(receipt.pending.is_none());
    assert_eq!(receipt.program, original_program);
}

fn reached_descendant() -> ([(Table, f64); 2], [Receipt; 2], Vec<Stage>) {
    let mut cohort = scoped_cohort();
    let receipts_and_domain = std::array::from_fn::<_, 2, _>(|origin| {
        let table = &mut cohort[origin].0;
        let view = table.view(0);
        let own = view.knowledge.certain(0);
        assert_eq!(own.len(), 6);
        let legal = crate::legal_actions::eligible(table.to_act().unwrap().1);
        let boundary = Program {
            name: "final-global-then-scoped-execution",
            version: 1,
            start: Stage(1),
            nodes: vec![
                Node::Tail,
                Node::Reconsider {
                    remaining: 1,
                    retained: Stage(0),
                },
            ],
        };
        let binding = bind_reconsideration(&boundary, boundary.start, &view, &legal).unwrap();
        let authored = authored_programs_with_reconsideration(&view, &legal, 0).unwrap();
        let mut expected_domain = authored.iter().map(|p| p.suffix(p.start).unwrap()).collect::<Vec<_>>();
        let retained_suffix = binding.program.suffix(binding.retained).unwrap();
        if !expected_domain.contains(&retained_suffix) {
            expected_domain.push(retained_suffix);
        }
        assert_eq!(
            binding
                .alternatives
                .iter()
                .map(|stage| binding.program.suffix(*stage).unwrap())
                .collect::<Vec<_>>(),
            expected_domain
        );
        let resource = authored
            .iter()
            .find(|p| p.name == "cash-activation-control-transfer-recovery")
            .unwrap();
        let resource_suffix = resource.suffix(resource.start).unwrap();
        let resource_stage = *binding
            .alternatives
            .iter()
            .find(|stage| binding.program.suffix(**stage).unwrap() == resource_suffix)
            .unwrap();
        // Preserve the complete original binding, its retained route and every
        // global alternative; select this path only for diagnostic reachability.
        assert!(binding.alternatives.contains(&binding.retained));
        let mut receipt = Receipt::new(boundary, &view).unwrap();
        receipt
            .install_reconsideration(&binding, &view, &legal, &reader(&view))
            .unwrap();
        let composed = receipt.program.clone();
        receipt.stage = resource_stage;
        scripted_choice(table, &mut receipt, 0); // no answer: actual development successor
        scripted_choice(table, &mut receipt, 0); // existing opening cash arm
        assert_eq!(scripted_choice(table, &mut receipt, 1).len(), 2);
        scripted_ace_trick(table, &mut receipt, [8, 7, 6, 5], own);
        assert_eq!(scripted_choice(table, &mut receipt, 1).len(), 2);
        scripted_ace_trick(table, &mut receipt, [17, 16, 15, 14], own);
        scripted_choice(table, &mut receipt, 0); // stop activation (no invented question rule)
        assert_eq!(scripted_choice(table, &mut receipt, 0).len(), 2); // no-trump transfer -> final_cash
        assert_eq!(scripted_choice(table, &mut receipt, 1).len(), 2);
        scripted_ace_trick(table, &mut receipt, [26, 25, 24, 23], own);
        assert_eq!(receipt.program, composed);
        assert!(matches!(
            receipt.program.nodes[receipt.stage.0],
            Node::Guard {
                predicate: Guard::OwnLead,
                ..
            }
        ));
        let view = table.view(0);
        let legal = crate::legal_actions::eligible(table.to_act().unwrap().1);
        let inspected = inspect(&receipt.program, receipt.stage, &view, &legal, &reader(&view)).unwrap();
        let Frontier::Choice(domain) = inspected.frontier else {
            panic!("final_cash inner gate must remain an authored Choice")
        };
        assert_eq!(domain.len(), 2);
        assert_eq!(receipt.program.nodes[domain[0].0], Node::Tail);
        assert!(matches!(receipt.program.nodes[domain[1].0],
            Node::Action { actions: Actions::Standing(cards), next } if cards == own && next == domain[0]));
        assert_eq!(table.game.state.all_tricks.len(), 6);
        assert_eq!(view.knowledge.hand_len(0), 3);
        assert_eq!(
            view.knowledge.certain(0),
            CardSet::from_cards(&[Card::from_index(4), Card::from_index(13), Card::from_index(22)])
        );
        (receipt, domain)
    });
    let [(left, domain), (right, other_domain)] = receipts_and_domain;
    assert_eq!(domain, other_domain);
    assert_eq!(left, right); // same observer input, not identical private worlds
    assert_eq!(cohort[0].1.to_bits(), 0.4_f64.to_bits());
    assert_eq!(cohort[1].1.to_bits(), 0.6_f64.to_bits());
    (cohort, [left, right], domain)
}

#[test]
fn authored_final_cash_descendant_completes_tail_and_bounds_unexecuted_standing() {
    let (cohort, receipts, domain) = reached_descendant();
    let mut context = context();
    context.provenance.response_model = "synthetic-follow-high-root-low";
    let mut oracle_receipts = receipts.clone();
    for receipt in &mut oracle_receipts {
        receipt.stage = domain[0];
    }
    // Ordinary literal Tail, still inside the same owning Program and both real
    // worlds. This is an oracle for selected completion, not an alternative domain.
    let mut oracle_calls = Vec::new();
    let mut oracle_leaf = |view: &SeatView<'_>, legal: &[GameAction], budget| {
        let response = scoped_response(view, legal, budget)?;
        oracle_calls.push((Information::of(view), response.action));
        Ok(response)
    };
    let oracle =
        descendant_probe::finish(&cohort, &oracle_receipts, &context, original_limits(), &mut oracle_leaf).unwrap();
    let mut actual_calls = Vec::new();
    let mut actual_leaf = |view: &SeatView<'_>, legal: &[GameAction], budget| {
        let response = scoped_response(view, legal, budget)?;
        actual_calls.push((Information::of(view), response.action));
        Ok(response)
    };
    let actual = descendant_probe::finish(&cohort, &receipts, &context, original_limits(), &mut actual_leaf).unwrap();
    assert_eq!(
        actual_calls, oracle_calls,
        "bounded rivals must not execute response leaves"
    );
    assert_eq!(actual.work.transitions, oracle.work.transitions);
    assert_eq!(actual.work.represented_transitions, oracle.work.represented_transitions);
    assert_eq!(actual.work.response_calls, oracle.work.response_calls);
    assert_eq!(
        actual.work.represented_solver_nodes,
        oracle.work.represented_solver_nodes
    );
    assert_eq!(actual.terminals.len(), 2);
    for (origin, (terminal, reference)) in actual.terminals.iter().zip(&oracle.terminals).enumerate() {
        assert_eq!(terminal.origin, origin);
        assert_eq!(terminal.weight.to_bits(), cohort[origin].1.to_bits());
        assert_eq!(terminal.outcome, reference.outcome);
        assert_eq!(terminal.first, reference.first);
        assert_eq!(terminal.tricks, reference.tricks);
        assert!(!terminal.interrupted);
        assert_eq!(terminal.outcome.team_points, [140, 0]);
        assert_eq!(terminal.outcome.team_tricks, [9, 0]);
        assert_eq!(terminal.outcome.playing_team, Some(0));
        assert_eq!(terminal.outcome.game_value, 420);
        assert_eq!(
            engine::game_scores(&terminal.outcome, engine::SCHWARZ_FACTOR_FIFTHS),
            [-420, 0]
        );
    }
    assert!(oracle.choices.is_empty());
    assert_eq!(actual.choices.len(), 1);
    let choice = &actual.choices[0];
    assert_eq!(choice.scope, ChoiceScope::Local);
    assert_eq!(choice.selected, 0);
    assert_eq!(choice.information, receipts[0].accepted);
    assert_eq!(choice.context, context);
    assert_eq!(
        choice.membership,
        sequence::Membership::authored(
            &cohort[0].0.view(0),
            &crate::legal_actions::eligible(cohort[0].0.to_act().unwrap().1)
        )
        .unwrap()
    );
    assert!(choice.consumed.iter().any(|read| {
        read.information == receipts[0].accepted
            && matches!(
                read.reading,
                crate::concepts::sequence::ReadMeaning::Guard {
                    predicate: Guard::OwnLead,
                    truth: Some(true)
                }
            )
    }));
    assert_eq!(choice.support, vec![(0, 0.4), (1, 0.6)]);
    assert_eq!(
        choice.alternatives,
        domain
            .iter()
            .map(|stage| { Selection::Stage(receipts[0].program.suffix(*stage).unwrap()) })
            .collect::<Vec<_>>()
    );
    let Some(SelectionProof::CompletionDominance(proof)) = &choice.proof else {
        panic!("authentic Local must carry completion dominance, not a full competitor assessment")
    };
    assert!(choice.assessment().is_none());
    assert_eq!(proof.scope().alternatives, choice.alternatives);
    assert_eq!(
        proof.scope().support,
        vec![(0, 0.4_f64.to_bits()), (1, 0.6_f64.to_bits())]
    );
    assert_eq!(
        proof.values().selected_value.difference.to_bits(),
        (-420.0_f64).to_bits()
    );
    assert_eq!(proof.values().competitors.len(), 1);
    assert_eq!(proof.values().competitors[0].0, 1);
    assert!(proof.values().competitors[0].2.upper.total_cmp(&(-420.0)) != std::cmp::Ordering::Greater);
    assert!(actual
        .choices
        .iter()
        .all(|choice| choice.scope != ChoiceScope::Parent && choice.scope != ChoiceScope::Global));
    assert_eq!(
        actual.work.completed_candidates, 0,
        "no enclosing candidate was evaluated"
    );
}

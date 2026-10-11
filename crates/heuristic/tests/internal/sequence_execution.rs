use super::*;
use crate::concepts::sequence::{Actions, Guard, Node};
use crate::concepts::value::CohortMethod;
use engine::{ActionType, AnswerType, Card, CardSet, GamePhase, QuestionType, Suit};

fn provenance() -> Provenance {
    Provenance {
        belief_model: "fixture-joint",
        response_model: "finite-visible-first-card",
        cohort: CohortMethod::Enumerated,
    }
}
fn context() -> Context {
    Context {
        params: crate::Params::default(),
        seed: 7,
        posterior_revision: 1,
        provenance: provenance(),
    }
}
fn leaf(_: &SeatView<'_>, legal: &[GameAction], budget: LeafBudget) -> Result<GameAction, SequenceError> {
    if budget.remaining_transitions == 0 || budget.deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return Err(SequenceError::Budget);
    }
    Ok(*legal
        .iter()
        .find(|a| matches!(a.action_type, ActionType::Answer(_)))
        .or_else(|| {
            legal
                .iter()
                .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
        })
        .or_else(|| legal.iter().find(|a| a.action_type == ActionType::StopBidding))
        .unwrap_or(&legal[0]))
}
fn at_lead_with(hands: [Vec<Card>; 4]) -> Table {
    let mut table = Table::new(&hands);
    let bid = *table
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| matches!(a.action_type, ActionType::NewBid(_)))
        .unwrap();
    table.apply(bid);
    while table.game.state.phase != GamePhase::PassingBack {
        let action = table.to_act().unwrap().1[0];
        table.apply(action);
    }
    let give = CardSet(0b1111);
    let back = *table
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| matches!(a.action_type, ActionType::Pass(cards) if CardSet::from_cards(&cards) == give))
        .unwrap();
    table.apply(back);
    while table.game.state.all_tricks.is_empty() {
        let (_, legal) = table.to_act().unwrap();
        let action = leaf(
            &table.view(table.to_act().unwrap().0),
            legal,
            LeafBudget {
                deadline: None,
                remaining_transitions: 100,
            },
        )
        .unwrap();
        table.apply(action);
    }
    assert_eq!(table.to_act().unwrap().0, 0);
    assert_eq!(table.game.state.phase, GamePhase::StartTrick);
    table
}
fn hands() -> [Vec<Card>; 4] {
    std::array::from_fn(|seat| (0..9).map(|i| Card::from_index((9 * seat + i) as u8)).collect())
}
fn at_lead() -> Table {
    at_lead_with(hands())
}
fn direct(action: GameAction) -> Candidate {
    Candidate::new(Program {
        name: "direct",
        version: 1,
        start: Stage(1),
        nodes: vec![
            Node::Tail,
            Node::Action {
                actions: Actions::Exact(action.action_type),
                next: Stage(0),
            },
        ],
    })
}
fn mind(view: &SeatView<'_>) -> Mind {
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
fn equivalent_programs_reuse_complete_work_and_keep_actual_consumed_dependencies() {
    let table = at_lead();
    let (root, legal) = table.to_act().unwrap();
    let first = *legal
        .iter()
        .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
        .unwrap();
    let a = direct(first);
    let mut b = a.clone();
    b.program.name = "same-semantics";
    b.program.version = 2;
    let context = context();
    let result = evaluate(
        &[a, b],
        &[(table, 1.0)],
        root,
        provenance(),
        &context,
        Limits::default(),
        &mut leaf,
    )
    .unwrap();
    assert_eq!(result.values.selected, 0);
    assert_eq!(result.work.completed_candidates, 2);
    assert!(result.work.suffix_hits > 0);
    assert!(result.work.represented_transitions > result.work.transitions);
    assert_eq!(result.executions[0].terminals, result.executions[1].terminals);
    assert!(result.executions[0].terminals[0]
        .consumed
        .iter()
        .any(|read| matches!(&read.reading, ReadMeaning::Binding { .. })));
    assert!(!result.executions[0].terminals[0].tricks.is_empty());
    assert!(result.work.retained_bytes <= Limits::default().memo_bytes);
}

#[test]
fn activation_truth_changes_the_executed_suffix_and_warm_lookup_rejects_new_posterior() {
    let table = at_lead();
    let (root, _) = table.to_act().unwrap();
    let side = "g-Z".parse::<Card>().unwrap();
    let program = |target| {
        Candidate::new(Program {
            name: "question-then-bound-side",
            version: 1,
            start: Stage(4),
            nodes: vec![
                Node::Tail,
                Node::Action {
                    actions: Actions::Exact(ActionType::CardPlayed(side)),
                    next: Stage(0),
                },
                Node::Guard {
                    predicate: Guard::OwnLead,
                    yes: Stage(1),
                    no: Stage(0),
                    unknown: Stage(0),
                },
                Node::Guard {
                    predicate: Guard::Trump(target),
                    yes: Stage(2),
                    no: Stage(0),
                    unknown: Stage(0),
                },
                Node::Action {
                    actions: Actions::Exact(ActionType::Question(QuestionType::Yours)),
                    next: Stage(3),
                },
            ],
        })
    };
    let context = context();
    let comparison = evaluate(
        &[program(Suit::Bells), program(Suit::Red)],
        &[(table.clone(), 1.0)],
        root,
        provenance(),
        &context,
        Limits::default(),
        &mut leaf,
    )
    .unwrap();
    let good = &comparison.executions[0].terminals[0];
    let failed = &comparison.executions[1].terminals[0];
    assert!(good.consumed.iter().any(|read| matches!(
        &read.reading,
        ReadMeaning::Guard {
            predicate: Guard::Trump(Suit::Bells),
            truth: Some(true)
        }
    )));
    assert!(failed.consumed.iter().any(|read| matches!(
        &read.reading,
        ReadMeaning::Guard {
            predicate: Guard::Trump(Suit::Red),
            truth: Some(false)
        }
    )));
    assert!(good.consumed.iter().any(|read| matches!(&read.reading,
        ReadMeaning::Binding { actions: Actions::Exact(ActionType::CardPlayed(card)), .. } if *card == side)));
    assert!(!failed.consumed.iter().any(|read| matches!(&read.reading,
        ReadMeaning::Binding { actions: Actions::Exact(ActionType::CardPlayed(card)), .. } if *card == side)));
    assert_ne!(
        good.tricks[0].cards, failed.tricks[0].cards,
        "the dependency changes the executed action, not just its label"
    );
    let execution = &comparison.executions[0];
    let mut receipt = Receipt::new(execution.candidate.program.clone(), &table.view(root)).unwrap();
    receipt
        .propose(execution.first.clone(), &table.view(root), table.to_act().unwrap().1)
        .unwrap();
    let mut live = table;
    live.apply(execution.first.action);
    let answer = *live
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| a.action_type == ActionType::Answer(AnswerType::YesPair(Suit::Bells)))
        .unwrap();
    live.apply(answer);
    receipt.synchronize(&live.view(root)).unwrap();
    let view = live.view(root);
    let reader = mind(&view);
    let legal = live.to_act().unwrap().1;
    let membership = Membership::authored(&view, legal).unwrap();
    let offer = prepared_lookup(
        &receipt,
        &view,
        legal,
        &reader,
        &context,
        &membership,
        &execution.prepared,
    )
    .unwrap();
    assert_eq!(offer.action.action_type, ActionType::CardPlayed(side));
    let mut new_context = context.clone();
    new_context.posterior_revision += 1;
    assert_eq!(
        prepared_lookup(
            &receipt,
            &view,
            legal,
            &reader,
            &new_context,
            &membership,
            &execution.prepared
        ),
        Err(NeedsReview)
    );
    let mut opportunities = membership.clone();
    opportunities.programs.clear();
    assert_eq!(
        prepared_lookup(
            &receipt,
            &view,
            legal,
            &reader,
            &context,
            &opportunities,
            &execution.prepared
        ),
        Err(NeedsReview)
    );
}

#[test]
fn group_choice_uses_one_joint_expectation_and_preserves_identical_world_weights() {
    let table = at_lead();
    let (root, legal) = table.to_act().unwrap();
    let cards = legal
        .iter()
        .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
        .take(2)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(cards.len(), 2);
    let choice = Candidate::new(Program {
        name: "joint-choice",
        version: 1,
        start: Stage(3),
        nodes: vec![
            Node::Tail,
            Node::Action {
                actions: Actions::Exact(cards[0].action_type),
                next: Stage(0),
            },
            Node::Action {
                actions: Actions::Exact(cards[1].action_type),
                next: Stage(0),
            },
            Node::Choice(vec![Stage(1), Stage(2)]),
        ],
    });
    // Unequal nondyadic weights retain cohort alignment. This fixture tests the
    // conditional-integration invariant, not a claim of calibrated probability.
    let mut alternate_hands = hands();
    let one = alternate_hands[1][0];
    alternate_hands[1][0] = alternate_hands[3][0];
    alternate_hands[3][0] = one;
    let alternate = at_lead_with(alternate_hands);
    assert!(Information::of(&table.view(root)).matches(&alternate.view(root)));
    assert!(
        !Information::of(&table.view(1)).matches(&alternate.view(1)),
        "hypothetical actors have different private inputs"
    );
    let cohort = [(table, 1.0 / 3.0), (alternate, 2.0 / 3.0)];
    let context = context();
    let result = evaluate(
        &[choice, direct(cards[0]), direct(cards[1])],
        &cohort,
        root,
        provenance(),
        &context,
        Limits::default(),
        &mut leaf,
    )
    .unwrap();
    let record = result.executions[0]
        .prepared
        .iter()
        .find(|choice| choice.assessment.is_some())
        .unwrap();
    assert_eq!(record.support, [(0, 1.0 / 3.0), (1, 2.0 / 3.0)]);
    let selected = record.selected + 1;
    for origin in 0..2 {
        assert_eq!(
            result.executions[0].terminals[origin].outcome,
            result.executions[selected].terminals[origin].outcome
        );
        assert_eq!(
            result.executions[0].terminals[origin].first,
            result.executions[selected].terminals[origin].first
        );
        assert_eq!(result.executions[0].terminals[origin].weight, cohort[origin].1);
    }
    let left = &result.executions[1].terminals[0].tricks[0];
    let right = &result.executions[2].terminals[0].tricks[0];
    assert_eq!(left.winner, right.winner);
    assert_ne!(left.cards, right.cards);
    assert_ne!(
        left.root.knowledge, right.root.knowledge,
        "equal winners with different spent resources do not merge"
    );
}

#[test]
fn exhausted_lanes_are_explicit_deferred_coverage_and_leaf_receives_the_remaining_deadline() {
    let table = at_lead();
    let (root, legal) = table.to_act().unwrap();
    let candidate = direct(
        *legal
            .iter()
            .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
            .unwrap(),
    );
    let context = context();
    let cohort = [(table, 1.0)];
    let failed = evaluate(
        &[candidate.clone()],
        &cohort,
        root,
        provenance(),
        &context,
        Limits {
            transitions: 1,
            ..Limits::default()
        },
        &mut leaf,
    )
    .unwrap_err();
    assert_eq!(failed.reason, SequenceError::Budget);
    assert_eq!(failed.work.completed_candidates, 0);
    assert_eq!(failed.total_candidates, 1);
    let expired = evaluate(
        &[candidate.clone()],
        &cohort,
        root,
        provenance(),
        &context,
        Limits {
            deadline: Some(Instant::now()),
            ..Limits::default()
        },
        &mut leaf,
    )
    .unwrap_err();
    assert_eq!(expired.reason, SequenceError::Budget);
    let mut observed = false;
    let deadline = Instant::now() + std::time::Duration::from_secs(60);
    let mut checked_leaf = |view: &SeatView<'_>, legal: &[GameAction], budget: LeafBudget| {
        assert_eq!(budget.deadline, Some(deadline));
        assert!(budget.remaining_transitions < 8192);
        observed = true;
        leaf(view, legal, budget)
    };
    evaluate(
        &[candidate],
        &cohort,
        root,
        provenance(),
        &context,
        Limits {
            deadline: Some(deadline),
            ..Limits::default()
        },
        &mut checked_leaf,
    )
    .unwrap();
    assert!(observed);
}

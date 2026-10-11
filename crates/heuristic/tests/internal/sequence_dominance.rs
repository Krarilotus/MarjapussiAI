//! Real Local authorization oracle; no synthetic all-complete loser values.
use super::*;
use crate::concepts::sequence::Guard;
use crate::concepts::value::bounded::{BoundMeaning, BoundValue};
use crate::concepts::value::{CohortMethod, RuntimeOperator, RuntimeValue};
use engine::{Card, CardSet};

#[path = "sequence_dominance_cache.rs"]
mod cache_accounting;

fn context() -> Context {
    Context {
        params: crate::Params::default(),
        seed: 0,
        posterior_revision: 0,
        provenance: Provenance {
            belief_model: "declared-local-fixture",
            response_model: "first-legal-card-v1",
            cohort: CohortMethod::Enumerated,
        },
    }
}
fn leaf(_: &SeatView<'_>, legal: &[GameAction], _: LeafBudget) -> Result<LeafResponse, LeafFailure> {
    legal
        .iter()
        .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
        .copied()
        .map(LeafResponse::unit)
        .ok_or_else(|| LeafFailure::unit(SequenceError::WrongContext))
}
fn fixture() -> Table {
    fixture_until(7)
}
fn fixture_until(tricks: usize) -> Table {
    let hands = std::array::from_fn(|seat| (seat * 9..seat * 9 + 9).map(|i| Card::from_index(i as u8)).collect());
    let mut table = Table::new(&hands);
    while table.game.state.all_tricks.len() < tricks {
        let (_, legal) = table.to_act().unwrap();
        let action = match table.game.state.phase {
            GamePhase::WaitingForStart => legal[0],
            GamePhase::Bidding => *legal.iter().find(|a| a.action_type == ActionType::StopBidding).unwrap(),
            _ => *legal
                .iter()
                .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
                .min_by_key(|a| {
                    if let ActionType::CardPlayed(card) = a.action_type {
                        card.index()
                    } else {
                        unreachable!()
                    }
                })
                .unwrap(),
        };
        assert!(table.game.is_legal(&action));
        table.apply(action);
    }
    assert_eq!(table.to_act().unwrap().0, 0);
    table
}
fn cards(table: &Table) -> Vec<ActionType> {
    table
        .to_act()
        .unwrap()
        .1
        .iter()
        .filter_map(|a| matches!(a.action_type, ActionType::CardPlayed(_)).then_some(a.action_type))
        .collect()
}
fn direct(action: ActionType) -> Candidate {
    Candidate::new(Program {
        name: "direct-card-tail-oracle",
        version: 1,
        start: Stage(1),
        nodes: vec![
            Node::Tail,
            Node::Action {
                actions: Actions::Exact(action),
                next: Stage(0),
            },
        ],
    })
}
fn local(actions: &[ActionType]) -> Candidate {
    let mut nodes = vec![Node::Tail];
    for action in actions {
        nodes.push(Node::Action {
            actions: Actions::Exact(*action),
            next: Stage(0),
        });
    }
    let choice = Stage(nodes.len());
    nodes.push(Node::Choice((1..=actions.len()).map(Stage).collect()));
    let start = Stage(nodes.len());
    nodes.push(Node::Guard {
        predicate: Guard::OwnLead,
        yes: choice,
        no: Stage(0),
        unknown: Stage(0),
    });
    Candidate::new(Program {
        name: "guarded-local-card-tail",
        version: 1,
        start,
        nodes,
    })
}
fn limits(cache: bool) -> Limits {
    Limits {
        memo_entries: if cache { 128 } else { 0 },
        memo_bytes: if cache { 16 * 1024 * 1024 } else { 0 },
        reuse_suffixes: cache,
        ..Limits::default()
    }
}
fn run(candidate: &Candidate, cohort: &[(Table, f64)], context: &Context, cache: bool) -> Comparison {
    evaluate(
        std::slice::from_ref(candidate),
        cohort,
        0,
        context.provenance,
        context,
        limits(cache),
        &mut leaf,
    )
    .unwrap()
}
fn proof(choice: &PreparedChoice) -> &CompletionDominance {
    let Some(SelectionProof::CompletionDominance(proof)) = &choice.proof else {
        panic!("not dominance: {choice:?}")
    };
    proof
}

#[test]
fn guarded_local_keeps_original_alias_domain_and_actual_completed_incumbent() {
    let table = fixture();
    let actions = cards(&table);
    assert_eq!(actions.len(), 2);
    let candidate = local(&[actions[0], actions[1], actions[0]]);
    let cohort = [(table.clone(), 0.25), (table.clone(), 0.75)];
    let context = context();
    let actual = run(&candidate, &cohort, &context, false);
    let choice = actual
        .prepared()
        .iter()
        .find(|c| matches!(c.proof, Some(SelectionProof::CompletionDominance(_))))
        .unwrap();
    let certificate = proof(choice);
    assert_eq!(choice.alternatives.len(), 3);
    assert_eq!(choice.selected, 0);
    assert_eq!(choice.alternatives[0], choice.alternatives[2]);
    assert!(choice.assessment().is_none());
    assert!(certificate.validates(choice));
    assert_eq!(
        certificate.values().selected_value.difference.to_bits(),
        140.0_f64.to_bits()
    );
    assert_eq!(
        certificate
            .values()
            .competitors
            .iter()
            .map(|(i, _, _)| *i)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(
        certificate.values().competitors[0].1,
        certificate.values().competitors[1].1
    );
    assert!(certificate
        .values()
        .competitors
        .iter()
        .all(|(_, _, b)| b.upper == 140.0));
    assert_eq!(
        certificate.scope().support,
        vec![(0, 0.25_f64.to_bits()), (1, 0.75_f64.to_bits())]
    );
    assert!(actual.selection.root().assessment().unwrap().cohort_complete);
    assert!(actual.selection.records().iter().any(|r| r == choice));
    // The separate oracle evaluates EVERY original flat alternative ordinarily.
    let domain = [direct(actions[0]), direct(actions[1]), direct(actions[0])];
    let oracle = evaluate(
        &domain,
        &cohort,
        0,
        context.provenance,
        &context,
        limits(false),
        &mut leaf,
    )
    .unwrap();
    assert_eq!(oracle.values.selected, choice.selected);
    assert_eq!(actual.executions[0].first.action, oracle.executions[0].first.action);
    for (a, b) in actual.executions[0]
        .terminals
        .iter()
        .zip(&oracle.executions[0].terminals)
    {
        assert_eq!(
            (a.origin, a.weight.to_bits(), a.outcome, a.tricks.clone()),
            (b.origin, b.weight.to_bits(), b.outcome, b.tricks.clone())
        );
        assert!(!a.interrupted);
    }
    assert!(actual.work.represented_transitions < oracle.work.represented_transitions);
    assert_eq!(actual.work.tail_aliases, 0);
    assert_eq!(actual.work.frontier_aliases, 0);
    let trace = &certificate.values().trace;
    assert_eq!(
        trace
            .iter()
            .filter(|n| matches!(
                n.operator,
                RuntimeOperator::Value(crate::concepts::value::Operator::ConditionalOutcomes { .. })
            ))
            .count(),
        1
    );
    assert!(trace
        .iter()
        .any(|n| n.meaning == BoundMeaning::ConditionalPublicEnvelope));
    assert!(trace.iter().any(|n| matches!(n.value,RuntimeValue::Bound(BoundValue::Certified {ref excluded,..}) if excluded==&vec![(1,140.0),(2,140.0)])));
    for (i, n) in trace.iter().enumerate() {
        assert!(n
            .dependencies
            .iter()
            .all(|r| trace[..i].iter().any(|n| n.reference == *r)));
    }
}

#[test]
fn conditional_support_preserves_noncontiguous_parent_origins_and_live_reachability() {
    let table = fixture();
    let actions = cards(&table);
    let mut hands = std::array::from_fn(|seat| table.hand(seat as u8));
    let a = CardSet::single(hands[1].cards()[0].index());
    let b = CardSet::single(hands[3].cards()[0].index());
    hands[1] = (hands[1] - a) | b;
    hands[3] = (hands[3] - b) | a;
    let other = Table::from_view(&table.view(0), hands).unwrap();
    assert_eq!(Information::of(&table.view(0)), Information::of(&other.view(0)));
    let cohort: Vec<_> = (0..8)
        .map(|origin| {
            (
                if [2, 7].contains(&origin) {
                    table.clone()
                } else {
                    other.clone()
                },
                (origin + 1) as f64 / 36.0,
            )
        })
        .collect();
    // First visible trick splits the actual joint support; the next Local has
    // two original aliases of the one remaining card. Parent origin IDs survive.
    let candidate = Candidate::new(Program {
        name: "conditional-local-subset",
        version: 1,
        start: Stage(4),
        nodes: vec![
            Node::Tail,
            Node::Action {
                actions: Actions::Exact(actions[1]),
                next: Stage(0),
            },
            Node::Choice(vec![Stage(1), Stage(1)]),
            Node::Guard {
                predicate: Guard::OwnLead,
                yes: Stage(2),
                no: Stage(0),
                unknown: Stage(0),
            },
            Node::Action {
                actions: Actions::Exact(actions[0]),
                next: Stage(3),
            },
        ],
    });
    let context = context();
    let actual = run(&candidate, &cohort, &context, false);
    let record = actual
        .selection
        .records()
        .iter()
        .find(|c| c.support.iter().map(|(i, _)| *i).eq([2, 7]))
        .unwrap();
    let certificate = proof(record);
    assert_eq!(
        certificate.scope().support,
        vec![(2, cohort[2].1.to_bits()), (7, cohort[7].1.to_bits())]
    );
    let authorization = actual.selection.authorization(&record.information);
    assert!(authorization
        .records
        .iter()
        .any(|i| &actual.selection.records()[*i] == record));
    // Advance the real selected receipt, preserving its parent certificate.
    let mut live = table.clone();
    let mut receipt = actual.receipt.clone();
    live.apply(actual.executions[0].first.action);
    let view = live.view(0);
    receipt.synchronize(&view).unwrap();
    while live.to_act().unwrap().0 != 0 {
        let (_, legal) = live.to_act().unwrap();
        let action = leaf(
            &live.view(live.to_act().unwrap().0),
            legal,
            LeafBudget {
                deadline: None,
                remaining_transitions: 100,
                remaining_solver_nodes: None,
            },
        )
        .unwrap()
        .action;
        live.apply(action);
    }
    let view = live.view(0);
    receipt.synchronize(&view).unwrap();
    let legal = crate::legal_actions::eligible(live.to_act().unwrap().1);
    // Reuse production honest replay/sync; a current-hand-only Mind does not
    // contain this advanced public state and inspect must reject it.
    let mut reader = crate::HeuristicAgent::new(crate::Params::default());
    let mind = reader.mind(&view);
    assert_eq!(mind.knowledge(), view.knowledge);
    let offer = prepared_lookup(
        &receipt,
        &view,
        &legal,
        mind,
        &context,
        &record.membership,
        actual.selection.records(),
    )
    .unwrap();
    assert_eq!(offer.action.action_type, actions[1]);
    let mut cache = CompletedCache::default();
    evaluate_with_cache(
        std::slice::from_ref(&candidate),
        &[(table.clone(), 1.0)],
        0,
        context.provenance,
        &context,
        limits(true),
        &mut cache,
        &mut leaf,
    )
    .unwrap();
    let warmed = evaluate_with_cache(
        std::slice::from_ref(&candidate),
        &[(other.clone(), 1.0)],
        0,
        context.provenance,
        &context,
        limits(true),
        &mut cache,
        &mut leaf,
    )
    .unwrap();
    let fresh = run(&candidate, &[(other.clone(), 1.0)], &context, false);
    assert_eq!(warmed.executions, fresh.executions);
    assert_eq!(warmed.selection, fresh.selection);
    // Public summary equality does NOT substitute for latent BranchKey.
    let branches = |table: &Table| Branch {
        table: table.clone(),
        origin: 2,
        weight: cohort[2].1,
        receipt: Receipt::new(candidate.program.clone(), &table.view(0)).unwrap(),
        first: None,
        first_program: None,
        tricks: vec![],
        consumed: vec![],
        interrupted: false,
    };
    let suffix = candidate.program.suffix(candidate.stage).unwrap();
    assert!(BranchKey::of(&branches(&table), suffix.clone(), None) != BranchKey::of(&branches(&other), suffix, None));
}

#[test]
fn proof_binding_rejects_domain_scope_support_and_context_changes() {
    let table = fixture();
    let actions = cards(&table);
    let context = context();
    let actual = run(&local(&actions), &[(table, 1.0)], &context, false);
    let record = actual
        .prepared()
        .iter()
        .find(|c| matches!(c.proof, Some(SelectionProof::CompletionDominance(_))))
        .unwrap();
    for mutation in 0..7 {
        let mut changed = record.clone();
        match mutation {
            0 => changed.support[0].0 = 7,
            1 => changed.support[0].1 = f64::from_bits(1.0_f64.to_bits() + 1),
            2 => changed.alternatives.reverse(),
            3 => changed.context.posterior_revision += 1,
            4 => changed.selected = 1,
            5 => changed.scope = ChoiceScope::Global,
            _ => changed.membership.programs.clear(),
        }
        assert!(!proof(&changed).validates(&changed), "mutation {mutation}");
        let mut execution = actual.executions[0].clone();
        execution.prepared = vec![changed];
        assert!(SelectedPlan::completed(
            actual.selection.root().information.clone(),
            &context,
            actual.selection.root().membership.clone(),
            &actual.values,
            actual.selection.root().support.clone(),
            &execution
        )
        .is_err());
    }
    assert!(support_subsequence(
        &[(2, 0.25), (7, 0.75)],
        &[(0, 0.1), (2, 0.25), (5, 0.5), (7, 0.75)]
    ));
    assert!(!support_subsequence(&[(7, 0.75), (2, 0.25)], &[(2, 0.25), (7, 0.75)]));
}

#[test]
fn losing_authored_gate_and_incumbent_errors_cannot_be_rescued_by_bounds() {
    let table = fixture();
    let actions = cards(&table);
    let context = context();
    let mut candidate = local(&actions);
    candidate.program.nodes.push(Node::Deferred);
    let gate = Stage(candidate.program.nodes.len() - 1);
    let choice = candidate.program.start.0 - 1;
    if let Node::Choice(choices) = &mut candidate.program.nodes[choice] {
        choices[1] = gate;
    } else {
        unreachable!()
    }
    let error = evaluate(
        &[candidate],
        &[(table.clone(), 1.0)],
        0,
        context.provenance,
        &context,
        limits(false),
        &mut leaf,
    )
    .unwrap_err();
    assert_eq!(error.reason, SequenceError::Budget);
    assert_eq!(error.limit, Some(DeferredLimit::AuthoredGate));
    let candidate = local(&actions);
    let error = evaluate(
        std::slice::from_ref(&candidate),
        &[(table.clone(), 1.0)],
        0,
        context.provenance,
        &context,
        limits(false),
        &mut |_: &SeatView<'_>, _: &[GameAction], _: LeafBudget| Err(LeafFailure::unit(SequenceError::IllegalAction)),
    )
    .unwrap_err();
    assert_eq!(error.reason, SequenceError::IllegalAction);
    let error = evaluate(
        &[candidate],
        &[(table, 1.0)],
        0,
        context.provenance,
        &context,
        Limits {
            comparisons: 0,
            ..limits(false)
        },
        &mut leaf,
    )
    .unwrap_err();
    assert_eq!(error.reason, SequenceError::Budget);
    assert_eq!(error.work.comparisons, 0);
}

#[test]
fn cold_warm_and_zero_capacity_preserve_semantics_and_proof_charging() {
    let table = fixture();
    let candidate = local(&cards(&table));
    let context = context();
    let cohort = [(table, 1.0)];
    let cold = run(&candidate, &cohort, &context, false);
    let mut cache = CompletedCache::default();
    let first = evaluate_with_cache(
        std::slice::from_ref(&candidate),
        &cohort,
        0,
        context.provenance,
        &context,
        limits(true),
        &mut cache,
        &mut leaf,
    )
    .unwrap();
    let warm = evaluate_with_cache(
        &[candidate],
        &cohort,
        0,
        context.provenance,
        &context,
        limits(true),
        &mut cache,
        &mut leaf,
    )
    .unwrap();
    assert_eq!(cold.values, first.values);
    assert_eq!(cold.executions, first.executions);
    assert_eq!(first.values, warm.values);
    assert_eq!(first.executions, warm.executions);
    assert_eq!(first.selection, warm.selection);
    assert_eq!(cold.work.represented_transitions, warm.work.represented_transitions);
    assert_eq!(cold.work.represented_comparisons, warm.work.represented_comparisons);
    assert_eq!(cold.work.represented_solver_nodes, warm.work.represented_solver_nodes);
    assert_eq!(
        cold.work.accounting(),
        "sibling-frontier-dag-v2+completion-dominance-local-v1"
    );
}

#[test]
fn stage_and_offer_eligibility_are_narrow_and_do_not_query_future_guards() {
    let table = fixture();
    let action = table
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
        .copied()
        .unwrap();
    let tail = Program {
        name: "literal-tail",
        version: 1,
        start: Stage(0),
        nodes: vec![Node::Tail],
    }
    .suffix(Stage(0))
    .unwrap();
    assert_eq!(exact_card(&Selection::Offer { action, next: tail }), Some(action));
    assert_eq!(
        stage_card(&direct(action.action_type).program.suffix(Stage(1)).unwrap()),
        Some(action.action_type)
    );
    for node in [
        Node::Deferred,
        Node::Response { next: Stage(0) },
        Node::Reconsider {
            remaining: 1,
            retained: Stage(0),
        },
        Node::Guard {
            predicate: Guard::OwnLead,
            yes: Stage(0),
            no: Stage(0),
            unknown: Stage(0),
        },
    ] {
        let suffix = Suffix {
            start: Stage(1),
            nodes: vec![Node::Tail, node],
        };
        assert!(stage_card(&suffix).is_none());
    }
    let unknown = Selection::Stage(direct(ActionType::StopBidding).program.suffix(Stage(1)).unwrap());
    let Selection::Stage(suffix) = unknown else {
        unreachable!()
    };
    assert!(stage_card(&suffix).is_none());
}

#[test]
fn equal_frontier_sharing_declines_dominance_projection_without_rebates() {
    let table = fixture();
    let actions = cards(&table);
    let context = context();
    let mut candidate = local(&[actions[0], actions[1], actions[0]]);
    let inner = match &candidate.program.nodes[candidate.program.start.0] {
        Node::Guard { yes, .. } => *yes,
        _ => panic!("local wrapper"),
    };
    let second = Stage(candidate.program.nodes.len());
    candidate.program.nodes.push(Node::Guard {
        predicate: Guard::OwnCards(CardSet::EMPTY),
        yes: inner,
        no: Stage(0),
        unknown: Stage(0),
    });
    let first = candidate.stage;
    candidate.stage = Stage(candidate.program.nodes.len());
    candidate.program.start = candidate.stage;
    candidate.program.nodes.push(Node::Choice(vec![first, second]));
    let actual = run(&candidate, &[(table, 1.0)], &context, false);
    assert!(actual
        .prepared()
        .iter()
        .any(|r| matches!(r.proof, Some(SelectionProof::CompletionDominance(_)))));
    assert_eq!(actual.work.frontier_aliases, 0);
    assert_eq!(actual.work.frontier_alias_transitions, 0);
    assert_eq!(actual.work.frontier_alias_comparisons, 0);
    assert_eq!(actual.work.frontier_alias_solver_nodes, 0);
    assert_eq!(actual.work.represented_comparisons, 4);
}

fn session<'a, F: FnMut(&SeatView<'_>, &[GameAction], LeafBudget) -> Result<LeafResponse, LeafFailure>>(
    context: &'a Context,
    leaf: &'a mut F,
) -> Session<'a, F> {
    Session {
        root: 0,
        provenance: context.provenance,
        context,
        limits: limits(false),
        work: Work::default(),
        leaf,
        responses: vec![],
        hard_reads: vec![],
        memo: vec![],
        tails: vec![],
        canonical: vec![],
        memberships: vec![],
        reusable_bytes: 0,
        response_bytes: 0,
        limit: None,
    }
}
fn group(table: &Table, candidate: &Candidate, origins: &[(usize, f64)]) -> Vec<Waiting> {
    let Node::Guard { yes, .. } = candidate.program.nodes[candidate.stage.0] else {
        panic!("wrapper")
    };
    let Node::Choice(ref alternatives) = candidate.program.nodes[yes.0] else {
        panic!("choice")
    };
    origins
        .iter()
        .map(|(origin, weight)| {
            let mut receipt = Receipt::new(candidate.program.clone(), &table.view(0)).unwrap();
            receipt.stage = yes;
            Waiting {
                branch: Branch {
                    table: table.clone(),
                    origin: *origin,
                    weight: *weight,
                    receipt,
                    first: None,
                    first_program: None,
                    tricks: vec![],
                    consumed: vec![],
                    interrupted: false,
                },
                alternatives: alternatives.iter().copied().map(Alternative::Stage).collect(),
            }
        })
        .collect()
}
#[test]
fn deadline_before_certificate_and_undo_or_nonplay_snapshot_are_not_proofs() {
    let table = fixture();
    let candidate = local(&cards(&table));
    let context = context();
    let mut leaf = leaf;
    let mut session = session(&context, &mut leaf);
    let group = group(&table, &candidate, &[(2, 0.25), (7, 0.75)]);
    let program = &group[0].branch.receipt.program;
    let alternatives: Vec<_> = group[0]
        .alternatives
        .iter()
        .map(|a| session.selection(program, a).unwrap())
        .collect();
    let view = table.view(0);
    let membership = session.membership(&view, table.to_act().unwrap().1).unwrap();
    assert!(session
        .dominance_input(&group, &alternatives, ChoiceScope::Global, &membership)
        .unwrap()
        .is_none());
    let snapshot = session
        .dominance_input(&group, &alternatives, ChoiceScope::Local, &membership)
        .unwrap()
        .unwrap();
    let suffix = program.suffix(group[0].branch.receipt.stage).unwrap();
    let children = session.choice_children(&group, 0).unwrap();
    let mut records = vec![];
    let completed = session.finish(children, &mut records).unwrap();
    session.limits.deadline = Some(Instant::now());
    let error = session
        .certify_local(
            snapshot,
            &Information::of(&view),
            &suffix,
            &membership,
            &alternatives,
            &[(2, 0.25), (7, 0.75)],
            &completed,
        )
        .unwrap_err();
    assert_eq!(error, SequenceError::Budget);
    assert_eq!(session.limit, Some(DeferredLimit::Deadline));
    assert_eq!(session.work.comparisons, 0);
    assert!(knowledge::PublicOutcomeEnvelope::from_forward_play(
        view.knowledge,
        &GamePhase::Bidding,
        &membership.legal,
        view.events
    )
    .is_err());
    let mut events = view.events.to_vec();
    events.push(observation::SeatEvent::Undone);
    assert!(knowledge::PublicOutcomeEnvelope::from_forward_play(
        view.knowledge,
        &table.game.state.phase,
        &membership.legal,
        &events
    )
    .is_err());
}

#[test]
fn failed_separation_finishes_remaining_original_routes_without_reexecuting_incumbent() {
    let table = fixture_until(6);
    let actions = cards(&table);
    let context = context();
    let domain = [actions[0], actions[1]];
    let candidate = local(&domain);
    let cohort = [(table, 1.0)];
    let initial_plays = cohort[0]
        .0
        .view(0)
        .events
        .iter()
        .filter(|event| matches!(event, observation::SeatEvent::CardPlayed { seat: 0, .. }))
        .count();
    let count = |counts: &mut [usize; 2], view: &SeatView<'_>| {
        let card = view
            .events
            .iter()
            .filter_map(|event| match event {
                observation::SeatEvent::CardPlayed { seat: 0, card } => Some(*card),
                _ => None,
            })
            .nth(initial_plays)
            .unwrap();
        let route = domain
            .iter()
            .position(|action| matches!(action,ActionType::CardPlayed(c) if c.index()==card))
            .unwrap();
        counts[route] += 1;
    };
    let mut actual_calls = [0; 2];
    let actual = evaluate(
        &[candidate],
        &cohort,
        0,
        context.provenance,
        &context,
        limits(false),
        &mut |view: &SeatView<'_>, legal: &[GameAction], budget: LeafBudget| {
            count(&mut actual_calls, view);
            leaf(view, legal, budget)
        },
    )
    .unwrap();
    let choice = actual
        .prepared()
        .iter()
        .find(|c| c.scope == ChoiceScope::Local && c.alternatives.len() == 2)
        .unwrap();
    let values = choice
        .assessment()
        .expect("loose public bounds require complete fallback");
    assert!(values.cohort_complete);
    assert_eq!(values.candidates.len(), 2);
    let mut oracle_calls = [0; 2];
    let oracle = evaluate(
        &[direct(domain[0]), direct(domain[1])],
        &cohort,
        0,
        context.provenance,
        &context,
        limits(false),
        &mut |view: &SeatView<'_>, legal: &[GameAction], budget: LeafBudget| {
            count(&mut oracle_calls, view);
            leaf(view, legal, budget)
        },
    )
    .unwrap();
    assert!(actual_calls.iter().all(|n| *n > 0));
    assert_eq!(
        actual_calls, oracle_calls,
        "incumbent executed exactly once, as in exhaustive oracle"
    );
    assert_eq!(values.selected, oracle.values.selected);
    assert_eq!(values.candidates, oracle.values.candidates);
    assert_eq!(actual.work.response_calls, oracle.work.response_calls);
    // One unsuccessful proof, one full Local comparison, one full outer comparison.
    assert_eq!(actual.work.comparisons, 3);
    assert_eq!(actual.work.represented_comparisons, 3);
    assert_eq!(actual.work.frontier_aliases, 0);
    assert_eq!(actual.work.tail_aliases, 0);
}

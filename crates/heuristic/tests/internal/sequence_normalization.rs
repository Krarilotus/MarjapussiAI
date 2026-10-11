use super::*;
use crate::concepts::sequence::{Actions, Guard, Node};
use crate::concepts::value::CohortMethod;
use engine::{ActionType, AnswerType, Card, CardSet};

fn context() -> Context {
    Context {
        params: crate::Params::default(),
        seed: 31,
        posterior_revision: 1,
        provenance: Provenance {
            belief_model: "normalization-fixture",
            response_model: "visible-first-legal-v1",
            cohort: CohortMethod::Enumerated,
        },
    }
}

fn response(_: &SeatView<'_>, legal: &[GameAction], budget: LeafBudget) -> Result<LeafResponse, LeafFailure> {
    if budget.remaining_transitions == 0 || budget.remaining_solver_nodes.is_some_and(|nodes| nodes < 3) {
        return Err(LeafFailure::unit(SequenceError::Budget));
    }
    let action = *legal
        .iter()
        .find(|action| matches!(action.action_type, ActionType::CardPlayed(_)))
        .or_else(|| {
            legal
                .iter()
                .find(|action| action.action_type == ActionType::StopBidding)
        })
        .or_else(|| {
            legal
                .iter()
                .find(|action| !matches!(action.action_type, ActionType::UndoRequest))
        })
        .unwrap();
    Ok(LeafResponse {
        action,
        canonical_solver_nodes: 3,
    })
}

fn opening() -> Table {
    let hands = std::array::from_fn(|seat| {
        (0..9)
            .map(|index| Card::from_index((seat * 9 + index) as u8))
            .collect::<Vec<_>>()
    });
    let mut table = Table::new(&hands);
    // Legal all-pass auction gives an observer-visible history without private
    // transfers. The forced first card is deliberately still unplayed.
    for _ in 0..16 {
        let (_, legal) = table.to_act().unwrap();
        if legal
            .iter()
            .any(|action| matches!(action.action_type, ActionType::CardPlayed(_)))
        {
            return table;
        }
        let action = *legal
            .iter()
            .find(|action| action.action_type == ActionType::StopBidding)
            .unwrap();
        table.apply(action);
    }
    panic!("all-pass auction did not reach play");
}

fn cohort() -> (u8, Vec<(Table, f64)>) {
    let first = opening();
    let root = first.to_act().unwrap().0;
    let mut hands = std::array::from_fn(|seat| first.hand(seat as u8));
    let a = (root + 1) as usize % 4;
    let b = (root + 3) as usize % 4;
    let ca = CardSet::single(hands[a].cards()[0].index());
    let cb = CardSet::single(hands[b].cards()[0].index());
    hands[a] = (hands[a] - ca) | cb;
    hands[b] = (hands[b] - cb) | ca;
    let second = Table::from_view(&first.view(root), hands).expect("legal independent hidden-card completion");
    assert_eq!(Information::of(&first.view(root)), Information::of(&second.view(root)));
    assert_ne!(
        Information::of(&first.view(a as u8)),
        Information::of(&second.view(a as u8))
    );
    (root, vec![(first, 0.25), (second, 0.75)])
}

fn program(order: &[usize]) -> Program {
    Program {
        name: "same-tail-different-evidence",
        version: 1,
        start: Stage(6),
        nodes: vec![
            Node::Tail,
            Node::Deferred,
            Node::Guard {
                predicate: Guard::OwnCards(CardSet::EMPTY),
                yes: Stage(0),
                no: Stage(1),
                unknown: Stage(1),
            },
            Node::Action {
                actions: Actions::Exact(ActionType::Answer(AnswerType::NoPair)),
                next: Stage(0),
            },
            Node::Guard {
                predicate: Guard::OwnCards(CardSet::ALL),
                yes: Stage(1),
                no: Stage(3),
                unknown: Stage(1),
            },
            Node::Guard {
                predicate: Guard::Known(beliefs::ConceptQuery::new(beliefs::Concept::Count {
                    seat: 1,
                    cards: CardSet::single(9),
                    min: 1,
                    max: 1,
                })),
                yes: Stage(0),
                no: Stage(0),
                unknown: Stage(0),
            },
            Node::Choice(order.iter().copied().map(Stage).collect()),
        ],
    }
}

fn limits(cached: bool) -> Limits {
    Limits {
        transitions: 20_000,
        comparisons: 1024,
        memo_entries: if cached { 128 } else { 0 },
        memo_bytes: if cached { 64 * 1024 * 1024 } else { 0 },
        reuse_suffixes: cached,
        ..Limits::default()
    }
}

#[test]
fn sibling_tail_keeps_every_value_tie_and_edge_evidence() {
    let (root, cohort) = cohort();
    let context = context();
    for order in [
        [0, 2, 3, 4, 5],
        [3, 2, 0, 4, 5],
        [4, 0, 2, 3, 5],
        [2, 0, 3, 4, 5],
        [5, 0, 2, 3, 4],
    ] {
        let program = program(&order);
        // Independent reference: separate top-level candidates have no sibling
        // comparison to normalize. No shared-tail helper supplies expected data.
        let separate: Vec<_> = order
            .iter()
            .map(|stage| Candidate {
                program: program.clone(),
                stage: Stage(*stage),
            })
            .collect();
        let reference = evaluate(
            &separate,
            &cohort,
            root,
            context.provenance,
            &context,
            limits(false),
            &mut response,
        )
        .unwrap();
        let shared = evaluate(
            &[Candidate::new(program.clone())],
            &cohort,
            root,
            context.provenance,
            &context,
            limits(false),
            &mut response,
        )
        .unwrap();
        let choice = shared
            .prepared()
            .iter()
            .find(|choice| choice.assessment.is_some())
            .unwrap();
        let values = choice.assessment.as_ref().unwrap();
        assert!(values.cohort_complete);
        assert_eq!(choice.alternatives.len(), order.len());
        assert_eq!(choice.selected, 0, "first equal alternative still wins");
        assert_eq!(choice.support, [(0, 0.25), (1, 0.75)]);
        for (index, expected) in reference.values.candidates.iter().enumerate() {
            assert_eq!(values.candidates[index].difference, expected.difference);
            assert_eq!(values.candidates[index].our_score, expected.our_score);
            assert_eq!(values.candidates[index].opponent_score, expected.opponent_score);
            assert_eq!(
                choice.alternatives[index],
                Selection::Stage(program.suffix(Stage(order[index])).unwrap())
            );
        }
        for (actual, expected) in shared.executions[0]
            .terminals
            .iter()
            .zip(&reference.executions[0].terminals)
        {
            assert_eq!(actual.outcome, expected.outcome);
            assert_eq!(actual.interrupted, expected.interrupted);
            assert_eq!(actual.first, expected.first);
            assert_eq!(actual.first_program, expected.first_program);
            assert_eq!(actual.tricks, expected.tricks);
            // The outer Choice read is the only extra consumed dependency.
            let without_choice: Vec<_> = actual
                .consumed
                .iter()
                .filter(|read| !matches!(read.reading, ReadMeaning::Alternatives(_)))
                .cloned()
                .collect();
            assert_eq!(without_choice, expected.consumed);
        }
        assert_eq!(shared.work.accounting(), "sibling-frontier-dag-v2");
        assert_eq!(shared.work.tail_aliases, order.len() - 1);
        assert_eq!(
            shared.work.represented_transitions * order.len(),
            reference.work.represented_transitions
        );
        assert_eq!(
            shared.work.tail_alias_transitions,
            shared.work.represented_transitions * (order.len() - 1)
        );
        assert_eq!(
            shared.work.tail_alias_solver_nodes,
            shared.work.represented_solver_nodes * (order.len() - 1) as u64
        );
    }
}

#[test]
fn sibling_tail_aliases_reattach_their_own_reads_and_interruption() {
    let (root, cohort) = cohort();
    let context = context();
    let order = [0, 2, 3, 4, 5];
    let program = program(&order);
    let candidates: Vec<_> = order
        .iter()
        .map(|stage| Candidate {
            program: program.clone(),
            stage: Stage(*stage),
        })
        .collect();
    let reference = evaluate(
        &candidates,
        &cohort,
        root,
        context.provenance,
        &context,
        limits(false),
        &mut response,
    )
    .unwrap();
    let children = |stage| {
        cohort
            .iter()
            .enumerate()
            .map(|(origin, (table, weight))| {
                let mut receipt = Receipt::new(program.clone(), &table.view(root)).unwrap();
                receipt.stage = Stage(stage);
                Branch {
                    table: table.clone(),
                    origin,
                    weight: *weight,
                    receipt,
                    first: None,
                    first_program: None,
                    tricks: vec![],
                    consumed: vec![],
                    interrupted: false,
                }
            })
            .collect::<Vec<_>>()
    };
    let mut leaf = response;
    let mut session = Session {
        root,
        provenance: context.provenance,
        context: &context,
        limits: limits(false),
        work: Work::default(),
        leaf: &mut leaf,
        responses: vec![],
        hard_reads: vec![],
        memo: vec![],
        tails: vec![],
        canonical: vec![],
        memberships: vec![],
        reusable_bytes: 0,
        response_bytes: 0,
        limit: None,
    };
    let first = children(0);
    let edges = session.normalized_tail_edges(&first).unwrap().unwrap();
    let read_starts = first
        .iter()
        .zip(&edges)
        .map(|(branch, edge)| (branch.origin, edge.consumed.len()))
        .collect();
    let source = session.finish_normalized_tails(first, edges).unwrap();
    assert_eq!(source, reference.executions[0].terminals);
    let dependency = TailDependency {
        outcome_index: 0,
        read_starts,
        transitions: session.work.represented_transitions,
        solver_nodes: session.work.represented_solver_nodes,
    };
    let outcomes = [source];
    for (index, stage) in order.iter().enumerate().skip(1) {
        let branches = children(*stage);
        let edges = session.normalized_tail_edges(&branches).unwrap().unwrap();
        let aliases = session.alias_tail(&branches, &edges, &dependency, &outcomes).unwrap();
        assert_eq!(aliases, reference.executions[index].terminals);
        if *stage == 5 {
            assert!(aliases[0]
                .consumed
                .iter()
                .any(|read| matches!(read.reading, ReadMeaning::Guard { truth: None, .. })));
        }
    }
    assert_eq!(session.work.represented_transitions, dependency.transitions);
    assert_eq!(session.work.represented_solver_nodes, dependency.solver_nodes);
}

#[test]
fn sibling_tail_unique_allowance_matches_cache_disabled_cold_and_warm() {
    let (root, cohort) = cohort();
    let context = context();
    let candidates = [Candidate::new(program(&[3, 0, 2, 4]))];
    let uncached = evaluate(
        &candidates,
        &cohort,
        root,
        context.provenance,
        &context,
        limits(false),
        &mut response,
    )
    .unwrap();
    let mut cache = CompletedCache::default();
    for cached in [false, true, true] {
        let exact = Limits {
            transitions: uncached.work.represented_transitions,
            comparisons: uncached.work.represented_comparisons,
            solver_nodes: Some(uncached.work.represented_solver_nodes),
            ..limits(cached)
        };
        let run = evaluate_with_cache(
            &candidates,
            &cohort,
            root,
            context.provenance,
            &context,
            exact,
            &mut cache,
            &mut response,
        )
        .unwrap();
        assert_eq!(run.values, uncached.values);
        assert_eq!(run.executions, uncached.executions);
        assert_eq!(run.receipt, uncached.receipt);
        assert_eq!(run.prepared(), uncached.prepared());
        assert_eq!(run.work.represented_transitions, uncached.work.represented_transitions);
        assert_eq!(run.work.represented_comparisons, uncached.work.represented_comparisons);
        assert_eq!(
            run.work.represented_solver_nodes,
            uncached.work.represented_solver_nodes
        );
        assert!(
            evaluate_with_cache(
                &candidates,
                &cohort,
                root,
                context.provenance,
                &context,
                Limits {
                    transitions: exact.transitions - 1,
                    ..exact
                },
                &mut cache,
                &mut response,
            )
            .is_err(),
            "unique dependency must complete; no partial values at one less transition"
        );
    }
}

#[test]
fn sibling_tail_completed_parent_memo_preserves_unique_work_and_selected_support() {
    #[cfg(feature = "profile")]
    let assert_events = |expected: &[(&str, u64)]| {
        let timings = beliefs::profile::snapshot();
        for (label, count) in expected {
            assert_eq!(timings.get(*label).map_or(0, |timing| timing.calls), *count, "{label}");
        }
    };
    #[cfg(feature = "profile")]
    beliefs::profile::reset();
    let (root, cohort) = cohort();
    let context = context();
    let first = Candidate::new(program(&[3, 0, 2, 4]));
    let mut equivalent = first.clone();
    equivalent.program.name = "cosmetically-distinct-equivalent-parent";
    assert_eq!(
        first.program.suffix(first.stage).unwrap(),
        equivalent.program.suffix(equivalent.stage).unwrap()
    );
    let candidates = [first, equivalent];
    let uncached = evaluate(
        &candidates,
        &cohort,
        root,
        context.provenance,
        &context,
        limits(false),
        &mut response,
    )
    .unwrap();
    #[cfg(feature = "profile")]
    {
        assert_events(&[
            ("sequence.finish.lookup_miss", 2),
            ("sequence.finish.lookup_hit", 0),
            ("sequence.finish.not_retained.disabled", 2),
            ("sequence.finish.not_retained.entry_cap", 0),
            ("sequence.finish.not_retained.byte_cap", 0),
            ("sequence.finish.stored", 0),
            ("sequence.run_ledger", 0),
        ]);
        beliefs::profile::reset();
    }
    let cached = evaluate(
        &candidates,
        &cohort,
        root,
        context.provenance,
        &context,
        Limits {
            transitions: uncached.work.represented_transitions,
            comparisons: uncached.work.represented_comparisons,
            solver_nodes: Some(uncached.work.represented_solver_nodes),
            ..limits(true)
        },
        &mut response,
    )
    .unwrap();
    #[cfg(feature = "profile")]
    {
        assert_events(&[
            ("sequence.finish.lookup_miss", 1),
            ("sequence.finish.lookup_hit", 1),
            ("sequence.finish.hit_served", 1),
            ("sequence.finish.hit_copy", 1),
            ("sequence.finish.store_copy", 1),
            ("sequence.finish.stored", 1),
            ("sequence.finish.not_retained.disabled", 0),
            ("sequence.finish.not_retained.entry_cap", 0),
            ("sequence.finish.not_retained.byte_cap", 0),
            ("sequence.run_ledger", 1),
        ]);
        // Reuse this real completed-parent fixture to classify admission loss.
        // Zero entries AND bytes must select the earlier entry-cap branch,
        // without doing the payload ledger calculation for rejected admission.
        for (entries, label, ledgers) in [
            (0, "sequence.finish.not_retained.entry_cap", 0),
            (128, "sequence.finish.not_retained.byte_cap", 2),
        ] {
            beliefs::profile::reset();
            let run = evaluate(
                &candidates,
                &cohort,
                root,
                context.provenance,
                &context,
                Limits {
                    memo_entries: entries,
                    memo_bytes: 0,
                    ..limits(true)
                },
                &mut response,
            )
            .unwrap();
            assert_eq!(run.values, uncached.values);
            assert_eq!(run.executions, uncached.executions);
            assert_eq!(run.receipt, uncached.receipt);
            assert_events(&[
                ("sequence.finish.lookup_miss", 2),
                ("sequence.finish.lookup_hit", 0),
                ("sequence.finish.stored", 0),
                ("sequence.finish.not_retained.disabled", 0),
                (label, 2),
                ("sequence.run_ledger", ledgers),
            ]);
        }
        beliefs::profile::reset();
        let rejected = evaluate(
            &candidates,
            &cohort,
            root,
            context.provenance,
            &context,
            Limits {
                transitions: uncached.work.represented_transitions - 1,
                ..limits(true)
            },
            &mut response,
        )
        .unwrap_err();
        assert_eq!(rejected.reason, SequenceError::Budget);
        assert_eq!(rejected.work.completed_candidates, 1);
        assert_events(&[
            ("sequence.finish.lookup_miss", 1),
            ("sequence.finish.lookup_hit", 1),
            ("sequence.finish.stored", 1),
            ("sequence.finish.hit_copy", 0),
            ("sequence.finish.hit_served", 0),
        ]);
    }

    // One local Choice per parent, plus the outer candidate comparison.
    // Tail/response hits alone CANNOT eliminate a ValueStore comparison or the
    // construction of the second parent's three mandatory sibling aliases.
    // Together these observations require a completed PARENT finish memo hit,
    // rather than accepting an arbitrary nonzero suffix_hits counter.
    assert_eq!(uncached.work.comparisons, 3);
    assert_eq!(cached.work.comparisons, 2);
    assert_eq!(cached.work.suffix_hits, 1);
    assert_eq!(uncached.work.tail_aliases, 6);
    assert_eq!(cached.work.tail_aliases, 3);
    assert!(cached.work.transitions > 0);
    assert_eq!(cached.work.transitions * 2, uncached.work.transitions);

    assert_eq!(cached.values, uncached.values);
    assert_eq!(cached.executions, uncached.executions);
    assert_eq!(cached.prepared(), uncached.prepared());
    assert_eq!(cached.receipt, uncached.receipt);
    assert_eq!(cached.values.selected, 0);
    assert!(cached.values.cohort_complete);
    assert_eq!(cached.values.candidates.len(), 2);
    assert_eq!(cached.work.completed_candidates, 2);
    assert_eq!(
        cached.work.represented_transitions,
        uncached.work.represented_transitions
    );
    assert_eq!(
        cached.work.represented_comparisons,
        uncached.work.represented_comparisons
    );
    assert_eq!(
        cached.work.represented_solver_nodes,
        uncached.work.represented_solver_nodes
    );
    assert_eq!(cached.work.accounting(), uncached.work.accounting());
    let selected = cached
        .prepared()
        .iter()
        .find(|choice| choice.assessment.is_some())
        .unwrap();
    assert_eq!(selected.support, [(0, 0.25), (1, 0.75)]);
    assert_eq!(selected.context, context);
    assert_eq!(selected.alternatives.len(), 4);
    assert_eq!(selected.selected, 0);
    assert!(selected.assessment.as_ref().unwrap().cohort_complete);
}

#[test]
fn sibling_tail_does_not_alias_engine_actions_and_preserves_prefix_owner() {
    let mut table = opening();
    while table
        .to_act()
        .unwrap()
        .1
        .iter()
        .filter(|action| matches!(action.action_type, ActionType::CardPlayed(_)))
        .count()
        < 2
    {
        let (seat, legal) = table.to_act().unwrap();
        let action = response(
            &table.view(seat),
            legal,
            LeafBudget {
                deadline: None,
                remaining_transitions: 100,
                remaining_solver_nodes: None,
            },
        )
        .unwrap()
        .action;
        table.apply(action);
    }
    let (root, legal) = table.to_act().unwrap();
    let actions: Vec<_> = legal
        .iter()
        .filter(|action| matches!(action.action_type, ActionType::CardPlayed(_)))
        .take(2)
        .copied()
        .collect();
    let context = context();
    let actions_program = Program {
        name: "nonempty-actions-not-aliased",
        version: 1,
        start: Stage(3),
        nodes: vec![
            Node::Tail,
            Node::Action {
                actions: Actions::Exact(actions[0].action_type),
                next: Stage(0),
            },
            Node::Action {
                actions: Actions::Exact(actions[1].action_type),
                next: Stage(0),
            },
            Node::Choice(vec![Stage(1), Stage(2)]),
        ],
    };
    let run = evaluate(
        &[Candidate::new(actions_program)],
        &[(table.clone(), 1.0)],
        root,
        context.provenance,
        &context,
        limits(false),
        &mut response,
    )
    .unwrap();
    assert_eq!(run.work.tail_aliases, 0);
    assert_eq!(
        run.prepared()
            .iter()
            .find(|choice| choice.assessment.is_some())
            .unwrap()
            .alternatives
            .len(),
        2
    );

    let mut prefix = program(&[3, 0, 2, 4]);
    prefix.nodes.push(Node::Action {
        actions: Actions::Exact(actions[0].action_type),
        next: prefix.start,
    });
    prefix.start = Stage(prefix.nodes.len() - 1);
    let expected = prefix.suffix(prefix.start).unwrap();
    let run = evaluate(
        &[Candidate::new(prefix)],
        &[(table, 1.0)],
        root,
        context.provenance,
        &context,
        limits(false),
        &mut response,
    )
    .unwrap();
    assert!(run.work.tail_aliases > 0);
    assert_eq!(run.receipt.program.suffix(run.receipt.stage).unwrap(), expected);
    assert_eq!(run.receipt.pending.as_ref().unwrap().action, actions[0]);
}

#[test]
fn sibling_tail_scope_rebuilds_weighted_values_and_context_on_every_comparison() {
    let (root, original) = cohort();
    let mut context = context();
    let candidates = [Candidate::new(program(&[4, 0, 2, 3]))];
    let mut cache = CompletedCache::default();
    evaluate_with_cache(
        &candidates,
        &original,
        root,
        context.provenance,
        &context,
        limits(true),
        &mut cache,
        &mut response,
    )
    .unwrap();
    let current = [(original[1].0.clone(), 0.6), (original[0].0.clone(), 0.4)];
    context.posterior_revision += 1;
    context.seed += 1;
    let warm = evaluate_with_cache(
        &candidates,
        &current,
        root,
        context.provenance,
        &context,
        limits(true),
        &mut cache,
        &mut response,
    )
    .unwrap();
    let fresh = evaluate(
        &candidates,
        &current,
        root,
        context.provenance,
        &context,
        limits(false),
        &mut response,
    )
    .unwrap();
    assert_eq!(warm.values, fresh.values);
    assert_eq!(warm.executions, fresh.executions);
    assert_eq!(warm.receipt, fresh.receipt);
    let choice = warm.prepared().iter().find(|choice| choice.assessment.is_some()).unwrap();
    assert_eq!(choice.support, [(0, 0.6), (1, 0.4)]);
    assert_eq!(choice.context, context);
    assert_eq!(warm.work.represented_transitions, fresh.work.represented_transitions);
}

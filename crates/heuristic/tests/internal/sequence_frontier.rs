use super::*;
use crate::concepts::sequence::{Actions, Guard, Node};
use crate::concepts::value::CohortMethod;
use engine::{ActionType, Card, CardSet};

#[path = "sequence_frontier_lazy.rs"]
mod lazy;

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
    let mut first = opening();
    while first.game.state.all_tricks.is_empty() {
        let (actor, legal) = first.to_act().unwrap();
        let action = response(
            &first.view(actor),
            legal,
            LeafBudget {
                deadline: None,
                remaining_transitions: 100,
                remaining_solver_nodes: None,
            },
        )
        .unwrap()
        .action;
        first.apply(action);
    }
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

fn program(table: &Table, root: u8, actions: bool, order: &[usize]) -> Program {
    let cards: Vec<_> = table
        .to_act()
        .unwrap()
        .1
        .iter()
        .filter(|action| matches!(action.action_type, ActionType::CardPlayed(_)))
        .take(2)
        .copied()
        .collect();
    assert_eq!(cards.len(), 2, "fixture must reach genuinely multiple legal plays");
    let mask = cards
        .iter()
        .fold(CardSet::EMPTY, |set, action| match action.action_type {
            ActionType::CardPlayed(card) => set | CardSet::single(card.index()),
            _ => unreachable!(),
        });
    let target = Stage(if actions { 5 } else { 4 });
    let unknown_seat = (root + 1) % 4;
    let unknown_card = table.hand(unknown_seat).cards()[0];
    Program {
        name: "frontier-projection-fixture",
        version: 1,
        start: Stage(9),
        nodes: vec![
            Node::Tail,
            Node::Deferred,
            Node::Action {
                actions: Actions::Exact(cards[0].action_type),
                next: Stage(0),
            },
            Node::Action {
                actions: Actions::Exact(cards[1].action_type),
                next: Stage(0),
            },
            Node::Choice(vec![Stage(2), Stage(3)]),
            Node::Action {
                actions: Actions::Cards(mask),
                next: Stage(0),
            },
            Node::Guard {
                predicate: Guard::OwnCards(CardSet::EMPTY),
                yes: target,
                no: Stage(1),
                unknown: Stage(1),
            },
            Node::Guard {
                predicate: Guard::OwnCards(CardSet::ALL),
                yes: Stage(1),
                no: target,
                unknown: Stage(1),
            },
            Node::Guard {
                predicate: Guard::Known(beliefs::ConceptQuery::new(beliefs::Concept::Count {
                    seat: unknown_seat,
                    cards: CardSet::single(unknown_card.index()),
                    min: 1,
                    max: 1,
                })),
                yes: target,
                no: target,
                unknown: target,
            },
            Node::Choice(order.iter().copied().map(Stage).collect()),
        ],
    }
}

fn branches(program: &Program, stage: usize, root: u8, cohort: &[(Table, f64)]) -> Vec<Branch> {
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
                tricks: Vec::new(),
                consumed: Vec::new(),
                interrupted: false,
            }
        })
        .collect()
}

fn session<'a, F: FnMut(&SeatView<'_>, &[GameAction], LeafBudget) -> Result<LeafResponse, LeafFailure>>(
    root: u8,
    context: &'a Context,
    leaf: &'a mut F,
) -> Session<'a, F> {
    Session {
        root,
        provenance: context.provenance,
        context,
        limits: limits(false),
        work: Work::default(),
        leaf,
        responses: Vec::new(),
        hard_reads: Vec::new(),
        memo: Vec::new(),
        tails: Vec::new(),
        canonical: Vec::new(),
        memberships: Vec::new(),
        reusable_bytes: 0,
        response_bytes: 0,
        limit: None,
    }
}

#[test]
fn sibling_frontier_preserves_accepted_first_prefix_and_later_local_authorization() {
    let (root, cohort) = cohort();
    let context = context();
    let mut program = program(&cohort[0].0, root, true, &[6, 7]);
    // The current multi-action choice continues to another genuine Local
    // comparison on the next root turn, rather than directly to a Tail.
    let Node::Action { next, .. } = &mut program.nodes[5] else {
        unreachable!()
    };
    *next = Stage(10);
    program.nodes.push(Node::Choice(vec![Stage(0), Stage(2)]));
    let prefix_action = *cohort[0]
        .0
        .to_act()
        .unwrap()
        .1
        .iter()
        .filter(|action| matches!(action.action_type, ActionType::CardPlayed(_)))
        .nth(2)
        .expect("third legal play preserves both compared cards");
    program.nodes.push(Node::Action {
        actions: Actions::Exact(prefix_action.action_type),
        next: Stage(9),
    });
    program.start = Stage(11);
    let mut leaf = response;
    let mut session = session(root, &context, &mut leaf);
    let initial = branches(&program, 11, root, &cohort[..1]).remove(0);
    let mut prior_records = Vec::new();
    let mut reached = match session.drive(initial, &mut prior_records).unwrap() {
        Err(waiting) => waiting.branch,
        Ok(_) => panic!("prefix must reach the authored parent Choice"),
    };
    assert_eq!(reached.first.as_ref().unwrap().action, prefix_action);
    assert!(reached.first_program.is_some());
    assert!(
        reached.receipt.pending.is_none(),
        "prefix action was accepted by the engine"
    );
    assert!(!reached.tricks.is_empty(), "real completed-trick prefix");
    assert!(!reached.consumed.is_empty(), "real preceding action/Choice reads");
    // The interruption bit is inherited trace state, never eligibility evidence.
    // Exercise its transport too; the engine/history/receipt above are genuine.
    reached.interrupted = true;
    let children = |stage| {
        let mut branch = reached.clone();
        branch.receipt.stage = Stage(stage);
        vec![branch]
    };
    let first = children(6);
    let edge = session.normalized_frontier(&first).unwrap().unwrap();
    let before = session.work;
    let mut source_records = Vec::new();
    let source = session.finish(first, &mut source_records).unwrap();
    assert!(
        source_records
            .iter()
            .skip(1)
            .any(|record| record.scope == ChoiceScope::Local),
        "selected descendant must carry its own later Local assessment"
    );
    let dependency = FrontierDependency {
        edge,
        outcome_index: 0,
        transitions: session.work.represented_transitions - before.represented_transitions,
        comparisons: session.work.represented_comparisons - before.represented_comparisons,
        solver_nodes: session.work.represented_solver_nodes - before.represented_solver_nodes,
    };
    let own = children(7);
    let edge = session.normalized_frontier(&own).unwrap().unwrap();
    let outcomes = vec![source];
    let proofs = vec![source_records];
    let (actual, records) = session
        .alias_frontier(&own, &edge, &dependency, &outcomes, &proofs)
        .unwrap()
        .unwrap();
    let mut independent = Vec::new();
    let expected = session.finish(own, &mut independent).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(records, independent);
    assert_eq!(&records[1..], &proofs[0][1..]);
    for terminal in actual {
        assert_eq!(terminal.first, reached.first);
        assert_eq!(terminal.first_program, reached.first_program);
        assert!(terminal.tricks.starts_with(&reached.tricks));
        assert!(terminal.consumed.starts_with(&reached.consumed));
        assert!(terminal.interrupted);
    }
}

#[test]
fn sibling_frontier_values_order_and_evidence_match_separate_alternatives() {
    let (root, cohort) = cohort();
    let context = context();
    for actions in [false, true] {
        for order in [[6, 7, 8], [7, 8, 6], [8, 6, 7]] {
            let program = program(&cohort[0].0, root, actions, &order);
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
            let execution = &shared.executions[0];
            let outer = &execution.prepared[0];
            assert_eq!(outer.scope, ChoiceScope::Local);
            assert_eq!(outer.selected, 0, "preserve first-tie order");
            assert_eq!(outer.support, [(0, 0.25), (1, 0.75)]);
            let values = outer.assessment.as_ref().unwrap();
            for (actual, expected) in values.candidates.iter().zip(&reference.values.candidates) {
                assert_eq!(actual.difference, expected.difference);
                assert_eq!(actual.our_score, expected.our_score);
                assert_eq!(actual.opponent_score, expected.opponent_score);
            }
            assert_eq!(&execution.prepared[1..], reference.executions[0].prepared.as_slice());
            let mut expected = reference.executions[0].terminals.clone();
            for terminal in &mut expected {
                terminal.consumed.splice(0..0, outer.consumed.iter().cloned());
            }
            assert_eq!(execution.terminals, expected);
            assert_eq!(shared.work.frontier_aliases, 2);
            assert_eq!(shared.work.accounting(), "sibling-frontier-dag-v2");
            assert_eq!(
                shared.work.represented_transitions * 3,
                reference.work.represented_transitions
            );
            assert_eq!(
                shared.work.frontier_alias_transitions,
                shared.work.represented_transitions * 2
            );
            assert_eq!(shared.work.frontier_alias_comparisons, 2);
        }
    }
}

#[test]
fn sibling_frontier_projection_rebuilds_each_entry_and_action_owner_only() {
    let (root, cohort) = cohort();
    let context = context();
    for actions in [false, true] {
        let program = program(&cohort[0].0, root, actions, &[6, 7, 8]);
        let mut leaf = response;
        let mut session = session(root, &context, &mut leaf);
        let first = branches(&program, 6, root, &cohort);
        let edge = session.normalized_frontier(&first).unwrap().unwrap();
        let mut source_records = Vec::new();
        let source = session.finish(first, &mut source_records).unwrap();
        let dependency = FrontierDependency {
            edge,
            outcome_index: 0,
            transitions: session.work.represented_transitions,
            comparisons: session.work.represented_comparisons,
            solver_nodes: session.work.represented_solver_nodes,
        };
        let outcomes = vec![source];
        let proofs = vec![source_records];
        for stage in [7, 8] {
            let own = branches(&program, stage, root, &cohort);
            let edge = session.normalized_frontier(&own).unwrap().unwrap();
            let (projected, records) = session
                .alias_frontier(&own, &edge, &dependency, &outcomes, &proofs)
                .unwrap()
                .expect("eligible alias");
            let mut independent = Vec::new();
            let expected = session.finish(own, &mut independent).unwrap();
            assert_eq!(projected, expected);
            assert_eq!(records, independent);
            assert_eq!(&records[1..], &proofs[0][1..], "descendant authorization untouched");
            assert_eq!(records[0].semantic_suffix, program.suffix(Stage(stage)).unwrap());
            assert_ne!(records[0].consumed, proofs[0][0].consumed);
            if actions {
                assert_ne!(
                    projected[0].first_program, outcomes[0][0].first_program,
                    "multi-Action owner belongs to original alias guard entry"
                );
            } else {
                assert_eq!(
                    projected[0].first_program, outcomes[0][0].first_program,
                    "Choice's selected dependency owns the first offer"
                );
            }
        }
        // A mismatched complete proof/prefix is a miss, not a projected claim.
        let own = branches(&program, 7, root, &cohort);
        let edge = session.normalized_frontier(&own).unwrap().unwrap();
        for corruption in 0..5 {
            let mut broken = proofs.clone();
            match corruption {
                0 => broken[0][0].scope = ChoiceScope::Global,
                1 => broken[0][0].support[0].1 += 0.1,
                2 => broken[0][0].consumed.clear(),
                3 => broken[0][0].assessment.as_mut().unwrap().cohort_complete = false,
                _ => broken[0][0].context.posterior_revision += 1,
            }
            let before = session.work;
            assert!(session
                .alias_frontier(&own, &edge, &dependency, &outcomes, &broken)
                .unwrap()
                .is_none());
            assert_eq!(session.work, before);
        }
        let mut broken = outcomes.clone();
        broken[0][0].consumed.remove(0);
        assert!(session
            .alias_frontier(&own, &edge, &dependency, &broken, &proofs)
            .unwrap()
            .is_none());
    }
}

#[test]
fn sibling_frontier_cache_off_cold_warm_and_completed_parent_have_same_allowance() {
    let (root, cohort) = cohort();
    let context = context();
    for actions in [false, true] {
        let program = program(&cohort[0].0, root, actions, &[7, 8, 6]);
        let mut cosmetic = program.clone();
        cosmetic.name = "equivalent-parent";
        let candidates = [Candidate::new(program), Candidate::new(cosmetic)];
        let fresh = evaluate(
            &candidates,
            &cohort,
            root,
            context.provenance,
            &context,
            limits(false),
            &mut response,
        )
        .unwrap();
        let mut exact = limits(true);
        exact.transitions = fresh.work.represented_transitions;
        exact.comparisons = fresh.work.represented_comparisons;
        exact.solver_nodes = Some(fresh.work.represented_solver_nodes);
        let mut cache = CompletedCache::default();
        for _ in 0..2 {
            let cached = evaluate_with_cache(
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
            assert_eq!(cached.values, fresh.values);
            assert_eq!(cached.executions, fresh.executions);
            assert_eq!(cached.receipt, fresh.receipt);
            assert_eq!(cached.selection, fresh.selection);
            assert_eq!(cached.work.represented_transitions, fresh.work.represented_transitions);
            assert_eq!(cached.work.represented_comparisons, fresh.work.represented_comparisons);
            assert_eq!(
                cached.work.represented_solver_nodes,
                fresh.work.represented_solver_nodes
            );
            assert!(cached.work.suffix_hits > 0);
            // Leaf hits cannot skip value-store comparisons or alias creation.
            // These two deltas specifically require the completed parent hit.
            assert_eq!(fresh.work.comparisons - cached.work.comparisons, 2);
            assert_eq!(fresh.work.frontier_aliases, 4);
            assert_eq!(cached.work.frontier_aliases, 2);
            let mut too_small = exact;
            too_small.transitions -= 1;
            assert!(evaluate_with_cache(
                &candidates,
                &cohort,
                root,
                context.provenance,
                &context,
                too_small,
                &mut cache,
                &mut response
            )
            .is_err());
        }
        let mut too_small = limits(false);
        too_small.transitions = exact.transitions - 1;
        assert!(evaluate(
            &candidates,
            &cohort,
            root,
            context.provenance,
            &context,
            too_small,
            &mut response
        )
        .is_err());
    }
}

#[test]
fn sibling_frontier_rejects_different_successors_domains_and_nonlocal_frontiers() {
    let (root, cohort) = cohort();
    let context = context();
    let mut program = program(&cohort[0].0, root, true, &[6, 7, 8]);
    // Same concrete first actions but a genuinely different continuation.
    let Node::Action { actions, .. } = program.nodes[5].clone() else {
        unreachable!()
    };
    program.nodes.push(Node::Action {
        actions,
        next: Stage(4),
    });
    program.nodes.push(Node::Choice(vec![Stage(3), Stage(2)]));
    program.nodes.push(Node::Response { next: Stage(0) });
    program.nodes.push(Node::Reconsider {
        remaining: 1,
        retained: Stage(0),
    });
    let mut leaf = response;
    let mut session = session(root, &context, &mut leaf);
    let original = session
        .normalized_frontier(&branches(&program, 5, root, &cohort))
        .unwrap()
        .unwrap();
    let different = session
        .normalized_frontier(&branches(&program, 10, root, &cohort))
        .unwrap()
        .unwrap();
    assert_ne!(original.domain, different.domain);
    let choice = session
        .normalized_frontier(&branches(&program, 4, root, &cohort))
        .unwrap()
        .unwrap();
    let reordered = session
        .normalized_frontier(&branches(&program, 11, root, &cohort))
        .unwrap()
        .unwrap();
    assert_ne!(choice.domain, reordered.domain);
    assert_ne!(original.domain, choice.domain);
    for stage in [0, 1, 2, 3, 12, 13] {
        assert!(session
            .normalized_frontier(&branches(&program, stage, root, &cohort))
            .unwrap()
            .is_none());
    }
    let mut pending = branches(&program, 6, root, &cohort);
    let Frontier::Actions(offers) = session.inspect(&pending[0]).unwrap().frontier else {
        unreachable!()
    };
    pending[0].receipt.pending = Some(offers[0].clone());
    assert!(session.normalized_frontier(&pending).unwrap().is_none());
}

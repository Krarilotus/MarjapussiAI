use super::*;

/// Frozen eager V2 child loop: independent scheduling reference for lazy prep.
fn eager_child<F: FnMut(&SeatView<'_>, &[GameAction], LeafBudget) -> Result<LeafResponse, LeafFailure>>(
    session: &mut Session<'_, F>,
    children: Vec<Branch>,
    dependencies: &mut Vec<FrontierDependency>,
    outcomes: &[Vec<Terminal>],
    records: &[Vec<PreparedChoice>],
    child_records: &mut Vec<PreparedChoice>,
) -> Vec<Terminal> {
    let edge = session.normalized_frontier(&children).unwrap();
    if let Some(edge) = &edge {
        for dependency in dependencies.iter() {
            if let Some((terminals, projected)) = session
                .alias_frontier(&children, edge, dependency, outcomes, records)
                .unwrap()
            {
                *child_records = projected;
                return terminals;
            }
        }
    }
    let before = session.work;
    let terminals = session.finish(children, child_records).unwrap();
    if let Some(edge) = edge.filter(|edge| edge.validates(child_records, session.context)) {
        dependencies.push(FrontierDependency {
            edge,
            outcome_index: outcomes.len(),
            transitions: session.work.represented_transitions - before.represented_transitions,
            comparisons: session.work.represented_comparisons - before.represented_comparisons,
            solver_nodes: session.work.represented_solver_nodes - before.represented_solver_nodes,
        });
    }
    terminals
}

#[test]
fn sibling_frontier_lazy_matches_eager_with_late_misses_and_duplicate_raw_successors() {
    let (root, cohort) = cohort();
    let context = context();
    for actions in [false, true] {
        let mut program = program(&cohort[0].0, root, actions, &[6, 7, 8]);
        program.nodes.push(Node::Tail); // 10: distinct raw node, equal suffix.
        let (duplicate, mismatch) = if actions {
            let Node::Action { actions, .. } = program.nodes[5].clone() else {
                unreachable!()
            };
            program.nodes.push(Node::Action {
                actions: actions.clone(),
                next: Stage(10),
            }); // 11
            program.nodes.push(Node::Response { next: Stage(0) }); // 12
            program.nodes.push(Node::Action {
                actions,
                next: Stage(12),
            }); // 13
            (Stage(11), Stage(13))
        } else {
            let Node::Action { actions, .. } = program.nodes[2].clone() else {
                unreachable!()
            };
            program.nodes.push(Node::Action {
                actions,
                next: Stage(10),
            }); // 11
            program.nodes.push(Node::Choice(vec![Stage(11), Stage(3)])); // 12
            program.nodes.push(Node::Response { next: Stage(0) }); // 13
            let Node::Action { actions, .. } = program.nodes[3].clone() else {
                unreachable!()
            };
            program.nodes.push(Node::Action {
                actions,
                next: Stage(13),
            }); // 14
                // First successor equals the original, only the LAST one differs.
            program.nodes.push(Node::Choice(vec![Stage(2), Stage(14)])); // 15
            (Stage(12), Stage(15))
        };
        let Node::Guard { no, .. } = &mut program.nodes[7] else {
            unreachable!()
        };
        *no = duplicate;
        assert_eq!(
            program.suffix(duplicate).unwrap(),
            program.suffix(Stage(if actions { 5 } else { 4 })).unwrap()
        );
        let order = [mismatch.0, 6, 7, 8];
        for cached in [false, true] {
            let mut eager_leaf = response;
            let mut lazy_leaf = response;
            let mut eager = session(root, &context, &mut eager_leaf);
            let mut lazy = session(root, &context, &mut lazy_leaf);
            eager.limits = limits(cached);
            lazy.limits = limits(cached);
            for round in 0..2 {
                // Each round has a new lexical arena. The second cached round
                // admits completed parent memo results rather than fresh runs.
                let mut eager_dependencies = Vec::new();
                let mut lazy_dependencies = Vec::new();
                let mut eager_outcomes = Vec::new();
                let mut lazy_outcomes = Vec::new();
                let mut eager_records = Vec::new();
                let mut lazy_records = Vec::new();
                let before = lazy.work;
                for stage in order {
                    let mut expected_records = Vec::new();
                    let expected = eager_child(
                        &mut eager,
                        branches(&program, stage, root, &cohort),
                        &mut eager_dependencies,
                        &eager_outcomes,
                        &eager_records,
                        &mut expected_records,
                    );
                    let mut actual_records = Vec::new();
                    let actual = lazy
                        .finish_frontier_child(
                            branches(&program, stage, root, &cohort),
                            &mut lazy_dependencies,
                            &lazy_outcomes,
                            &lazy_records,
                            &mut actual_records,
                        )
                        .unwrap();
                    assert_eq!(actual, expected, "all origins, prefixes, owners and outcomes");
                    assert_eq!(
                        actual_records, expected_records,
                        "all entry and selected descendant certificates"
                    );
                    assert_eq!(lazy.work.represented_transitions, eager.work.represented_transitions);
                    assert_eq!(lazy.work.represented_comparisons, eager.work.represented_comparisons);
                    assert_eq!(lazy.work.represented_solver_nodes, eager.work.represented_solver_nodes);
                    assert_eq!(lazy.work.frontier_aliases, eager.work.frontier_aliases);
                    assert_eq!(
                        lazy.work.frontier_alias_transitions,
                        eager.work.frontier_alias_transitions
                    );
                    assert_eq!(
                        lazy.work.frontier_alias_comparisons,
                        eager.work.frontier_alias_comparisons
                    );
                    eager_outcomes.push(expected);
                    lazy_outcomes.push(actual);
                    eager_records.push(expected_records);
                    lazy_records.push(actual_records);
                }
                assert_eq!(
                    lazy.work.frontier_aliases - before.frontier_aliases,
                    2,
                    "late mismatch cannot hide the later equivalent representative"
                );
                if cached && round == 1 {
                    assert_eq!(
                        lazy.work.comparisons, before.comparisons,
                        "completed parent hits, not just tail/response hits"
                    );
                }
            }
        }
    }
}

#[test]
fn sibling_frontier_lazy_probe_and_shape_rejection_prepare_no_canonical_proof() {
    let (root, cohort) = cohort();
    let context = context();
    let program = program(&cohort[0].0, root, true, &[6, 7, 8]);
    let mut leaf = response;
    let mut session = session(root, &context, &mut leaf);
    session.limits = limits(true);
    let children = branches(&program, 6, root, &cohort);
    let probe = session.frontier_probe(&children).unwrap().unwrap();
    let tail = program.suffix(Stage(0)).unwrap();
    assert!(!domain_shape_matches(
        &probe,
        &[Selection::Stage(tail.clone()), Selection::Stage(tail.clone())]
    ));
    let Frontier::Actions(offers) = &probe else {
        unreachable!()
    };
    let mut domain: Vec<_> = offers
        .iter()
        .map(|offer| Selection::Offer {
            action: offer.action,
            next: tail.clone(),
        })
        .collect();
    assert!(domain_shape_matches(&probe, &domain));
    domain.reverse();
    assert!(!domain_shape_matches(&probe, &domain), "ordered action mismatch");
    domain.pop();
    assert!(!domain_shape_matches(&probe, &domain), "arity mismatch");
    assert!(session.canonical.is_empty());
    assert!(session.memberships.is_empty());
    assert_eq!(session.work, Work::default());
    // The final proof gate still does this work, only when it is needed.
    assert!(session.normalized_frontier(&children).unwrap().is_some());
    assert!(!session.canonical.is_empty());
    assert!(!session.memberships.is_empty());
}

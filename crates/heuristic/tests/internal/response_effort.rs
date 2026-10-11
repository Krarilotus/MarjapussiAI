use super::*;
use crate::{HeuristicAgent, Params, RuleId};
use agent::Table;
use engine::ActionType;
use std::time::Duration;

/// Legal all-pass play, avoiding questions only while constructing a prefix.
/// The response itself retains all legal questions and its eight-world solver.
fn two_card_table() -> Table {
    let hands = std::array::from_fn(|seat| (0..9).map(|i| engine::Card::from_index((4 * i + seat) as u8)).collect());
    let mut table = Table::new(&hands);
    for _ in 0..128 {
        let (seat, legal) = table.to_act().expect("fixture must reach an endgame");
        let cards: Vec<_> = legal
            .iter()
            .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
            .copied()
            .collect();
        if table.view(seat).knowledge.hand_len(seat) == 2 && cards.len() > 1 {
            assert_eq!(table.view(seat).knowledge.declarer(), None);
            return table;
        }
        let action = legal
            .iter()
            .find(|a| a.action_type == ActionType::StopBidding)
            .copied()
            .or_else(|| cards.first().copied())
            .expect("bid or legal card");
        table.apply(action);
    }
    panic!("fixture did not make finite progress");
}

fn actor() -> HeuristicAgent {
    HeuristicAgent::new(Params {
        plan_worlds: 0,
        return_worlds: 0,
        endgame_tricks: 2,
        endgame_worlds: 8,
        ..Params::default()
    })
}

fn deadline() -> Option<Instant> {
    // Safety guard for the test, not a new production allowance or node cap.
    Some(Instant::now() + Duration::from_secs(60))
}

#[test]
fn response_reports_equal_canonical_effort_after_solver_reuse_and_honors_cap() {
    let table = two_card_table();
    let (seat, legal) = table.to_act().unwrap();
    let view = table.view(seat);
    let mut policy = actor();
    let full = policy
        .completed_response_with_effort_before(&view, legal, deadline(), None)
        .unwrap();
    assert_eq!(full.decision.rule, RuleId::HE1);
    assert!(full.canonical_solver_nodes > 0);
    let warm = policy
        .completed_response_with_effort_before(&view, legal, deadline(), Some(full.canonical_solver_nodes))
        .unwrap();
    assert_eq!(warm, full);
    let cap = full.canonical_solver_nodes - 1;
    let failed = policy
        .completed_response_with_effort_before(&view, legal, deadline(), Some(cap))
        .unwrap_err();
    assert_eq!(failed.canonical_solver_nodes, cap);
    assert!(matches!(
        failed.reason,
        ResponseFailureReason::Solver(
            solver::CompletionFailureReason::Incomplete { .. }
                | solver::CompletionFailureReason::Solver(solver::SolverError::Interrupted)
        )
    ));
    assert_eq!(
        policy
            .completed_response_with_effort_before(&view, legal, deadline(), None)
            .unwrap(),
        full
    );
    assert_eq!(
        policy.completed_response_before(&view, legal, deadline()).unwrap(),
        full.decision
    );
}

#[test]
fn solver_failure_cannot_publish_fallback_and_forced_action_needs_zero_nodes() {
    let table = two_card_table();
    let (seat, legal) = table.to_act().unwrap();
    let view = table.view(seat);
    let mut policy = actor();
    let failed = policy
        .completed_response_with_effort_before(&view, legal, deadline(), Some(0))
        .unwrap_err();
    assert_eq!(
        failed,
        ResponseFailure {
            reason: ResponseFailureReason::Solver(solver::CompletionFailureReason::Solver(
                solver::SolverError::Interrupted
            )),
            canonical_solver_nodes: 0,
        }
    );
    let card = *legal
        .iter()
        .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
        .unwrap();
    let forced = policy
        .completed_response_with_effort_before(&view, &[card], deadline(), Some(0))
        .unwrap();
    assert_eq!(forced.decision.rule, RuleId::HX1);
    assert_eq!(forced.canonical_solver_nodes, 0);
    let expired = Some(Instant::now() - Duration::from_secs(1));
    let failed = policy
        .completed_response_with_effort_before(&view, &[card], expired, None)
        .unwrap_err();
    assert_eq!(
        failed,
        ResponseFailure {
            reason: ResponseFailureReason::DeadlineElapsed,
            canonical_solver_nodes: 0
        }
    );
    // Actual play still selects its bounded policy action after a strict failure.
    assert!(legal.contains(&policy.decide_before(&view, legal, expired).action));
}

#[test]
fn cumulative_allowance_and_original_failure_survive_outer_deadline() {
    let state = ResponseState::new(Some(20));
    let failure = solver::CompletionFailure {
        reason: solver::CompletionFailureReason::Incomplete { completed_worlds: 2 },
        effort: solver::CanonicalEffort { nodes: 7 },
    };
    assert!(state.record_solver(Err(failure)).is_none());
    assert_eq!(state.remaining_solver_nodes(), Some(13));
    state.fail(ResponseFailureReason::DeadlineElapsed);
    assert_eq!(
        state.failure(),
        Some(ResponseFailure {
            reason: ResponseFailureReason::Solver(failure.reason),
            canonical_solver_nodes: 7,
        })
    );
    assert_eq!(ResponseState::new(None).remaining_solver_nodes(), None);
}

#[test]
fn strict_response_rejects_recursive_forecasts_before_touching_actor_state() {
    let table = two_card_table();
    let (seat, legal) = table.to_act().unwrap();
    let view = table.view(seat);
    for (plan_worlds, return_worlds) in [(1, 0), (0, 1), (8, 8)] {
        let mut policy = actor();
        policy.params.plan_worlds = plan_worlds;
        policy.params.return_worlds = return_worlds;
        let parameters = policy.params.clone();
        // A forced response must not bypass the model-isolation contract either.
        for actions in [legal, &legal[..1]] {
            let failed = policy
                .completed_response_with_effort_before(&view, actions, deadline(), None)
                .unwrap_err();
            assert_eq!(
                failed,
                ResponseFailure {
                    reason: ResponseFailureReason::NonIsolated {
                        plan_worlds,
                        return_worlds,
                        retained_intent: false,
                    },
                    canonical_solver_nodes: 0,
                }
            );
            assert!(policy.mind.is_none(), "rejection must precede synchronization");
            assert!(policy.solver.borrow().is_none());
            assert!(policy.pending_decision.is_none());
            assert_eq!(policy.params, parameters, "no silent parameter changes");
        }
        assert!(policy.completed_response_before(&view, legal, deadline()).is_none());
        // The ordinary live entry point retains its original policy contract.
        assert_eq!(policy.decide(&view, &legal[..1]).action, legal[0]);
    }
}

#[test]
fn strict_response_rejects_retained_intent_without_consuming_or_clearing_it() {
    use crate::concepts::sequence::{Actions, Node, Program, Receipt, Stage};
    use crate::concepts::value::{CohortMethod, Provenance};
    use crate::planning::retained::RetainedPlan;
    use crate::planning::sequence::Context;
    use crate::planning::{BeliefScope, PlanningContext};

    let table = two_card_table();
    let (seat, legal) = table.to_act().unwrap();
    let view = table.view(seat);
    let mut policy = actor();
    let params = policy.params.clone();
    let program = Program {
        name: "isolation-retained-intent",
        version: 1,
        start: Stage(1),
        nodes: vec![
            Node::Tail,
            Node::Action {
                actions: Actions::Cards(view.knowledge.certain(seat)),
                next: Stage(0),
            },
        ],
    };
    let provenance = Provenance {
        belief_model: "isolation-test",
        response_model: "isolation-test",
        cohort: CohortMethod::Enumerated,
    };
    let plan = RetainedPlan {
        receipt: Receipt::new(program, &view).unwrap(),
        prepared: std::sync::Arc::new(Vec::new()),
        parameters: params.clone(),
        origin: PlanningContext {
            observer: seat,
            belief_scope: BeliefScope::CurrentInformation,
            knowledge: *view.knowledge,
            events: view.events.to_vec(),
            kinds: view.kinds,
            series: view.series,
            response_params: params.clone(),
            seed: 0,
            possible_allocations: 1,
            requested_worlds: 1,
            proposal_draws: 0,
        },
        context: Context {
            params: params.clone(),
            seed: 0,
            posterior_revision: 0,
            provenance,
        },
        review_pending: true,
        raw_events: view.events.to_vec(),
    };
    *policy.intent.get_mut() = Some(plan.clone());
    let failed = policy
        .completed_response_with_effort_before(&view, &legal[..1], deadline(), None)
        .unwrap_err();
    assert_eq!(
        failed,
        ResponseFailure {
            reason: ResponseFailureReason::NonIsolated {
                plan_worlds: 0,
                return_worlds: 0,
                retained_intent: true,
            },
            canonical_solver_nodes: 0,
        }
    );
    assert_eq!(policy.intent.borrow().as_ref(), Some(&plan));
    assert_eq!(policy.params, params);
    assert!(policy.mind.is_none());
    assert!(policy.solver.borrow().is_none());
}

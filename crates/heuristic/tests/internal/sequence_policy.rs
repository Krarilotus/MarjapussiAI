use super::*;
use crate::concepts::sequence::{Actions, Information, Node, Offer, Program, Receipt, Stage};
use crate::concepts::value::{CohortMethod, Provenance};
use crate::planning::retained::RetainedPlan;
use crate::planning::sequence::Context;
use crate::planning::{BeliefScope, PlanningContext};
use agent::Table;

fn table() -> Table {
    let hands = std::array::from_fn(|seat| (0..9).map(|i| engine::Card::from_index((4 * i + seat) as u8)).collect());
    let mut table = Table::new(&hands);
    while table
        .to_act()
        .unwrap()
        .1
        .iter()
        .any(|a| a.action_type == ActionType::StopBidding)
    {
        let action = *table
            .to_act()
            .unwrap()
            .1
            .iter()
            .find(|a| a.action_type == ActionType::StopBidding)
            .unwrap();
        table.apply(action);
    }
    loop {
        let (_, legal) = table.to_act().unwrap();
        let choices: Vec<_> = legal
            .iter()
            .filter(|a| a.action_type != ActionType::UndoRequest)
            .copied()
            .collect();
        if choices.len() > 1 {
            break;
        }
        table.apply(choices[0]);
    }
    table
}
fn planned(table: &Table) -> (HeuristicAgent, GameAction) {
    let (seat, legal) = table.to_act().unwrap();
    let action = *legal
        .iter()
        .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
        .unwrap();
    let view = table.view(seat);
    let program = Program {
        name: "policy-intent-regression",
        version: 1,
        start: Stage(1),
        nodes: vec![
            Node::Tail,
            Node::Action {
                actions: Actions::Exact(action.action_type),
                next: Stage(2),
            },
            Node::Action {
                actions: Actions::Cards(view.knowledge.certain(seat)),
                next: Stage(0),
            },
        ],
    };
    let params = Params {
        plan_worlds: 1,
        return_worlds: 0,
        endgame_tricks: 0,
        ..Params::default()
    };
    let mut policy = HeuristicAgent::new(params.clone());
    let mut reader = beliefs::Mind::new(
        seat,
        view.knowledge.dealt_certain(seat),
        view.kinds,
        params.beliefs.clone(),
    );
    for event in view.events {
        reader.apply(event).unwrap();
    }
    let canonical = SeatView {
        seat,
        knowledge: reader.knowledge(),
        events: reader.events(),
        kinds: view.kinds,
        series: view.series,
    };
    let provenance = Provenance {
        belief_model: "test-measure",
        response_model: "test-response",
        cohort: CohortMethod::Enumerated,
    };
    let context = Context {
        params: params.clone(),
        seed: 0,
        posterior_revision: 0,
        provenance,
    };
    let origin = PlanningContext {
        observer: seat,
        belief_scope: BeliefScope::CurrentInformation,
        knowledge: *view.knowledge,
        events: canonical.events.to_vec(),
        kinds: view.kinds,
        series: view.series,
        response_params: params.clone(),
        seed: 0,
        possible_allocations: 1,
        requested_worlds: 1,
        proposal_draws: 0,
    };
    let mut receipt = Receipt::new(program, &canonical).unwrap();
    receipt
        .propose(Offer { action, next: Stage(2) }, &canonical, legal)
        .unwrap();
    *policy.intent.get_mut() = Some(RetainedPlan {
        receipt,
        pending_legal: crate::legal_actions::eligible(legal),
        prepared: std::sync::Arc::new(Vec::new()),
        parameters: params,
        origin,
        context,
        review_pending: false,
        raw_events: view.events.to_vec(),
    });
    (policy, action)
}
#[test]
fn retry_and_clone_preserve_private_intent_without_spending_cards_or_replanning() {
    let table = table();
    let (seat, legal) = table.to_act().unwrap();
    let view = table.view(seat);
    let (mut policy, expected) = planned(&table);
    let before = policy.intent.borrow().as_ref().unwrap().receipt.clone();
    for _ in 0..2 {
        assert_eq!(policy.decide(&view, legal).action, expected);
        assert_eq!(policy.intent.borrow().as_ref().unwrap().receipt, before);
        assert_eq!(policy.last_sequence().unwrap().mode, PlanMode::Retry);
        assert_eq!(policy.last_sequence().unwrap().work.transitions, 0);
    }
    let mut fork = policy.clone();
    assert!(fork.mind.is_none());
    assert_eq!(fork.decide(&view, legal).action, expected);
    assert_eq!(fork.intent.borrow().as_ref().unwrap().receipt, before);
    // Dropping disposable solver/reader caches leaves semantic intent intact.
    policy.mind = None;
    *policy.solver.get_mut() = None;
    assert_eq!(policy.decide(&view, legal).action, expected);
    assert_eq!(policy.intent.borrow().as_ref().unwrap().receipt, before);
}

#[test]
fn retained_retry_requires_the_complete_original_eligible_domain() {
    let table = table();
    let (seat, raw_legal) = table.to_act().unwrap();
    let legal = crate::legal_actions::eligible(raw_legal);
    let view = table.view(seat);
    let (mut policy, action) = planned(&table);
    let plan = policy.intent.get_mut().as_mut().unwrap();
    assert!(legal.len() > 1);
    assert_eq!(plan.pending(&view, &legal).unwrap().action, action);
    let singleton = [action];
    assert!(
        plan.pending(&view, &singleton).is_none(),
        "a narrowed domain is not a retry"
    );
    // A proposal initially authorized in a restricted domain must not hide a
    // newly available alternative merely because its old action remains legal.
    plan.pending_legal = singleton.to_vec();
    assert_eq!(plan.pending(&view, &singleton).unwrap().action, action);
    assert!(plan.pending(&view, &legal).is_none(), "an expanded domain needs review");
    // Eligible-domain normalization is shared with live/model policy. Undo is
    // not an alternative and input permutation does not change that domain.
    plan.pending_legal = legal.clone();
    let mut reordered = raw_legal.to_vec();
    reordered.reverse();
    assert_eq!(plan.pending(&view, &reordered).unwrap().action, action);
}

#[test]
fn accepted_and_forced_proposals_clear_and_replace_the_pending_domain() {
    let mut table = table();
    let seat = table.to_act().unwrap().0;
    let (mut policy, first) = planned(&table);
    let params = policy.params.clone();
    table.apply(first);
    loop {
        let (actor, legal) = table.to_act().unwrap();
        if actor == seat {
            break;
        }
        let action = *legal
            .iter()
            .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
            .unwrap();
        table.apply(action);
    }
    let view = table.view(seat);
    let plan = policy.intent.get_mut().as_mut().unwrap();
    plan.synchronize(&view, &view, &params).unwrap();
    assert!(plan.receipt.pending.is_none());
    assert!(plan.pending_legal.is_empty(), "accepted proposals have no retry domain");
    let next = *table
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
        .unwrap();
    policy.decide(&view, &[next]);
    let plan = policy.intent.borrow();
    let plan = plan.as_ref().unwrap();
    assert_eq!(plan.pending_legal, vec![next]);
    assert_eq!(plan.pending(&view, &[next]).unwrap().action, next);
}
#[test]
fn forced_actions_confirm_the_previous_step_and_propose_the_next_exactly_once() {
    let mut table = table();
    let seat = table.to_act().unwrap().0;
    let (mut policy, first) = planned(&table);
    assert_eq!(policy.decide(&table.view(seat), &[first]).action, first);
    assert_eq!(policy.intent.borrow().as_ref().unwrap().receipt.stage, Stage(1));
    table.apply(first);
    loop {
        let (actor, legal) = table.to_act().unwrap();
        if actor == seat {
            break;
        }
        let action = *legal
            .iter()
            .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
            .unwrap();
        table.apply(action);
    }
    let next = *table
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
        .unwrap();
    assert_eq!(policy.decide(&table.view(seat), &[next]).action, next);
    let receipt = policy.intent.borrow().as_ref().unwrap().receipt.clone();
    assert_eq!(receipt.stage, Stage(2));
    assert_eq!(receipt.pending.as_ref().unwrap().action, next);
    assert_eq!(receipt.accepted, Information::of(&table.view(seat)));
    policy.decide(&table.view(seat), &[next]);
    assert_eq!(policy.intent.borrow().as_ref().unwrap().receipt, receipt);
}
#[test]
fn changing_the_model_invalidates_old_intent_even_on_a_forced_turn() {
    let table = table();
    let seat = table.to_act().unwrap().0;
    let (mut policy, first) = planned(&table);
    policy.params.seed = 99;
    let decision = policy.decide(&table.view(seat), &[first]);
    assert_eq!(decision.action, first);
    assert!(policy.intent.borrow().is_none());
    assert!(decision.why.iter().any(|f| f.key == "sequence_context_reset"));
}

#[test]
fn undo_repair_uses_canonical_events_but_detects_each_new_raw_undo_once() {
    let mut table = table();
    let seat = table.to_act().unwrap().0;
    let (mut old, first) = planned(&table);
    old.decide(&table.view(seat), &[first]);
    table.apply(first);
    for (actor, kind) in [
        (seat, ActionType::UndoRequest),
        ((seat + 1) % 4, ActionType::UndoAccept),
        ((seat + 3) % 4, ActionType::UndoAccept),
    ] {
        let action = GameAction {
            player: engine::PlaceAtTable(actor),
            action_type: kind,
        };
        assert!(table.game.is_legal(&action));
        table.apply(action);
    }
    let reset = old.decide(&table.view(seat), &[first]);
    assert!(reset.why.iter().any(|f| f.key == "sequence_context_reset"));
    assert!(old.intent.borrow().is_none());
    // Install a newly formed semantic plan after the accepted undo. Its raw
    // envelope contains the undo marker, while its executable history does not.
    let (mut repaired, action) = planned(&table);
    assert!(repaired
        .intent
        .borrow()
        .as_ref()
        .unwrap()
        .raw_events
        .contains(&observation::SeatEvent::Undone));
    assert!(!repaired
        .intent
        .borrow()
        .as_ref()
        .unwrap()
        .receipt
        .accepted
        .events
        .contains(&observation::SeatEvent::Undone));
    for _ in 0..2 {
        let retry = repaired.decide(&table.view(seat), &[action]);
        assert!(!retry.why.iter().any(|f| f.key == "sequence_context_reset"));
        assert!(repaired.intent.borrow().is_some());
    }
    table.apply(action);
    loop {
        let (actor, legal) = table.to_act().unwrap();
        if actor == seat {
            break;
        }
        let action = *legal
            .iter()
            .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
            .unwrap();
        table.apply(action);
    }
    let next = *table
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
        .unwrap();
    repaired.decide(&table.view(seat), &[next]);
    assert_eq!(repaired.intent.borrow().as_ref().unwrap().receipt.stage, Stage(2));
}

#[test]
fn an_observed_global_choice_does_not_certify_an_unrecorded_tail_action() {
    use crate::concepts::sequence::with_reconsideration;
    use crate::planning::sequence::{evaluate, Candidate, LeafBudget, LeafResponse, Limits};
    let hands = std::array::from_fn(|seat| (0..9).map(|c| engine::Card::from_index((9 * seat + c) as u8)).collect());
    let mut table = Table::new(&hands);
    for kind in [
        ActionType::NewBid(120),
        ActionType::StopBidding,
        ActionType::StopBidding,
        ActionType::StopBidding,
    ] {
        let action = *table
            .to_act()
            .unwrap()
            .1
            .iter()
            .find(|a| a.action_type == kind)
            .unwrap();
        table.apply(action);
    }
    let mut received = None;
    while table.game.state.phase != engine::GamePhase::PassingBack {
        let action = table.to_act().unwrap().1[0];
        if let ActionType::Pass(cards) = action.action_type {
            received = Some(engine::CardSet::from_cards(&cards));
        }
        table.apply(action);
    }
    let returned = *table.to_act().unwrap().1.iter().find(|a| {
        matches!(a.action_type, ActionType::Pass(cards) if Some(engine::CardSet::from_cards(&cards)) == received)
    }).unwrap();
    table.apply(returned);
    let mut leaf = |_: &SeatView<'_>, legal: &[GameAction], _: LeafBudget| {
        Ok(LeafResponse::unit(
            *legal
                .iter()
                .find(|a| matches!(a.action_type, ActionType::Answer(_)))
                .or_else(|| {
                    legal
                        .iter()
                        .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
                })
                .unwrap(),
        ))
    };
    // Keep the pair until the last two cards. Every suit is held by a single
    // seat, so either remaining green card wins; the response-tail tie is real.
    while table.view(0).knowledge.hand_len(0) > 2 || table.to_act().unwrap().0 != 0 {
        let (seat, legal) = table.to_act().unwrap();
        let action = if seat == 0 {
            *legal
                .iter()
                .find(|action| {
                    matches!(action.action_type,
                ActionType::CardPlayed(card) if !engine::CardSet::HALVES.contains(card.index()))
                })
                .unwrap()
        } else {
            leaf(
                &table.view(seat),
                legal,
                LeafBudget {
                    deadline: None,
                    remaining_transitions: 100,
                    remaining_solver_nodes: None,
                },
            )
            .unwrap()
            .action
        };
        table.apply(action);
    }
    let (seat, legal) = table.to_act().unwrap();
    let announce = *legal
        .iter()
        .find(|a| matches!(a.action_type, ActionType::AnnounceTrump(_)))
        .unwrap();
    let params = Params {
        plan_worlds: 1,
        return_worlds: 0,
        endgame_tricks: 0,
        ..Params::default()
    };
    let (mut policy, _) = planned(&table);
    let (origin, context) = {
        let plan = policy.intent.borrow();
        let plan = plan.as_ref().unwrap();
        (plan.origin.clone(), plan.context.clone())
    };
    policy.params = params.clone();
    let program = with_reconsideration(
        &Program {
            name: "activate-then-global-choice",
            version: 1,
            start: Stage(1),
            nodes: vec![
                Node::Tail,
                Node::Action {
                    actions: Actions::Exact(announce.action_type),
                    next: Stage(0),
                },
            ],
        },
        1,
    )
    .unwrap();
    let compared = evaluate(
        &[Candidate::new(program)],
        &[(table.clone(), 1.0)],
        seat,
        context.provenance,
        &context,
        Limits {
            transitions: 1_000_000,
            comparisons: 4096,
            memo_bytes: 64 * 1024 * 1024,
            ..Limits::default()
        },
        &mut leaf,
    )
    .unwrap();
    assert_eq!(compared.executions[0].first.action, announce);
    *policy.intent.get_mut() = Some(RetainedPlan {
        receipt: compared.receipt,
        pending_legal: crate::legal_actions::eligible(legal),
        prepared: std::sync::Arc::new(compared.prepared),
        parameters: params,
        origin,
        context,
        review_pending: false,
        raw_events: table.view(seat).events.to_vec(),
    });
    table.apply(announce);
    let (seat, legal) = table.to_act().unwrap();
    let view = table.view(seat);
    let choice = policy.decide(&view, legal);
    assert!(
        !matches!(
            policy.last_sequence().map(|trace| trace.mode),
            Some(PlanMode::Prepared | PlanMode::Retry)
        ),
        "a ranked tail without a recorded first response requires review"
    );
    assert!(legal.contains(&choice.action));
}

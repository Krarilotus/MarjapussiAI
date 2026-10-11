use super::super::{
    sequence::{Context, Limits},
    BeliefScope,
};
use super::*;
use crate::concepts::sequence::{inspect, Frontier, Information, Offer, Receipt};
use crate::concepts::value::{CohortMethod, Provenance};
use agent::{SeatView, Table};
use beliefs::{BeliefParams, Mind};
use engine::{ActionType, Card, CardSet, GamePhase};
use std::cell::{Cell, RefCell};

fn table() -> Table {
    let hands: [Vec<Card>; 4] =
        std::array::from_fn(|seat| (0..9).map(|card| Card::from_index((seat * 9 + card) as u8)).collect());
    Table::new(&hands)
}

fn reader(view: &SeatView<'_>) -> Mind {
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

fn response(
    _: &SeatView<'_>,
    legal: &[GameAction],
    _: sequence::LeafBudget,
) -> Result<sequence::LeafResponse, sequence::LeafFailure> {
    Ok(sequence::LeafResponse::unit(
        *legal
            .iter()
            .find(|a| a.action_type == ActionType::StopBidding)
            .or_else(|| legal.iter().find(|a| matches!(a.action_type, ActionType::Pass(_))))
            .or_else(|| {
                legal
                    .iter()
                    .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
            })
            .unwrap_or(&legal[0]),
    ))
}

fn respond(table: &mut Table) {
    let (seat, legal) = table.to_act().unwrap();
    let action = response(
        &table.view(seat),
        legal,
        sequence::LeafBudget {
            deadline: None,
            remaining_transitions: 1000,
            remaining_solver_nodes: None,
        },
    )
    .unwrap();
    table.apply(action.action);
}

fn minimum_bid(table: &Table) -> GameAction {
    *table
        .to_act()
        .unwrap()
        .1
        .iter()
        .filter(|a| matches!(a.action_type, ActionType::NewBid(_)))
        .min_by_key(|a| match a.action_type {
            ActionType::NewBid(value) => value,
            _ => unreachable!(),
        })
        .unwrap()
}

fn at_return() -> Table {
    let mut table = table();
    table.apply(minimum_bid(&table));
    for _ in 0..16 {
        if table.game.state.phase == GamePhase::PassingBack {
            return table;
        }
        respond(&mut table);
    }
    panic!("fixture did not reach return");
}

fn context() -> Context {
    Context {
        params: crate::Params::default(),
        seed: 7,
        posterior_revision: 0,
        provenance: Provenance {
            belief_model: "synthetic-joint",
            response_model: "visible-first-legal",
            cohort: CohortMethod::Enumerated,
        },
    }
}

#[test]
fn auction_receipt_waits_through_exchange_before_binding_play() {
    let mut live = table();
    let bid = minimum_bid(&live);
    let root = bid.player.0;
    let candidate = intervention(bid, PHASE_RESPONSES);
    let mut receipt = Receipt::new(candidate.program, &live.view(root)).unwrap();
    let legal = live.to_act().unwrap().1;
    let Frontier::Actions(offers) = inspect(
        &receipt.program,
        receipt.stage,
        &live.view(root),
        legal,
        &reader(&live.view(root)),
    )
    .unwrap()
    .frontier
    else {
        panic!("intervention")
    };
    assert_eq!(offers.len(), 1);
    assert_eq!(offers[0].action, bid);
    receipt.propose(offers[0].clone(), &live.view(root), legal).unwrap();
    live.apply(bid);
    for _ in 0..16 {
        if live.to_act().unwrap().0 == root {
            break;
        }
        respond(&mut live);
    }
    assert_eq!(live.game.state.phase, GamePhase::PassingBack);
    receipt.synchronize(&live.view(root)).unwrap();
    let legal = live.to_act().unwrap().1;
    let Frontier::Response { next } = inspect(
        &receipt.program,
        receipt.stage,
        &live.view(root),
        legal,
        &reader(&live.view(root)),
    )
    .unwrap()
    .frontier
    else {
        panic!("return must be an earlier-phase response")
    };
    let action = legal[0];
    assert!(matches!(action.action_type, ActionType::Pass(_)));
    receipt
        .propose(Offer { action, next }, &live.view(root), legal)
        .unwrap();
    live.apply(action);
    receipt.synchronize(&live.view(root)).unwrap();
    assert!(receipt.pending.is_none());
    let inspection = inspect(
        &receipt.program,
        receipt.stage,
        &live.view(root),
        live.to_act().unwrap().1,
        &reader(&live.view(root)),
    )
    .unwrap();
    assert!(matches!(inspection.frontier, Frontier::Reconsider { .. }));
}

#[test]
fn return_receipt_spends_exact_cards_before_formation_and_rejects_another_allocation() {
    let original = at_return();
    let (root, legal) = original.to_act().unwrap();
    let action = legal[0];
    let ActionType::Pass(cards) = action.action_type else {
        panic!("return")
    };
    let returned = CardSet::from_cards(&cards);
    let own = original.view(root).knowledge.certain(root);
    let candidate = intervention(action, PHASE_RESPONSES);
    let mut receipt = Receipt::new(candidate.program, &original.view(root)).unwrap();
    let Frontier::Actions(offers) = inspect(
        &receipt.program,
        receipt.stage,
        &original.view(root),
        legal,
        &reader(&original.view(root)),
    )
    .unwrap()
    .frontier
    else {
        panic!("allocation")
    };
    receipt.propose(offers[0].clone(), &original.view(root), legal).unwrap();
    let mut live = original.clone();
    live.apply(action);
    receipt.synchronize(&live.view(root)).unwrap();
    assert_eq!(live.view(root).knowledge.certain(root), own - returned);
    assert!(returned.is_subset(live.view(root).knowledge.certain(engine::partner(root))));
    assert!(matches!(
        inspect(
            &receipt.program,
            receipt.stage,
            &live.view(root),
            live.to_act().unwrap().1,
            &reader(&live.view(root))
        )
        .unwrap()
        .frontier,
        Frontier::Reconsider { .. }
    ));

    let alternative = *legal.iter().find(|other| **other != action).unwrap();
    let mut wrong = Receipt::new(receipt.program.clone(), &original.view(root)).unwrap();
    wrong.propose(offers[0].clone(), &original.view(root), legal).unwrap();
    let mut changed = original.clone();
    changed.apply(alternative);
    assert_eq!(
        wrong.synchronize(&changed.view(root)),
        Err(SequenceError::HistoryChanged)
    );
}

#[test]
fn an_exhausted_phase_gate_defers_instead_of_finishing_a_tail() {
    let initial = table();
    let bid = minimum_bid(&initial);
    let context = context();
    let result = sequence::evaluate(
        &[intervention(bid, 0)],
        &[(initial, 1.0)],
        bid.player.0,
        context.provenance,
        &context,
        Limits::default(),
        &mut response,
    );
    let failure = result.unwrap_err();
    assert_eq!(failure.reason, SequenceError::Budget);
    assert_eq!(failure.work.completed_candidates, 0);
}

#[test]
fn post_exchange_root_choices_keep_the_joint_hidden_cohort() {
    // Two root Aces survive the first return. A single Ace would force the
    // opening lead and therefore produce no strategic choice assessment.
    let mut hands: [Vec<Card>; 4] =
        std::array::from_fn(|seat| (0..9).map(|card| Card::from_index((seat * 9 + card) as u8)).collect());
    hands[0][0] = Card::from_index(17);
    hands[1][8] = Card::from_index(0);
    for hand in &mut hands {
        hand.sort();
    }
    let first = Table::new(&hands);
    // Only opponents' cards vary. Partner's observed pass and root's return
    // remain identical, so the choice cannot be split by hidden hindsight.
    let hidden = hands[1][1];
    hands[1][1] = hands[3][1];
    hands[3][1] = hidden;
    let second = Table::new(&hands);
    let mut live = first.clone();
    let bid = minimum_bid(&first);
    assert_eq!(
        Information::of(&first.view(bid.player.0)),
        Information::of(&second.view(bid.player.0))
    );
    let openings = [first.clone(), second.clone()].map(|mut table| {
        table.apply(bid);
        for _ in 0..16 {
            let (actor, legal) = table.to_act().unwrap();
            if actor == bid.player.0 && legal.iter().any(|a| matches!(a.action_type, ActionType::CardPlayed(_))) {
                let leads: Vec<_> = legal
                    .iter()
                    .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
                    .copied()
                    .collect();
                assert_eq!(leads.len(), 2, "a real post-exchange root choice");
                return (Information::of(&table.view(actor)), leads);
            }
            respond(&mut table);
        }
        panic!("fixture did not reach its opening choice");
    });
    assert_eq!(
        openings[0], openings[1],
        "joint information survives the actual exchanges"
    );
    assert_ne!(
        first.game.info.player_start_cards[1],
        second.game.info.player_start_cards[1]
    );
    let mut candidate = intervention(bid, PHASE_RESPONSES);
    // Isolate the bridge from catalogue breadth: every legal lead is compared
    // through the normal shared evaluator after the actual return response.
    let formation = candidate
        .program
        .nodes
        .iter()
        .position(|node| matches!(node, Node::Reconsider { .. }))
        .unwrap();
    candidate.program.nodes[formation] = Node::Action {
        actions: Actions::Cards(CardSet::ALL),
        next: Stage(0),
    };
    let context = context();
    let result = sequence::evaluate(
        &[candidate],
        &[(first, 0.4), (second, 0.6)],
        bid.player.0,
        context.provenance,
        &context,
        Limits {
            transitions: 100_000,
            ..Limits::default()
        },
        &mut response,
    )
    .unwrap();
    assert_eq!(result.executions[0].terminals.len(), 2);
    assert_eq!(
        result.executions[0].terminals.iter().map(|t| t.weight).sum::<f64>(),
        1.0
    );
    let choice = result
        .prepared()
        .iter()
        .find(|choice| choice.assessment.is_some() && choice.support.len() == 2)
        .expect("one future choice for the whole indistinguishable root group");
    assert_eq!(choice.support, vec![(0, 0.4), (1, 0.6)]);
    assert!(choice
        .information
        .events
        .iter()
        .any(|event| matches!(event, observation::SeatEvent::Pass { from, .. } if *from == bid.player.0)));

    let initial = live.view(bid.player.0);
    let origin = super::super::PlanningContext {
        observer: initial.seat,
        belief_scope: BeliefScope::CurrentInformation,
        knowledge: *initial.knowledge,
        events: initial.events.to_vec(),
        kinds: initial.kinds,
        series: initial.series,
        response_params: context.params.clone(),
        seed: context.seed,
        possible_allocations: 2,
        requested_worlds: 2,
        proposal_draws: 0,
    };
    let mut plan = RetainedPlan {
        receipt: result.receipt,
        pending_legal: crate::legal_actions::eligible(live.to_act().unwrap().1),
        selection: Some(result.selection),
        parameters: context.params.clone(),
        origin,
        context,
        review_pending: false,
        raw_events: initial.events.to_vec(),
    };
    live.apply(bid);
    for _ in 0..16 {
        if live.to_act().unwrap().0 == bid.player.0 {
            break;
        }
        respond(&mut live);
    }
    let current = live.view(bid.player.0);
    let legal = live.to_act().unwrap().1;
    plan.receipt.synchronize(&current).unwrap();
    let response = prepared_response(&plan, &current, legal, &reader(&current))
        .expect("the selected auction receipt carries its modeled return response");
    assert!(matches!(response.action.action_type, ActionType::Pass(_)));
    plan.context.seed += 1;
    assert!(prepared_response(&plan, &current, legal, &reader(&current)).is_none());
}

/// Exercise the actual live retained entry point, including acceptance and
/// canonical eligible-domain identity, without starting a fallback comparison.
fn try_prepared_turn(
    plan: &mut RetainedPlan,
    table: &Table,
) -> Option<(Decision, super::super::retained::SequenceTrace)> {
    let (seat, raw_legal) = table.to_act().unwrap();
    let legal = crate::legal_actions::eligible(raw_legal);
    let view = table.view(seat);
    let params = plan.parameters.clone();
    plan.synchronize(&view, &view, &params).unwrap();
    assert!(plan.receipt.pending.is_none());
    assert!(plan.pending_legal.is_empty());
    let mind = reader(&view);
    let intent = RefCell::new(Some(plan.clone()));
    let ctx = Ctx {
        me: seat,
        k: mind.knowledge(),
        mind: &mind,
        params: &params,
        solver: &RefCell::new(None),
        seed: plan.context.seed,
        deadline: None,
        raw_events: view.events,
        routes: RefCell::new(None),
        concepts: RefCell::new(crate::concepts::Reads::default()),
        intent: &intent,
        completed: &RefCell::new(sequence::CompletedCache::default()),
        formation_attempted: &Cell::new(false),
        response_completion: None,
        series: view.series,
    };
    let decision = super::super::retained::prepared(&ctx, &legal)?;
    let trace = ctx.concepts.into_inner().sequence.unwrap();
    assert_eq!(trace.mode, super::super::retained::PlanMode::Prepared);
    assert!(matches!(trace.scope, BeliefScope::PreparedConditional { .. }));
    assert_eq!(
        trace.work,
        sequence::Work::default(),
        "live reuse does no evaluation work"
    );
    *plan = intent.into_inner().unwrap();
    assert_eq!(plan.pending_legal, legal);
    Some((decision, trace))
}

fn prepared_turn(plan: &mut RetainedPlan, table: &Table) -> (Decision, super::super::retained::SequenceTrace) {
    try_prepared_turn(plan, table).expect("an exactly represented continuation must reach Prepared")
}

// A declared test response model: the root's Tail spends its lowest card;
// other seats follow high and discard low. Every choice uses only that actor's
// view and legal domain. Passing returns the cards actually observed arriving.
fn scoped_response(
    view: &SeatView<'_>,
    legal: &[GameAction],
    budget: sequence::LeafBudget,
) -> Result<sequence::LeafResponse, sequence::LeafFailure> {
    if let Some(action) = legal.iter().find(|a| a.action_type == ActionType::StopBidding) {
        return Ok(sequence::LeafResponse::unit(*action));
    }
    if legal.iter().any(|a| matches!(a.action_type, ActionType::Pass(_))) {
        if let Some(received) = view.events.iter().rev().find_map(|event| match event {
            observation::SeatEvent::Pass {
                to, cards: Some(cards), ..
            } if *to == view.seat => Some(*cards),
            _ => None,
        }) {
            let action = legal
                .iter()
                .find(|action| {
                    matches!(action.action_type,
                ActionType::Pass(cards) if CardSet::from_cards(&cards) == received)
                })
                .unwrap();
            return Ok(sequence::LeafResponse::unit(*action));
        }
        return response(view, legal, budget);
    }
    let public = view
        .knowledge
        .public_state(view.seat, engine::PlayPhase::from_actions(legal));
    let following = public.trick_len > 0
        && legal.iter().any(|action| {
            matches!(action.action_type,
        ActionType::CardPlayed(card) if card.index() / 9 == public.trick[0] / 9)
        });
    let cards = legal
        .iter()
        .filter(|action| matches!(action.action_type, ActionType::CardPlayed(_)));
    let rank = |action: &&GameAction| match action.action_type {
        ActionType::CardPlayed(card) => card.index(),
        _ => unreachable!(),
    };
    let action = if view.seat != 0 && following {
        cards.max_by_key(rank)
    } else {
        cards.min_by_key(rank)
    };
    match action {
        Some(action) => Ok(sequence::LeafResponse::unit(*action)),
        None => response(view, legal, budget),
    }
}

fn scoped_respond(table: &mut Table) {
    let (seat, legal) = table.to_act().unwrap();
    let legal = crate::legal_actions::eligible(legal);
    let action = scoped_response(
        &table.view(seat),
        &legal,
        sequence::LeafBudget {
            deadline: None,
            remaining_transitions: 1000,
            remaining_solver_nodes: None,
        },
    )
    .unwrap()
    .action;
    table.apply(action);
}

fn scoped_cohort() -> [(Table, f64); 2] {
    // Three high red leads are spent through legal play. The remaining root
    // hand has three Aces and three lower cards: cashing only one Ace before
    // the lowest-card Tail loses another suit's high cards. Cashing all three
    // is a real multi-step resource continuation with engine-score consequences.
    let ids = [
        [4, 8, 13, 17, 22, 26, 33, 34, 35],
        [0, 3, 7, 9, 16, 18, 25, 27, 28],
        [1, 6, 10, 12, 15, 19, 24, 29, 30],
        [2, 5, 11, 14, 20, 21, 23, 31, 32],
    ];
    let hands = ids.map(|hand| hand.into_iter().map(Card::from_index).collect::<Vec<_>>());
    let mut hidden = hands.clone();
    hidden[1][0] = Card::from_index(2);
    hidden[3][0] = Card::from_index(0);
    let tables = [hands, hidden].map(|hands| {
        let mut table = Table::new(&hands);
        let bid = *table
            .to_act()
            .unwrap()
            .1
            .iter()
            .filter(|a| matches!(a.action_type, ActionType::NewBid(_)))
            .max_by_key(|a| match a.action_type {
                ActionType::NewBid(value) => value,
                _ => unreachable!(),
            })
            .unwrap();
        table.apply(bid);
        while !table.to_act().is_some_and(|(seat, legal)| {
            seat == 0
                && legal
                    .iter()
                    .any(|action| matches!(action.action_type, ActionType::CardPlayed(_)))
        }) {
            scoped_respond(&mut table);
        }
        for trick in [[35, 27, 29, 31], [34, 28, 30, 32], [33, 3, 12, 21]] {
            for card in trick {
                let action = *table
                    .to_act()
                    .unwrap()
                    .1
                    .iter()
                    .find(|action| action.action_type == ActionType::CardPlayed(Card::from_index(card)))
                    .unwrap();
                table.apply(action);
            }
        }
        assert_eq!(table.to_act().unwrap().0, 0);
        assert_eq!(table.view(0).knowledge.hand_len(0), 6);
        table
    });
    assert_eq!(Information::of(&tables[0].view(0)), Information::of(&tables[1].view(0)));
    [(tables[0].clone(), 0.4), (tables[1].clone(), 0.6)]
}

#[test]
fn selected_parent_authorizes_multiple_local_steps_after_the_final_global_boundary() {
    use crate::concepts::sequence::Program;
    use sequence::{Candidate, ChoiceScope, Membership, Selection};
    let cohort = scoped_cohort();
    let mut live = cohort[0].0.clone();
    let mut context = context();
    context.provenance.response_model = "synthetic-follow-high-root-low";
    let candidate = Candidate::new(Program {
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
    });
    let calls = Cell::new(0);
    let mut leaf = |view: &SeatView<'_>, legal: &[GameAction], budget| {
        calls.set(calls.get() + 1);
        scoped_response(view, legal, budget)
    };
    let compared = sequence::evaluate(
        &[candidate],
        &cohort,
        0,
        context.provenance,
        &context,
        Limits {
            transitions: 500_000,
            comparisons: 32_768,
            memo_bytes: 64 * 1024 * 1024,
            ..Limits::default()
        },
        &mut leaf,
    )
    .unwrap();
    let parent = compared.selection.root();
    assert_eq!(parent.scope, ChoiceScope::Parent);
    assert_eq!(parent.support, vec![(0, 0.4), (1, 0.6)]);
    assert!(parent.assessment.as_ref().unwrap().cohort_complete);
    assert!(compared
        .selection
        .records()
        .iter()
        .any(|record| record.scope == ChoiceScope::Global));
    let view = live.view(0);
    let mut plan = RetainedPlan {
        receipt: compared.receipt,
        pending_legal: crate::legal_actions::eligible(live.to_act().unwrap().1),
        selection: Some(compared.selection),
        parameters: context.params.clone(),
        origin: super::super::PlanningContext {
            observer: 0,
            belief_scope: BeliefScope::CurrentInformation,
            knowledge: *view.knowledge,
            events: view.events.to_vec(),
            kinds: view.kinds,
            series: view.series,
            response_params: context.params.clone(),
            seed: context.seed,
            possible_allocations: 2,
            requested_worlds: 2,
            proposal_draws: 0,
        },
        context: context.clone(),
        review_pending: false,
        raw_events: view.events.to_vec(),
    };
    let first = plan.receipt.pending.as_ref().unwrap().action;
    // A separate, genuinely completed narrow plan is not this parent's future
    // authorization, even when it proposes exactly the same initial card.
    let unrelated = sequence::evaluate(
        &[Candidate::new(Program {
            name: "unrelated-direct-tail",
            version: 1,
            start: Stage(1),
            nodes: vec![
                Node::Tail,
                Node::Action {
                    actions: Actions::Exact(first.action_type),
                    next: Stage(0),
                },
            ],
        })],
        &cohort,
        0,
        context.provenance,
        &context,
        Limits::default(),
        &mut scoped_response,
    )
    .unwrap();
    let completed_calls = calls.get();
    live.apply(first);
    let mut unexpected = live.clone();
    let (actor, legal) = unexpected.to_act().unwrap();
    let predicted = scoped_response(
        &unexpected.view(actor),
        legal,
        sequence::LeafBudget {
            deadline: None,
            remaining_transitions: 1000,
            remaining_solver_nodes: None,
        },
    )
    .unwrap()
    .action;
    let other = *legal
        .iter()
        .find(|action| matches!(action.action_type, ActionType::CardPlayed(_)) && **action != predicted)
        .unwrap();
    unexpected.apply(other);
    while unexpected.to_act().unwrap().0 != 0 {
        scoped_respond(&mut unexpected);
    }
    assert!(
        try_prepared_turn(&mut plan.clone(), &unexpected).is_none(),
        "unrepresented evidence needs repair"
    );
    while live.to_act().unwrap().0 != 0 {
        scoped_respond(&mut live);
    }
    let mut wrong_parent = plan.clone();
    wrong_parent.selection = Some(unrelated.selection);
    assert!(
        try_prepared_turn(&mut wrong_parent, &live).is_none(),
        "an unrelated complete parent cannot lend its scope"
    );
    // A changed local binding must fail even if its selected action remains
    // legal and its predicted global catalogue is unchanged.
    let view = live.view(0);
    let legal = crate::legal_actions::eligible(live.to_act().unwrap().1);
    plan.synchronize(&view, &view, &plan.parameters.clone()).unwrap();
    assert!(!plan
        .receipt
        .program
        .suffix(plan.receipt.stage)
        .unwrap()
        .nodes
        .iter()
        .any(|node| matches!(node, Node::Reconsider { .. })));
    let mut narrowed = plan.authorized_records().unwrap().to_vec();
    let record = narrowed
        .iter_mut()
        .find(|record| {
            record.information.matches(&view)
                && record.semantic_suffix == plan.receipt.program.suffix(plan.receipt.stage).unwrap()
        })
        .unwrap();
    assert!(record.alternatives.len() > 1);
    record.alternatives = vec![record.alternatives[record.selected].clone()];
    record.selected = 0;
    record.assessment = None;
    record.scope = ChoiceScope::FixedAction;
    assert!(sequence::prepared_lookup(
        &plan.receipt,
        &view,
        &legal,
        &reader(&view),
        &context,
        &Membership::authored(&view, &legal).unwrap(),
        &narrowed
    )
    .is_err());
    let original_membership = Membership::authored(&view, &legal).unwrap();
    let expected = sequence::prepared_lookup(
        &plan.receipt,
        &view,
        &legal,
        &reader(&view),
        &context,
        &original_membership,
        plan.authorized_records().unwrap(),
    )
    .unwrap();
    let removed = *legal.iter().find(|action| **action != expected.action).unwrap();
    let changed_domain: Vec<_> = legal.iter().copied().filter(|action| *action != removed).collect();
    assert!(changed_domain.contains(&expected.action));
    assert!(
        sequence::prepared_lookup(
            &plan.receipt,
            &view,
            &changed_domain,
            &reader(&view),
            &context,
            &Membership::authored(&view, &changed_domain).unwrap(),
            plan.authorized_records().unwrap(),
        )
        .is_err(),
        "a still-legal selected action does not excuse a changed predicted domain"
    );
    for _ in 0..2 {
        let (decision, trace) = prepared_turn(&mut plan, &live);
        let authorization = trace.authorization.as_ref().unwrap();
        assert_eq!(authorization.parent.support, vec![(0, 0.4), (1, 0.6)]);
        assert_eq!(authorization.records.len(), trace.choices.len());
        for (index, choice) in authorization.records.iter().zip(&trace.choices) {
            assert_eq!(choice, &plan.selection.as_ref().unwrap().records()[*index]);
            assert_eq!(choice.support, vec![(0, 0.4), (1, 0.6)]);
            assert_ne!(choice.scope, ChoiceScope::Global);
        }
        assert!(trace.choices.iter().any(|choice| choice
            .alternatives
            .iter()
            .any(|selection| matches!(selection, Selection::Offer { action, .. } if *action == decision.action))));
        live.apply(decision.action);
        while live.to_act().unwrap().0 != 0 {
            scoped_respond(&mut live);
        }
    }
    assert_eq!(
        calls.get(),
        completed_calls,
        "accepted local steps never re-evaluate the parent"
    );
}

#[test]
fn auction_and_return_reuse_the_real_depth_two_boundary_with_joint_support() {
    // One opening Ace and a short pair, spread low side cards. The former
    // long all-standing hand expanded large cash permutations before it could
    // finish any candidate. Keep the full catalogue/depth and joint support.
    let mut hands = [
        [5, 6, 8, 9, 10, 18, 19, 27, 28],
        [4, 7, 13, 14, 15, 16, 17, 22, 23],
        [0, 1, 2, 3, 11, 12, 20, 21, 29],
        [24, 25, 26, 30, 31, 32, 33, 34, 35],
    ]
    .map(|hand| hand.into_iter().map(Card::from_index).collect::<Vec<_>>());
    let first = Table::new(&hands);
    // Opponents differ; both partnership hands and observed exchanges agree.
    let hidden = hands[1][2];
    hands[1][2] = hands[3][3];
    hands[3][3] = hidden;
    let second = Table::new(&hands);
    let bid = *first
        .to_act()
        .unwrap()
        .1
        .iter()
        .filter(|a| matches!(a.action_type, ActionType::NewBid(_)))
        .max_by_key(|a| match a.action_type {
            ActionType::NewBid(value) => value,
            _ => unreachable!(),
        })
        .unwrap();
    let root = bid.player.0;
    let auction = [(first, 0.4), (second, 0.6)];
    let returns = auction.clone().map(|(mut table, weight)| {
        table.apply(bid);
        for _ in 0..16 {
            if table.to_act().unwrap().0 == root
                && table
                    .to_act()
                    .unwrap()
                    .1
                    .iter()
                    .any(|a| matches!(a.action_type, ActionType::Pass(_)))
            {
                return (table, weight);
            }
            scoped_respond(&mut table);
        }
        panic!("fixture did not reach return");
    });
    let returned = scoped_response(
        &returns[0].0.view(root),
        &crate::legal_actions::eligible(returns[0].0.to_act().unwrap().1),
        sequence::LeafBudget {
            deadline: None,
            remaining_transitions: 1000,
            remaining_solver_nodes: None,
        },
    )
    .unwrap()
    .action;
    for (cohort, action) in [(auction, bid), (returns, returned)] {
        let mut live = cohort[0].0.clone();
        assert_eq!(
            Information::of(&live.view(root)),
            Information::of(&cohort[1].0.view(root))
        );
        let mut context = context();
        context.provenance.response_model = "synthetic-follow-high-root-low";
        let candidate = intervention(action, PHASE_RESPONSES);
        assert!(candidate.program.nodes.iter().any(|node| matches!(
            node,
            Node::Reconsider {
                remaining: MAX_BOUNDARY_DEPTH,
                ..
            }
        )));
        let result = sequence::evaluate(
            &[candidate],
            &cohort,
            root,
            context.provenance,
            &context,
            Limits {
                transitions: 2_000_000,
                comparisons: 131_072,
                memo_bytes: 64 * 1024 * 1024,
                ..Limits::default()
            },
            &mut scoped_response,
        )
        .unwrap();
        assert_eq!(result.executions[0].terminals.len(), 2);
        let initial = live.view(root);
        let mut plan = RetainedPlan {
            receipt: result.receipt,
            pending_legal: crate::legal_actions::eligible(live.to_act().unwrap().1),
            selection: Some(result.selection),
            parameters: context.params.clone(),
            origin: super::super::PlanningContext {
                observer: root,
                belief_scope: BeliefScope::CurrentInformation,
                knowledge: *initial.knowledge,
                events: initial.events.to_vec(),
                kinds: initial.kinds,
                series: initial.series,
                response_params: context.params.clone(),
                seed: context.seed,
                possible_allocations: 2,
                requested_worlds: 2,
                proposal_draws: 0,
            },
            context,
            review_pending: false,
            raw_events: initial.events.to_vec(),
        };
        live.apply(action);
        if matches!(action.action_type, ActionType::NewBid(_)) {
            while live.to_act().unwrap().0 != root {
                scoped_respond(&mut live);
            }
            let (decision, trace) = prepared_turn(&mut plan, &live);
            assert!(matches!(decision.action.action_type, ActionType::Pass(_)));
            assert!(trace
                .choices
                .iter()
                .any(|choice| choice.assessment.is_none() && choice.support == vec![(0, 0.4), (1, 0.6)]));
            live.apply(decision.action);
        }
        let view = live.view(root);
        let legal = crate::legal_actions::eligible(live.to_act().unwrap().1);
        plan.synchronize(&view, &view, &plan.parameters.clone()).unwrap();
        let Frontier::Reconsider { stage, remaining, .. } =
            inspect(&plan.receipt.program, plan.receipt.stage, &view, &legal, &reader(&view))
                .unwrap()
                .frontier
        else {
            panic!("real post-exchange formation boundary");
        };
        assert_eq!(remaining, MAX_BOUNDARY_DEPTH);
        let binding =
            crate::concepts::sequence::bind_reconsideration(&plan.receipt.program, stage, &view, &legal).unwrap();
        let boundary = binding.program.suffix(binding.program.start).unwrap();
        let expected = plan
            .authorized_records()
            .unwrap()
            .iter()
            .find(|choice| choice.information.matches(&view) && choice.semantic_suffix == boundary)
            .unwrap();
        assert_eq!(expected.support, vec![(0, 0.4), (1, 0.6)]);
        let assessment = expected.assessment.as_ref().unwrap();
        assert!(assessment.cohort_complete);
        assert!(
            assessment.candidates[assessment.selected].difference > assessment.candidates[0].difference,
            "fixture must select an authored continuation over the unrecorded response tail"
        );
        let expected = expected.clone();
        let (decision, trace) = prepared_turn(&mut plan, &live);
        assert!(legal.contains(&decision.action));
        assert!(
            trace.choices.contains(&expected),
            "publish the actual jointly assessed boundary certificate"
        );
        assert_eq!(plan.receipt.program, binding.program);
    }
}

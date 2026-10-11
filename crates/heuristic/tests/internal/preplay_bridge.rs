use super::*;
use crate::concepts::sequence::{inspect, Frontier, Offer, Receipt};
use crate::concepts::value::{CohortMethod, Provenance};
use agent::{SeatView, Table};
use beliefs::{BeliefParams, Mind};
use engine::{ActionType, Card, CardSet, GamePhase};

fn table(swapped: bool) -> Table {
    let mut hands: [Vec<Card>; 4] =
        std::array::from_fn(|seat| (0..9).map(|card| Card::from_index((seat * 9 + card) as u8)).collect());
    if swapped {
        let old = hands[1][8];
        hands[1][8] = hands[3][8];
        hands[3][8] = old;
    }
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

fn response(_: &SeatView<'_>, legal: &[GameAction], _: sequence::LeafBudget) -> Result<GameAction, SequenceError> {
    Ok(*legal
        .iter()
        .find(|a| a.action_type == ActionType::StopBidding)
        .or_else(|| legal.iter().find(|a| matches!(a.action_type, ActionType::Pass(_))))
        .or_else(|| {
            legal
                .iter()
                .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
        })
        .unwrap_or(&legal[0]))
}

fn respond(table: &mut Table) {
    let (seat, legal) = table.to_act().unwrap();
    let action = response(
        &table.view(seat),
        legal,
        sequence::LeafBudget {
            deadline: None,
            remaining_transitions: 1000,
        },
    )
    .unwrap();
    table.apply(action);
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
    let mut table = table(false);
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
    let mut live = table(false);
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
    let initial = table(false);
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
    let first = table(false);
    let second = table(true);
    let mut live = first.clone();
    let bid = minimum_bid(&first);
    assert_eq!(
        Information::of(&first.view(bid.player.0)),
        Information::of(&second.view(bid.player.0))
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
        .prepared
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
        prepared: Arc::new(result.prepared),
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

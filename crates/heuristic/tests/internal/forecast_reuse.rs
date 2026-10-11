use super::common::{Allowance, Evaluated, Failure, Mode};
use super::sequence::{Candidate, CompletedCache};
use super::*;
use crate::concepts::sequence::{Actions, Node, Program, SequenceError, Stage};
use crate::Params;
use std::cell::Cell;

fn fixture(hands: [Vec<engine::Card>; 4]) -> Table {
    // Keep the caller's deal order; no private production fixture is imported.
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
    for _ in 0..2 {
        let action = *table
            .to_act()
            .unwrap()
            .1
            .iter()
            .find(|a| matches!(a.action_type, ActionType::Pass(_)))
            .unwrap();
        table.apply(action);
    }
    table
}

fn ordered_fixture() -> Table {
    fixture(std::array::from_fn(|seat| {
        (0..9).map(|i| engine::Card::from_index((seat * 9 + i) as u8)).collect()
    }))
}

fn direct(action: GameAction) -> Candidate {
    Candidate::new(Program {
        name: "reuse-fixture-intervention",
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

fn forecast(table: Table, params: &Params) -> Forecast {
    let root = table.to_act().unwrap().0;
    let view = table.view(root);
    let context = PlanningContext {
        observer: root,
        belief_scope: BeliefScope::CurrentInformation,
        knowledge: *view.knowledge,
        events: view.events.to_vec(),
        kinds: view.kinds,
        series: view.series,
        response_params: params.clone(),
        seed: 7,
        possible_allocations: 1,
        requested_worlds: 1,
        proposal_draws: 0,
    };
    Forecast {
        tables: vec![(table, 1.0)],
        players: RefCell::new(std::array::from_fn(|_| HeuristicAgent::new(params.clone()))),
        deadline: Instant::now() + Duration::from_secs(300),
        provenance: Provenance {
            belief_model: "synthetic-joint",
            response_model: "seat-policy-completed-primitives-v2",
            cohort: CohortMethod::Enumerated,
        },
        context,
    }
}

fn with_context<T>(forecast: &Forecast, cache: &RefCell<CompletedCache>, call: impl FnOnce(&Ctx<'_>) -> T) -> T {
    let view = forecast.tables[0].0.view(forecast.context.observer);
    let params = &forecast.context.response_params;
    let mut mind = beliefs::Mind::new(
        view.seat,
        view.knowledge.dealt_certain(view.seat),
        view.kinds,
        params.beliefs.clone(),
    );
    for event in view.events {
        mind.apply(event).unwrap();
    }
    let ctx = Ctx {
        me: view.seat,
        k: mind.knowledge(),
        mind: &mind,
        params,
        solver: &RefCell::new(None),
        seed: forecast.context.seed,
        deadline: Some(forecast.deadline),
        raw_events: view.events,
        routes: RefCell::new(None),
        concepts: RefCell::new(crate::concepts::Reads::default()),
        intent: &RefCell::new(None),
        completed: cache,
        formation_attempted: &Cell::new(false),
        response_completion: None,
        series: view.series,
    };
    call(&ctx)
}

fn params(endgame: u32) -> Params {
    Params {
        plan_worlds: 0,
        return_worlds: 0,
        endgame_tricks: endgame,
        endgame_worlds: 8,
        ..Params::default()
    }
}

#[test]
fn common_comparison_reuses_completed_responses_when_candidates_expand() {
    let forecast = forecast(ordered_fixture(), &params(0));
    let legal = forecast.tables[0].0.to_act().unwrap().1;
    let candidates = [
        direct(
            *legal
                .iter()
                .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
                .unwrap(),
        ),
        direct(*legal.iter().find(|a| a.action_type == ActionType::NewBid(125)).unwrap()),
    ];
    let cache = RefCell::new(CompletedCache::default());
    with_context(&forecast, &cache, |ctx| {
        let first = forecast
            .evaluate_programs(ctx, &candidates[..1], Allowance::Formation)
            .unwrap();
        let repeated = forecast
            .evaluate_programs(ctx, &candidates[..1], Allowance::Formation)
            .unwrap();
        assert_eq!(repeated.comparison.values, first.comparison.values);
        assert_eq!(repeated.comparison.receipt, first.comparison.receipt);
        assert_eq!(repeated.comparison.executions, first.comparison.executions);
        assert_eq!(
            repeated.comparison.work.response_calls, 0,
            "completed response work is not replayed"
        );
        assert!(repeated.comparison.work.suffix_hits > 0);
        assert_eq!(
            repeated.comparison.work.represented_transitions,
            first.comparison.work.represented_transitions
        );
        let expanded = forecast
            .evaluate_programs(ctx, &candidates, Allowance::Formation)
            .unwrap();
        assert_eq!(
            expanded.comparison.executions[0].terminals,
            first.comparison.executions[0].terminals
        );
        assert_eq!(
            expanded.comparison.values.candidates[0].difference,
            first.comparison.values.candidates[0].difference
        );
        assert_eq!(expanded.comparison.executions[0].terminals[0].outcome.game_value, 120);
        assert_eq!(expanded.comparison.executions[1].terminals[0].outcome.game_value, 125);
    });
}

#[test]
fn reused_common_response_actors_match_fresh_actors_across_hypothesis_forks() {
    let params = params(2);
    let mut actors = std::array::from_fn(|_| HeuristicAgent::new(params.clone()));
    let mut rng = Rng::new(4107);
    for _ in 0..12 {
        let mut deck: Vec<u8> = (0..36).collect();
        for i in (1..36).rev() {
            deck.swap(i, rng.next() as usize % (i + 1));
        }
        let table = fixture(std::array::from_fn(|s| {
            deck[s * 9..(s + 1) * 9]
                .iter()
                .map(|&card| engine::Card::from_index(card))
                .collect()
        }));
        let actions: Vec<_> = table
            .to_act()
            .unwrap()
            .1
            .iter()
            .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
            .take(2)
            .copied()
            .collect();
        for action in actions {
            let reused = forecast(table.clone(), &params);
            *reused.players.borrow_mut() = actors;
            let fresh = forecast(table.clone(), &params);
            let candidate = [direct(action)];
            let actual = with_context(&reused, &RefCell::new(CompletedCache::default()), |ctx| {
                reused.evaluate_programs(ctx, &candidate, Allowance::Formation).unwrap()
            });
            let expected = with_context(&fresh, &RefCell::new(CompletedCache::default()), |ctx| {
                fresh.evaluate_programs(ctx, &candidate, Allowance::Formation).unwrap()
            });
            assert_eq!(actual.context, expected.context);
            assert_eq!(actual.comparison.values, expected.comparison.values);
            assert_eq!(actual.comparison.receipt, expected.comparison.receipt);
            assert_eq!(actual.comparison.executions, expected.comparison.executions);
            assert_eq!(
                actual.comparison.work.represented_solver_nodes, expected.comparison.work.represented_solver_nodes,
                "canonical effort must not depend on earlier hypothetical actors"
            );
            actors = reused.players.into_inner();
        }
    }
}

#[test]
fn common_install_keeps_the_assessed_receipt_context_and_deferral_invalidates_only_predictions() {
    let forecast = forecast(ordered_fixture(), &params(0));
    let legal = forecast.tables[0].0.to_act().unwrap().1;
    let action = *legal
        .iter()
        .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
        .unwrap();
    with_context(&forecast, &RefCell::new(CompletedCache::default()), |ctx| {
        let evaluated = forecast
            .evaluate_programs(ctx, &[direct(action)], Allowance::Formation)
            .unwrap();
        for mode in [
            Mode::Preplay {
                replacement_rule: RuleId::PBack,
            },
            Mode::Formation,
            Mode::Repair,
        ] {
            let copy = Evaluated {
                comparison: evaluated.comparison.clone(),
                context: evaluated.context.clone(),
            };
            let baseline = crate::decision(&action, RuleId::HP1, vec![]);
            let mut selected = forecast.install(ctx, copy, mode, baseline);
            assert_eq!(selected.action, action);
            let installed = ctx.intent.borrow().as_ref().unwrap().clone();
            assert_eq!(installed.receipt, evaluated.comparison.receipt);
            assert_eq!(installed.context, evaluated.context);
            assert_eq!(installed.origin, forecast.context);
            let reads = ctx.concepts.borrow();
            let assessment = reads.assessment.as_ref().unwrap();
            assert_eq!(assessment.values, evaluated.comparison.values);
            assert_eq!(assessment.actions[assessment.values.selected], action);
            assert_eq!(assessment.context, forecast.context);
            drop(reads);
            common::defer(
                ctx,
                &mut selected,
                mode,
                Failure::before_evaluation(SequenceError::Budget, Some(1)),
            );
            let slot = ctx.intent.borrow();
            let deferred = slot.as_ref().unwrap();
            assert!(deferred.review_pending);
            assert!(deferred.receipt.pending.is_none());
            assert!(deferred.prepared.is_empty());
            assert_eq!(deferred.receipt.program, installed.receipt.program);
            assert_eq!(deferred.origin, installed.origin);
            assert_eq!(selected.action, action);
        }
    });
}

#[test]
fn common_reuse_respects_changed_visible_metadata_and_private_hands() {
    let params = params(0);
    let cache = RefCell::new(CompletedCache::default());
    let original = ordered_fixture();
    let mut changed_series = original.clone();
    changed_series.series.team_totals[0] = 490;
    let mut changed_kinds = changed_series.clone();
    changed_kinds.kinds[1] = agent::SeatKind::Human;
    let mut hands = original.game.info.player_start_cards.clone();
    let card = hands[0][0];
    hands[0][0] = hands[1][0];
    hands[1][0] = card;
    let changed_hand = fixture(hands);
    for table in [original, changed_series, changed_kinds, changed_hand] {
        let action = *table
            .to_act()
            .unwrap()
            .1
            .iter()
            .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
            .unwrap();
        let forecast = forecast(table, &params);
        let candidate = [direct(action)];
        let warm = with_context(&forecast, &cache, |ctx| {
            forecast
                .evaluate_programs(ctx, &candidate, Allowance::Formation)
                .unwrap()
        });
        let cold = with_context(&forecast, &RefCell::new(CompletedCache::default()), |ctx| {
            forecast
                .evaluate_programs(ctx, &candidate, Allowance::Formation)
                .unwrap()
        });
        assert_eq!(warm.comparison.values, cold.comparison.values);
        assert_eq!(warm.comparison.receipt, cold.comparison.receipt);
        assert_eq!(warm.comparison.executions, cold.comparison.executions);
        assert!(
            warm.comparison.work.response_calls > 0,
            "changed permitted input requires actual response work"
        );
    }
}

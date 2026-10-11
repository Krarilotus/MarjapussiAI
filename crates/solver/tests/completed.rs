//! Completed-response reuse must be independent of previous search history;
//! partial worlds/secondary criteria must never escape as usable responses.

use engine::{get_all_cards, ActionType, FastState, Game, GamePhase, PublicState};
use knowledge::{Knowledge, Tracker, Uniform};
use observation::{seat_events, start_hand};
use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};
use solver::{evaluate_complete, evaluate_limited, CompletionFailureReason, Limits, Objective, Solver, SolverError};

// Same synthetic seed and legal-prefix construction as the existing bounded
// evaluation fixture in worlds.rs. No private game data or player identifiers.
fn position(left: usize) -> (Knowledge, PublicState) {
    let mut rng = StdRng::seed_from_u64(41);
    let mut deck = get_all_cards();
    deck.shuffle(&mut rng);
    let hands = [0, 9, 18, 27].map(|i| deck[i..i + 9].to_vec());
    let mut game = Game::new(
        "completed-response".into(),
        ["S0", "S1", "S2", "S3"].map(String::from),
        Some(hands),
    );
    for _ in 0..256 {
        let seat = game.state.player_at_turn.0;
        if matches!(
            game.state.phase,
            GamePhase::Trick | GamePhase::StartTrick | GamePhase::AnsweringPair | GamePhase::AnsweringHalf(_)
        ) && game.state.players[seat as usize].cards.len() <= left
        {
            let mut tracker = Tracker::new(seat, start_hand(&game, seat));
            for event in seat_events(&game, seat) {
                tracker.apply(&event).unwrap();
            }
            return (*tracker.knowledge(), FastState::from_game(&game).unwrap().public());
        }
        assert!(!game.ended(), "fixture must reach the requested endgame");
        let legal: Vec<_> = game
            .legal_actions()
            .into_iter()
            .filter(|a| a.action_type != ActionType::UndoRequest)
            .collect();
        game.apply_action_mut(legal[rng.random_range(0..legal.len())]);
    }
    panic!("fixture made no finite progress");
}

fn run(
    solver: &mut Solver,
    k: &Knowledge,
    table: &PublicState,
    objective: Objective,
    limits: Limits,
) -> Result<solver::CompletedEvaluation, solver::CompletionFailure> {
    evaluate_complete(
        solver,
        k,
        table,
        objective,
        8,
        &Uniform,
        &mut StdRng::seed_from_u64(42),
        limits,
    )
}

#[test]
fn completed_values_and_effort_ignore_prior_tt_and_counter_state() {
    let (k, table) = position(2);
    let objective = Objective::Points;
    let mut reference = Solver::with_tt_bits(16);
    let expected = evaluate_limited(
        &mut reference,
        &k,
        &table,
        objective,
        8,
        &Uniform,
        &mut StdRng::seed_from_u64(42),
        Limits::default(),
    )
    .unwrap();
    assert!(expected.complete);
    let expected_nodes = reference.nodes();
    assert!(expected_nodes > 0);
    let mut reused = Solver::with_tt_bits(16);
    for _ in 0..2 {
        let actual = run(&mut reused, &k, &table, objective, Limits::default()).unwrap();
        assert_eq!(actual.decision, expected);
        assert_eq!(actual.effort.nodes, expected_nodes);
        // Pollute both the table and cumulative counter with another objective.
        evaluate_limited(
            &mut reused,
            &k,
            &table,
            Objective::Reach {
                team: 1 - k.me() % 2,
                target: 40,
            },
            8,
            &Uniform,
            &mut StdRng::seed_from_u64(3),
            Limits::default(),
        )
        .unwrap();
    }
    let exact_cap = run(
        &mut reused,
        &k,
        &table,
        objective,
        Limits {
            nodes: Some(expected_nodes),
            deadline: None,
        },
    )
    .unwrap();
    assert_eq!(exact_cap.decision, expected);
    assert_eq!(exact_cap.effort.nodes, expected_nodes);
}

#[test]
fn errors_and_zero_work_interruption_preserve_reason_and_reset_effort() {
    let (k, table) = position(2);
    let mut solver = Solver::with_tt_bits(16);
    run(&mut solver, &k, &table, Objective::Points, Limits::default()).unwrap();
    for limits in [
        Limits {
            nodes: Some(0),
            deadline: None,
        },
        Limits {
            nodes: None,
            deadline: Some(std::time::Instant::now()),
        },
    ] {
        let failed = run(&mut solver, &k, &table, Objective::Points, limits).unwrap_err();
        assert_eq!(failed.reason, CompletionFailureReason::Solver(SolverError::Interrupted));
        assert_eq!(failed.effort.nodes, 0);
    }
    let zero = |_: &knowledge::Hands| 0.0;
    let failed = evaluate_complete(
        &mut solver,
        &k,
        &table,
        Objective::Points,
        8,
        &zero,
        &mut StdRng::seed_from_u64(42),
        Limits::default(),
    )
    .unwrap_err();
    assert_eq!(failed.reason, CompletionFailureReason::Solver(SolverError::ZeroWeight));
    assert_eq!(failed.effort.nodes, 0);
}

#[test]
fn incomplete_world_prefix_is_not_a_completed_response() {
    let (k, table) = position(4);
    assert!(k.deals().count() > 8);
    let mut solver = Solver::with_tt_bits(16);
    let full = run(&mut solver, &k, &table, Objective::Points, Limits::default()).unwrap();
    let cap = full.effort.nodes - 1;
    let failed = run(
        &mut solver,
        &k,
        &table,
        Objective::Points,
        Limits {
            nodes: Some(cap),
            deadline: None,
        },
    )
    .unwrap_err();
    assert!(
        matches!(failed.reason, CompletionFailureReason::Incomplete { completed_worlds } if completed_worlds > 0 && completed_worlds < full.decision.worlds)
    );
    assert_eq!(failed.effort.nodes, cap);
    assert_eq!(
        run(&mut solver, &k, &table, Objective::Points, Limits::default()).unwrap(),
        full
    );
}

#[test]
fn incomplete_secondary_contract_criteria_are_rejected_even_after_all_worlds() {
    let (k, table) = position(2);
    // Deliberately unreachable diagnostic objective exercises the existing
    // second criteria pass; it is not a proposed legal auction value.
    let objective = Objective::Contract {
        playing_team: k.me() % 2,
        game_value: 1000,
    };
    let mut solver = Solver::with_tt_bits(16);
    let full = run(&mut solver, &k, &table, objective, Limits::default()).unwrap();
    let cap = full.effort.nodes - 1;
    let failed = run(
        &mut solver,
        &k,
        &table,
        objective,
        Limits {
            nodes: Some(cap),
            deadline: None,
        },
    )
    .unwrap_err();
    assert_eq!(
        failed.reason,
        CompletionFailureReason::Incomplete {
            completed_worlds: full.decision.worlds
        }
    );
    assert_eq!(failed.effort.nodes, cap);
}

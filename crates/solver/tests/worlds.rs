//! Guard tests for move evaluation over worlds: it runs at every late decision of random games;
//! it gives the exact values when only one deal is consistent; small deal spaces are solved
//! deal by deal; the contract objective follows HC-9; swapping hidden cards the seat cannot
//! see leaves the decision unchanged (ARCHITECTURE invariant 1).

use engine::CardSet;
use engine::Game;
use engine::GamePhase;
use engine::{get_all_cards, Card};
use engine::{ActionType, GameAction};
use engine::{FastState, Move};
use knowledge::{Hands, Knowledge, Tracker, Uniform};
use observation::{seat_events, start_hand, Seat};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use solver::{evaluate, Criterion, Measure, Objective, Solver};

fn deal(rng: &mut StdRng) -> [Vec<Card>; 4] {
    let mut deck = get_all_cards();
    deck.shuffle(rng);
    [0, 9, 18, 27].map(|i| deck[i..i + 9].to_vec())
}

fn game_from(hands: &[Vec<Card>; 4]) -> Game {
    Game::new(
        "worlds".into(),
        ["S0", "S1", "S2", "S3"].map(String::from),
        Some(hands.clone()),
    )
}

fn knowledge_of(game: &Game, seat: Seat) -> Knowledge {
    let mut tracker = Tracker::new(seat, start_hand(game, seat));
    for event in seat_events(game, seat) {
        tracker.apply(&event).unwrap();
    }
    *tracker.knowledge()
}

/// A card-play decision with at most `left` tricks to go.
fn is_decision(game: &Game, left: usize) -> bool {
    matches!(
        game.state.phase,
        GamePhase::Trick | GamePhase::StartTrick | GamePhase::AnsweringPair | GamePhase::AnsweringHalf(_)
    ) && game.state.players[game.state.player_at_turn.0 as usize].cards.len() <= left
}

fn random_action(game: &Game, rng: &mut StdRng) -> GameAction {
    let legal: Vec<_> = game
        .legal_actions()
        .into_iter()
        .filter(|a| !matches!(a.action_type, ActionType::UndoRequest))
        .collect();
    legal[rng.random_range(0..legal.len())]
}

#[test]
fn evaluation_runs_at_every_late_decision_and_is_exact_when_the_deal_is_known() {
    let mut solver = Solver::new();
    let (mut decisions, mut known) = (0, 0);
    for seed in 0..120u64 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut game = game_from(&deal(&mut rng));
        while !game.ended() {
            if is_decision(&game, 4) {
                let me = game.state.player_at_turn.0;
                let k = knowledge_of(&game, me);
                let fast = FastState::from_game(&game).unwrap();
                let table = fast.public();
                let points = evaluate(&mut solver, &k, &table, Objective::Points, 8, &Uniform, &mut rng)
                    .unwrap_or_else(|e| panic!("seed {seed}: {e:?} at {fast:?}"));
                assert_eq!(points.ess, points.worlds as f64);
                assert_eq!(points.all_deals, k.deals().count() <= 8);
                let team = me % 2;
                let target = fast.points(team) + 25;
                let reach = Objective::Reach { team, target };
                let made = evaluate(&mut solver, &k, &table, reach, 8, &Uniform, &mut rng).unwrap();
                for v in points.moves.iter().chain(&made.moves) {
                    assert!(v.means[0] >= 0.0);
                }
                if k.deals().count() == 1 {
                    let exact = solver.move_values(&fast, team);
                    for ((p, r), (m, v)) in points.moves.iter().zip(&made.moves).zip(exact) {
                        let total = fast.points(team) + v;
                        assert_eq!((p.mv, r.mv), (m, m));
                        assert_eq!(p.means[0], total as f64, "seed {seed}: points of {m:?}");
                        assert_eq!(
                            r.means[0],
                            (total >= target) as u8 as f64,
                            "seed {seed}: reach of {m:?}"
                        );
                    }
                    known += 1;
                }
                decisions += 1;
            }
            let action = random_action(&game, &mut rng);
            game.apply_action_mut(action);
        }
    }
    eprintln!("{decisions} decisions evaluated, {known} with a single consistent deal");
    assert!(known >= 200, "too few known deals: {known}");
}

#[test]
fn belief_weights_set_the_effective_sample_size() {
    let mut rng = StdRng::seed_from_u64(1);
    let mut game = game_from(&deal(&mut rng));
    while !is_decision(&game, 5) {
        let action = random_action(&game, &mut rng);
        game.apply_action_mut(action);
    }
    let me = game.state.player_at_turn.0;
    let k = knowledge_of(&game, me);
    let table = FastState::from_game(&game).unwrap().public();
    let next = (me + 1) % 4;
    let card = k.possible(next).iter().next().unwrap();
    let belief = |h: &Hands| if h[next as usize].contains(card) { 4.0 } else { 1.0 };
    let d = evaluate(
        &mut Solver::new(),
        &k,
        &table,
        Objective::Points,
        200,
        &belief,
        &mut rng,
    )
    .unwrap();
    assert_eq!(d.worlds as u64, k.deals().count().min(200));
    assert!(
        d.ess > d.worlds as f64 / 4.0 && d.ess < d.worlds as f64,
        "ess {}",
        d.ess
    );
    let zero = |_: &Hands| 0.0;
    let err = evaluate(&mut Solver::new(), &k, &table, Objective::Points, 20, &zero, &mut rng);
    assert_eq!(err, Err(solver::SolverError::ZeroWeight));
}

#[test]
fn small_deal_spaces_are_solved_deal_by_deal() {
    let mut solver = Solver::new();
    let mut checked = 0;
    for seed in 0..150u64 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut game = game_from(&deal(&mut rng));
        while !game.ended() {
            let me = game.state.player_at_turn.0;
            let k = is_decision(&game, 5).then(|| knowledge_of(&game, me));
            let count = k.as_ref().map_or(0, |k| k.deals().count());
            if let (Some(k), true) = (k, (2..=40).contains(&count)) {
                let table = FastState::from_game(&game).unwrap().public();
                let run = |seed: u64| {
                    let mut rng = StdRng::seed_from_u64(seed);
                    evaluate(
                        &mut Solver::new(),
                        &k,
                        &table,
                        Objective::Points,
                        40,
                        &Uniform,
                        &mut rng,
                    )
                    .unwrap()
                };
                let d = run(1);
                assert!(d.all_deals && d.worlds as u64 == count, "seed {seed}");
                assert_eq!(run(2), d, "seed {seed}: an exact decision depends on the RNG");
                // The plain average over every deal, solved one by one.
                let mut sums = vec![0.0; d.moves.len()];
                k.deals().for_each_deal(|hands| {
                    let s = FastState::from_public(table, *hands).unwrap();
                    for (sum, (_, v)) in sums.iter_mut().zip(solver.move_values(&s, me % 2)) {
                        *sum += (s.points(me % 2) + v) as f64;
                    }
                });
                for (m, sum) in d.moves.iter().zip(sums) {
                    let mean = sum / count as f64;
                    assert!((m.means[0] - mean).abs() < 1e-9, "seed {seed}: mean of {:?}", m.mv);
                }
                checked += 1;
            }
            let action = random_action(&game, &mut rng);
            game.apply_action_mut(action);
        }
    }
    eprintln!("{checked} decisions with 2..=40 consistent deals solved deal by deal");
    assert!(checked >= 300, "too few small deal spaces: {checked}");
}

/// The exact measure after `m` in the true state (solver functions checked in tests/exact.rs).
fn exact_measure(solver: &mut Solver, s: &FastState, m: Move, measure: Measure) -> f64 {
    let c = s.after(m);
    match measure {
        Measure::Points { team } => (c.points(team) + solver.value(&c, team)) as f64,
        Measure::ReachPoints { team, target } => (c.points(team) + solver.value(&c, team) >= target) as u8 as f64,
        Measure::ReachTricks { team, target } => {
            (c.tricks_won(team) as i32 + solver.tricks(&c, team) >= target as i32) as u8 as f64
        }
    }
}

#[test]
fn contract_objective_follows_hc9() {
    let max = |measure| Criterion {
        measure,
        maximize: true,
    };
    let min = |measure| Criterion {
        measure,
        maximize: false,
    };
    let mut solver = Solver::new();
    // made, open, lost, defenders without a trick, defenders with a trick
    let mut seen = [0u32; 5];
    let mut known = 0;
    for seed in 0..60u64 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut game = game_from(&deal(&mut rng));
        while !game.ended() {
            if is_decision(&game, 4) {
                let me = game.state.player_at_turn.0;
                let k = knowledge_of(&game, me);
                let fast = FastState::from_game(&game).unwrap();
                let table = fast.public();
                // The criteria depend only on the contract, the score so far and the worlds, so
                // every decision is checked with either team declaring, and contracts already
                // made, still open and out of reach.
                for s in 0..2u8 {
                    let n = 1 - s;
                    for value in [fast.points(s) - 5, fast.points(s) + 15, fast.points(s) + 40, 400] {
                        let objective = Objective::Contract {
                            playing_team: s,
                            game_value: value,
                        };
                        let sampler = rng.clone();
                        let d =
                            evaluate(&mut solver, &k, &table, objective, 8, &Uniform, &mut sampler.clone()).unwrap();
                        if me % 2 == s {
                            // Site scoring: trickless declarers are punished too (schwarz âˆ’2A).
                            let first: Vec<Criterion> = if fast.tricks_won(s) == 0 {
                                vec![max(Measure::ReachTricks { team: s, target: 1 })]
                            } else {
                                vec![]
                            };
                            let with = |rest: Vec<Criterion>| [first.clone(), rest].concat();
                            // The defenders still without a trick: keep them schwarz (the owner's
                            // late_question_insurance), after the contract, before own points.
                            let keep_schwarz: Vec<Criterion> = if fast.tricks_won(n) == 0 {
                                vec![min(Measure::ReachTricks { team: n, target: 1 })]
                            } else {
                                vec![]
                            };
                            if fast.points(s) >= value {
                                assert_eq!(
                                    d.criteria,
                                    with([keep_schwarz.clone(), vec![min(Measure::Points { team: n })]].concat()),
                                    "seed {seed}: made"
                                );
                                seen[0] += 1;
                            } else if d.criteria.last() == Some(&min(Measure::Points { team: n })) {
                                // Fallback: the contract fails in every world after every move.
                                assert_eq!(
                                    d.criteria,
                                    with(vec![min(Measure::Points { team: n })]),
                                    "seed {seed}: lost"
                                );
                                let reach = Objective::Reach { team: s, target: value };
                                let r = evaluate(&mut solver, &k, &table, reach, 8, &Uniform, &mut sampler.clone());
                                assert!(
                                    r.unwrap().moves.iter().all(|m| m.means[0] == 0.0),
                                    "seed {seed}: chance left"
                                );
                                seen[2] += 1;
                            } else {
                                let reach = Measure::ReachPoints { team: s, target: value };
                                let own = Measure::Points { team: s };
                                assert_eq!(
                                    d.criteria,
                                    with([vec![max(reach)], keep_schwarz.clone(), vec![max(own)]].concat()),
                                    "seed {seed}: open"
                                );
                                seen[1] += 1;
                            }
                        } else if fast.tricks_won(n) == 0 {
                            let trick = Measure::ReachTricks { team: n, target: 1 };
                            let own = Measure::Points { team: n };
                            assert_eq!(d.criteria, vec![max(trick), max(own)], "seed {seed}: trickless");
                            seen[3] += 1;
                        } else {
                            assert_eq!(d.criteria, vec![max(Measure::Points { team: n })], "seed {seed}");
                            seen[4] += 1;
                        }
                        if d.all_deals && d.worlds == 1 {
                            for v in &d.moves {
                                for (c, mean) in d.criteria.iter().zip(&v.means) {
                                    let exact = exact_measure(&mut solver, &fast, v.mv, c.measure);
                                    assert_eq!(*mean, exact, "seed {seed}: {:?} after {:?}", c.measure, v.mv);
                                }
                            }
                            known += 1;
                        }
                    }
                }
            }
            let action = random_action(&game, &mut rng);
            game.apply_action_mut(action);
        }
    }
    eprintln!("HC-9 cases [made, open, lost, defenders without a trick, with a trick]: {seen:?}; {known} known deals");
    assert!(seen.iter().all(|&n| n >= 20), "too little coverage: {seen:?}");
}

fn replay(hands: &[Vec<Card>; 4], actions: &[GameAction]) -> Option<Game> {
    let mut game = game_from(hands);
    for action in actions {
        game = game.apply_action(*action).ok()?;
    }
    Some(game)
}

fn public_events(game: &Game) -> Vec<String> {
    game.all_events
        .iter()
        .map(|e| format!("{:?} {:?}", e.last_action, e.callback))
        .collect()
}

#[test]
fn hidden_cards_do_not_change_the_decision() {
    let mut swaps = 0;
    for seed in 0..20_000u64 {
        if swaps >= 150 {
            break;
        }
        let mut rng = StdRng::seed_from_u64(seed);
        let hands = deal(&mut rng);
        let mut game = game_from(&hands);
        let mut actions = vec![];
        let left = rng.random_range(2..=5);
        while !game.ended() && !is_decision(&game, left) {
            let action = random_action(&game, &mut rng);
            game.apply_action_mut(action);
            actions.push(action);
        }
        if game.ended() {
            continue;
        }
        let me = game.state.player_at_turn.0;
        let others: Vec<u8> = (0..4).filter(|&o| o != me).collect();
        let i = rng.random_range(0..3);
        let (x, y) = (others[i], others[(i + 1 + rng.random_range(0..2)) % 3]);
        let touched = actions.iter().fold(CardSet::EMPTY, |set, a| match &a.action_type {
            ActionType::Pass(cards) => set | CardSet::from_cards(cards),
            ActionType::CardPlayed(card) => set | CardSet::of(card),
            _ => set,
        });
        let known = knowledge_of(&game, me);
        let candidates = |from: u8, to: u8| -> Vec<Card> {
            hands[from as usize]
                .iter()
                .filter(|c| !touched.contains(c.index()) && known.may_hold(to).contains(c.index()))
                .cloned()
                .collect()
        };
        let (from_x, from_y) = (candidates(x, y), candidates(y, x));
        if from_x.is_empty() || from_y.is_empty() {
            continue;
        }
        let (a, b) = (
            from_x[rng.random_range(0..from_x.len())],
            from_y[rng.random_range(0..from_y.len())],
        );
        let mut swapped = hands.clone();
        for card in swapped.iter_mut().flatten() {
            *card = if *card == a {
                b
            } else if *card == b {
                a
            } else {
                *card
            };
        }
        let Some(other) = replay(&swapped, &actions) else {
            continue;
        };
        if public_events(&other) != public_events(&game) {
            continue;
        }
        let decide = |g: &Game| {
            let table = FastState::from_game(g).unwrap().public();
            let mut rng = StdRng::seed_from_u64(seed);
            let d = evaluate(
                &mut Solver::new(),
                &knowledge_of(g, me),
                &table,
                Objective::Points,
                16,
                &Uniform,
                &mut rng,
            );
            (table, d.unwrap())
        };
        assert_ne!(
            FastState::from_game(&other).unwrap().hands(),
            FastState::from_game(&game).unwrap().hands()
        );
        assert_eq!(
            decide(&other),
            decide(&game),
            "seed {seed}: the decision depends on hidden cards"
        );
        swaps += 1;
    }
    assert!(swaps >= 150, "only {swaps} valid swaps");
}

#[test]
fn interrupted_evaluation_compares_only_completed_worlds_and_preserves_cache_bounds() {
    use solver::{evaluate_limited, Limits, SolverError};
    use std::time::Instant;
    let mut rng = StdRng::seed_from_u64(41);
    let mut game = game_from(&deal(&mut rng));
    while !is_decision(&game, 4) {
        game.apply_action_mut(random_action(&game, &mut rng));
    }
    let me = game.state.player_at_turn.0;
    let k = knowledge_of(&game, me);
    assert!(k.deals().count() > 8);
    let table = FastState::from_game(&game).unwrap().public();
    let mut full_solver = Solver::new();
    let full = evaluate(
        &mut full_solver,
        &k,
        &table,
        Objective::Points,
        8,
        &Uniform,
        &mut StdRng::seed_from_u64(42),
    )
    .unwrap();
    let mut interrupted = Solver::new();
    assert_eq!(
        evaluate_limited(
            &mut interrupted,
            &k,
            &table,
            Objective::Points,
            8,
            &Uniform,
            &mut StdRng::seed_from_u64(42),
            Limits {
                deadline: Some(Instant::now()),
                nodes: None
            }
        ),
        Err(SolverError::Interrupted)
    );
    let partial = evaluate_limited(
        &mut interrupted,
        &k,
        &table,
        Objective::Points,
        8,
        &Uniform,
        &mut StdRng::seed_from_u64(42),
        Limits {
            deadline: None,
            nodes: Some(full_solver.nodes() - 1),
        },
    )
    .unwrap();
    assert!(!partial.complete && !partial.all_deals);
    assert!(partial.worlds > 0 && partial.worlds < 8);
    let prefix = evaluate(
        &mut Solver::new(),
        &k,
        &table,
        Objective::Points,
        partial.worlds,
        &Uniform,
        &mut StdRng::seed_from_u64(42),
    )
    .unwrap();
    assert_eq!(
        partial.moves, prefix.moves,
        "no half-evaluated world may bias a candidate"
    );
    let resumed = evaluate(
        &mut interrupted,
        &k,
        &table,
        Objective::Points,
        8,
        &Uniform,
        &mut StdRng::seed_from_u64(42),
    )
    .unwrap();
    assert_eq!(resumed, full, "interruption must not cache unfinished work as exact");
}

#[test]
fn a_node_limit_that_cannot_reach_the_required_worlds_stops_early() {
    use solver::{evaluate_moves, Effort, Limits, SolverError};
    let mut rng = StdRng::seed_from_u64(11);
    let mut game = game_from(&deal(&mut rng));
    while !is_decision(&game, 7) {
        let action = random_action(&game, &mut rng);
        game.apply_action_mut(action);
    }
    let me = game.state.player_at_turn.0;
    let k = knowledge_of(&game, me);
    let table = FastState::from_game(&game).unwrap().public();
    let effort = |required| Effort {
        worlds: 64,
        limits: Limits {
            deadline: None,
            nodes: Some(1_000_000),
        },
        racing: None,
        min_worlds: required,
    };
    let mut solver = Solver::new();
    let run = |solver: &mut Solver, required| {
        solver.reset_nodes();
        let result = evaluate_moves(
            solver,
            &k,
            &table,
            Objective::Points,
            &Uniform,
            &mut StdRng::seed_from_u64(3),
            effort(required),
            |_| true,
        );
        (result, solver.nodes())
    };
    // Without a requirement the limit yields the worlds it can complete.
    let (partial, _) = run(&mut solver, 0);
    let worlds = partial.unwrap().worlds;
    assert!(worlds < 64);
    // Requiring every world, the projection stops as soon as two worlds show it cannot fit.
    solver.clear();
    let (stopped, nodes) = run(&mut solver, 64);
    assert_eq!(stopped, Err(SolverError::Interrupted));
    assert!(nodes < 1_000_000, "{nodes} nodes spent before stopping");
}

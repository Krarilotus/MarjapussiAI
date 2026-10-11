use super::*;
use agent::{Explanation, SeatKind};
use engine::{ActionType, GameFinishedInfo};
use eval::{matchstats::MatchObserver, play_game_with, suite::DealSuite, RunError};
use std::{cell::RefCell, rc::Rc};

struct Fixed;
impl Agent for Fixed {
    fn act(&mut self, _view: &SeatView, legal: &[GameAction]) -> Decision {
        let action = legal
            .iter()
            .find(|a| a.action_type == ActionType::NewBid(140))
            .copied()
            .or_else(|| legal.iter().find(|a| a.action_type == ActionType::StopBidding).copied())
            .unwrap_or(legal[0]);
        Decision {
            action,
            explanation: Explanation {
                label: "fixed",
                facts: vec![],
            },
        }
    }
    fn kind(&self) -> SeatKind {
        SeatKind::Heuristic
    }
}

fn source(index: u64, suite: &DealSuite) -> Value {
    let mut hands = suite.hands(index / 4);
    if index % 4 >= 2 {
        hands.rotate_left(2);
    }
    let mut players: Vec<Box<dyn Agent>> = (0..4).map(|_| Box::new(Fixed) as Box<dyn Agent>).collect();
    let (info, log) = play_game_with(&hands, &mut players, |_, _, _| {});
    json!({
        "deal": index / 4, "deal_seed": suite.deal_seed(index / 4), "new_team": index % 2,
        "turned": index % 4 >= 2,
        "decisions": log.iter().enumerate().map(|(step, d)| json!({
            "step": step, "seat": d.seat,
            "side": if u64::from(d.seat % 2) == index % 2 { "new" } else { "old" },
            "action": format!("{:?}", d.decision.action), "rule": "fixed", "facts": {}, "ms": 0,
        })).collect::<Vec<_>>(),
        "scores": info.scores(), "team_points": info.team_points, "contract": info.game_value.0,
        "declarer": info.declarer.map(|s| s.0), "made": info.won, "all_passed": info.no_one_played,
        "schwarz": info.schwarz_game, "tricks": info.tricks.len(),
    })
}

fn fixture(index: u64) -> Fixture {
    let suite = DealSuite::named("seed20261011", 1).unwrap();
    Fixture::from_qualification(index, source(index, &suite), suite).unwrap()
}

fn registry() -> Vec<Policy> {
    ["new", "old", "candidate", "baseline"]
        .map(|name| Policy {
            name: name.into(),
            parameters: "fixed synthetic".into(),
            make: Box::new(|seed, _| {
                assert_eq!(seed, POLICY_SEED);
                Box::new(Fixed)
            }),
        })
        .into_iter()
        .collect()
}

fn options(extra: &[&str]) -> Options {
    let args = [
        "--input",
        "unused.ndjson",
        "--output",
        "unused-out.ndjson",
        "--deals",
        "1",
        "--cases",
        "1:0",
        "--candidate",
        "candidate",
        "--baseline-new",
        "new",
        "--baseline-old",
        "old",
    ];
    let cli = Cli::from_args(
        args.into_iter().chain(extra.iter().copied()).map(String::from),
        OPTIONS,
        &[],
    );
    Options::parse(&cli, &registry()).unwrap()
}

#[test]
fn original_prefix_rejects_wrong_identity_actor_action_and_scores() {
    let suite = DealSuite::named("seed20261011", 1).unwrap();
    let row = source(0, &suite);
    for (field, bad) in [
        ("deal", json!(1)),
        ("deal_seed", json!(0)),
        ("new_team", json!(1)),
        ("turned", json!(true)),
        ("scores", json!([0, 0])),
    ] {
        let mut wrong = row.clone();
        wrong[field] = bad;
        assert!(Fixture::from_qualification(0, wrong, suite.clone()).is_err(), "{field}");
    }
    for (field, bad) in [
        ("seat", json!(3)),
        ("step", json!(9)),
        ("side", json!("old")),
        ("action", json!("not legal")),
    ] {
        let mut wrong = row.clone();
        wrong["decisions"][0][field] = bad;
        assert!(Fixture::from_qualification(0, wrong, suite.clone()).is_err(), "{field}");
    }
    let mut short = row;
    short["decisions"].as_array_mut().unwrap().pop();
    assert!(Fixture::from_qualification(0, short, suite).is_err());
}

#[test]
fn turned_hands_and_private_passes_use_the_real_filtered_view() {
    let a = fixture(0);
    let b = fixture(2);
    let original = a.prefix(0).unwrap();
    let turned = b.prefix(0).unwrap();
    for seat in 0..4 {
        assert_eq!(turned.hand(seat), original.hand((seat + 2) % 4));
    }
    let step = a.phases.iter().position(|p| *p == "passing_back").unwrap() + 1;
    let table = a.prefix(step).unwrap();
    let involved = a
        .actions
        .iter()
        .take(step)
        .filter_map(|a| matches!(a.action_type, ActionType::Pass(_)).then_some(a.player.0))
        .collect::<Vec<_>>();
    let outsider = (0..4).find(|s| !involved.contains(s)).unwrap();
    for event in table.view(outsider).events {
        if let observation::SeatEvent::Pass { cards, .. } = event {
            assert!(cards.is_none());
        }
    }
    let snapshot = visible(&table, outsider);
    assert_eq!(
        snapshot["own_hand"],
        json!(cards(table.view(outsider).knowledge.certain(outsider)))
    );
    assert!(snapshot["posterior"].is_null());
    assert_eq!(table.kinds, [SeatKind::Heuristic; 4]);
}

#[test]
fn bounded_continuation_preserves_kinds_and_matches_engine_outcomes() {
    let f = fixture(0);
    let prefix = f.phases.iter().position(|p| *p == "start_trick").unwrap();
    let (table, mut observer) = f.prefix_observed(prefix).unwrap();
    struct Script {
        cursor: Rc<RefCell<usize>>,
        actions: Rc<Vec<GameAction>>,
    }
    impl Agent for Script {
        fn act(&mut self, view: &SeatView, legal: &[GameAction]) -> Decision {
            assert_eq!(view.kinds, [SeatKind::Heuristic; 4]);
            let mut cursor = self.cursor.borrow_mut();
            let action = self.actions[*cursor];
            *cursor += 1;
            assert!(legal.contains(&action));
            Decision {
                action,
                explanation: Explanation::default(),
            }
        }
    }
    let cursor = Rc::new(RefCell::new(prefix));
    let actions = Rc::new(f.actions.clone());
    let mut players: Vec<Box<dyn Agent>> = (0..4)
        .map(|_| {
            Box::new(Script {
                cursor: cursor.clone(),
                actions: actions.clone(),
            }) as Box<dyn Agent>
        })
        .collect();
    let (info, log) = play_table_with(table, &mut players, 128, |table, _, _| observer.observe(table)).unwrap();
    assert_eq!(prefix + log.len(), f.actions.len());
    assert_eq!(observer.finish(&info), f.original);
    let limit = play_table_with(f.prefix(0).unwrap(), &mut players, 0, |_, _, _| {}).unwrap_err();
    assert_eq!(limit, RunError::ActionLimit(0));
}

#[test]
fn illegal_policy_action_is_rejected_before_the_hook_or_engine() {
    struct Wrong;
    impl Agent for Wrong {
        fn act(&mut self, _: &SeatView, legal: &[GameAction]) -> Decision {
            let mut action = legal[0];
            action.player.0 = (action.player.0 + 1) % 4;
            Decision {
                action,
                explanation: Explanation::default(),
            }
        }
    }
    let mut players: Vec<Box<dyn Agent>> = (0..4).map(|_| Box::new(Wrong) as Box<dyn Agent>).collect();
    let error = play_table_with(fixture(0).prefix(0).unwrap(), &mut players, 1, |_, _, _| {
        panic!("illegal hook")
    })
    .unwrap_err();
    assert!(matches!(error, RunError::IllegalAction(_)));
}

#[test]
fn old_focal_action_and_partnership_are_separate_and_notes_align() {
    let f = fixture(1); // Seat 0 is old here, not new.
    let policies = registry();
    for replacement in ["action", "seat", "partnership"] {
        let o = options(&["--mode", "continue", "--replacement", replacement]);
        let result = continuation(&f, 0, 2, &policies, &o, None).unwrap();
        assert_eq!(result["candidate_seat"], 0);
        assert_eq!(result["candidate_team"], 0);
        let decisions = result["decisions"].as_array().unwrap();
        let actions = result["record"]["actions"].as_array().unwrap();
        assert_eq!(actions.len(), decisions.len());
        for (i, (d, a)) in decisions.iter().zip(actions).enumerate() {
            let seat = d["seat"].as_u64().unwrap();
            let candidate = match replacement {
                "action" => i == 0,
                "seat" => seat == 0,
                _ => seat % 2 == 0,
            };
            assert_eq!(
                d["policy"],
                if candidate {
                    "candidate"
                } else if seat % 2 == 1 {
                    "new"
                } else {
                    "old"
                }
            );
            assert_eq!(d["seat"], a["seat"]);
            assert_eq!(d["rule"], a["rule"]);
        }
        assert!(result["first_divergence"].is_null());
        assert_eq!(result["outcome"], metrics_json(&f.original));
    }
}

#[test]
fn teacher_is_fresh_seed_zero_and_all_steps_include_unchanged_choices() {
    let f = fixture(1);
    let o = options(&["--steps", "all", "--cases", "1"]);
    assert_eq!(o.steps_for(&f).unwrap(), (0..f.actions.len()).collect::<Vec<_>>());
    let calls = Rc::new(RefCell::new(Vec::new()));
    let recorded = calls.clone();
    let policy = Policy {
        name: "baseline8ca".into(),
        parameters: "source frozen at 8ca35c9".into(),
        make: Box::new(move |seed, seat| {
            recorded.borrow_mut().push((seed, seat));
            Box::new(Fixed)
        }),
    };
    for step in [0, 1] {
        let result = teacher(&f, step, &policy, None).unwrap();
        assert!(result["first_divergence"].is_null());
        assert!(result["outcome"].is_null());
    }
    assert_eq!(
        *calls.borrow(),
        vec![(0, f.actions[0].player.0), (0, f.actions[1].player.0)]
    );
}

#[test]
fn full_mode_replaces_explicit_recorded_old_team_and_ignores_prefix_filters() {
    let f = fixture(1);
    let o = options(&[
        "--mode",
        "full",
        "--replacement",
        "partnership",
        "--candidate-team",
        "old",
        "--phases",
        "trick",
    ]);
    assert_eq!(o.steps_for(&f).unwrap(), [0]);
    assert_eq!(target_seat(&f, 0, &o), 0);
    let result = continuation(&f, 0, 2, &registry(), &o, None).unwrap();
    assert_eq!(
        result["record"]["players"],
        json!(["candidate", "new", "candidate", "new"])
    );
}

#[test]
fn option_errors_reject_ambiguous_or_unbounded_plans() {
    for (args, reason) in [
        (vec!["--mode", "continue"], "explicit --baseline-new"),
        (vec!["--max-runs", "513"], "--max-runs"),
        (vec!["--cases", "125,125"], "distinct game indices"),
        (vec!["--phases", "madeup"], "unknown phase"),
        (
            vec!["--candidate", "candidate,candidate"],
            "distinct candidate policies",
        ),
    ] {
        let base = ["--input", "i", "--output", "o", "--candidate", "candidate"];
        let cli = Cli::from_args(base.into_iter().chain(args).map(String::from), OPTIONS, &[]);
        let error = Options::parse(&cli, &registry()).err().unwrap();
        assert!(error.contains(reason), "expected {reason}, got {error}");
    }
}

#[test]
fn failed_summary_retains_partial_count_and_end_profile() {
    let count = Rc::new(RefCell::new(0));
    let captured = count.clone();
    let profile = Profiling {
        reset: Box::new(|| {}),
        snapshot: Box::new(move || {
            *captured.borrow_mut() += 1;
            json!({"calls": 3})
        }),
    };
    let failed = summary(2, 1, &Err("illegal action".into()), Some(&profile), 4.0);
    assert_eq!(failed["status"], "failed");
    assert_eq!(failed["completed"], 1);
    assert_eq!(failed["expected"], 2);
    assert_eq!(failed["error"], "illegal action");
    assert_eq!(failed["profile"]["calls"], 3);
    assert_eq!(*count.borrow(), 1);
    let complete = summary(2, 2, &Ok(()), None, 4.0);
    assert_eq!(complete["status"], "complete");
    assert!(complete["profile"].is_null());
}

#[test]
fn dataset_scan_requires_all_four_ordered_rows_and_bounds_line_memory() {
    let o = options(&[]);
    let mut bytes = Vec::new();
    for index in 0..4 {
        serde_json::to_writer(&mut bytes, &source(index, &o.suite)).unwrap();
        bytes.push(b'\n');
    }
    let fixtures = options::load_reader(std::io::Cursor::new(&bytes), &o).unwrap();
    assert_eq!(fixtures.len(), 1);
    assert_eq!(fixtures[0].index, 1);
    let newline = bytes.iter().rposition(|&b| b == b'\n').unwrap();
    let before_last = bytes[..newline].iter().rposition(|&b| b == b'\n').unwrap() + 1;
    assert!(options::load_reader(std::io::Cursor::new(&bytes[..before_last]), &o).is_err());
    let large = vec![b'x'; 1_048_577];
    let error = options::load_reader(std::io::Cursor::new(large), &o).err().unwrap();
    assert!(error.contains("exceeds 1 MiB"));
}

#[test]
fn all_pass_scoring_and_contract_schwarz_remain_engine_owned() {
    let mut table = Table::new(&DealSuite::named("seed7", 1).unwrap().hands(0));
    while let Some((_, legal)) = table.to_act() {
        let action = legal
            .iter()
            .find(|a| a.action_type == ActionType::StopBidding)
            .copied()
            .unwrap_or(legal[0]);
        table.apply(action);
    }
    let info = GameFinishedInfo::from(table.game);
    let stats = MatchObserver::default().finish(&info);
    assert!(stats.contract.is_none());
    assert!(!stats.schwarz);
    assert_eq!(stats.scores, info.scores());
    assert_eq!(
        stats.team_points,
        [
            stats.card_points[0] + stats.pair_points[0],
            stats.card_points[1] + stats.pair_points[1]
        ]
    );
}

fn recorded_policy(recorder: AssessmentRecorder, calls: Rc<RefCell<usize>>) -> Policy {
    struct Counted {
        actual: Box<dyn Agent>,
        calls: Rc<RefCell<usize>>,
    }
    impl Agent for Counted {
        fn act(&mut self, view: &SeatView, legal: &[GameAction]) -> Decision {
            *self.calls.borrow_mut() += 1;
            self.actual.act(view, legal)
        }
        fn kind(&self) -> SeatKind {
            self.actual.kind()
        }
    }
    Policy {
        name: "candidate".into(),
        parameters: "current fast; tool recording".into(),
        make: Box::new(move |seed, _| {
            let params = heuristic::Params {
                seed,
                plan_worlds: 0,
                return_worlds: 0,
                endgame_tricks: 0,
                ..Default::default()
            };
            let actual = crate::selfplay::record_current(heuristic::HeuristicAgent::new(params), recorder.clone());
            Box::new(Counted {
                actual,
                calls: calls.clone(),
            })
        }),
    }
}

#[test]
fn teacher_records_the_same_call_once_and_rejects_leftover_evidence_before_calling() {
    let f = fixture(0);
    let recorder = AssessmentRecorder::default();
    let calls = Rc::new(RefCell::new(0));
    let policy = recorded_policy(recorder.clone(), calls.clone());
    let result = teacher(&f, 0, &policy, Some(&recorder)).unwrap();
    assert_eq!(*calls.borrow(), 1);
    let row = &result["decisions"][0];
    assert_eq!(row["assessment_recorded"], true);
    assert!(row["capture_ms"].is_number());
    assert!(row["assessment_serialization_ms"].is_number());
    assert_eq!(
        row["event_count"],
        f.prefix(0).unwrap().view(f.actions[0].player.0).events.len()
    );
    recorder.ensure_empty().unwrap();
    let table = f.prefix(0).unwrap();
    let (seat, legal) = table.to_act().unwrap();
    let mut actual = (policy.make)(0, seat);
    actual.act(&table.view(seat), legal); // Deliberately leave one same-context record queued.
    assert_eq!(*calls.borrow(), 2);
    assert!(teacher(&f, 0, &policy, Some(&recorder))
        .unwrap_err()
        .contains("boundary"));
    assert_eq!(*calls.borrow(), 2); // No replay or extra policy call to reconstruct evidence.
}

#[test]
fn continue_and_full_capture_only_instrumented_actual_calls_with_correct_action_override() {
    let f = fixture(0);
    for (mode, replacement, prefix) in [("continue", "action", 1), ("full", "partnership", 0)] {
        let recorder = AssessmentRecorder::default();
        let calls = Rc::new(RefCell::new(0));
        let mut policies = registry();
        policies[2] = recorded_policy(recorder.clone(), calls.clone());
        let options = options(&["--mode", mode, "--replacement", replacement]);
        let result = continuation(&f, prefix, 2, &policies, &options, Some(&recorder)).unwrap();
        let rows = result["decisions"].as_array().unwrap();
        let recorded = rows.iter().filter(|row| row["assessment_recorded"] == true).count();
        assert_eq!(recorded, *calls.borrow());
        assert!(recorded > 0);
        for row in rows {
            assert_eq!(row["assessment_recorded"], row["policy"] == "candidate");
            if row["policy"] != "candidate" {
                assert!(row["assessment"].is_null());
                assert_eq!(row["value_assessment_present"], false);
                assert_eq!(row["sequence_present"], false);
            }
        }
        if replacement == "action" {
            assert_eq!(recorded, 1);
        }
        recorder.ensure_empty().unwrap();
    }
}

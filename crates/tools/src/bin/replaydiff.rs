//! Ask the current HP1 for its decision at chosen steps of recorded self-play games (an
//! offline teacher comparison: what another policy did there, and what HP1 would do).
//!
//! `replaydiff --run DIR` reads `DIR/worker-*.ndjson` (marja-bench output) and, from stdin,
//! one JSON object per line `{"index": game index, "step": decision index}`. It replays each
//! game through the engine to that step (record actions matched against the legal actions)
//! and prints one JSON line: the recorded action and rule, HP1's action, rule and facts, the
//! seat's hand, the trick number and the trump. HP1 sees only the seat's view.
//!
//! `replaydiff --game FILE` reads one game instead, as the testbed stores it (`{"deal": [4 hands],
//! "actions": [...]}`, e.g. from the owner's local play database); its index is 0.
//!
//! For benchmark prefixes use `--heuristic-kinds --warm-history --params FILE`.
//! Warming calls only the queried seat's agent, always applies RECORDED actions,
//! and never continues beyond the queried state. `--beliefs` reads that agent's
//! seat-visible marginals and auction likelihoods AFTER timing its decision.
//! `--profile` requires the tools profile feature and records only the target act.
//! Timings include wall/start time and, on Windows, current-thread kernel+user CPU
//! time. Profile scopes use wall time too; profiling is not speed qualification.
//! This public-API tool cannot expose/inject the private Routes pool or HO rankings;
//! chosen plan facts are reported only when the policy itself emits them.

use std::collections::HashMap;
use std::io::{BufRead, Write};

use agent::{SeatKind, Table};
use engine::{Card, GamePhase};
use heuristic::HeuristicAgent;
use serde_json::{json, Value};
use tools::cli::Cli;
use tools::record::action_json;

#[cfg(windows)]
fn thread_cpu_100ns() -> Result<u64, String> {
    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThread() -> *mut std::ffi::c_void;
        fn GetThreadTimes(
            thread: *mut std::ffi::c_void,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
    }
    let (mut creation, mut exit, mut kernel, mut user) =
        (FileTime::default(), FileTime::default(), FileTime::default(), FileTime::default());
    // GetCurrentThread is a borrowed pseudo-handle. All output pointers refer to
    // writable FILETIME-compatible values that live through this synchronous call.
    let ok = unsafe { GetThreadTimes(GetCurrentThread(), &mut creation, &mut exit, &mut kernel, &mut user) };
    if ok == 0 {
        return Err(format!("GetThreadTimes: {}", std::io::Error::last_os_error()));
    }
    let ticks = |t: FileTime| ((t.high as u64) << 32) | t.low as u64;
    Ok(ticks(kernel) + ticks(user))
}

#[cfg(not(windows))]
fn thread_cpu_100ns() -> Result<u64, String> {
    Err("unavailable: GetThreadTimes is Windows-only".into())
}

fn same(recorded: &Value, legal: &Value) -> bool {
    let fields = ["type", "seat", "value", "card", "suit"];
    if fields.iter().any(|f| recorded.get(f) != legal.get(f)) {
        return false;
    }
    let cards = |v: &Value| {
        let mut c: Vec<String> = v["cards"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default();
        c.sort();
        c
    };
    cards(recorded) == cards(legal)
}

fn main() {
    let cli = Cli::parse(
        &["--run", "--game", "--params", "--label"],
        &["--heuristic-kinds", "--warm-history", "--beliefs", "--profile"],
    );
    assert!(!cli.flag("--profile") || cfg!(feature = "profile"), "--profile needs --features profile");
    let params = cli.params("--params");
    let mut games: HashMap<u64, Value> = HashMap::new();
    if let Some(file) = cli.str("--game") {
        let g: Value = serde_json::from_str(&std::fs::read_to_string(file).expect("game file")).expect("json");
        games.insert(0, json!({ "index": 0, "record": { "deal": g["deal"], "actions": g["actions"] } }));
    }
    for entry in cli.str("--run").map(|run| std::fs::read_dir(run).expect("run dir")).into_iter().flatten() {
        let path = entry.expect("dir entry").path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !(name.starts_with("worker-") && name.ends_with(".ndjson")) {
            continue;
        }
        for line in std::io::BufReader::new(std::fs::File::open(&path).expect("open")).lines() {
            let g: Value = serde_json::from_str(&line.expect("line")).expect("json");
            games.insert(g["index"].as_u64().unwrap(), g);
        }
    }
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in std::io::stdin().lock().lines() {
        let want: Value = serde_json::from_str(&line.expect("stdin")).expect("query");
        let (index, step) = (want["index"].as_u64().unwrap(), want["step"].as_u64().unwrap() as usize);
        let g = &games[&index];
        let deal: [Vec<Card>; 4] = std::array::from_fn(|s| {
            g["record"]["deal"][s]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c.as_str().unwrap().parse().expect("card"))
                .collect()
        });
        let actions = g["record"]["actions"].as_array().unwrap();
        assert!(step < actions.len(), "game {index}: step {step} is outside recorded actions");
        let target_seat = actions[step]["seat"].as_u64().expect("recorded target seat") as u8;
        let mut player = HeuristicAgent::new(params.clone());
        let mut table = Table::new(&deal);
        if cli.flag("--heuristic-kinds") {
            table.kinds = [SeatKind::Heuristic; 4];
        }
        for (i, recorded) in actions.iter().enumerate() {
            let (seat, legal) = table.to_act().expect("game continues");
            assert_eq!(recorded["seat"].as_u64(), Some(seat as u64), "recorded acting seat at step {i}");
            let raising = table.game.state.phase == GamePhase::Raising;
            if i == step {
                let view = table.view(seat);
                assert_eq!(seat, target_seat, "recorded target seat disagrees with engine");
                for (field, actual) in [("seat", seat as u64), ("event_count", view.events.len() as u64),
                                        ("policy_seed", params.seed)] {
                    if let Some(expected) = want[field].as_u64() {
                        assert_eq!(expected, actual, "game {index} step {step}: {field}");
                    }
                }
                if let Some(expected) = g["decisions"][i]["event_count"].as_u64() {
                    assert_eq!(expected, view.events.len() as u64, "recorded event count");
                }
                #[cfg(feature = "profile")]
                if cli.flag("--profile") {
                    beliefs::profile::reset();
                }
                let start_unix_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_millis() as u64);
                let cpu_before = thread_cpu_100ns();
                let started = std::time::Instant::now();
                let d = player.decide(&view, legal);
                let ms = started.elapsed().as_secs_f64() * 1000.0;
                let cpu_after = thread_cpu_100ns();
                let cpu = cpu_before.and_then(|before| cpu_after.and_then(|after| {
                    after.checked_sub(before).map(|ticks| ticks as f64 / 10_000.0)
                        .ok_or_else(|| "unavailable: thread CPU counter decreased".into())
                }));
                let timing = json!({"start_unix_ms": start_unix_ms, "wall_ms": ms,
                    "thread_cpu_ms": cpu.as_ref().ok(),
                    "thread_cpu_status": cpu.as_ref().map(|_| "available").unwrap_or_else(|e| e.as_str()),
                    "profile_compiled": cfg!(feature = "profile")});
                assert!(legal.contains(&d.action), "diagnostic policy returned an illegal action");
                #[cfg(feature = "profile")]
                let profile = cli.flag("--profile").then(|| {
                    beliefs::profile::snapshot().into_iter().map(|(name, t)| {
                        (name.to_string(), json!({"calls": t.calls,
                            "inclusive_ms": t.inclusive_ns as f64 / 1e6,
                            "exclusive_ms": t.exclusive_ns as f64 / 1e6}))
                    }).collect::<serde_json::Map<String, Value>>()
                });
                #[cfg(not(feature = "profile"))]
                let profile: Option<Value> = None;
                // Do not prewarm the target decision with diagnostic reads.
                let belief = cli.flag("--beliefs").then(|| {
                    let mind = player.mind(&view);
                    let holds: Vec<Vec<u32>> = (0..4).map(|s| {
                        (0..36).map(|c| mind.p_holds(s, c).to_bits()).collect()
                    }).collect();
                    let classes: Vec<_> = (0..4).map(|s| {
                        mind.class_likelihood(s, seat).map(|l| l.iter().map(|x| x.to_bits()).collect::<Vec<_>>())
                    }).collect();
                    json!({"p_holds_f32_bits_by_seat_card_index": holds,
                        "auction_class_likelihood_f32_bits_as_read_by_actor": classes,
                        "current_team_pair_f32_bits_by_suit": engine::SUITS.map(|s| mind.team_pair(s).to_bits()),
                        "epochs": format!("{:?}", mind.epochs()),
                        "scope": "post-decision seat-view reads; not an HO world-pool snapshot"})
                });
                let hand = view.knowledge.certain(seat);
                let facts: serde_json::Map<String, Value> =
                    d.why.iter().map(|f| (f.key.to_string(), json!(f.value))).collect();
                let line = json!({
                    "index": index, "step": step, "seat": seat, "trick": table.game.state.all_tricks.len() + 1,
                    "label": cli.str("--label"), "params": params.to_toml(),
                    "lineup": g["lineup"], "deal_index": g["deal_index"], "deal_seed": g["deal_seed"],
                    "policies": g["policies"], "phase": format!("{:?}", table.game.state.phase),
                    "recorded_policy_slot": g["lineup"][seat as usize],
                    "recorded_phase": g["decisions"][i]["phase"],
                    "event_count": view.events.len(),
                    "decision_seed": params.seed ^ (seat as u64) << 56 ^ view.events.len() as u64,
                    "warm_history": cli.flag("--warm-history"), "ms": ms, "timing": timing, "profile": profile,
                    "seat_kinds": view.kinds.map(|k| format!("{k:?}")),
                    "visible_events": view.events.iter().map(|e| format!("{e:?}")).collect::<Vec<_>>(),
                    "certain": (0..4).map(|s| view.knowledge.certain(s).0).collect::<Vec<_>>(),
                    "may_hold": (0..4).map(|s| view.knowledge.may_hold(s).0).collect::<Vec<_>>(),
                    "rights": (0..4).map(|s| format!("{:?}", view.knowledge.rights(s))).collect::<Vec<_>>(),
                    "legal": legal.iter().map(|a| action_json(a, raising)).collect::<Vec<_>>(),
                    "beliefs": belief,
                    "trump": view.knowledge.trump().map(|s| format!("{s:?}")),
                    "hand": hand.iter().map(|c| Card::from_index(c).to_string()).collect::<Vec<_>>(),
                    "recorded": recorded, "recorded_rule": g["decisions"][i]["rule"],
                    "recorded_facts": g["decisions"][i]["facts"], "recorded_ms": g["decisions"][i]["ms"],
                    "hp1": action_json(&d.action, raising), "hp1_rule": d.rule.code(), "hp1_facts": facts,
                });
                writeln!(out, "{line}").expect("write");
                break;
            }
            let action = *legal
                .iter()
                .find(|a| same(recorded, &action_json(a, raising)))
                .unwrap_or_else(|| panic!("game {index} step {i}: no legal action matches {recorded}"));
            if cli.flag("--warm-history") && seat == target_seat {
                // Recreate this policy's per-seat cache history, without using its
                // hypothetical choices to alter the recorded information set.
                let _ = player.decide(&table.view(seat), legal);
            }
            table.apply(action);
        }
    }
}

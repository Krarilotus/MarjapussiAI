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

use std::collections::HashMap;
use std::io::{BufRead, Write};

use agent::Table;
use engine::{Card, GamePhase};
use heuristic::{HeuristicAgent, Params};
use serde_json::{json, Value};
use tools::cli::Cli;
use tools::record::action_json;

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
    let cli = Cli::parse(&["--run", "--game"], &[]);
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
        let mut table = Table::new(&deal);
        for (i, recorded) in actions.iter().enumerate() {
            let (seat, legal) = table.to_act().expect("game continues");
            let raising = table.game.state.phase == GamePhase::Raising;
            if i == step {
                let view = table.view(seat);
                let d = HeuristicAgent::new(Params::default()).decide(&view, legal);
                let hand = view.knowledge.certain(seat);
                let facts: serde_json::Map<String, Value> =
                    d.why.iter().map(|f| (f.key.to_string(), json!(f.value))).collect();
                let line = json!({
                    "index": index, "step": step, "seat": seat, "trick": table.game.state.all_tricks.len() + 1,
                    "trump": view.knowledge.trump().map(|s| format!("{s:?}")),
                    "hand": hand.iter().map(|c| Card::from_index(c).to_string()).collect::<Vec<_>>(),
                    "recorded": recorded, "recorded_rule": g["decisions"][i]["rule"],
                    "hp1": action_json(&d.action, raising), "hp1_rule": d.rule.code(), "hp1_facts": facts,
                });
                writeln!(out, "{line}").expect("write");
                break;
            }
            let action = *legal
                .iter()
                .find(|a| same(recorded, &action_json(a, raising)))
                .unwrap_or_else(|| panic!("game {index} step {i}: no legal action matches {recorded}"));
            table.apply(action);
        }
    }
}

//! Bid or pass at the same auction decision, both played out (Astra's review §11.5, the
//! controlled comparison for "takes too much or executes badly"): for every take-or-defend
//! decision of the `new` policy in a marja-bench run (a bid decision against an opponent's
//! contract at 140 or more, where B-take was evaluated: its facts carry `p_take`), the game is
//! replayed to that decision and continued twice, once with the bid and once with the pass,
//! by four fresh HP1 agents (today's execution for every seat). Prints one JSON line per
//! decision with the deciding team's engine score difference in both branches.
//!
//! `branchdiff --run DIR [--max N]`

use std::io::{BufRead, Write};

use agent::{SeatKind, Table};
use engine::{ActionType, Card, GameAction, GameFinishedInfo};
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

/// Plays `table` to its end with fresh HP1 agents; the score difference of `team`.
fn finish(mut table: Table, team: usize) -> (i32, Value) {
    let mut agents: [HeuristicAgent; 4] = std::array::from_fn(|_| HeuristicAgent::new(Params::default()));
    while let Some((seat, legal)) = table.to_act() {
        let legal = legal.to_vec();
        let d = agents[seat as usize].decide(&table.view(seat), &legal);
        table.apply(d.action);
    }
    let info = GameFinishedInfo::from(table.game.clone());
    let s = info.scores();
    let o = info.outcome();
    (
        s[team] - s[1 - team],
        json!({
            "declaring_team": o.playing_team,
            "value": o.game_value,
            "team_points": o.team_points,
            "tricks": o.team_tricks,
        }),
    )
}

fn main() {
    let cli = Cli::parse(&["--run", "--max"], &[]);
    let run = cli.str("--run").expect("--run DIR");
    let max = cli.get("--max", usize::MAX);
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut done = 0usize;
    // The manifest: what continued the positions (Astra 17.1: newer continuations from the
    // positions an older policy reached).
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    eprintln!(
        "{}",
        json!({ "manifest": { "continuation_commit": commit, "params": format!("{:?}", Params::default()), "actors": "4 x HP1 (kinds Heuristic)", "source_run": run } })
    );
    let mut paths: Vec<_> = std::fs::read_dir(run)
        .expect("run dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy();
            n.starts_with("worker-") && n.ends_with(".ndjson")
        })
        .collect();
    paths.sort();
    for path in paths {
        for line in std::io::BufReader::new(std::fs::File::open(&path).expect("open")).lines() {
            let g: Value = serde_json::from_str(&line.expect("line")).expect("json");
            let new_seats: Vec<u64> = (0..4u64)
                .filter(|&s| g["policies"][g["lineup"][s as usize].as_u64().unwrap() as usize]["name"] == "new")
                .collect();
            let decisions = g["decisions"].as_array().unwrap();
            for (i, d) in decisions.iter().enumerate() {
                if done >= max {
                    return;
                }
                let seat = d["seat"].as_u64().unwrap();
                let takes_question = d["facts"]
                    .as_array()
                    .is_some_and(|f| f.iter().any(|x| x["key"] == "p_take"));
                if d["phase"] != "bidding" || !new_seats.contains(&seat) || !takes_question {
                    continue;
                }
                let deal: [Vec<Card>; 4] = std::array::from_fn(|s| {
                    g["record"]["deal"][s]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|c| c.as_str().unwrap().parse().expect("card"))
                        .collect()
                });
                let actions = g["record"]["actions"].as_array().unwrap();
                let step = d["step"].as_u64().unwrap() as usize;
                let mut table = Table::new(&deal);
                table.kinds = [SeatKind::Heuristic; 4];
                for recorded in &actions[..step] {
                    let (_, legal) = table.to_act().expect("game continues");
                    let action = *legal
                        .iter()
                        .find(|a| same(recorded, &action_json(a, false)))
                        .expect("a legal action matches");
                    table.apply(action);
                }
                let (to_move, legal) = table.to_act().expect("the decision");
                let legal: Vec<GameAction> = legal.to_vec();
                let value = table.game.state.value.0;
                let bid = legal
                    .iter()
                    .find(|a| matches!(a.action_type, ActionType::NewBid(v) if v == value + 5))
                    .copied();
                let pass = legal.iter().find(|a| a.action_type == ActionType::StopBidding).copied();
                let (Some(bid), Some(pass)) = (bid, pass) else { continue };
                let team = (to_move % 2) as usize;
                let branch = |a: GameAction| {
                    let mut t = Table::new(&deal);
                    // Every continuing actor is HP1, and the readers know it (Astra 17.1).
                    t.kinds = [SeatKind::Heuristic; 4];
                    for recorded in &actions[..step] {
                        let (_, legal) = t.to_act().expect("game continues");
                        let action = *legal.iter().find(|x| same(recorded, &action_json(x, false))).unwrap();
                        t.apply(action);
                    }
                    t.apply(a);
                    finish(t, team)
                };
                let (score_bid, bid_end) = branch(bid);
                let (score_pass, pass_end) = branch(pass);
                let facts: serde_json::Map<String, Value> = d["facts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|f| (f["key"].as_str().unwrap().to_string(), f["value"].clone()))
                    .collect();
                let recorded_bid = d["action"]["type"] == "bid";
                let row = json!({
                    "index": g["index"], "step": step, "seat": seat, "value": value,
                    "rule": d["rule"], "recorded_bid": recorded_bid, "facts": facts,
                    "score_bid": score_bid, "score_pass": score_pass, "bid_minus_pass": score_bid - score_pass,
                    "bid_end": bid_end, "pass_end": pass_end,
                });
                writeln!(out, "{row}").expect("write");
                done += 1;
                let _ = i;
            }
        }
    }
}

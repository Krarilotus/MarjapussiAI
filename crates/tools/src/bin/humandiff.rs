//! Where today's agent decides clearly worse than strong players (offline): in the human games,
//! at every non-forced decision of a seat in the chosen player groups where HP1 would act
//! differently, the game is continued twice from that decision, once with the player's action
//! and once with HP1's, by four HP1 agents, and the deciding team's engine score difference is
//! compared. A positive `human_minus_hp1` means the player's choice did better under the same
//! continuation (a one-deal realisation, not an ex-ante value: hidden cards are used only to
//! play the continuations out).
//!
//! `humandiff --games PATH --groups ref-A,ref-E [--from 2024-01]` (`MARJA_PLAYER_GROUPS` maps
//! names to the pseudonymous groups; names never leave `legacy`). One JSON line per disagreement.

use std::io::Write;

use agent::{SeatKind, Table};
use engine::{GameAction, GameFinishedInfo};
use heuristic::{HeuristicAgent, Params};
use serde_json::{json, Value};
use tools::cli::Cli;
use tools::legacy;
use tools::record::action_json;

/// Plays `table` to its end with fresh HP1 agents; the score difference of `team` and the ending.
fn finish(mut table: Table, team: usize) -> (i32, Value) {
    table.kinds = [SeatKind::Heuristic; 4];
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
        json!({ "declaring_team": o.playing_team, "value": o.game_value, "team_points": o.team_points, "tricks": o.team_tricks }),
    )
}

fn main() {
    let cli = Cli::parse(&["--games", "--groups", "--from"], &[]);
    let path = cli.str("--games").expect("--games PATH");
    let groups: Vec<String> = cli
        .str("--groups")
        .expect("--groups")
        .split(',')
        .map(String::from)
        .collect();
    let from = cli.str("--from").unwrap_or("0000-00").to_string();
    let (recs, _) = legacy::read(path);
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for (index, rec) in recs.iter().enumerate() {
        let Ok(meta) = legacy::meta(rec) else { continue };
        if meta.month < from {
            continue;
        }
        let seat_groups = legacy::seat_groups(rec);
        if !seat_groups.iter().any(|g| groups.iter().any(|w| w == g)) {
            continue;
        }
        let mut rows: Vec<Value> = Vec::new();
        let replayed = legacy::replay(rec, |step| {
            let group = seat_groups[step.seat as usize];
            if step.claimed_by.is_some() || step.legal.len() < 2 || !groups.iter().any(|w| w == group) {
                return;
            }
            let view = step.table.view(step.seat);
            let d = HeuristicAgent::new(Params::default()).decide(&view, step.legal);
            if legacy::same(&d.action, step.human) {
                return;
            }
            let team = (step.seat % 2) as usize;
            let branch = |a: GameAction| {
                let mut t = step.table.clone();
                t.apply(a);
                finish(t, team)
            };
            let (human_score, human_end) = branch(*step.human);
            let (hp1_score, hp1_end) = branch(d.action);
            let after_exchange = step.table.game.state.phase == engine::GamePhase::Raising;
            let facts: serde_json::Map<String, Value> =
                d.why.iter().map(|f| (f.key.to_string(), json!(f.value))).collect();
            // What the deciding player saw (for the review with Astra): its dealt and current
            // hand, the auction, the contract, the trump and the tricks so far.
            let state = &step.table.game.state;
            let cards = |s: engine::CardSet| {
                s.iter()
                    .map(|c| engine::Card::from_index(c).to_string())
                    .collect::<Vec<_>>()
            };
            let context = json!({
                "dealt": meta.hands[step.seat as usize].iter().map(|c| c.to_string()).collect::<Vec<_>>(),
                "hand": cards(step.table.hand(step.seat)),
                "auction": state.bidding_history.iter().map(|(a, p)| json!({ "seat": p.0, "action": format!("{a:?}") })).collect::<Vec<_>>(),
                "declarer": state.declarer().map(|p| p.0),
                "value": state.value.0,
                "trump": state.trump.map(|s| format!("{s:?}")),
                "called": state.trump_called.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>(),
                "tricks": state.all_tricks.iter().map(|t| json!({ "leader": t.leader.0, "cards": t.cards.iter().map(|c| c.to_string()).collect::<Vec<_>>(), "winner": t.winner.0 })).collect::<Vec<_>>(),
                "current_trick": state.current_trick.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
            });
            rows.push(json!({
                "game": index, "month": meta.month, "group": group, "seat": step.seat, "phase": step.phase,
                "trick": step.table.game.state.all_tricks.len() + 1,
                "human": action_json(step.human, after_exchange), "hp1": action_json(&d.action, after_exchange),
                "hp1_rule": d.rule.code(), "hp1_facts": facts, "context": context,
                "human_score": human_score, "hp1_score": hp1_score, "human_minus_hp1": human_score - hp1_score,
                "human_end": human_end, "hp1_end": hp1_end,
            }));
        });
        if replayed.is_err() {
            continue;
        }
        for row in rows {
            writeln!(out, "{row}").expect("write");
        }
    }
}

//! `study bidding` (C36): at every human bidding decision, HP1 is asked for its action from that
//! seat's view; the two are compared by situation cell. Output: Markdown on stdout.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use agent::Agent;
use engine::{ActionType, CardSet, Suit};
use eval::suite::DealSuite;
use heuristic::{HeuristicAgent, Params};
use observation::Seat;
use serde_json::Value;
use tools::legacy::{meta, replay, seat_groups};

/// The actor's hand class: ace or not, and the best of red pair, Bells pair, small pair (Acorns
/// or Green), 3+ halves, 2 halves, fewer.
pub(crate) fn hand_class(h: CardSet) -> String {
    let ace = if (h & CardSet::ACES).is_empty() { "-" } else { "A" };
    let pair = |s: Suit| CardSet::halves(s).is_subset(h);
    let halves = (h & CardSet::HALVES).len();
    let shape = if pair(Suit::Red) {
        "red pair"
    } else if pair(Suit::Bells) {
        "Bells pair"
    } else if pair(Suit::Acorns) || pair(Suit::Green) {
        "small pair"
    } else if halves >= 3 {
        "3+ halves"
    } else if halves == 2 {
        "2 halves"
    } else {
        "<2 halves"
    };
    format!("{ace} {shape}")
}

pub(crate) fn action_key(bid: Option<i32>, value: i32) -> String {
    match bid {
        None => "pass".into(),
        Some(140) => "=140".into(),
        Some(b) if b - value <= 20 => format!("+{}", b - value),
        Some(_) => "jump".into(),
    }
}

pub(crate) fn band(v: i32) -> &'static str {
    match v {
        ..=115 => "115",
        116..=135 => "120-135",
        140 => "140",
        _ => "145+",
    }
}

#[derive(Default)]
struct Cell {
    n: u64,
    agree: u64,
    human: BTreeMap<String, u64>,
    hp1: BTreeMap<String, u64>,
    /// (game, step, HP1's rule, its facts)
    examples: Vec<(usize, usize, &'static str, String)>,
    groups: BTreeMap<(&'static str, &'static str), (u64, u64)>,
}

pub(crate) fn dist(m: &BTreeMap<String, u64>, n: u64) -> String {
    let mut v: Vec<_> = m.iter().collect();
    v.sort_by(|a, b| b.1.cmp(a.1));
    v.iter()
        .map(|(k, c)| format!("{k} {:.0} %", 100.0 * **c as f64 / n.max(1) as f64))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn bid_of(t: &ActionType) -> Option<i32> {
    match t {
        ActionType::NewBid(v) => Some(*v),
        _ => None,
    }
}

pub fn run(recs: &[Value], selfplay: u64) -> String {
    let mut cells: BTreeMap<String, Cell> = BTreeMap::new();
    let mut by_group: BTreeMap<(&'static str, &'static str), (u64, u64)> = BTreeMap::new();
    let mut by_rule: BTreeMap<String, (u64, u64, BTreeMap<String, u64>)> = BTreeMap::new();
    let params = Params::default();
    for (id, rec) in recs.iter().enumerate() {
        let Ok(m) = meta(rec) else { continue };
        let era = if m.month.as_str() <= "2023-12" {
            "≤ 2023"
        } else {
            "≥ 2024"
        };
        let groups = seat_groups(rec);
        let mut players: Vec<HeuristicAgent> = (0..4).map(|_| HeuristicAgent::new(params.clone())).collect();
        let mut step = 0usize;
        let mut first: Option<Seat> = None;
        let _ = replay(rec, |s| {
            let at = step;
            step += 1;
            if !matches!(s.phase, "first bid" | "later bid") {
                return;
            }
            let f = *first.get_or_insert(s.seat);
            let state = &s.table.game.state;
            let value = state.value.0;
            let mate = (s.seat + 2) % 4;
            let partner = match state.bidding_history.iter().rev().find(|(_, p)| p.0 == mate) {
                None => "none",
                Some((ActionType::NewBid(_), _)) => "step",
                Some(_) => "pass",
            };
            let opp_actions: Vec<_> = state
                .bidding_history
                .iter()
                .filter(|(_, p)| p.0 % 2 != s.seat % 2)
                .collect();
            let opponents = if opp_actions.iter().any(|(t, _)| matches!(t, ActionType::NewBid(_))) {
                "step"
            } else if opp_actions.is_empty() {
                "none"
            } else {
                "pass"
            };
            let d = players[s.seat as usize].decide(&s.table.view(s.seat), s.legal);
            let (hb, ab) = (bid_of(&s.human.action_type), bid_of(&d.action.action_type));
            let agree = hb == ab;
            let pos = (s.seat + 4 - f) % 4 + 1;
            let key = format!(
                "pos {pos} | partner {partner} | opponents {opponents} | {} | {}",
                hand_class(s.table.hand(s.seat)),
                band(value)
            );
            let c = cells.entry(key).or_default();
            c.n += 1;
            c.agree += agree as u64;
            *c.human.entry(action_key(hb, value)).or_default() += 1;
            *c.hp1.entry(action_key(ab, value)).or_default() += 1;
            let g = (groups[s.seat as usize], era);
            let e = c.groups.entry(g).or_default();
            e.0 += 1;
            e.1 += agree as u64;
            let e = by_group.entry(g).or_default();
            e.0 += 1;
            e.1 += agree as u64;
            // B-tell where B-gate part 1's hand held (telling came first) is listed apart.
            let part = |k: &str| d.why.iter().find(|f| f.key == k).map(|f| f.value);
            let rule = match (d.rule.code(), part("gate_part"), part("gate_part1_hand")) {
                ("B-gate", Some(p), _) => format!("B-gate part {p}"),
                ("B-tell", _, Some(_)) => "B-tell (gate part 1 hand held)".to_string(),
                (code, _, _) => code.to_string(),
            };
            let r = by_rule.entry(rule).or_default();
            r.0 += 1;
            r.1 += agree as u64;
            *r.2.entry(action_key(hb, value)).or_default() += 1;
            if !agree && c.examples.len() < 2 {
                let facts = d
                    .why
                    .iter()
                    .map(|f| format!("{}={}", f.key, f.value))
                    .collect::<Vec<_>>()
                    .join(" ");
                c.examples.push((id, at, d.rule.code(), facts));
            }
        });
    }
    let mut out = String::new();
    let (n, a) = cells.values().fold((0, 0), |(n, a), c| (n + c.n, a + c.agree));
    let pct = |a: u64, n: u64| 100.0 * a as f64 / n.max(1) as f64;
    let _ = writeln!(
        out,
        "- **{n} human bidding decisions, HP1 agrees on {:.1} %** (the same bid or pass).\n",
        pct(a, n)
    );
    let _ = writeln!(
        out,
        "### By HP1's rule\n\n| HP1's rule | decisions | agreement | what the humans did |\n|---|---|---|---|"
    );
    for (r, (n, a, human)) in &by_rule {
        let _ = writeln!(out, "| {r} | {n} | {:.1} % | {} |", pct(*a, *n), dist(human, *n));
    }
    let _ = writeln!(
        out,
        "\n### By player and era\n\n| player | era | decisions | agreement |\n|---|---|---|---|"
    );
    for ((g, e), (n, a)) in &by_group {
        let _ = writeln!(out, "| {g} | {e} | {n} | {:.1} % |", pct(*a, *n));
    }
    let mut ranked: Vec<_> = cells.iter().collect();
    ranked.sort_by(|x, y| (y.1.n - y.1.agree).cmp(&(x.1.n - x.1.agree)));
    let _ = writeln!(
        out,
        "\n### The 10 cells with the most disagreements (disagreement × n)\n\nCell = auction position | the partner's last action | the opponents' actions | my hand class (A = an ace) | value band."
    );
    for (i, (key, c)) in ranked.iter().take(10).enumerate() {
        let _ = writeln!(
            out,
            "\n**{}. {key}**: n = {}, agreement {:.0} %, {} disagreements.",
            i + 1,
            c.n,
            pct(c.agree, c.n),
            c.n - c.agree
        );
        let _ = writeln!(out, "- humans: {}", dist(&c.human, c.n));
        let _ = writeln!(out, "- HP1: {}", dist(&c.hp1, c.n));
        let split: Vec<String> = c
            .groups
            .iter()
            .map(|((g, e), (n, a))| format!("{g} {e}: {n} ({:.0} %)", pct(*a, *n)))
            .collect();
        let _ = writeln!(out, "- by player and era: {}", split.join("; "));
        for (g, s, rule, facts) in &c.examples {
            let _ = writeln!(
                out,
                "- example: game {g} step {s} (`study transcript {g}`): HP1 {rule} [{facts}]"
            );
        }
    }
    let _ = writeln!(
        out,
        "\n### Every cell with n ≥ 20, by n\n\n| cell | n | agreement | humans | HP1 |\n|---|---|---|---|---|"
    );
    let mut by_n: Vec<_> = cells.iter().filter(|(_, c)| c.n >= 20).collect();
    by_n.sort_by(|x, y| y.1.n.cmp(&x.1.n));
    for (key, c) in by_n {
        let _ = writeln!(
            out,
            "| {key} | {} | {:.0} % | {} | {} |",
            c.n,
            pct(c.agree, c.n),
            dist(&c.human, c.n),
            dist(&c.hp1, c.n)
        );
    }
    if selfplay > 0 {
        out.push_str(&self_play(selfplay));
    }
    out
}

/// HP1 self-play on the `holdout` suite (information only): all-pass, make rate, 140 blocks.
fn self_play(games: u64) -> String {
    let suite = DealSuite::named("holdout", games).expect("suite");
    let p = Params::default();
    let (mut all_pass, mut declared, mut made, mut blocks, mut block_games) = (0u64, 0u64, 0u64, 0u64, 0u64);
    for g in 0..games {
        let mut players: Vec<Box<dyn Agent>> = (0..4)
            .map(|_| Box::new(HeuristicAgent::new(p.clone())) as Box<dyn Agent>)
            .collect();
        let (info, log) = eval::play_game(&suite.hands(g), &mut players);
        let b = log
            .iter()
            .filter(|l| l.decision.explanation.label == "B-defend")
            .count() as u64;
        blocks += b;
        block_games += (b > 0) as u64;
        match info.playing_party {
            None => all_pass += 1,
            Some(t) => {
                declared += 1;
                made += (info.team_points[t.0 as usize % 2] >= info.game_value.0) as u64;
            }
        }
    }
    let pct = |a: u64, n: u64| 100.0 * a as f64 / n.max(1) as f64;
    format!(
        "\n### HP1 self-play, `holdout`, {games} games (information only)\n\n- all passed: {:.1} %;\n- contracts made: {made} of {declared} ({:.1} %);\n- B-defend: {blocks} bids, in {:.1} % of the games.\n",
        pct(all_pass, games),
        pct(made, declared),
        pct(block_games, games)
    )
}

//! Agreement of HP1 with the human games (spec §9.1). Each game is replayed through the
//! engine; at every human decision HP1 gets only that seat's view and the legal actions.
//! Baselines: uniform random legal, and the engine's first legal action.
//!
//! The data holds real player names: this tool reads them only to look up the cards and
//! prints aggregates only. With `--records FILE` it writes game records (pseudonyms, never names)
//! with HP1's suggestion at every human decision (`docs/design/game-records.md`).
//! `--calibration` adds the reliability of HP1's beliefs about the other seats' hands (C21).
//! `human_eval --games PATH [--params FILE] [--limit N] [--since YYYY-MM] [--until YYYY-MM]
//!            [--records FILE [--records-beliefs]] [--calibration]`

use std::collections::BTreeMap;

use engine::ActionType;
use engine::CardSet;
use heuristic::{HeuristicAgent, Params};
use serde_json::Value;
use std::io::Write as _;

use engine::GameFinishedInfo;
use eval::stats::{rate, Reliability};
use eval::Rng;
use tools::cli::Cli;
use tools::legacy::{self, same, Step};
use tools::record::{beliefs_json, game_record, params_hash, Meta, Note};

#[derive(Default, Clone, Copy)]
struct Tally {
    n: u64,
    hp: u64,
    hp_any_cross: u64,
    random: u64,
    first: u64,
    /// Passing: cards in common with the human's 4, summed.
    hp_cards: u64,
}

#[derive(Default)]
struct Report {
    phases: BTreeMap<&'static str, Tally>,
    /// Per HP1 rule: (fired, agreed).
    rules: BTreeMap<&'static str, (u64, u64)>,
    games: u64,
    replayed: u64,
    aborted: BTreeMap<String, u64>,
    /// Log events the replay skipped or reinterpreted, by kind.
    skipped: BTreeMap<&'static str, u64>,
    /// Why the dropped questions were not allowed (Judge J17 item 2).
    dropped_questions: BTreeMap<&'static str, u64>,
    pass_games: u64,
    /// `--calibration`: per belief feature, its reliability against the true hands.
    calibrate: bool,
    calibration: BTreeMap<(&'static str, &'static str), Reliability>,
}

/// C21 (S5): HP1's beliefs about every other seat's current hand, scored against the true hands,
/// for each feature the hard facts leave open (has an ace; pair in suit s; a half of suit s).
fn calibrate(report: &mut Report, step: &Step, mind: &beliefs::Mind) {
    let k = step.table.view(step.seat).knowledge;
    let phase = match step.phase {
        p if p.contains("bid") => 0,
        p if p.starts_with("pass") || p == "raise" => 1,
        _ => 2,
    };
    for y in (0..4u8).filter(|&y| y != step.seat) {
        const GROUPS: [[&str; 2]; 3] = [
            ["1 auction, partner", "1 auction, opponents"],
            ["2 exchange, partner", "2 exchange, opponents"],
            ["3 card play, partner", "3 card play, opponents"],
        ];
        let group = GROUPS[phase][(y != engine::partner(step.seat)) as usize];
        let (b, hand) = (mind.features(y), step.table.hand(y));
        let (certain, may) = (k.certain(y), k.may_hold(y));
        let mut add = |name, set: CardSet, all: bool, p: f32| {
            let open = if all {
                !set.is_subset(certain) && set.is_subset(may)
            } else {
                (certain & set).is_empty() && !(may & set).is_empty()
            };
            if open {
                let happened = if all {
                    set.is_subset(hand)
                } else {
                    !(hand & set).is_empty()
                };
                report
                    .calibration
                    .entry((name, group))
                    .or_default()
                    .add(p as f64, happened);
            }
        };
        add("has an ace", CardSet::ACES, false, b.ace);
        for s in engine::SUITS {
            add("pair in suit s", CardSet::halves(s), true, b.pair[s as usize]);
            add("a half of suit s", CardSet::halves(s), false, b.half[s as usize]);
        }
    }
}

/// Compares HP1 (both 140 variants), the baselines and the human at one decision.
fn record(
    report: &mut Report,
    step: &Step,
    players: &mut [HeuristicAgent],
    variants: &mut [HeuristicAgent],
    rng: &mut Rng,
) -> Option<heuristic::Decision> {
    let Step {
        table,
        seat,
        legal,
        human,
        phase,
    } = *step;
    if legal.len() < 2 {
        return None;
    }
    let view = table.view(seat);
    let d = players[seat as usize].decide(&view, legal);
    if report.calibrate {
        calibrate(report, step, players[seat as usize].mind(&view));
    }
    let mut t = report.phases.get(phase).copied().unwrap_or_default();
    t.n += 1;
    let agree = same(&d.action, human);
    t.hp += agree as u64;
    if phase.contains("bid") || phase == "raise" {
        let v = variants[seat as usize].decide(&view, legal);
        t.hp_any_cross += same(&v.action, human) as u64;
    }
    t.random += same(&legal[rng.below(legal.len())], human) as u64;
    t.first += same(&legal[0], human) as u64;
    if let (ActionType::Pass(a), ActionType::Pass(b)) = (&d.action.action_type, &human.action_type) {
        t.hp_cards += (CardSet::from_cards(a) & CardSet::from_cards(b)).len() as u64;
    }
    report.phases.insert(phase, t);
    let r = report.rules.entry(d.rule.code()).or_default();
    r.0 += 1;
    r.1 += agree as u64;
    Some(d)
}

/// Where game records go (`--records FILE`, `--records-beliefs`).
struct Out {
    writer: std::io::BufWriter<std::fs::File>,
    beliefs: bool,
    pseudonyms: legacy::Pseudonyms,
}

/// Replays one game; returns why it stopped early, if it did.
fn replay(
    rec: &Value,
    report: &mut Report,
    params: &Params,
    game_no: u64,
    out: Option<&mut Out>,
) -> Result<(), String> {
    let mut players: Vec<HeuristicAgent> = (0..4).map(|_| HeuristicAgent::new(params.clone())).collect();
    let mut any_cross = params.clone();
    any_cross.beliefs.conventions.skip_on_any_cross = true;
    let mut variants: Vec<HeuristicAgent> = (0..4).map(|_| HeuristicAgent::new(any_cross.clone())).collect();
    let mut rng = Rng::new(game_no);
    let with_beliefs = out.as_ref().is_some_and(|o| o.beliefs);
    let mut notes = vec![];
    let replayed = legacy::replay(rec, |step| {
        let beliefs = with_beliefs.then(|| beliefs_json(players[step.seat as usize].mind(&step.table.view(step.seat))));
        let hp1 = record(report, step, &mut players, &mut variants, &mut rng);
        notes.push(Note {
            decision: None,
            hp1: hp1.map(Into::into),
            beliefs,
        });
    })?;
    for (kind, n) in &replayed.skipped {
        *report.skipped.entry(kind).or_default() += *n as u64;
    }
    for (kind, n) in &replayed.dropped_questions {
        *report.dropped_questions.entry(kind).or_default() += *n as u64;
    }
    let table = replayed.table;
    if table.game.state.value == engine::NO_BID {
        report.pass_games += 1;
    }
    if let Some(out) = out {
        let meta = Meta {
            source: "human",
            id: format!("human-{game_no}"),
            seed: None,
            params_hash: Some(params_hash(params)),
            date: legacy::meta(rec)?.month,
            players: out.pseudonyms.of(rec),
        };
        let line = game_record(&meta, &GameFinishedInfo::from(table.game), &notes);
        writeln!(out.writer, "{line}").map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn main() {
    let cli = Cli::parse(
        &["--games", "--params", "--limit", "--records", "--since", "--until"],
        &["--records-beliefs", "--calibration"],
    );
    let path = cli.str("--games").expect("--games PATH").to_string();
    let params = cli.params("--params");
    let limit = cli.get("--limit", u64::MAX);
    let (records_path, records_beliefs) = (cli.str("--records"), cli.flag("--records-beliefs"));
    let (records, bad) = legacy::read(&path);
    let mut report = Report {
        calibrate: cli.flag("--calibration"),
        ..Report::default()
    };
    if bad > 0 {
        report.aborted.insert("json".into(), bad as u64);
    }
    let mut out = records_path.map(|p| Out {
        writer: std::io::BufWriter::new(std::fs::File::create(p).unwrap_or_else(|e| panic!("--records {p}: {e}"))),
        beliefs: records_beliefs,
        pseudonyms: legacy::Pseudonyms::default(),
    });
    let wall = std::time::Instant::now();
    // Judge S4: a time split (months as YYYY-MM, inclusive), e.g. agreement out of sample.
    let (since, until) = (cli.str("--since").unwrap_or(""), cli.str("--until").unwrap_or("9999"));
    for (n, rec) in records.iter().enumerate() {
        if n as u64 >= limit {
            break;
        }
        let month = legacy::meta(rec).map(|m| m.month).unwrap_or_default();
        if month.as_str() < since || month.as_str() > until {
            continue;
        }
        report.games += 1;
        match replay(rec, &mut report, &params, n as u64, out.as_mut()) {
            Ok(()) => report.replayed += 1,
            Err(e) => *report.aborted.entry(e).or_default() += 1,
        }
    }
    print(&report, wall.elapsed().as_secs_f64());
}

fn print(r: &Report, secs: f64) {
    println!("# HP1 vs the human games");
    println!();
    println!(
        "{} games read, {} replayed to the end ({} of them pass games); decisions before a replay stop are counted. {secs:.1} s.",
        r.games, r.replayed, r.pass_games
    );
    if !r.aborted.is_empty() {
        println!("Stopped early (games left out after the stop):");
        for (k, v) in &r.aborted {
            println!("- {v} × {k}");
        }
    }
    if !r.skipped.is_empty() {
        println!("Log events skipped or reinterpreted by the replay:");
        for (k, v) in &r.skipped {
            println!("- {k}: {v}");
        }
    }
    if !r.dropped_questions.is_empty() {
        println!("Why the dropped questions were not allowed by the engine:");
        for (k, v) in &r.dropped_questions {
            println!("- {k}: {v}");
        }
    }
    println!();
    println!("| phase | decisions | HP1 agrees | HP1, skip_on_any_cross | random legal | first legal |");
    println!("|---|---|---|---|---|---|");
    for (phase, t) in &r.phases {
        let variant = if phase.contains("bid") || *phase == "raise" {
            rate(t.hp_any_cross, t.n)
        } else {
            "–".into()
        };
        println!(
            "| {phase} | {} | {} | {variant} | {} | {} |",
            t.n,
            rate(t.hp, t.n),
            rate(t.random, t.n),
            rate(t.first, t.n)
        );
    }
    for (phase, t) in r.phases.iter().filter(|(p, _)| p.starts_with("pass")) {
        println!(
            "\n{phase}: HP1 shares {:.2} of the human's 4 cards on average.",
            t.hp_cards as f64 / t.n.max(1) as f64
        );
    }
    println!();
    println!("| HP1 rule | fired | human agrees |");
    println!("|---|---|---|");
    for (rule, (n, k)) in &r.rules {
        println!("| {rule} | {n} | {} |", rate(*k, *n));
    }
    if !r.calibration.is_empty() {
        println!("\n## Calibration of HP1's beliefs (other seats' current hands, open features only)\n");
        println!("| feature | phase | n | Brier | ECE |\n|---|---|---|---|---|");
        for ((name, group), rel) in &r.calibration {
            println!(
                "| {name} | {group} | {} | {:.4} | {:.4} |",
                rel.n(),
                rel.brier(),
                rel.ece()
            );
        }
        for ((name, group), rel) in &r.calibration {
            println!("\n**{name}, {group}**\n\n{}", rel.table());
        }
    }
}

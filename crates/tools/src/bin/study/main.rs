//! Reading aids for studying the human games by hand (C33). Not a model: it replays each game
//! through the engine (`tools::legacy`) and prints, to stdout only. Seats are pseudonymised as
//! N, E, S, W (seats 0–3); a game is its index among the parsed records of the dataset.
//!
//! `study transcript <game>`: one compact page per game.
//! `study find [filters] [--list N]`: the human decisions matching every filter, with counts.
//! `study bidding [--selfplay N]`: HP1 against every human bidding decision, by situation cell
//! (C36), in Markdown; `--selfplay` adds HP1 self-play rates on `holdout` (information only).
//!
//! The dataset path comes from `MARJA_GAMES` (or `--games PATH`); it holds real names, which
//! never leave `tools::legacy`.
//!
//! Filters of `find` (all optional, combined with "and"):
//! - `--phase first-bid|later-bid|raise|pass-forth|pass-back|trump|lead|follow|answer`;
//! - `--pos 1..4` (auction position: 1 = the first bidder), `--action pass|bid|+5|+10|+15|+20|+25…`;
//! - `--value V`, `--value-min V`, `--value-max V` (the value before the action);
//! - the actor's hand now: `--aces N`, `--tens N`, `--ace-ten` (A and 10 of one suit),
//!   `--halves N`, `--pairs N`, `--pair red|bells|acorns|green`, `--standing N` (cards that
//!   stand from the own hand alone), `--long N` (longest suit), `--blank N` (empty suits);
//! - `--partner pass|bid|none` (the partner's previous bidding action), `--cross140`;
//! - `--result made|failed|allpass`, `--margin-over X` (made or failed by more than X).

mod bidding;

use std::collections::BTreeMap;

use engine::{suit_of, ActionType, Card, CardSet, GameFinishedInfo, Suit, SUITS};
use observation::Seat;
use tools::cli::Cli;
use tools::legacy::{read, replay};

const SEATS: [&str; 4] = ["N", "E", "S", "W"];

/// One action, before it was applied; automatic completions remain in transcripts.
struct Act {
    step: usize,
    seat: Seat,
    phase: &'static str,
    action: ActionType,
    /// The game value before the action.
    value: i32,
    hand: CardSet,
    /// Auction position (1 = the first bidder); 0 outside the auction.
    pos: u8,
    /// The partner's previous bidding action: `Some(None)` passed, `Some(Some(v))` bid v.
    partner: Option<Option<i32>>,
    claimed_by: Option<Seat>,
}

/// One replayed game.
struct Game {
    month: String,
    dealt: [CardSet; 4],
    acts: Vec<Act>,
    info: GameFinishedInfo,
}

fn load(path: &str) -> Vec<Option<Game>> {
    let (recs, _) = read(path);
    recs.iter()
        .map(|rec| {
            let month = tools::legacy::meta(rec).ok()?.month;
            let mut acts = vec![];
            let mut first: Option<Seat> = None;
            let mut dealt = [CardSet::EMPTY; 4];
            let r = replay(rec, |s| {
                if acts.is_empty() {
                    dealt = std::array::from_fn(|y| s.table.hand(y as Seat));
                }
                let state = &s.table.game.state;
                let bidding = matches!(s.phase, "first bid" | "later bid");
                if bidding && first.is_none() {
                    first = Some(s.seat);
                }
                let mate = (s.seat + 2) % 4;
                let partner = state
                    .bidding_history
                    .iter()
                    .rev()
                    .find(|(_, p)| p.0 == mate)
                    .map(|(t, _)| match t {
                        ActionType::NewBid(v) => Some(*v),
                        _ => None,
                    });
                acts.push(Act {
                    step: acts.len(),
                    seat: s.seat,
                    phase: s.phase,
                    action: s.human.action_type,
                    value: state.value.0,
                    hand: s.table.hand(s.seat),
                    pos: match (bidding, first) {
                        (true, Some(f)) => (s.seat + 4 - f) % 4 + 1,
                        _ => 0,
                    },
                    partner,
                    claimed_by: s.claimed_by,
                });
            })
            .ok()?;
            Some(Game {
                month,
                dealt,
                acts,
                info: GameFinishedInfo::from(r.table.game),
            })
        })
        .collect()
}

fn sorted(set: CardSet) -> String {
    let mut cards: Vec<u8> = set.iter().collect();
    cards.sort_by_key(|&c| (suit_of(c) as u8, std::cmp::Reverse(c)));
    cards
        .iter()
        .map(|&c| Card::from_index(c).to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn card(c: &Card) -> String {
    c.to_string()
}

/// The playing team, the value, its points and whether it made it, or None (all passed).
fn result(info: &GameFinishedInfo) -> Option<(u8, i32, i32, bool)> {
    let team = info.playing_party?.0 % 2;
    let pts = info.team_points[team as usize];
    Some((team, info.game_value.0, pts, pts >= info.game_value.0))
}

fn act_text(a: &Act) -> String {
    match a.action {
        ActionType::NewBid(v) => format!("{v} (+{})", v - a.value),
        ActionType::StopBidding => "pass".into(),
        ActionType::Pass(cards) => cards.iter().map(card).collect::<Vec<_>>().join(" "),
        ActionType::CardPlayed(c) => card(&c),
        other => format!("{other:?}"),
    }
}

fn transcript(games: &[Option<Game>], id: usize) {
    let Some(Some(g)) = games.get(id) else {
        println!("game {id}: not replayable (or out of range; {} records)", games.len());
        return;
    };
    println!("# Game {id} ({}), seats N E S W\n", g.month);
    println!("Dealt:");
    for (name, hand) in SEATS.iter().zip(g.dealt) {
        println!("  {name}: {}", sorted(hand));
    }
    println!("\nAuction:");
    for a in g.acts.iter().filter(|a| a.pos > 0) {
        let mate = match a.partner {
            Some(Some(v)) => format!("partner bid {v}"),
            Some(None) => "partner passed".into(),
            None => "partner not yet".into(),
        };
        println!(
            "  {} (pos {}) at {}: {}   [{mate}]",
            SEATS[a.seat as usize],
            a.pos,
            a.value,
            act_text(a)
        );
    }
    match g.info.declarer {
        Some(d) => println!("Contract: {} for {}", SEATS[d.0 as usize], g.info.game_value.0),
        None => println!("Contract: none, all passed"),
    }
    let mut trick: Vec<String> = vec![];
    let (mut done, mut cards) = (0, 0);
    for a in &g.acts {
        match a.phase {
            "pass forth" | "pass back" => println!("{} {}: {}", a.phase, SEATS[a.seat as usize], act_text(a)),
            "raise" if matches!(a.action, ActionType::NewBid(_)) => {
                println!("raise {}: {}", SEATS[a.seat as usize], act_text(a))
            }
            "first bid" | "later bid" => {}
            _ => {
                if trick.is_empty() && done == 0 {
                    println!("\nPlay:");
                }
                trick.push(format!("{} {}", SEATS[a.seat as usize], act_text(a)));
                cards += matches!(a.action, ActionType::CardPlayed(_)) as usize;
                if cards == 4 && done < g.info.tricks.len() {
                    let t = &g.info.tricks[done];
                    println!(
                        "  T{} {} → {} takes {}",
                        done + 1,
                        trick.join(", "),
                        SEATS[t.winner.0 as usize],
                        t.points.0
                    );
                    trick.clear();
                    done += 1;
                    cards = 0;
                }
            }
        }
    }
    let tricks = |team: u8| g.info.tricks.iter().filter(|t| t.winner.0 % 2 == team).count();
    println!(
        "\nResult: N/S {} points in {} tricks, E/W {} points in {} tricks; scores {:?}",
        g.info.team_points[0],
        tricks(0),
        g.info.team_points[1],
        tricks(1),
        g.info.scores()
    );
    if let Some((team, value, pts, made)) = result(&g.info) {
        let side = ["N/S", "E/W"][team as usize];
        let verdict = if made { "made" } else { "failed" };
        let schwarz = if g.info.schwarz_game { ", schwarz" } else { "" };
        println!("{side} {verdict} {value} by {}{schwarz}", pts - value);
    }
}

fn suit_named(name: &str) -> Suit {
    match name {
        "red" => Suit::Red,
        "bells" => Suit::Bells,
        "acorns" => Suit::Acorns,
        "green" => Suit::Green,
        other => panic!("--pair {other}: red, bells, acorns or green"),
    }
}

fn find(games: &[Option<Game>], cli: &Cli) {
    let n = |k: &str| {
        cli.str(k)
            .map(|v| v.parse::<i64>().unwrap_or_else(|_| panic!("{k} {v}")))
    };
    let count_ok = |k: &str, x: u32| n(k).is_none_or(|v| v == x as i64);
    let mut by_action: BTreeMap<String, u64> = BTreeMap::new();
    let (mut hits, mut games_hit) = (vec![], 0);
    for (id, g) in games.iter().enumerate() {
        let Some(g) = g else { continue };
        let res = result(&g.info);
        let result_ok = match cli.str("--result") {
            None => true,
            Some("allpass") => res.is_none(),
            Some("made") => res.is_some_and(|r| r.3),
            Some("failed") => res.is_some_and(|r| !r.3),
            Some(other) => panic!("--result {other}"),
        };
        let margin_ok = n("--margin-over").is_none_or(|x| res.is_some_and(|r| ((r.2 - r.1) as i64).abs() > x));
        if !result_ok || !margin_ok {
            continue;
        }
        let mut any = false;
        for a in g.acts.iter().filter(|a| a.claimed_by.is_none()) {
            let phase_ok = cli.str("--phase").is_none_or(|p| {
                p.replace('-', " ").starts_with(a.phase) || (p == "trump" && a.phase == "trump or lead")
            });
            let bid = match a.action {
                ActionType::NewBid(v) => Some(v),
                _ => None,
            };
            let action_ok = match cli.str("--action") {
                None => true,
                Some("pass") => a.action == ActionType::StopBidding,
                Some("bid") => bid.is_some(),
                Some(step) => bid.is_some_and(|v| format!("+{}", v - a.value) == step),
            };
            let h = a.hand;
            let lens = SUITS.map(|s| (h & CardSet::suit(s)).len());
            let pair_ok = cli
                .str("--pair")
                .is_none_or(|s| CardSet::halves(suit_named(s)).is_subset(h));
            let partner_ok = match cli.str("--partner") {
                None => true,
                Some("pass") => a.partner == Some(None),
                Some("bid") => matches!(a.partner, Some(Some(_))),
                Some("none") => a.partner.is_none(),
                Some(other) => panic!("--partner {other}"),
            };
            let ok = phase_ok
                && action_ok
                && pair_ok
                && partner_ok
                && n("--pos").is_none_or(|p| p == a.pos as i64)
                && n("--value").is_none_or(|v| v == a.value as i64)
                && n("--value-min").is_none_or(|v| a.value as i64 >= v)
                && n("--value-max").is_none_or(|v| a.value as i64 <= v)
                && count_ok("--aces", (h & CardSet::ACES).len())
                && count_ok("--tens", (h & CardSet::TENS).len())
                && count_ok("--halves", (h & CardSet::HALVES).len())
                && count_ok("--pairs", h.pair_suits().count() as u32)
                && count_ok("--standing", knowledge::own_standing_of(h).len())
                && count_ok("--long", lens.iter().copied().max().unwrap_or(0))
                && count_ok("--blank", lens.iter().filter(|&&l| l == 0).count() as u32)
                && (!cli.flag("--ace-ten")
                    || SUITS
                        .iter()
                        .any(|&s| (h & CardSet::suit(s) & (CardSet::ACES | CardSet::TENS)).len() == 2))
                && (!cli.flag("--cross140") || (a.value < 140 && bid.is_some_and(|v| v > 140)));
            if ok {
                any = true;
                let key = match a.action {
                    ActionType::NewBid(v) => format!("+{}", v - a.value),
                    ActionType::StopBidding => "pass".into(),
                    ActionType::CardPlayed(_) | ActionType::Pass(_) => a.phase.to_string(),
                    other => format!("{other:?}"),
                };
                *by_action.entry(key).or_default() += 1;
                hits.push((id, a, res));
            }
        }
        games_hit += any as u64;
    }
    println!(
        "{} situations in {} games; by action {:?}",
        hits.len(),
        games_hit,
        by_action
    );
    let list = cli.get("--list", 20usize);
    for (id, a, res) in hits.iter().take(list) {
        let outcome = match res {
            None => "all passed".to_string(),
            Some((t, v, p, m)) => format!(
                "{} {} {v} by {}",
                ["N/S", "E/W"][*t as usize],
                if *m { "made" } else { "failed" },
                p - v
            ),
        };
        println!(
            "- game {id} step {} {} {} at {}: {} | hand {} | {outcome}",
            a.step,
            SEATS[a.seat as usize],
            a.phase,
            a.value,
            act_text(a),
            sorted(a.hand)
        );
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = if args.is_empty() { String::new() } else { args.remove(0) };
    let game = if cmd == "transcript" && !args.is_empty() && !args[0].starts_with("--") {
        Some(args.remove(0))
    } else {
        None
    };
    let cli = Cli::from_args(
        args,
        &[
            "--games",
            "--list",
            "--phase",
            "--pos",
            "--action",
            "--value",
            "--value-min",
            "--value-max",
            "--aces",
            "--tens",
            "--halves",
            "--pairs",
            "--pair",
            "--standing",
            "--long",
            "--blank",
            "--partner",
            "--result",
            "--margin-over",
            "--selfplay",
        ],
        &["--ace-ten", "--cross140"],
    );
    let path = cli
        .str("--games")
        .map(String::from)
        .or_else(|| std::env::var("MARJA_GAMES").ok())
        .expect("set MARJA_GAMES (or --games PATH) to the human games");
    if cmd == "bidding" {
        let (recs, _) = read(&path);
        print!("{}", bidding::run(&recs, cli.get("--selfplay", 0u64)));
        return;
    }
    let games = load(&path);
    match (cmd.as_str(), game) {
        ("transcript", Some(id)) => transcript(&games, id.parse().expect("game number")),
        ("find", None) => find(&games, &cli),
        _ => eprintln!("usage: study transcript <game> | study find [filters]  (see the source header)"),
    }
}

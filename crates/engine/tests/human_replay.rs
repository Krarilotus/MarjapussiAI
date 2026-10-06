//! Guard: the 1,579 human games from marjapussi.de replay to the recorded outcomes.
//!
//! The records hold real player names; they are read only to look up the hands, and only
//! aggregates and a digest are printed. The dataset path comes from `MARJA_GAMES`, else the
//! default location in the MarjaAI workspace; without it the test is skipped (with a note).
//! Replay rules follow the heuristic branch's `tools::legacy`: the legacy log mixes question
//! kinds (fixed up from the next answer), and impossible answers and stop-bids are skipped.
//!
//! Two records stop on an illegal card (queue item E2, classified 2026-10-06): both are from
//! 2022-09, the site's first month. One plays a trump while holding the led suit (follow-suit
//! rule), the other follows trump with a lower card while holding one that beats the trick
//! (must-overtake rule). marjapussi.de's own implementations (`marjapussi-py`: legacy
//! `utils.allowed_general` and `rules.allowed_cards`) reject both cards as well, so these are
//! data from an early site version, not an engine rule bug. The positions are printed below.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::{finished_line, site_scores, Digest};
use marjapussi_engine::game::cards::{Card, Suit};
use marjapussi_engine::game::gameevent::{ActionType, AnswerType, GameAction, QuestionType};
use marjapussi_engine::game::gameinfo::GameFinishedInfo;
use marjapussi_engine::game::player::PlaceAtTable;
use marjapussi_engine::game::Game;
use serde_json::Value;

/// Sum of `GameFinishedInfo::scores()` over the replayed games, per team (E1; recomputed for
/// the site's scoring, E4).
const SCORE_TOTALS: [i64; 2] = [34_630, 28_255];

/// Recorded on `main` at 68c98f0 as 0x9a38_7d77_f3bc_8e2d; recomputed once on feat/engine-v2
/// for owner decision D2: announced pairs now count in all-pass games, which changes
/// `team_points` of one replayed game (+40). With that line reverted the old digest returns.
const EXPECTED: u64 = 0xcb8c_f66a_3bd2_9d91;

fn dataset() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("MARJA_GAMES") {
        return Some(PathBuf::from(p));
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .map(|a| a.join("sources/marjapussi-rs/ml/dataset/games.ndjson"))
        .find(|p| p.exists())
}

fn suit(c: char) -> Option<Suit> {
    Some(match c {
        'g' => Suit::Green,
        'e' => Suit::Acorns,
        's' => Suit::Bells,
        'r' => Suit::Red,
        _ => return None,
    })
}

fn split(raw: &str) -> Option<(u8, &str, &str)> {
    let mut it = raw.splitn(3, ',');
    Some((it.next()?.parse().ok()?, it.next()?, it.next()?))
}

fn question(val: &str) -> Option<ActionType> {
    if let Some(s) = val.strip_prefix("my") {
        return Some(ActionType::AnnounceTrump(suit(s.chars().next()?)?));
    }
    if let Some(s) = val.strip_prefix("ou") {
        return Some(ActionType::Question(QuestionType::YourHalf(suit(s.chars().next()?)?)));
    }
    Some(ActionType::Question(QuestionType::Yours))
}

fn answer(val: &str) -> Option<ActionType> {
    let s = |rest: &str| suit(rest.chars().next()?);
    Some(ActionType::Answer(if let Some(r) = val.strip_prefix("my") {
        AnswerType::YesPair(s(r)?)
    } else if let Some(r) = val.strip_prefix("no") {
        match s(r) {
            Some(x) => AnswerType::NoHalf(x),
            None => AnswerType::NoPair,
        }
    } else if let Some(r) = val.strip_prefix("ou") {
        AnswerType::YesHalf(s(r)?)
    } else {
        AnswerType::NoPair
    }))
}

/// Equal actions; passes compare as card sets.
fn same(a: &GameAction, b: &GameAction) -> bool {
    match (&a.action_type, &b.action_type) {
        (ActionType::Pass(x), ActionType::Pass(y)) => {
            let mut x = x.clone();
            let mut y = y.clone();
            x.sort();
            y.sort();
            x == y
        }
        (x, y) => x == y,
    }
}

fn hands(rec: &Value) -> Result<[Vec<Card>; 4], String> {
    let names: Vec<&str> = rec["players"]
        .as_array()
        .ok_or("no players")?
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    if names.len() != 4 {
        return Err("players".into());
    }
    let mut hands: [Vec<Card>; 4] = Default::default();
    for (i, name) in names.iter().enumerate() {
        let cards = rec["cards"][*name].as_array().ok_or("cards")?;
        hands[i] = cards
            .iter()
            .map(|c| c.as_str().unwrap_or("").parse::<Card>().map_err(|_| "card".to_string()))
            .collect::<Result<_, _>>()?;
    }
    Ok(hands)
}

/// Replays one record to its end, or returns why it stopped.
fn replay(rec: &Value) -> Result<Game, String> {
    let actions: Vec<&str> = rec["actions"]
        .as_array()
        .ok_or("actions")?
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    let mut game = Game::new(
        "replay".into(),
        ["S0", "S1", "S2", "S3"].map(String::from),
        Some(hands(rec)?),
    );
    for _ in 0..4 {
        let start = game.legal_actions[0].clone();
        game.apply_action_mut(start);
    }
    let mut pass_group: Vec<Card> = vec![];
    let mut skip_answer = false;
    for (i, raw) in actions.iter().enumerate() {
        let (seat, code, val) = split(raw).ok_or("action format")?;
        if skip_answer && code == "ANSW" {
            skip_answer = false;
            continue;
        }
        let action_type = match code {
            "PASS" | "PBCK" => {
                pass_group.push(val.parse::<Card>().map_err(|_| "card")?);
                if pass_group.len() < 4 {
                    continue;
                }
                let mut cards = std::mem::take(&mut pass_group);
                cards.sort();
                cards.reverse();
                ActionType::Pass(cards)
            }
            "PROV" => match val.parse::<i32>().map_err(|_| "bid")? {
                0 => ActionType::StopBidding,
                v => ActionType::NewBid(v),
            },
            "PRMO" => match val.parse::<i32>().map_err(|_| "raise")? {
                0 => continue,
                v => ActionType::NewBid(v),
            },
            "TRCK" => ActionType::CardPlayed(val.parse::<Card>().map_err(|_| "card")?),
            "QUES" => question(val).ok_or("question")?,
            "ANSW" => answer(val).ok_or("answer")?,
            _ => return Err(format!("code {code}")),
        };
        if game.ended() {
            return Err("game over early".into());
        }
        let turn = game.state.player_at_turn.0;
        let legal: Vec<GameAction> = game
            .legal_actions
            .iter()
            .filter(|a| a.player.0 == turn && a.action_type != ActionType::UndoRequest)
            .cloned()
            .collect();
        let mut action = GameAction {
            action_type,
            player: PlaceAtTable(seat),
        };
        let legal_has = |a: &GameAction| legal.iter().any(|l| same(l, a));
        if !legal_has(&action) && code == "QUES" {
            let next = actions.get(i + 1).and_then(|r| split(r));
            let mut candidates = vec![];
            if let Some((_, "ANSW", nv)) = next {
                if let Some(ActionType::Answer(AnswerType::NoHalf(s) | AnswerType::YesHalf(s))) = answer(nv) {
                    candidates.push(ActionType::Question(QuestionType::YourHalf(s)));
                }
                if nv == "nmy" || nv.starts_with("my") {
                    candidates.push(ActionType::Question(QuestionType::Yours));
                }
            }
            match candidates
                .into_iter()
                .map(|t| GameAction {
                    action_type: t,
                    player: PlaceAtTable(seat),
                })
                .find(legal_has)
            {
                Some(a) => action = a,
                None => {
                    skip_answer = matches!(next, Some((_, "ANSW", _)));
                    continue;
                }
            }
        }
        if !legal_has(&action) {
            if matches!(code, "ANSW" | "PROV") && !matches!(action.action_type, ActionType::NewBid(_)) {
                continue;
            }
            if let ActionType::CardPlayed(card) = &action.action_type {
                // E2: the position, without names, for classifying the recorded card.
                let s = &game.state;
                let trick = if s.current_trick.len() == 4 {
                    &[][..]
                } else {
                    &s.current_trick[..]
                };
                eprintln!(
                    "illegal card: game of {} trick {} seat {seat} trump {:?} called {:?} trick so far {trick:?} hand {:?} played {card} engine allows {:?}",
                    rec["created"].as_str().unwrap_or("").get(..7).unwrap_or("?"),
                    s.all_tricks.len() + 1,
                    s.trump,
                    s.trump_called,
                    s.players[seat as usize].cards,
                    legal.iter().map(|a| format!("{:?}", a.action_type)).collect::<Vec<_>>(),
                );
            }
            return Err(format!("illegal {code} in {:?}", game.state.phase));
        }
        if turn != seat {
            return Err("turn".into());
        }
        let human = legal.iter().find(|l| same(l, &action)).cloned().unwrap();
        game.apply_action_mut(human);
    }
    if game.ended() {
        Ok(game)
    } else {
        Err("not ended".into())
    }
}

/// The site's per-game scoring (`SeriesTable.vue`), on the record's own fields (the record's
/// points and schwarz flag, which old site versions got wrong), by team.
fn record_site_scores(rec: &Value) -> Option<[i32; 2]> {
    let names: Vec<&str> = rec["players"].as_array()?.iter().filter_map(|v| v.as_str()).collect();
    let pts = |i: usize| rec["players_points"][names[i]].as_i64().map(|p| p as i32);
    let points = [pts(0)? + pts(2)?, pts(1)? + pts(3)?];
    let mut value = rec["game_value"].as_i64()? as i32;
    if value == 115 {
        value = 0;
    }
    let playing = rec["playing_player"]
        .as_str()
        .and_then(|p| names.iter().position(|n| *n == p));
    let schwarz = rec["schwarz_game"].as_bool().unwrap_or(false);
    let round = |p: i32| (p + 2).div_euclid(5) * 5;
    let mut gets = points.map(round);
    for t in 0..2 {
        if playing.map(|p| p % 2) == Some(t) {
            gets[t] = if points[t] >= value { value } else { -value };
        }
    }
    let before = gets;
    for t in 0..2 {
        if points[t] == 0 && schwarz && value >= 120 && before[1 - t] > 0 {
            gets[t] = -10 * value / 5;
        }
    }
    Some(gets)
}

#[test]
fn human_games_replay_like_recorded() {
    let Some(path) = dataset() else {
        eprintln!("SKIPPED: games.ndjson not found (set MARJA_GAMES)");
        return;
    };
    let text = std::fs::read_to_string(&path).expect("read games.ndjson");
    let mut digest = Digest::new();
    let mut stopped: BTreeMap<String, u32> = BTreeMap::new();
    let (mut records, mut ended, mut contract_made, mut schwarz) = (0, 0, 0, 0);
    let mut team_points = [0i64; 2];
    let mut score_totals = [0i64; 2];
    let (mut site_points_equal, mut site_scores_equal, mut s_schwarz) = (0, 0, 0);
    let mut differences: BTreeMap<&str, u32> = BTreeMap::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        records += 1;
        let rec: Value = serde_json::from_str(line).expect("json record");
        match replay(&rec) {
            Ok(game) => {
                ended += 1;
                let info = GameFinishedInfo::from(game);
                contract_made += (info.won == Some(true)) as u32;
                schwarz += info.schwarz_game as u32;
                team_points[0] += info.team_points[0] as i64;
                team_points[1] += info.team_points[1] as i64;
                digest.add(&finished_line(&info));
                let scores = info.scores();
                assert_eq!(scores, site_scores(&info), "scores differ from the site's table");
                score_totals[0] += scores[0] as i64;
                score_totals[1] += scores[1] as i64;
                let outcome = info.outcome();
                if let Some(s) = outcome.playing_team {
                    s_schwarz += (outcome.team_tricks[s as usize] == 0) as u32;
                }
                let names: Vec<&str> = rec["players"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|v| v.as_str())
                    .collect();
                let recorded = [0, 1].map(|t| {
                    let p = |i: usize| rec["players_points"][names[i]].as_i64().unwrap_or(-1) as i32;
                    p(t) + p(t + 2)
                });
                site_points_equal += (recorded == info.team_points) as u32;
                site_scores_equal += (record_site_scores(&rec) == Some(scores)) as u32;
                // Every difference to the site's table on the record's own fields must have a
                // known cause (old site versions recorded other points or no schwarz flag).
                if record_site_scores(&rec) != Some(scores) {
                    let flagged = rec["schwarz_game"].as_bool().unwrap_or(false);
                    let cause = if recorded != info.team_points {
                        "team points differ from the record (old site version)"
                    } else if !flagged && outcome.team_tricks.contains(&0) {
                        "N schwarz, record without the schwarz flag (old site version)"
                    } else {
                        "unexplained"
                    };
                    *differences.entry(cause).or_default() += 1;
                }
            }
            Err(why) => {
                digest.add(&why);
                *stopped.entry(why).or_default() += 1;
            }
        }
    }
    eprintln!(
        "{records} records: {ended} replayed to the end ({contract_made} contracts made, {schwarz} schwarz, team points {team_points:?}); stopped {stopped:?}; digest {:#018x}",
        digest.0
    );
    eprintln!(
        "scores: totals {score_totals:?}; team points equal to the site's record in {site_points_equal}/{ended}; \
         scores equal to the site's table in {site_scores_equal}/{ended}; playing party schwarz in {s_schwarz}; \
         differences by cause {differences:?}"
    );
    assert!(
        !differences.contains_key("unexplained"),
        "a score differs from the site without a known cause"
    );
    assert_eq!(records, 1_579);
    assert_eq!(
        digest.0, EXPECTED,
        "human replay outcomes changed (digest {:#018x})",
        digest.0
    );
    assert_eq!(score_totals, SCORE_TOTALS, "human replay scores changed");
}

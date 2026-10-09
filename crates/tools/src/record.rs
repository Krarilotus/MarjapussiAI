//! Game records: one JSON object per line (NDJSON), the same schema for agent and human games.
//! The schema is documented in `docs/design/game-records.md` (owner: this module).

use agent::Decision;
use beliefs::Mind;
use engine::CardSet;
use engine::GameFinishedInfo;
use engine::{ActionType, AnswerType, GameAction, GameCallback, QuestionType};
use engine::{Card, Suit};
use heuristic::Params;
use serde_json::{json, Map, Value};

pub const SCHEMA: &str = "marjapussi.game-record/1";

/// Who played, and how the game came about.
pub struct Meta {
    /// "hp1" or "human".
    pub source: &'static str,
    pub id: String,
    pub seed: Option<u64>,
    pub params_hash: Option<String>,
    /// "YYYY-MM-DD" for agent games, "YYYY-MM" for human games.
    pub date: String,
    /// Pseudonyms ("P17") or agent names ("HP1"), never real names.
    pub players: [String; 4],
}

/// What a record says about one decision besides the action itself.
#[derive(Default)]
pub struct Note {
    /// The decider's own rule and facts (agent games).
    pub decision: Option<Decision>,
    /// What HP1 would have done from the same seat view (human games).
    pub hp1: Option<Decision>,
    /// The decider's beliefs before acting (optional, large).
    pub beliefs: Option<Value>,
}

/// The suit letter of the engine's card notation (`Suit::code`; Judge S2).
fn suit(s: Suit) -> String {
    s.code().to_string()
}

fn r2(x: f32) -> f64 {
    (x as f64 * 100.0).round() / 100.0
}

/// The action as a small JSON object (`raise` vs `bid` is decided by the caller).
pub fn action_json(a: &GameAction, after_exchange: bool) -> Value {
    let (kind, extra): (&str, Value) = match &a.action_type {
        ActionType::NewBid(v) if after_exchange => ("raise", json!({ "value": v })),
        ActionType::NewBid(v) => ("bid", json!({ "value": v })),
        ActionType::StopBidding => ("pass_bid", json!({})),
        ActionType::Pass(cards) => (
            "pass_cards",
            json!({ "cards": cards.iter().map(|c| c.to_string()).collect::<Vec<_>>() }),
        ),
        ActionType::CardPlayed(c) => ("card", json!({ "card": c.to_string() })),
        ActionType::AnnounceTrump(s) => ("announce", json!({ "suit": suit(*s) })),
        ActionType::Question(QuestionType::Yours) => ("ask_pair", json!({})),
        ActionType::Question(QuestionType::YourHalf(s)) => ("ask_half", json!({ "suit": suit(*s) })),
        ActionType::Answer(AnswerType::YesPair(s)) => ("answer_pair", json!({ "suit": suit(*s) })),
        ActionType::Answer(AnswerType::NoPair) => ("answer_no_pair", json!({})),
        ActionType::Answer(AnswerType::YesHalf(s)) => ("answer_half", json!({ "suit": suit(*s) })),
        ActionType::Answer(AnswerType::NoHalf(s)) => ("answer_no_half", json!({ "suit": suit(*s) })),
        ActionType::Start => ("start", json!({})),
        ActionType::UndoRequest => ("undo_request", json!({})),
        ActionType::UndoAccept => ("undo_accept", json!({})),
        ActionType::UndoDecline => ("undo_decline", json!({})),
    };
    let mut m = Map::new();
    m.insert("seat".into(), json!(a.player.0));
    m.insert("type".into(), json!(kind));
    if let Value::Object(e) = extra {
        m.extend(e);
    }
    Value::Object(m)
}

fn decision_json(d: &Decision, after_exchange: bool) -> Value {
    let why: Map<String, Value> = d
        .explanation
        .facts
        .iter()
        .map(|f| (f.key.to_string(), json!(r2(f.value))))
        .collect();
    json!({ "action": action_json(&d.action, after_exchange), "rule": d.explanation.label, "why": why })
}

/// A compact belief snapshot: per other seat the believed features of its hand now, its voids,
/// and P(holds) for the uncertain aces, tens, kings and obers.
pub fn beliefs_json(mind: &Mind) -> Value {
    let honours = CardSet(beliefs::conventions::LUSCHE.0 ^ CardSet::ALL.0);
    let me = mind.seat();
    let mut seats = Map::new();
    for y in (0..4u8).filter(|&y| y != me) {
        let s = mind.features(y);
        let cards: Map<String, Value> = honours
            .iter()
            .filter_map(|c| {
                let p = mind.p_holds(y, c);
                (p > 0.0 && p < 1.0).then(|| (Card::from_index(c).to_string(), json!(r2(p))))
            })
            .collect();
        seats.insert(
            y.to_string(),
            json!({
                "ace": r2(s.ace),
                "pair": s.pair.map(r2),
                "half": s.half.map(r2),
                "void": engine::SUITS.map(|t| r2(mind.p_void(y, t))),
                "cards": cards,
            }),
        );
    }
    json!({ "seat": me, "about": seats })
}

/// FNV-1a of the flat TOML of `params`: identifies a parameter set in records.
pub fn params_hash(params: &Params) -> String {
    let h = params.to_toml().bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100_0000_01b3)
    });
    format!("{h:016x}")
}

/// The full record of a finished game. `notes` holds one entry per action after the 4 starts,
/// in order.
pub fn game_record(meta: &Meta, info: &GameFinishedInfo, notes: &[Note]) -> Value {
    let deal: Vec<Vec<String>> = info
        .info
        .player_start_cards
        .iter()
        .map(|h| h.iter().map(|c| c.to_string()).collect())
        .collect();
    let mut actions = vec![];
    let mut passes = 0;
    let events = info
        .all_events
        .iter()
        .filter(|e| e.last_action.action_type != ActionType::Start);
    for (i, e) in events.enumerate() {
        let after_exchange = passes == 2;
        let mut a = action_json(&e.last_action, after_exchange);
        if let ActionType::Pass(_) = e.last_action.action_type {
            a["dir"] = json!(if passes == 0 { "forth" } else { "back" });
            passes += 1;
        }
        match &e.callback {
            Some(GameCallback::NewTrump(s)) => a["trump"] = json!(suit(*s)),
            Some(GameCallback::OnlyHalf(s)) => a["only_half"] = json!(suit(*s)),
            Some(GameCallback::NoHalf(_) | GameCallback::StillTrump(_)) | None => {}
        }
        if let Some(note) = notes.get(i) {
            if let Some(d) = &note.decision {
                a["rule"] = json!(d.explanation.label);
                a["why"] = decision_json(d, after_exchange)["why"].clone();
            }
            if let Some(d) = &note.hp1 {
                a["hp1"] = decision_json(d, after_exchange);
            }
            if let Some(b) = &note.beliefs {
                a["beliefs"] = b.clone();
            }
        }
        actions.push(a);
    }
    let declarer = info.playing_party.as_ref().and_then(|_| {
        info.bidding_history
            .iter()
            .rev()
            .find(|(t, _)| matches!(t, ActionType::NewBid(_)))
            .map(|(_, p)| p.0)
    });
    let tricks: Vec<Value> = info
        .tricks
        .iter()
        .map(|t| {
            json!({
                "cards": t.cards.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
                "winner": t.winner.0,
                "points": t.points.0,
            })
        })
        .collect();
    json!({
        "schema": SCHEMA,
        "source": meta.source,
        "id": meta.id,
        "seed": meta.seed,
        "params_hash": meta.params_hash,
        "date": meta.date,
        "players": meta.players,
        "deal": deal,
        "actions": actions,
        "result": {
            "declarer": declarer,
            "declarers": info.playing_party.as_ref().map(|p| p.0 % 2),
            "value": info.game_value.0,
            "made": info.won,
            "schwarz": info.schwarz_game,
            "team_points": info.team_points,
            "scores": info.scores(),
            "tricks": tricks,
        },
    })
}

/// Today's date as "YYYY-MM-DD" (local time), for agent records.
pub fn today() -> String {
    engine::current_time_string().chars().take(10).collect()
}

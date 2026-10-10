//! Synthetic archives, with grouped passes and a skipped no-raise marker.
use engine::{ActionType, Card, GameAction, GameFinishedInfo, GamePhase, Move};
use eval::Table;
use serde_json::{json, Value};
use tools::{
    legacy,
    record::{action_json, game_record, Meta, Note},
};

fn push(table: &mut Table, raw: &mut Vec<String>, action: GameAction) {
    let seat = action.player.0;
    match action.action_type {
        ActionType::NewBid(value) => raw.push(format!("{seat},PROV,{value}")),
        ActionType::StopBidding => raw.push(format!("{seat},PROV,0")),
        ActionType::Pass(cards) => {
            let phase = if table.game.state.phase == GamePhase::PassingForth {
                "PASS"
            } else {
                "PBCK"
            };
            raw.extend(cards.iter().map(|card| format!("{seat},{phase},{card}")));
        }
        ActionType::CardPlayed(card) => raw.push(format!("{seat},TRCK,{card}")),
        ActionType::AnnounceTrump(suit) => raw.push(format!("{seat},QUES,my{}", suit.code())),
        _ => panic!("unexpected fixture action"),
    }
    table.apply(action);
}

fn fixture() -> Value {
    fixture_with_lost_trick(false)
}

fn fixture_with_lost_trick(lose: bool) -> Value {
    let mut hands: [Vec<Card>; 4] = [
        "g-6 g-7 g-8 g-9 g-U g-O g-K g-Z g-A",
        "e-6 e-9 e-K s-6 s-9 s-K r-6 r-9 r-K",
        "e-7 e-U e-Z s-7 s-U s-Z r-7 r-U r-Z",
        "e-8 e-O e-A s-8 s-O s-A r-8 r-O r-A",
    ]
    .map(|codes| {
        codes
            .split_whitespace()
            .map(|code| code.parse::<Card>().unwrap())
            .collect()
    });
    if lose {
        for (own, other) in [("g-6", "s-8"), ("g-7", "s-O"), ("g-Z", "s-A")] {
            let own = own.parse::<Card>().unwrap();
            let other = other.parse::<Card>().unwrap();
            let own_index = hands[0].iter().position(|&card| card == own).unwrap();
            let other_index = hands[3].iter().position(|&card| card == other).unwrap();
            hands[0][own_index] = other;
            hands[3][other_index] = own;
        }
    }
    let mut table = Table::new(&hands);
    let mut raw = vec![];
    push(
        &mut table,
        &mut raw,
        GameAction {
            action_type: ActionType::NewBid(120),
            player: engine::PlaceAtTable(0),
        },
    );
    for seat in 1..4 {
        push(
            &mut table,
            &mut raw,
            GameAction {
                action_type: ActionType::StopBidding,
                player: engine::PlaceAtTable(seat),
            },
        );
    }
    let pass = *table
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| matches!(a.action_type, ActionType::Pass(_)))
        .unwrap();
    push(&mut table, &mut raw, pass);
    push(
        &mut table,
        &mut raw,
        GameAction {
            player: engine::PlaceAtTable(0),
            ..pass
        },
    );
    raw.push("0,PRMO,0".into());
    push(
        &mut table,
        &mut raw,
        Move::Card("g-A".parse::<Card>().unwrap().index()).to_action(0),
    );
    for _ in 0..3 {
        let (seat, legal) = table.to_act().unwrap();
        let action = if lose && seat == 3 {
            Move::Card("g-6".parse::<Card>().unwrap().index()).to_action(3)
        } else {
            legal[0]
        };
        push(&mut table, &mut raw, action);
    }
    let from = raw.len();
    push(&mut table, &mut raw, Move::Announce(engine::Suit::Green).to_action(0));
    while let Some((seat, legal)) = table.to_act() {
        let action = if lose && raw.len() == from + 1 {
            Move::Card("s-A".parse::<Card>().unwrap().index()).to_action(0)
        } else if !lose && seat == 0 {
            Move::Card(table.hand(0).iter().last().unwrap()).to_action(0)
        } else {
            *legal
                .iter()
                .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
                .unwrap()
        };
        push(&mut table, &mut raw, action);
    }
    let names = ["P0", "P1", "P2", "P3"];
    let cards: serde_json::Map<String, Value> = names
        .into_iter()
        .zip(hands)
        .map(|(name, cards)| {
            (
                name.into(),
                json!(cards.iter().map(Card::to_string).collect::<Vec<_>>()),
            )
        })
        .collect();
    json!({ "players": names, "cards": cards, "created": "2026-10-10", "actions": raw, "claim": { "by": 0, "from": from } })
}

#[test]
fn legacy_claim_conversion_preserves_actions_provenance_and_note_alignment() {
    let archive = fixture();
    let raw_from = archive["claim"]["from"].as_u64().unwrap() as usize;
    let mut notes = vec![];
    let replayed = legacy::replay(&archive, |step| {
        notes.push(Note {
            claimed_by: step.claimed_by,
            beliefs: step.claimed_by.is_none().then(|| json!({ "index": notes.len() })),
            ..Note::default()
        });
    })
    .unwrap();
    assert_eq!(replayed.skipped["PRMO,0 (no raise, a log marker)"], 1);
    let info = GameFinishedInfo::from(replayed.table.game);
    let meta = Meta {
        source: "human",
        id: "synthetic-claim".into(),
        seed: None,
        params_hash: None,
        date: "2026-10".into(),
        players: ["P0", "P1", "P2", "P3"].map(String::from),
    };
    let converted = game_record(&meta, &info, &notes);
    let actions = converted["actions"].as_array().unwrap();
    assert_eq!(actions.len(), notes.len());
    let from = raw_from - 7; // Eight raw pass lines become two actions; marker is skipped.
    for (index, action) in actions.iter().enumerate() {
        if index < from {
            assert!(action.get("claimed_by").is_none());
            assert_eq!(action["beliefs"]["index"], index);
        } else {
            assert_eq!(action["claimed_by"], 0);
            assert!(action.get("beliefs").is_none());
        }
    }
    assert_eq!(actions[from]["type"], "announce");
    // Replay /1 actions through the same engine, ignoring optional annotations.
    let hands = legacy::meta(&archive).unwrap().hands;
    let mut table = Table::new(&hands);
    let mut passes = 0;
    for recorded in actions {
        let action = *table
            .to_act()
            .unwrap()
            .1
            .iter()
            .find(|a| {
                action_json(a, passes == 2)
                    .as_object()
                    .unwrap()
                    .iter()
                    .all(|(key, value)| recorded[key] == *value)
            })
            .unwrap();
        passes += usize::from(matches!(action.action_type, ActionType::Pass(_)));
        table.apply(action);
    }
    assert!(table.game.ended());
    assert_eq!(GameFinishedInfo::from(table.game).team_points, info.team_points);
    let mut original = archive;
    original.as_object_mut().unwrap().remove("claim");
    let mut seen = 0;
    legacy::replay(&original, |step| {
        assert!(step.claimed_by.is_none());
        seen += 1;
    })
    .unwrap();
    assert_eq!(seen, actions.len());
}

#[test]
fn malformed_claims_are_rejected() {
    let archive = fixture();
    let from = archive["claim"]["from"].as_u64().unwrap();
    let len = archive["actions"].as_array().unwrap().len();
    for claim in [
        Value::Null,
        json!({"by": 4, "from": from}),
        json!({"by": -1, "from": from}),
        json!({"by": 0, "from": -1}),
        json!({"by": 0, "from": 1.5}),
        json!({"by": 0, "from": len}),
        json!({"by": 0, "from": 5}),
        json!({"by": 1, "from": from}),
        json!({"by": 0, "from": from + 2}),
    ] {
        let mut invalid = archive.clone();
        invalid["claim"] = claim;
        assert_no_callbacks(&invalid);
    }
    for raw in ["0,PRMO,0", "0,QUES,ourg", "1,QUES,myg", "0,TRCK,g-A"] {
        let mut invalid = archive.clone();
        invalid["actions"][from as usize] = json!(raw);
        assert_no_callbacks(&invalid);
    }
}

#[test]
fn claim_can_open_play_without_raising() {
    let mut archive = fixture();
    let opening = archive["actions"]
        .as_array()
        .unwrap()
        .iter()
        .position(|a| a == "0,TRCK,g-A")
        .unwrap();
    archive["claim"]["from"] = json!(opening);
    let replayed = legacy::replay(&archive, |_| {}).unwrap();
    assert_eq!(replayed.skipped["PRMO,0 (no raise, a log marker)"], 1);
}

#[test]
fn claim_can_start_with_a_card_after_a_manual_announcement() {
    let mut archive = fixture();
    let from = archive["claim"]["from"].as_u64().unwrap() as usize;
    archive["claim"]["from"] = json!(from + 1);
    let mut provenance = vec![];
    legacy::replay(&archive, |step| provenance.push(step.claimed_by)).unwrap();
    assert_eq!(provenance[from - 7], None, "manual announcement");
    assert!(provenance[..=from - 7].iter().all(Option::is_none));
    assert!(provenance[from - 6..].iter().all(|by| *by == Some(0)));
}

fn assert_no_callbacks(archive: &Value) -> String {
    let mut callbacks = 0;
    let error = legacy::replay(archive, |_| callbacks += 1)
        .err()
        .expect("malformed claim");
    assert_eq!(callbacks, 0, "malformed claims must not pollute statistics: {error}");
    error
}

#[test]
fn semantic_claim_errors_do_not_call_analysis() {
    let archive = fixture();
    let from = archive["claim"]["from"].as_u64().unwrap() as usize;
    let mut truncated = archive.clone();
    truncated["actions"].as_array_mut().unwrap().pop();
    assert_eq!(assert_no_callbacks(&truncated), "not ended");
    let mut late_illegal = archive.clone();
    *late_illegal["actions"].as_array_mut().unwrap().last_mut().unwrap() = json!("3,TRCK,g-A");
    assert_no_callbacks(&late_illegal);
    let mut pending = archive;
    pending["actions"][from] = json!("0,QUES,yours");
    pending["claim"]["from"] = json!(from + 1);
    assert_no_callbacks(&pending);
    let error = assert_no_callbacks(&fixture_with_lost_trick(true));
    assert!(error.contains("remaining trick not won by claimant"), "{error}");
}

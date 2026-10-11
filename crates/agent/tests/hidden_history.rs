use agent::{HistoryReplayError, HypothesizedPass, Table};
use engine::{ActionType, Card, CardSet};
use observation::SeatEvent;

fn act(table: &mut Table, kind: ActionType) {
    let action = *table
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| match (a.action_type, kind) {
            (ActionType::Pass(a), ActionType::Pass(b)) => CardSet::from_cards(&a) == CardSet::from_cards(&b),
            (a, b) => a == b,
        })
        .expect("legal fixture action");
    table.apply(action);
}

fn fixture(both: bool) -> Table {
    let hands = std::array::from_fn(|seat| (0..9).map(|i| Card::from_index((4 * i + seat) as u8)).collect());
    let mut table = Table::new(&hands);
    act(&mut table, ActionType::NewBid(120));
    for _ in 0..3 {
        act(&mut table, ActionType::StopBidding);
    }
    let cards: [Card; 4] = table.hand(2).cards()[..4].try_into().unwrap();
    act(&mut table, ActionType::Pass(cards));
    if both {
        act(&mut table, ActionType::Pass(cards));
    }
    table
}

fn assignments(table: &Table) -> Vec<HypothesizedPass> {
    table
        .view(2)
        .events
        .iter()
        .enumerate()
        .filter_map(|(event_index, event)| match event {
            SeatEvent::Pass { cards: Some(cards), .. } => Some(HypothesizedPass {
                event_index,
                cards: *cards,
            }),
            _ => None,
        })
        .collect()
}

#[test]
fn checked_hidden_forth_return_and_play_replay_preserves_every_filtered_view() {
    for both in [false, true] {
        let mut table = fixture(both);
        // Restore played cards BEFORE reversing transfers, including returned cards.
        let mut replayed_returned_card = false;
        for _ in 0..if both { 37 } else { 1 } {
            let hands = std::array::from_fn(|seat| table.hand(seat as u8));
            let hypothetical = assignments(&table);
            assert!(Table::from_view(&table.view(1), hands).is_none());
            let rebuilt = Table::from_view_with_passes(&table.view(1), hands, &hypothetical).unwrap();
            assert_eq!(rebuilt.to_act(), table.to_act());
            for seat in 0..4 {
                assert_eq!(rebuilt.hand(seat), table.hand(seat));
                assert_eq!(rebuilt.view(seat).events, table.view(seat).events);
                assert_eq!(rebuilt.view(seat).knowledge, table.view(seat).knowledge);
            }
            assert!(rebuilt
                .view(1)
                .events
                .iter()
                .filter_map(|e| match e {
                    SeatEvent::Pass { cards, .. } => Some(cards),
                    _ => None,
                })
                .all(Option::is_none));
            if both {
                if !(table.view(1).knowledge.played() & hypothetical[0].cards).is_empty() {
                    replayed_returned_card = true;
                    break;
                }
                let action = *table
                    .to_act()
                    .unwrap()
                    .1
                    .iter()
                    .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
                    .unwrap();
                table.apply(action);
            }
        }
        assert!(
            !both || replayed_returned_card,
            "must replay an actually played returned card"
        );
    }
}

#[test]
fn same_current_and_dealt_hands_do_not_identify_private_transfer_history() {
    let table = fixture(true);
    let hands = std::array::from_fn(|seat| table.hand(seat as u8));
    let first = assignments(&table);
    let alternate: CardSet = table.hand(2).cards()[5..]
        .iter()
        .fold(CardSet::EMPTY, |set, c| set | CardSet::of(c));
    assert_ne!(alternate, first[0].cards);
    let second: Vec<_> = first
        .iter()
        .map(|h| HypothesizedPass {
            event_index: h.event_index,
            cards: alternate,
        })
        .collect();
    let rebuilt = Table::from_view_with_passes(&table.view(1), hands, &second).unwrap();
    for seat in 0..4 {
        assert_eq!(rebuilt.hand(seat), hands[seat as usize]);
        assert_eq!(
            rebuilt.view(seat).knowledge.dealt_certain(seat),
            table.view(seat).knowledge.dealt_certain(seat)
        );
    }
    assert_eq!(rebuilt.view(1).events, table.view(1).events);
    assert_eq!(rebuilt.view(1).knowledge, table.view(1).knowledge);
    assert_ne!(rebuilt.view(0).events, table.view(0).events);
    assert_ne!(rebuilt.view(2).events, table.view(2).events);
}

#[test]
fn rejects_missing_duplicate_extra_visible_and_wrong_transfer_assignments() {
    let table = fixture(true);
    let hands = std::array::from_fn(|seat| table.hand(seat as u8));
    let good = assignments(&table);
    for bad in [
        vec![],
        vec![good[0]],
        vec![good[0], good[0]],
        vec![
            good[0],
            good[1],
            HypothesizedPass {
                event_index: 0,
                cards: good[0].cards,
            },
        ],
    ] {
        assert!(Table::from_view_with_passes(&table.view(1), hands, &bad).is_err());
    }
    assert!(matches!(
        Table::from_view_with_passes(&table.view(2), hands, &good),
        Err(HistoryReplayError::InvalidHypotheses)
    ));
    let mut bad = good.clone();
    bad[0].cards = CardSet::EMPTY;
    assert!(matches!(
        Table::from_view_with_passes(&table.view(1), hands, &bad),
        Err(HistoryReplayError::InvalidHypotheses)
    ));
    bad[0].cards = table.hand(1).cards()[..4]
        .iter()
        .fold(CardSet::EMPTY, |set, c| set | CardSet::of(c));
    assert!(matches!(
        Table::from_view_with_passes(&table.view(1), hands, &bad),
        Err(HistoryReplayError::InvalidHistory)
    ));
}

#[test]
fn interruption_never_publishes_a_partially_replayed_table() {
    let table = fixture(true);
    let hands = std::array::from_fn(|seat| table.hand(seat as u8));
    let good = assignments(&table);
    let mut calls = 0;
    Table::from_view_with_passes_checked(&table.view(1), hands, &good, &mut || {
        calls += 1;
        true
    })
    .unwrap();
    for allowed in [0, 3, calls - 1] {
        let mut count = 0;
        let result = Table::from_view_with_passes_checked(&table.view(1), hands, &good, &mut || {
            count += 1;
            count <= allowed
        });
        assert!(matches!(result, Err(HistoryReplayError::Interrupted)));
    }
}

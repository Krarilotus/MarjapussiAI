use agent::{SeatKind, SeriesScore, Table};
use engine::{ActionType, Card, CardSet, GameAction, GamePhase};

fn table() -> Table {
    let hands = std::array::from_fn(|s| (0..9).map(|c| Card::from_index((9 * s + c) as u8)).collect());
    let mut table = Table::new(&hands);
    table.series = SeriesScore {
        team_totals: [220, 415],
        games_left: Some(2),
        bonus_team: None,
    };
    table.kinds = [
        SeatKind::Human,
        SeatKind::Heuristic,
        SeatKind::Heuristic,
        SeatKind::Human,
    ];
    table
}

fn apply(table: &mut Table, predicate: impl Fn(&GameAction) -> bool) {
    let action = *table.to_act().unwrap().1.iter().find(|a| predicate(a)).unwrap();
    table.apply(action);
}

#[test]
fn visible_history_reproduces_legal_play_and_each_seats_private_observations() {
    let mut table = table();
    apply(&mut table, |a| matches!(a.action_type, ActionType::NewBid(120)));
    for _ in 0..3 {
        apply(&mut table, |a| a.action_type == ActionType::StopBidding);
    }
    let mut checked = 0;
    let mut announced = false;
    for _ in 0..64 {
        let hands = std::array::from_fn(|s| table.hand(s as u8));
        for seat in [0, 2] {
            let rebuilt = Table::from_view(&table.view(seat), hands).expect("visible history reconstructs");
            assert_eq!(rebuilt.series, table.series);
            assert_eq!(rebuilt.kinds, table.kinds);
            assert_eq!(rebuilt.to_act(), table.to_act());
            for observer in 0..4 {
                assert_eq!(rebuilt.view(observer).events, table.view(observer).events);
                assert_eq!(rebuilt.view(observer).knowledge, table.view(observer).knowledge);
            }
            checked += 1;
        }
        let Some((_, legal)) = table.to_act() else { break };
        let action = legal
            .iter()
            .find(|a| matches!(a.action_type, ActionType::AnnounceTrump(_)))
            .or_else(|| {
                legal
                    .iter()
                    .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
            })
            .unwrap_or(&legal[0]);
        announced |= matches!(action.action_type, ActionType::AnnounceTrump(_));
        table.apply(*action);
    }
    assert!(table.game.ended());
    assert!(checked > 70);
    assert!(announced);
}

#[test]
fn rejects_hidden_transfers_and_inconsistent_remaining_hands() {
    let mut table = table();
    let mut hands = std::array::from_fn(|s| table.hand(s as u8));
    hands.swap(0, 1);
    assert!(Table::from_view(&table.view(0), hands).is_none());
    hands = std::array::from_fn(|s| table.hand(s as u8));
    hands[1] = hands[0];
    assert!(Table::from_view(&table.view(0), hands).is_none());
    hands = std::array::from_fn(|s| table.hand(s as u8));
    hands[1] -= CardSet::single(hands[1].iter().next().unwrap());
    assert!(Table::from_view(&table.view(0), hands).is_none());
    apply(&mut table, |a| matches!(a.action_type, ActionType::NewBid(120)));
    for _ in 0..3 {
        apply(&mut table, |a| a.action_type == ActionType::StopBidding);
    }
    apply(&mut table, |a| matches!(a.action_type, ActionType::Pass(_)));
    assert_eq!(table.game.state.phase, GamePhase::PassingBack);
    hands = std::array::from_fn(|s| table.hand(s as u8));
    assert!(Table::from_view(&table.view(1), hands).is_none());
    assert!(Table::from_view(&table.view(2), hands).is_some());
}

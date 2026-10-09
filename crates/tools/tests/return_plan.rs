//! End-to-end return forecasts: actual exchange, scoring and visible opening.
//! Existing privacy/history suites cover hidden information and reuse.

use engine::{ActionType, GamePhase};
use eval::{deal, Table};
use heuristic::{Decision, HeuristicAgent, Params};

fn before_return(seed: u64) -> Table {
    let mut table = Table::new(&deal(seed));
    let mut players: [HeuristicAgent; 4] = std::array::from_fn(|_| {
        HeuristicAgent::new(Params {
            return_worlds: 0,
            ..Params::default()
        })
    });
    while table.game.state.phase != GamePhase::PassingBack {
        let (seat, legal) = table.to_act().expect("these fixtures have a declarer");
        let d = players[seat as usize].decide(&table.view(seat), legal);
        table.apply(d.action);
    }
    table
}

fn value(d: &Decision, key: &str) -> f32 {
    d.why
        .iter()
        .find(|f| f.key == key)
        .unwrap_or_else(|| panic!("missing {key}: {d:?}"))
        .value
}

#[test]
fn forecast_counts_real_scoring_and_follows_its_visible_opening() {
    for seed in [0, 7, 42] {
        let mut table = before_return(seed);
        let mut agent = HeuristicAgent::default();
        let (seat, legal) = table.to_act().unwrap();
        let d = agent.decide(&table.view(seat), legal);
        assert!(legal.contains(&d.action));
        assert_eq!(value(&d, "forecast_complete"), 1.0, "{seed}: {d:?}");
        assert_eq!(value(&d, "forecast_worlds"), 8.0);
        assert!((0.0..=1.0).contains(&value(&d, "forecast_made")));
        assert!((0.0..=1.0).contains(&value(&d, "forecast_last")));
        assert!((0.0..=9.0).contains(&value(&d, "forecast_tricks")));
        assert!(value(&d, "forecast_gain") >= 0.0);
        let names: std::collections::BTreeSet<_> = d.why.iter().map(|f| f.key).collect();
        assert_eq!(
            names.len(),
            d.why.len(),
            "an explanation must not repeat old candidate contributions"
        );
        let opening = value(&d, "forecast_opening") as u8;
        table.apply(d.action);
        for s in 0..4 {
            assert_eq!(table.hand(s).len(), 9);
        }
        let (next, legal) = table.to_act().unwrap();
        assert_eq!(next, seat);
        let reused = agent.decide(&table.view(next), legal);
        assert_eq!(
            reused.action.action_type,
            ActionType::CardPlayed(engine::Card::from_index(opening))
        );
    }
}

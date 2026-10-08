use engine::ActionType;
use eval::{deal, Table};
use heuristic::{HeuristicAgent, Params};

fn auction(bid: i32) -> Table {
    let mut table = Table::new(&deal(42));
    for kind in [ActionType::NewBid(bid), ActionType::StopBidding] {
        let action = *table
            .to_act()
            .unwrap()
            .1
            .iter()
            .find(|a| a.action_type == kind)
            .unwrap();
        table.apply(action);
    }
    table
}

#[test]
fn reused_agent_must_match_fresh_agent_on_another_legal_history() {
    let a = auction(120);
    let b = auction(125);
    let mut reused = HeuristicAgent::default();
    assert_eq!(reused.mind(&a.view(2)).knowledge().value(), 120);
    let expected = HeuristicAgent::default().mind(&b.view(2)).knowledge().value();
    assert_eq!(expected, 125);
    assert_eq!(reused.mind(&b.view(2)).knowledge().value(), expected);
}

#[test]
fn nonfinite_probability_parameters_must_be_rejected() {
    assert!(Params::from_toml("trust_partner = NaN").is_err());
}

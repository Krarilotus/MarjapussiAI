mod common;

use agent::Table;
use common::{bid_of, set};
use engine::{ActionType, CardSet, GamePhase};
use heuristic::{HeuristicAgent, Params};

fn table() -> Table {
    let own = set(&["r-A", "r-Z", "r-K", "r-O", "g-A", "g-Z", "e-A", "s-A", "s-Z"]);
    let rest = (CardSet::ALL - own).cards();
    Table::new(&[
        own.cards(),
        rest[..9].to_vec(),
        rest[9..18].to_vec(),
        rest[18..].to_vec(),
    ])
}

fn apply(table: &mut Table, kind: ActionType) {
    let action = *table
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| a.action_type == kind)
        .unwrap();
    table.apply(action);
}

fn params() -> Params {
    Params {
        plan_worlds: 8,
        return_worlds: 0,
        ..Params::default()
    }
}

#[test]
fn complete_contract_continuations_can_overbid_a_strong_hand_above_the_signal_window() {
    let mut table = table();
    for action in [
        ActionType::NewBid(120),
        ActionType::NewBid(145),
        ActionType::StopBidding,
        ActionType::StopBidding,
    ] {
        apply(&mut table, action);
    }
    let (seat, legal) = table.to_act().unwrap();
    let d = HeuristicAgent::new(params()).decide(&table.view(seat), legal);
    assert_eq!(bid_of(&d), Some(150), "{d:?}");
    assert!(
        d.why.iter().any(|f| f.key == "forecast_complete" && f.value == 1.0),
        "{d:?}"
    );
    assert!(d.why.iter().any(|f| f.key == "forecast_gain" && f.value > 0.0), "{d:?}");
}

// Privacy compares identical permitted inputs and completed semantic evidence.
// Cache/solver accounting has independent tests; never erase incomplete status.
fn assert_same_complete_forecast(left: &Table, right: &Table) {
    let (seat, legal) = left.to_act().unwrap();
    let (other_seat, other_legal) = right.to_act().unwrap();
    let a = left.view(seat);
    let b = right.view(other_seat);
    assert_eq!(
        (a.seat, a.knowledge, a.events, a.kinds, a.series, legal),
        (b.seat, b.knowledge, b.events, b.kinds, b.series, other_legal),
        "the hidden swap must preserve all permitted policy inputs"
    );
    let capture = |t: &Table| {
        let (seat, legal) = t.to_act().unwrap();
        let mut policy = HeuristicAgent::new(params());
        let mut decision = policy.decide(&t.view(seat), legal);
        assert!(
            decision.why.iter().any(|f| f.key == "forecast_complete" && f.value == 1.0),
            "privacy of completed forecasts is not established by a deferred fallback: {decision:?}"
        );
        let assessment = policy.last_assessment().expect("complete forecast has its value assessment").clone();
        let mut trace = policy.last_sequence().expect("complete forecast has its sequence evidence").clone();
        assert!(assessment.values.cohort_complete);
        assert_eq!(trace.failure, None);
        assert_eq!(trace.deferred_limit, None);
        assert_ne!(trace.mode, heuristic::PlanMode::Deferred);
        assert!(trace.program.is_some());
        assert_eq!(trace.total_candidates, Some(assessment.values.candidates.len()));
        assert_eq!(trace.work.completed_candidates, assessment.values.candidates.len());
        assert!(trace.terminals.iter().all(|terminal| !terminal.interrupted));
        // Only the explicit operational lane is normalized. Keep every action,
        // rule, value, probability, provenance, dependency and completion fact.
        trace.work = heuristic::sequence_evaluation::Work::default();
        decision.why.retain(|fact| {
            !matches!(
                fact.key,
                "sequence_transitions"
                    | "sequence_represented_transitions"
                    | "sequence_response_hits"
                    | "sequence_suffix_hits"
            )
        });
        (decision, assessment, trace)
    };
    assert_eq!(capture(left), capture(right));
}

#[test]
fn unseen_opponent_swap_preserves_auction_and_raising_forecasts() {
    let mut table = table();
    let mut hands = std::array::from_fn(|s| table.hand(s as u8).cards());
    let hidden = hands[1][0];
    hands[1][0] = hands[3][0];
    hands[3][0] = hidden;
    let mut other = Table::new(&hands);
    for t in [&mut table, &mut other] {
        for action in [
            ActionType::NewBid(120),
            ActionType::NewBid(145),
            ActionType::StopBidding,
            ActionType::StopBidding,
        ] {
            apply(t, action);
        }
    }
    assert_same_complete_forecast(&table, &other);
    for t in [&mut table, &mut other] {
        apply(t, ActionType::NewBid(150));
        apply(t, ActionType::StopBidding);
    }
    assert_eq!(table.game.state.phase, GamePhase::PassingForth);
    let pass = table.to_act().unwrap().1[0].action_type;
    for t in [&mut table, &mut other] {
        apply(t, pass);
        apply(t, pass);
    }
    assert_eq!(table.view(0).knowledge, other.view(0).knowledge);
    assert_same_complete_forecast(&table, &other);
}

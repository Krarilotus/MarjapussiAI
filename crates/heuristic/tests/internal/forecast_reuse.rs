use super::*;
use crate::Params;

#[test]
fn explanation_compares_the_best_rejected_action_even_when_candidate_one_wins() {
    let mut comparison = Comparison {
        means: [10.0, 40.0, 30.0]
            .map(|score| Outcome {
                score,
                ..Outcome::default()
            })
            .to_vec(),
        terminals: vec![],
    };
    assert_eq!(comparison.best(), 1);
    assert_eq!(comparison.alternative(comparison.best()), Some(2));
    comparison.means.truncate(1);
    assert_eq!(comparison.alternative(0), None);
    let outcome = Outcome {
        declaring: 0.25,
        made: 0.125,
        ..Outcome::default()
    };
    assert_eq!(outcome.made_given_declaring(), Some(0.5));
    assert_eq!(Outcome::default().made_given_declaring(), None);
}

#[test]
fn reused_forecast_actors_match_fresh_actors_across_hypothesis_forks() {
    let params = Params {
        plan_worlds: 0,
        return_worlds: 0,
        endgame_tricks: 2,
        endgame_worlds: 8,
        ..Params::default()
    };
    let players = || std::array::from_fn(|_| HeuristicAgent::new(params.clone()));
    let mut reused = players();
    let mut rng = Rng::new(4107);
    for _ in 0..12 {
        let mut deck: Vec<u8> = (0..36).collect();
        for i in (1..36).rev() {
            deck.swap(i, rng.next() as usize % (i + 1));
        }
        let hands = std::array::from_fn(|s| {
            deck[s * 9..(s + 1) * 9]
                .iter()
                .map(|&c| engine::Card::from_index(c))
                .collect()
        });
        let mut table = Table::new(&hands);
        for kind in [
            ActionType::NewBid(120),
            ActionType::StopBidding,
            ActionType::StopBidding,
            ActionType::StopBidding,
        ] {
            let action = *table
                .to_act()
                .unwrap()
                .1
                .iter()
                .find(|a| a.action_type == kind)
                .unwrap();
            table.apply(action);
        }
        for _ in 0..2 {
            let action = *table
                .to_act()
                .unwrap()
                .1
                .iter()
                .find(|a| matches!(a.action_type, ActionType::Pass(_)))
                .unwrap();
            table.apply(action);
        }
        let actions: Vec<_> = table
            .to_act()
            .unwrap()
            .1
            .iter()
            .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
            .take(2)
            .copied()
            .collect();
        for action in actions {
            let deadline = Instant::now() + Duration::from_secs(30);
            let actual = rollout(
                table.clone(),
                action,
                &mut reused,
                deadline,
                &mut cache::Decisions::default(),
            )
            .unwrap();
            let expected = rollout(
                table.clone(),
                action,
                &mut players(),
                deadline,
                &mut cache::Decisions::default(),
            )
            .unwrap();
            assert_eq!(actual, expected);
        }
    }
}

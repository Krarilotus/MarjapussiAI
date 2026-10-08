//! Frozen before the exchange cleanup at 46231e6: synthetic deals only.
//! Covers the complete auction and both exchanges, including sampled choices and explanations.

use agent::Agent;
use engine::{ActionType, GamePhase};
use eval::{deal, Table};
use heuristic::{HeuristicAgent, Params};

#[test]
fn exchange_decisions_match_the_pre_cleanup_checkpoint() {
    let mut trace = String::new();
    let mut passes = [0; 2];
    for temperature in [0.0, 1.0] {
        for seed in 0..32 {
            let mut table = Table::new(&deal(seed));
            let params = Params {
                sample_temperature: temperature,
                seed: 42,
                ..Params::default()
            };
            let mut players: Vec<_> = (0..4).map(|_| HeuristicAgent::new(params.clone())).collect();
            while let Some((seat, legal)) = table.to_act() {
                let phase = &table.game.state.phase;
                if !matches!(
                    phase,
                    GamePhase::Bidding | GamePhase::PassingForth | GamePhase::PassingBack
                ) {
                    break;
                }
                let d = players[seat as usize].act(&table.view(seat), legal);
                assert!(legal.contains(&d.action));
                if matches!(d.action.action_type, ActionType::Pass(_)) {
                    passes[usize::from(*phase == GamePhase::PassingBack)] += 1;
                    trace.push_str(&format!("{temperature} {seed} {d:?}\n"));
                }
                table.apply(d.action);
            }
        }
    }
    assert_eq!(passes, [64, 64]);
    assert_eq!(
        trace.lines().collect::<Vec<_>>(),
        include_str!("fixtures/passing-46231e6.txt").lines().collect::<Vec<_>>()
    );
}

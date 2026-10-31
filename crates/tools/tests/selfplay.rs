//! Whole-game determinism and explanations for voluntary pair-half plays.

use agent::Agent;
use eval::{deal, play_game, Table};
use heuristic::{HeuristicAgent, Params};

fn hp1_players(params: &Params) -> Vec<Box<dyn Agent>> {
    (0..4)
        .map(|_| Box::new(HeuristicAgent::new(params.clone())) as Box<dyn Agent>)
        .collect()
}

#[test]
fn fresh_and_reused_agents_play_the_same_game() {
    // Exercise the full policy with little work; separate suites check deeper/multi-world search.
    // Without deadlines the logical budgets alone decide, so both agents do the same work.
    let p = Params {
        endgame_tricks: 2,
        endgame_worlds: 1,
        exchange_worlds: 8,
        deadlines: false,
        ..Params::default()
    };
    let mut endgames = 0;
    for seed in 0..50 {
        let hands = deal(seed);
        let (result, log) = play_game(&hands, &mut hp1_players(&p));
        let mut table = Table::new(&hands);
        table.kinds = [agent::SeatKind::Heuristic; 4];
        for l in &log {
            let (seat, legal) = table.to_act().unwrap();
            assert_eq!(seat, l.seat);
            let fresh = HeuristicAgent::new(p.clone()).act(&table.view(seat), legal);
            for d in [&fresh, &l.decision] {
                endgames += d.explanation.facts.iter().filter(|f| f.key == "complete").count();
                assert!(
                    d.explanation
                        .facts
                        .iter()
                        .filter(|f| f.key == "complete")
                        .all(|f| f.value == 1.0),
                    "seed {seed}: determinism comparison must finish: {:?}",
                    d
                );
            }
            assert_eq!(fresh.action, l.decision.action, "seed {seed}");
            table.apply(fresh.action);
        }
        assert!(table.game.ended());
        assert_eq!(result.scores(), engine::GameFinishedInfo::from(table.game).scores());
        for l in log.iter().filter(|l| l.flags.voluntary_pair_half) {
            assert!(
                !l.decision.explanation.facts.is_empty(),
                "seed {seed}: unexplained half play"
            );
        }
    }
    assert!(endgames > 0, "the suite must exercise the solver");
}

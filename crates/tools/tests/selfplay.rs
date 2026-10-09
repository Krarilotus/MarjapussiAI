//! Whole-game determinism and explanations for voluntary pair-half plays.

use agent::Agent;
use eval::{deal, play_game};
use heuristic::{HeuristicAgent, Params};

fn hp1_players(params: &Params) -> Vec<Box<dyn Agent>> {
    (0..4)
        .map(|_| Box::new(HeuristicAgent::new(params.clone())) as Box<dyn Agent>)
        .collect()
}

#[test]
fn same_deal_same_game() {
    let p = Params::default();
    for seed in 0..50 {
        let hands = deal(seed);
        let (a, log_a) = play_game(&hands, &mut hp1_players(&p));
        let (b, log_b) = play_game(&hands, &mut hp1_players(&p));
        let actions = |log: &[eval::Logged]| log.iter().map(|l| l.decision.action).collect::<Vec<_>>();
        assert_eq!(actions(&log_a), actions(&log_b), "seed {seed}");
        assert_eq!(a.scores(), b.scores());
        for l in log_a.iter().filter(|l| l.flags.voluntary_pair_half) {
            assert!(
                !l.decision.explanation.facts.is_empty(),
                "seed {seed}: unexplained half play"
            );
        }
    }
}

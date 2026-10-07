//! Game records (`docs/design/game-records.md`): every action is recorded once, in order, with
//! its rule, and cards round-trip through the engine's notation.

use agent::Agent;
use engine::Card;
use eval::{deal, play_game_with};
use heuristic::{HeuristicAgent, Params};
use tools::record::{beliefs_json, game_record, Meta, Note, SCHEMA};

#[test]
fn an_agent_game_record_holds_every_action_with_its_rule() {
    let params = Params::default();
    for seed in 0..5 {
        let mut players: Vec<Box<dyn Agent>> = (0..4)
            .map(|_| Box::new(HeuristicAgent::new(params.clone())) as Box<dyn Agent>)
            .collect();
        let mut observers: Vec<HeuristicAgent> = (0..4).map(|_| HeuristicAgent::new(params.clone())).collect();
        let mut notes = vec![];
        let (info, log) = play_game_with(&deal(seed), &mut players, |table, seat, d| {
            notes.push(Note {
                decision: Some(d.clone()),
                hp1: None,
                beliefs: Some(beliefs_json(observers[seat as usize].mind(&table.view(seat)))),
            });
        });
        let meta = Meta {
            source: "hp1",
            id: format!("test-{seed}"),
            seed: Some(seed),
            params_hash: None,
            date: "2026-10-06".into(),
            players: ["HP1", "HP1", "HP1", "HP1"].map(String::from),
        };
        let rec = game_record(&meta, &info, &notes);
        assert_eq!(rec["schema"], SCHEMA);
        let actions = rec["actions"].as_array().unwrap();
        assert_eq!(actions.len(), log.len());
        for (a, l) in actions.iter().zip(&log) {
            assert_eq!(a["seat"], l.seat);
            assert_eq!(a["rule"], l.decision.explanation.label);
            assert_eq!(a["beliefs"]["seat"], l.seat);
            if let Some(c) = a["card"].as_str() {
                c.parse::<Card>().expect("card notation");
            }
        }
        let tricks = rec["result"]["tricks"].as_array().unwrap();
        assert_eq!(tricks.len(), 9);
        let deal = rec["deal"].as_array().unwrap();
        assert!(deal.iter().all(|h| h.as_array().unwrap().len() == 9));
    }
}

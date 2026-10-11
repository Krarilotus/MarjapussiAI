//! Four fixed policy slots, whole-deal worker shards, and one complete NDJSON game at a time.

use std::io::{BufWriter, Write};

use agent::Agent;
use beliefs::{conventions::Message, Signals};
use engine::GamePhase;
use eval::{matchstats::{MatchObserver, MatchStats}, play_game_with, suite::DealSuite, RandomAgent, Rng};
use heuristic::{HeuristicAgent, Params};
use serde_json::{json, Value};

use crate::{cli::Cli, record::{action_json, game_record, today, Meta, Note}};

pub const SCHEMA: &str = "marjapussi.selfplay/1";

/// A registered policy. The factory receives a deal/slot-derived stream seed and physical
/// seat. It is called afresh for every game; true-hand diagnostics never reach the factory.
pub struct Policy {
    pub name: String,
    pub parameters: String,
    pub make: Box<dyn Fn(u64, u8) -> Box<dyn Agent>>,
}

fn hp1(name: &str, params: Params) -> Policy {
    let parameters = format!("{}# signals = {:?}\n# seed is derived per deal and slot\n", params.to_toml(), params.beliefs.signals);
    Policy {
        name: name.into(),
        parameters,
        make: Box::new(move |seed, _seat| {
            let mut params = params.clone();
            params.seed = seed;
            Box::new(HeuristicAgent::new(params))
        }),
    }
}

pub fn builtin_policies() -> Vec<Policy> {
    let fast = Params { plan_worlds: 0, return_worlds: 0, endgame_tricks: 0, ..Params::default() };
    let mut facts = Params::default();
    facts.beliefs.signals = Signals::BaseOnly;
    vec![
        hp1("hp1", Params::default()),
        hp1("hp1-fast", fast),
        hp1("hp1-facts", facts),
        Policy {
            name: "random".into(),
            parameters: "uniform legal actions; SplitMix64 stream per deal and slot".into(),
            make: Box::new(|seed, _seat| Box::new(RandomAgent(Rng::new(seed)))),
        },
    ]
}

struct Options {
    slots: [usize; 4],
    games: u64,
    suite: DealSuite,
    worker_index: u64,
    workers: usize,
}

impl Options {
    fn parse(cli: &Cli, policies: &[Policy]) -> Result<Self, String> {
        let default = policies.first().ok_or("no policies registered")?.name.as_str();
        for (i, policy) in policies.iter().enumerate() {
            if policy.name.is_empty() || policies[..i].iter().any(|p| p.name == policy.name) {
                return Err(format!("empty or duplicate policy name: {}", policy.name));
            }
        }
        let mut slots = [0; 4];
        for (slot, arg) in ["--seat0", "--seat1", "--seat2", "--seat3"].into_iter().enumerate() {
            let name = cli.str(arg).unwrap_or(default);
            slots[slot] = policies.iter().position(|p| p.name == name)
                .ok_or_else(|| format!("{arg}: unknown policy {name}"))?;
        }
        let number = |arg, default| -> Result<u64, String> {
            cli.str(arg).map_or(Ok(default), |v| v.parse().map_err(|_| format!("{arg}: invalid integer {v}")))
        };
        let games = number("--games", 1_000)?;
        let seed = number("--seed", 1)?;
        let workers = number("--workers", 1)?;
        let worker_index = number("--worker-index", 0)?;
        if workers == 0 || worker_index >= workers {
            return Err("--workers must be positive and --worker-index smaller than --workers".into());
        }
        let workers = usize::try_from(workers).map_err(|_| "--workers exceeds this platform's range")?;
        let suite = DealSuite::named(&format!("seed{seed}"), games.div_ceil(4))?;
        Ok(Self { slots, games, suite, worker_index, workers })
    }
}

/// Opposite slots stay partners. Rotations 2/3 separately turn the deal by two seats,
/// as duplicate::Rotation::TeamsAndSeats does. A heterogeneous slot receives two initial
/// hands across the two team roles, not all four hand allocations.
fn lineup(rotation: u8) -> [u8; 4] {
    std::array::from_fn(|seat| (seat as u8 + 4 - rotation) % 4)
}

fn stream_seed(deal_seed: u64, slot: u8) -> u64 {
    // Reuse the eval RNG's mixing; stream identity follows the slot through all rotations.
    Rng::new(deal_seed.wrapping_add(u64::from(slot).wrapping_mul(0x9E37_79B9_7F4A_7C15))).next_u64()
}

fn phase(phase: &GamePhase) -> &'static str {
    match phase {
        GamePhase::WaitingForStart => "waiting_for_start",
        GamePhase::Bidding => "bidding",
        GamePhase::PassingForth => "passing_forth",
        GamePhase::PassingBack => "passing_back",
        GamePhase::Raising => "raising",
        GamePhase::Trick => "trick",
        GamePhase::StartTrick => "start_trick",
        GamePhase::AnsweringPair => "answering_pair",
        GamePhase::AnsweringHalf(_) => "answering_half",
        GamePhase::Ended => "ended",
        GamePhase::PendingUndo(_) => "pending_undo",
    }
}

fn message(message: Message) -> &'static str {
    match message {
        Message::Ace => "ace",
        Message::TwoHalves => "two_halves",
        Message::SmallOrHalves => "small_or_halves",
        Message::BigPair => "big_pair",
        Message::RedPair => "red_pair",
    }
}

fn metrics_json(m: &MatchStats) -> Value {
    let seats: Vec<_> = m.seats.iter().map(|s| json!({
        "auction_bids": s.auction_bids, "auction_passes": s.auction_passes,
        "raise_opportunities": s.raise_opportunities, "raises": s.raises,
        "raise_amount": s.raise_amount,
        "questions": s.questions, "melds": s.melds, "negative_answers": s.negative_answers,
    })).collect();
    let readings: Vec<_> = m.communication.readings.iter().map(|r| json!({
        "seat": r.seat, "target": r.target, "message": r.message.map(message),
        "supported": r.supported, "ambiguous": r.ambiguous,
        "candidates": r.candidates.iter().copied().map(message).collect::<Vec<_>>(),
        "available_at_pass": r.available_at_pass.iter().copied().map(message).collect::<Vec<_>>(),
    })).collect();
    json!({
        "scores": m.scores, "team_points": m.team_points,
        "card_points": m.card_points, "pair_points": m.pair_points,
        "card_point_ceiling": m.card_point_ceiling, "pair_point_ceiling": m.pair_point_ceiling,
        "tricks": m.tricks, "schwarz": m.schwarz, "schwarz_teams": m.schwarz_teams,
        "contract": m.contract.as_ref().map(|c| json!({
            "declarer": c.declarer, "team": c.team, "auction_value": c.auction_value,
            "final_value": c.final_value, "raise_amount": c.raise_amount, "made": c.made,
        })),
        "seats": seats,
        "communication": {
            "basis": "common_default_convention_support_not_receiver_understanding",
            "recognizable": m.communication.recognizable,
            "supported": m.communication.supported, "unsupported": m.communication.unsupported,
            "unrecognized_bids": m.communication.unrecognized_bids,
            "ambiguous_bids": m.communication.ambiguous_bids,
            "passes_with_available_message": m.communication.passes_with_available_message,
            "readings": readings,
        },
    })
}

fn game(policies: &[Policy], options: &Options, deal_index: u64, rotation: u8, index: u64) -> Value {
    let lineup = lineup(rotation);
    let seed = options.suite.deal_seed(deal_index);
    let mut hands = options.suite.hands(deal_index);
    let hands_turned = rotation >= 2;
    if hands_turned {
        hands.rotate_left(2);
    }
    let mut players: Vec<_> = lineup.iter().enumerate().map(|(seat, &slot)| {
        (policies[options.slots[slot as usize]].make)(stream_seed(seed, slot), seat as u8)
    }).collect();
    let mut phases = Vec::new();
    let mut observer = MatchObserver::default();
    let (info, log) = play_game_with(&hands, &mut players, |table, _, _| {
        phases.push(phase(&table.game.state.phase));
        observer.observe(table);
    });
    // The runner logs only decisions, not the four engine START events. game_record filters
    // precisely those START events, so these notes stay aligned with its action indices.
    let notes: Vec<_> = log.iter().map(|l| Note { decision: Some(l.decision.clone()), ..Note::default() }).collect();
    let meta = Meta {
        source: "selfplay",
        id: format!("selfplay-s{}-g{index}", options.suite.seed),
        seed: Some(seed),
        params_hash: None,
        date: today(),
        players: lineup.map(|slot| policies[options.slots[slot as usize]].name.clone()),
    };
    let decisions: Vec<_> = log.iter().zip(phases).map(|(l, phase)| json!({
        "seat": l.seat, "phase": phase,
        "action": action_json(&l.decision.action, phase == "raising"),
        "ms": l.nanos as f64 / 1_000_000.0,
        "rule": l.decision.explanation.label,
        "facts": l.decision.explanation.facts.iter().map(|f| json!({"key": f.key, "value": f.value})).collect::<Vec<_>>(),
    })).collect();
    let slots: Vec<_> = options.slots.iter().enumerate().map(|(slot, &i)| json!({
        "name": policies[i].name, "parameters": policies[i].parameters,
        "stream_seed": stream_seed(seed, slot as u8),
    })).collect();
    json!({
        "schema": SCHEMA, "index": index, "deal_index": deal_index, "deal_seed": seed,
        "rotation": rotation, "hands_turned": hands_turned, "lineup": lineup, "policies": slots,
        "record": game_record(&meta, &info, &notes), "decisions": decisions,
        "metrics": metrics_json(&observer.finish(&info)),
    })
}

fn write_games(policies: &[Policy], options: &Options, writer: &mut impl Write) -> Result<(), String> {
    for deal_index in (options.worker_index..options.suite.deals).step_by(options.workers) {
        for rotation in 0..4u8 {
            let Some(index) = (deal_index * 4).checked_add(u64::from(rotation)) else { break };
            if index >= options.games { break; }
            let envelope = game(policies, options, deal_index, rotation, index);
            serde_json::to_writer(&mut *writer, &envelope).map_err(|e| format!("write game {index}: {e}"))?;
            writeln!(writer).and_then(|_| writer.flush()).map_err(|e| format!("flush game {index}: {e}"))?;
        }
    }
    Ok(())
}

/// Parse the shared Cli options and flush one NDJSON envelope after every completed game.
/// Supplied policies form a registry; --seat0..3 select the four partnership slots by name.
pub fn run_with(policies: Vec<Policy>) -> Result<(), String> {
    let cli = Cli::parse(&["--seat0", "--seat1", "--seat2", "--seat3", "--games", "--seed", "--worker-index", "--workers"], &[]);
    let options = Options::parse(&cli, &policies)?;
    write_games(&policies, &options, &mut BufWriter::new(std::io::stdout().lock()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent::{Decision, Explanation, Fact, SeatView};
    use engine::GameAction;

    struct First(&'static str);
    impl Agent for First {
        fn act(&mut self, _view: &SeatView, legal: &[GameAction]) -> Decision {
            Decision {
                action: legal[0],
                explanation: Explanation {
                    label: self.0,
                    facts: vec![Fact { key: "precision", value: 0.12345678 }],
                },
            }
        }
    }

    fn policies() -> Vec<Policy> {
        ["a", "b", "c", "d"].map(|name| Policy {
            name: name.into(), parameters: "first legal".into(), make: Box::new(move |_, _| Box::new(First(name))),
        }).into_iter().collect()
    }

    fn options(worker: u64) -> Options {
        let args = ["--seat0", "a", "--seat1", "b", "--seat2", "c", "--seat3", "d",
            "--games", "7", "--seed", "5", "--workers", "2", "--worker-index"];
        let cli = Cli::from_args(args.into_iter().map(String::from).chain([worker.to_string()]),
            &["--seat0", "--seat1", "--seat2", "--seat3", "--games", "--seed", "--workers", "--worker-index"], &[]);
        Options::parse(&cli, &policies()).unwrap()
    }

    #[test]
    fn fixed_slots_and_turned_hands_match_duplicate_and_notes_skip_starts() {
        let policies = policies();
        let options = options(0);
        let games: Vec<_> = (0..4).map(|r| game(&policies, &options, 0, r, u64::from(r))).collect();
        assert_eq!(games[0]["lineup"], json!([0, 1, 2, 3]));
        assert_eq!(games[1]["lineup"], json!([3, 0, 1, 2]));
        assert_eq!(games[2]["lineup"], json!([2, 3, 0, 1]));
        assert_eq!(games[3]["lineup"], json!([1, 2, 3, 0]));
        let mut allocations: [std::collections::BTreeSet<usize>; 4] = std::array::from_fn(|_| Default::default());
        for (rotation, g) in games.iter().enumerate() {
            assert_eq!(g["hands_turned"], rotation >= 2);
            assert_eq!(g["index"], rotation);
            assert_eq!(g["deal_seed"], games[0]["deal_seed"]);
            let lineup = lineup(rotation as u8);
            for seat in 0..4 {
                let hand = (seat + if rotation >= 2 { 2 } else { 0 }) % 4;
                assert_eq!(g["record"]["deal"][seat], games[0]["record"]["deal"][hand]);
                allocations[lineup[seat] as usize].insert(hand);
            }
            assert_eq!((lineup[0] + 2) % 4, lineup[2]);
            assert_eq!((lineup[1] + 2) % 4, lineup[3]);
            let actions = g["record"]["actions"].as_array().unwrap();
            let decisions = g["decisions"].as_array().unwrap();
            assert_eq!(actions.len(), decisions.len());
            assert_eq!(actions[0]["type"], "pass_bid");
            for (a, d) in actions.iter().zip(decisions) {
                assert_eq!(a["seat"], d["seat"]);
                assert_eq!(a["rule"], d["rule"]);
                let seat = d["seat"].as_u64().unwrap() as usize;
                let slot = lineup[seat] as usize;
                assert_eq!(d["rule"], policies[options.slots[slot]].name);
                assert_eq!(g["record"]["players"][seat], policies[options.slots[slot]].name);
                assert_eq!(d["facts"][0]["value"], json!(0.12345678f32));
                assert_ne!(d["facts"][0]["value"], a["why"]["precision"]);
            }
            assert!(actions.iter().all(|a| a["type"] != "start"));
            assert_eq!(g["metrics"]["scores"], g["record"]["result"]["scores"]);
        }
        assert!(allocations.iter().all(|hands| hands.len() == 2));
        // No rotation/worker component enters a slot's random stream identity.
        assert_ne!(stream_seed(5, 0), stream_seed(5, 1));
    }

    #[test]
    fn workers_keep_whole_blocks_and_exact_requested_count() {
        let mut indices = Vec::new();
        for worker in 0..2 {
            let mut bytes = Vec::new();
            write_games(&policies(), &options(worker), &mut bytes).unwrap();
            for line in std::str::from_utf8(&bytes).unwrap().lines() {
                let g: Value = serde_json::from_str(line).unwrap();
                assert_eq!(g["deal_index"].as_u64().unwrap() % 2, worker);
                indices.push(g["index"].as_u64().unwrap());
            }
        }
        indices.sort_unstable();
        assert_eq!(indices, (0..7).collect::<Vec<_>>());
        assert!(Options::parse(&Cli::from_args(["--workers".into(), "0".into()], &["--workers"], &[]), &policies()).is_err());
    }
}

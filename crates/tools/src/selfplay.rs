//! Four fixed policy slots, whole-deal worker shards, and one complete NDJSON game at a time.

use std::{
    cell::RefCell,
    collections::VecDeque,
    io::{BufWriter, Write},
    rc::Rc,
    time::Instant,
};

use agent::{Agent, Decision, SeatKind, SeatView};
use beliefs::{conventions::Message, Signals};
use engine::GamePhase;
use eval::{
    matchstats::{MatchObserver, MatchStats},
    play_game_with,
    suite::DealSuite,
    RandomAgent, Rng,
};
use heuristic::value_concepts::{CohortMethod, NodeValue};
use heuristic::{HeuristicAgent, Params, PlanningAssessment};
use serde_json::{json, Value};

use crate::{
    cli::Cli,
    record::{action_json, game_record, today, Meta, Note},
};

pub const SCHEMA: &str = "marjapussi.selfplay/1";

/// A registered policy. The factory receives a deal/slot-derived stream seed and physical
/// seat. It is called afresh for every game; true-hand diagnostics never reach the factory.
pub struct Policy {
    pub name: String,
    pub parameters: String,
    pub make: Box<dyn Fn(u64, u8) -> Box<dyn Agent>>,
}

/// Tools-only capture: no new policy evaluation and no change to the Agent contract.
/// Each captured result belongs to the act call that produced it, not a later replay.
#[derive(Clone, Default)]
pub struct AssessmentRecorder(Rc<RefCell<VecDeque<CapturedDecision>>>);

struct CapturedDecision {
    seat: u8,
    event_count: usize,
    action: engine::GameAction,
    policy_nanos: u128,
    assessment: Option<PlanningAssessment>,
}

struct RecordingAgent {
    agent: HeuristicAgent,
    recorder: AssessmentRecorder,
}

impl Agent for RecordingAgent {
    fn act(&mut self, view: &SeatView, legal: &[engine::GameAction]) -> Decision {
        let started = Instant::now();
        let decision = self.agent.act(view, legal);
        let policy_nanos = started.elapsed().as_nanos();
        self.recorder.0.borrow_mut().push_back(CapturedDecision {
            seat: view.seat,
            event_count: view.events.len(),
            action: decision.action,
            policy_nanos,
            assessment: self.agent.last_assessment().cloned(),
        });
        decision
    }

    fn kind(&self) -> SeatKind {
        self.agent.kind()
    }
}

/// Wrap a current factory; historical policies can remain plain Agent objects.
pub fn record_current(agent: HeuristicAgent, recorder: AssessmentRecorder) -> Box<dyn Agent> {
    Box::new(RecordingAgent { agent, recorder })
}

/// Serialize the consumed evaluator result, never rerun an evaluator for diagnostics.
/// Runtime refs are scoped to this decision. `extensions` is reserved for the next
/// actual sequence receipt/trace; absence does not imply a sequence was evaluated.
pub fn assessment_json(a: &PlanningAssessment, raising: bool) -> Value {
    let v = &a.values;
    let cohort = match v.provenance.cohort {
        CohortMethod::Enumerated => json!({"method": "enumerated"}),
        CohortMethod::ImportanceSampled { draws, proposal_ess } => {
            json!({"method": "importance_sampled", "draws": draws, "proposal_ess": proposal_ess})
        }
        CohortMethod::Resampled { draws, proposal_ess } => {
            json!({"method": "resampled", "draws": draws, "proposal_ess": proposal_ess})
        }
    };
    let trace: Vec<_> = v
        .trace
        .iter()
        .map(|n| {
            let value = match &n.value {
                NodeValue::Number(x) => json!({"number": x}),
                NodeValue::Weights(x) => json!({"weights": x}),
                NodeValue::ConditionalScores(x) => json!({"conditional_engine_scores": x}),
                NodeValue::ConditionalOutcomes(x) => json!({"conditional_outcomes": x.iter().map(|o| json!({
                "playing_team": o.playing_team, "game_value": o.game_value,
                "team_points": o.team_points, "team_tricks": o.team_tricks,
            })).collect::<Vec<_>>()}),
                NodeValue::BranchValues(x) => json!({"branch_values": x}),
                NodeValue::Ranking { selected, alternative } => {
                    json!({"selected": selected, "alternative": alternative})
                }
                // Preserve additional actual evaluator outputs without inventing semantics.
                other => json!({"debug": format!("{other:?}")}),
            };
            json!({"ref": n.reference.0, "name": n.name, "meaning": format!("{:?}", n.meaning),
            "operator": format!("{:?}", n.operator),
            "dependencies": n.dependencies.iter().map(|r| r.0).collect::<Vec<_>>(), "value": value})
        })
        .collect();
    let candidates: Vec<_> = v
        .candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            json!({
                "index": i, "action": action_json(&a.actions[i], raising),
                "our_score": c.our_score, "opponent_score": c.opponent_score, "difference": c.difference,
                "declaring": c.declaring, "joint_made": c.joint_made, "made_given_declaring": c.made_given_declaring,
                "refs": {"scores": c.refs.scores.0.0, "our_score": c.refs.our_score.0.0,
                    "opponent_score": c.refs.opponent_score.0.0, "difference": c.refs.difference.0.0,
                    "declaring": c.refs.declaring.0.0, "joint_made": c.refs.joint_made.0.0,
                    "made_given_declaring": c.refs.made_given_declaring.0.0},
            })
        })
        .collect();
    let context = &a.context;
    json!({
        "schema": "marjapussi.planning-assessment/1", "root": v.root.0, "ranking": v.ranking.0.0,
        "selected": v.selected, "alternative": v.alternative, "root_team": v.root_team,
        "provenance": {"belief_model": v.provenance.belief_model, "response_model": v.provenance.response_model,
            "cohort": cohort, "limit": "numerical model integration, not proof or calibrated probability"},
        "definition": {"opponent_weight": v.definition.opponent_weight},
        "cohort_size": v.cohort_size, "cohort_weight": v.cohort_weight, "cohort_complete": v.cohort_complete,
        "evaluated_nodes": v.evaluated_nodes, "memo_hits": v.memo_hits,
        "context": {"observer": context.observer, "event_count": context.events.len(),
            "events_debug": format!("{:?}", context.events), "knowledge_debug": format!("{:?}", context.knowledge),
            "kinds": format!("{:?}", context.kinds),
            "series": {"team_totals": context.series.team_totals, "games_left": context.series.games_left},
            "response_params": context.response_params.to_toml(), "response_beliefs_debug": format!("{:?}", context.response_params.beliefs),
            "seed": context.seed, "possible_allocations": context.possible_allocations,
            "requested_worlds": context.requested_worlds, "proposal_draws": context.proposal_draws},
        "candidates": candidates, "trace": trace, "extensions": {},
    })
}

fn hp1(name: &str, params: Params, recorder: Option<AssessmentRecorder>) -> Policy {
    let parameters = format!(
        "{}# signals = {:?}\n# effective seed is supplied by worker; see policy_seed\n",
        params.to_toml(),
        params.beliefs.signals
    );
    Policy {
        name: name.into(),
        parameters,
        make: Box::new(move |seed, _seat| {
            let mut params = params.clone();
            params.seed = seed;
            let agent = HeuristicAgent::new(params);
            match &recorder {
                Some(recorder) => record_current(agent, recorder.clone()),
                None => Box::new(agent),
            }
        }),
    }
}

pub fn builtin_policies() -> Vec<Policy> {
    builtin(None)
}

pub fn builtin_policies_with_recording(recorder: AssessmentRecorder) -> Vec<Policy> {
    builtin(Some(recorder))
}

fn builtin(recorder: Option<AssessmentRecorder>) -> Vec<Policy> {
    let fast = Params {
        plan_worlds: 0,
        return_worlds: 0,
        endgame_tricks: 0,
        ..Params::default()
    };
    let mut facts = Params::default();
    facts.beliefs.signals = Signals::BaseOnly;
    vec![
        hp1("hp1", Params::default(), recorder.clone()),
        hp1("hp1-fast", fast, recorder.clone()),
        hp1("hp1-facts", facts, recorder),
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
    rotation: Rotation,
    policy_seed: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rotation {
    Teams,
    TeamsAndSeats,
}
impl Rotation {
    fn games_per_deal(self) -> u64 {
        if self == Self::Teams {
            2
        } else {
            4
        }
    }
    fn name(self) -> &'static str {
        if self == Self::Teams {
            "teams"
        } else {
            "teams-and-seats"
        }
    }
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
            slots[slot] = policies
                .iter()
                .position(|p| p.name == name)
                .ok_or_else(|| format!("{arg}: unknown policy {name}"))?;
        }
        let number = |arg, default| -> Result<u64, String> {
            cli.str(arg).map_or(Ok(default), |v| {
                v.parse().map_err(|_| format!("{arg}: invalid integer {v}"))
            })
        };
        let games = number("--games", 1_000)?;
        let seed = number("--seed", 1)?;
        let workers = number("--workers", 1)?;
        let worker_index = number("--worker-index", 0)?;
        let rotation = match cli.str("--rotation").unwrap_or("teams-and-seats") {
            "teams" => Rotation::Teams,
            "teams-and-seats" => Rotation::TeamsAndSeats,
            other => return Err(format!("--rotation: unknown mode {other}")),
        };
        let policy_seed = cli
            .str("--policy-seed")
            .map(|v| {
                v.parse::<u64>()
                    .map_err(|_| format!("--policy-seed: invalid integer {v}"))
            })
            .transpose()?;
        if workers == 0 || worker_index >= workers {
            return Err("--workers must be positive and --worker-index smaller than --workers".into());
        }
        let workers = usize::try_from(workers).map_err(|_| "--workers exceeds this platform's range")?;
        let suite = DealSuite::named(&format!("seed{seed}"), games.div_ceil(rotation.games_per_deal()))?;
        Ok(Self {
            slots,
            games,
            suite,
            worker_index,
            workers,
            rotation,
            policy_seed,
        })
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

pub(crate) fn phase(phase: &GamePhase) -> &'static str {
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

pub(crate) fn metrics_json(m: &MatchStats) -> Value {
    let seats: Vec<_> = m
        .seats
        .iter()
        .map(|s| {
            json!({
                "auction_bids": s.auction_bids, "auction_passes": s.auction_passes,
                "raise_opportunities": s.raise_opportunities, "raises": s.raises,
                "raise_amount": s.raise_amount,
                "questions": s.questions, "melds": s.melds, "negative_answers": s.negative_answers,
            })
        })
        .collect();
    let readings: Vec<_> = m
        .communication
        .readings
        .iter()
        .map(|r| {
            json!({
                "seat": r.seat, "target": r.target, "message": r.message.map(message),
                "supported": r.supported, "ambiguous": r.ambiguous,
                "candidates": r.candidates.iter().copied().map(message).collect::<Vec<_>>(),
                "available_at_pass": r.available_at_pass.iter().copied().map(message).collect::<Vec<_>>(),
            })
        })
        .collect();
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

fn game(
    policies: &[Policy],
    options: &Options,
    deal_index: u64,
    rotation: u8,
    index: u64,
    recorder: Option<&AssessmentRecorder>,
) -> Result<Value, String> {
    if recorder.is_some_and(|r| !r.0.borrow().is_empty()) {
        return Err("unconsumed assessment from previous game".into());
    }
    let lineup = lineup(rotation);
    let seed = options.suite.deal_seed(deal_index);
    let mut hands = options.suite.hands(deal_index);
    let hands_turned = options.rotation == Rotation::TeamsAndSeats && rotation >= 2;
    if hands_turned {
        hands.rotate_left(2);
    }
    let mut players: Vec<_> = lineup
        .iter()
        .enumerate()
        .map(|(seat, &slot)| {
            (policies[options.slots[slot as usize]].make)(
                options.policy_seed.unwrap_or_else(|| stream_seed(seed, slot)),
                seat as u8,
            )
        })
        .collect();
    let mut phases = Vec::new();
    let mut captures = Vec::new();
    let mut observer = MatchObserver::default();
    let (info, log) = play_game_with(&hands, &mut players, |table, seat, decision| {
        phases.push(phase(&table.game.state.phase));
        let view = table.view(seat);
        let event_count = view.events.len();
        let mut captured = recorder.and_then(|r| r.0.borrow_mut().pop_front());
        let context_matches = captured.as_ref().and_then(|c| c.assessment.as_ref()).is_none_or(|a| {
            a.context.observer == seat
                && a.context.knowledge == *view.knowledge
                && a.context.events.as_slice() == view.events
                && a.context.kinds == view.kinds
                && a.context.series == view.series
        });
        let selected_matches = captured
            .as_ref()
            .and_then(|c| c.assessment.as_ref())
            .map(|a| a.actions[a.values.selected] == decision.action);
        let serialization_started = Instant::now();
        let assessment = captured
            .as_ref()
            .and_then(|c| c.assessment.as_ref())
            .map(|a| assessment_json(a, table.game.state.phase == GamePhase::Raising));
        let serialization_nanos = if assessment.is_some() {
            serialization_started.elapsed().as_nanos()
        } else {
            0
        };
        // Retain one serialized result per decision, not a second raw assessment copy.
        if let Some(c) = &mut captured {
            c.assessment = None;
        }
        captures.push((
            event_count,
            captured,
            assessment,
            serialization_nanos,
            context_matches,
            selected_matches,
        ));
        observer.observe(table);
    });
    // The runner logs only decisions, not the four engine START events. game_record filters
    // precisely those START events, so these notes stay aligned with its action indices.
    let notes: Vec<_> = log
        .iter()
        .map(|l| Note {
            decision: Some(l.decision.clone()),
            ..Note::default()
        })
        .collect();
    let meta = Meta {
        source: "selfplay",
        id: format!("selfplay-s{}-g{index}", options.suite.seed),
        seed: Some(seed),
        params_hash: None,
        date: today(),
        players: lineup.map(|slot| policies[options.slots[slot as usize]].name.clone()),
    };
    let decisions: Vec<_> = log.iter().zip(phases).zip(captures).enumerate().map(|(step, ((l, phase), (event_count, capture, assessment, serialization_nanos, context_matches, selected_matches)))| {
        if let Some(c) = &capture {
            if c.seat != l.seat || c.event_count != event_count || c.action != l.decision.action {
                return Err(format!("assessment identity mismatch at game {index}, step {step}"));
            }
            if !context_matches {
                return Err(format!("assessment context mismatch at game {index}, step {step}"));
            }
        }
        let policy_nanos = capture.as_ref().map_or(l.nanos, |c| c.policy_nanos);
        Ok(json!({
        "step": step, "seat": l.seat, "phase": phase, "event_count": event_count,
        "action": action_json(&l.decision.action, phase == "raising"),
        "ms": policy_nanos as f64 / 1_000_000.0,
        "capture_ms": l.nanos.saturating_sub(policy_nanos) as f64 / 1_000_000.0,
        "assessment_serialization_ms": serialization_nanos as f64 / 1_000_000.0,
        "assessment_recorded": capture.is_some(), "assessment": assessment,
        "assessment_matches_chosen_action": selected_matches,
        "rule": l.decision.explanation.label,
        "facts": l.decision.explanation.facts.iter().map(|f| json!({"key": f.key, "value": f.value})).collect::<Vec<_>>(),
    }))}).collect::<Result<_, String>>()?;
    if recorder.is_some_and(|r| !r.0.borrow().is_empty()) {
        return Err(format!("unconsumed assessment after game {index}"));
    }
    let slots: Vec<_> = options
        .slots
        .iter()
        .enumerate()
        .map(|(slot, &i)| {
            json!({
                "name": policies[i].name, "parameters": policies[i].parameters,
                "stream_seed": stream_seed(seed, slot as u8),
                "policy_seed": options.policy_seed.unwrap_or_else(|| stream_seed(seed, slot as u8)),
            })
        })
        .collect();
    Ok(json!({
        "schema": SCHEMA, "index": index, "deal_index": deal_index, "deal_seed": seed,
        "rotation": rotation, "hands_turned": hands_turned, "lineup": lineup, "policies": slots,
        "rotation_mode": options.rotation.name(), "games_per_deal": options.rotation.games_per_deal(),
        "record": game_record(&meta, &info, &notes), "decisions": decisions,
        "metrics": metrics_json(&observer.finish(&info)),
    }))
}

fn write_games(policies: &[Policy], options: &Options, writer: &mut impl Write) -> Result<(), String> {
    write_recorded_games(policies, options, writer, None)
}

fn write_recorded_games(
    policies: &[Policy],
    options: &Options,
    writer: &mut impl Write,
    recorder: Option<&AssessmentRecorder>,
) -> Result<(), String> {
    for deal_index in (options.worker_index..options.suite.deals).step_by(options.workers) {
        for rotation in 0..options.rotation.games_per_deal() as u8 {
            let Some(index) = (deal_index * options.rotation.games_per_deal()).checked_add(u64::from(rotation)) else {
                break;
            };
            if index >= options.games {
                break;
            }
            let envelope = game(policies, options, deal_index, rotation, index, recorder)?;
            serde_json::to_writer(&mut *writer, &envelope).map_err(|e| format!("write game {index}: {e}"))?;
            writeln!(writer)
                .and_then(|_| writer.flush())
                .map_err(|e| format!("flush game {index}: {e}"))?;
        }
    }
    Ok(())
}

/// Parse the shared Cli options and flush one NDJSON envelope after every completed game.
/// Supplied policies form a registry; --seat0..3 select the four partnership slots by name.
pub fn run_with(policies: Vec<Policy>) -> Result<(), String> {
    run(policies, None)
}

pub fn run_with_recording(policies: Vec<Policy>, recorder: AssessmentRecorder) -> Result<(), String> {
    run(policies, Some(recorder))
}

fn run(policies: Vec<Policy>, recorder: Option<AssessmentRecorder>) -> Result<(), String> {
    let cli = Cli::parse(
        &[
            "--seat0",
            "--seat1",
            "--seat2",
            "--seat3",
            "--games",
            "--seed",
            "--worker-index",
            "--workers",
            "--rotation",
            "--policy-seed",
        ],
        &[],
    );
    let options = Options::parse(&cli, &policies)?;
    match recorder {
        Some(recorder) => write_recorded_games(
            &policies,
            &options,
            &mut BufWriter::new(std::io::stdout().lock()),
            Some(&recorder),
        ),
        None => write_games(&policies, &options, &mut BufWriter::new(std::io::stdout().lock())),
    }
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
                    facts: vec![Fact {
                        key: "precision",
                        value: 0.12345678,
                    }],
                },
            }
        }
    }

    fn policies() -> Vec<Policy> {
        ["a", "b", "c", "d"]
            .map(|name| Policy {
                name: name.into(),
                parameters: "first legal".into(),
                make: Box::new(move |_, _| Box::new(First(name))),
            })
            .into_iter()
            .collect()
    }

    fn options(worker: u64) -> Options {
        let args = [
            "--seat0",
            "a",
            "--seat1",
            "b",
            "--seat2",
            "c",
            "--seat3",
            "d",
            "--games",
            "7",
            "--seed",
            "5",
            "--workers",
            "2",
            "--worker-index",
        ];
        let cli = Cli::from_args(
            args.into_iter().map(String::from).chain([worker.to_string()]),
            &[
                "--seat0",
                "--seat1",
                "--seat2",
                "--seat3",
                "--games",
                "--seed",
                "--workers",
                "--worker-index",
            ],
            &[],
        );
        Options::parse(&cli, &policies()).unwrap()
    }

    #[test]
    fn fixed_slots_and_turned_hands_match_duplicate_and_notes_skip_starts() {
        let policies = policies();
        let options = options(0);
        let games: Vec<_> = (0..4)
            .map(|r| game(&policies, &options, 0, r, u64::from(r), None).unwrap())
            .collect();
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
        assert!(Options::parse(
            &Cli::from_args(["--workers".into(), "0".into()], &["--workers"], &[]),
            &policies()
        )
        .is_err());
    }

    #[test]
    fn teams_shards_128_original_deals_twice_and_supplies_live_seed_zero() {
        let received = Rc::new(RefCell::new(Vec::new()));
        let mut policies = policies();
        for policy in &mut policies {
            let received = received.clone();
            policy.make = Box::new(move |seed, _| {
                received.borrow_mut().push(seed);
                Box::new(First("first"))
            });
        }
        let mut rows = Vec::new();
        for worker in 0..2 {
            let mut options = options(worker);
            options.games = 256;
            options.rotation = Rotation::Teams;
            options.policy_seed = Some(0);
            options.slots = [0, 1, 0, 1];
            options.suite.deals = 128;
            let mut bytes = Vec::new();
            write_games(&policies, &options, &mut bytes).unwrap();
            for line in std::str::from_utf8(&bytes).unwrap().lines() {
                let row: Value = serde_json::from_str(line).unwrap();
                assert_eq!(row["deal_index"].as_u64().unwrap() % 2, worker);
                assert_eq!(row["hands_turned"], false);
                assert_eq!(row["rotation_mode"], "teams");
                assert!(row["policies"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|p| p["policy_seed"] == 0));
                rows.push(row);
            }
        }
        rows.sort_by_key(|g| g["index"].as_u64().unwrap());
        assert_eq!(rows.len(), 256);
        assert_eq!(received.borrow().len(), 1024);
        assert!(received.borrow().iter().all(|&seed| seed == 0));
        for (deal, pair) in rows.chunks_exact(2).enumerate() {
            assert_eq!(pair[0]["index"], 2 * deal);
            assert_eq!(pair[1]["index"], 2 * deal + 1);
            assert_eq!(pair[0]["deal_index"], deal);
            assert_eq!(pair[1]["deal_index"], deal);
            assert_eq!(pair[0]["record"]["deal"], pair[1]["record"]["deal"]);
            assert_eq!(pair[0]["lineup"], json!([0, 1, 2, 3]));
            assert_eq!(pair[1]["lineup"], json!([3, 0, 1, 2]));
            assert_eq!(pair[0]["record"]["players"], json!(["a", "b", "a", "b"]));
            assert_eq!(pair[1]["record"]["players"], json!(["b", "a", "b", "a"]));
        }
    }

    #[test]
    fn capture_is_act_local_preserves_kind_and_forced_decisions_have_no_stale_trace() {
        let recorder = AssessmentRecorder::default();
        let mut actor = record_current(HeuristicAgent::default(), recorder.clone());
        assert_eq!(actor.kind(), SeatKind::Heuristic);
        let table = eval::Table::new(&eval::deal(5));
        let (seat, legal) = table.to_act().unwrap();
        let view = table.view(seat);
        let decision = actor.act(&view, &legal[..1]);
        let capture = recorder.0.borrow_mut().pop_front().unwrap();
        assert_eq!(
            (capture.seat, capture.event_count, capture.action),
            (seat, view.events.len(), decision.action)
        );
        assert!(capture.assessment.is_none());
        recorder.0.borrow_mut().push_back(capture);
        assert!(
            game(&policies(), &options(0), 0, 0, 0, Some(&recorder)).is_err(),
            "stale records cannot silently attach to a new game"
        );
    }

    #[test]
    fn serialized_assessment_preserves_consumed_refs_values_and_honest_context() {
        use heuristic::value_concepts::{Provenance, ValueStore};
        let table = eval::Table::new(&eval::deal(5));
        let (seat, legal) = table.to_act().unwrap();
        let view = table.view(seat);
        let terminals = [
            (
                engine::GameOutcome {
                    playing_team: Some(0),
                    game_value: 120,
                    team_points: [130, 10],
                    team_tricks: [5, 4],
                },
                0.25,
            ),
            (
                engine::GameOutcome {
                    playing_team: Some(0),
                    game_value: 120,
                    team_points: [110, 30],
                    team_tricks: [4, 5],
                },
                0.75,
            ),
        ];
        let values = ValueStore::new(
            &[&terminals],
            usize::from(seat % 2),
            Provenance {
                belief_model: "test-joint",
                response_model: "test-terminals",
                cohort: CohortMethod::Enumerated,
            },
        )
        .unwrap()
        .assess()
        .unwrap();
        let assessment = PlanningAssessment {
            actions: vec![legal[0]],
            values,
            context: heuristic::PlanningContext {
                observer: seat,
                knowledge: *view.knowledge,
                events: view.events.to_vec(),
                kinds: view.kinds,
                series: view.series,
                response_params: Params::default(),
                seed: 0,
                possible_allocations: 2,
                requested_worlds: 2,
                proposal_draws: 0,
            },
        };
        let row = assessment_json(&assessment, false);
        assert_eq!(
            row["candidates"][0]["difference"],
            json!(assessment.values.candidates[0].difference)
        );
        assert_eq!(row["context"]["observer"], seat);
        assert_eq!(row["context"]["events_debug"], format!("{:?}", view.events));
        let trace = row["trace"].as_array().unwrap();
        let ids: std::collections::BTreeSet<_> = trace.iter().map(|n| n["ref"].as_u64().unwrap()).collect();
        assert_eq!(ids.len(), assessment.values.trace.len());
        assert!(ids.contains(&row["root"].as_u64().unwrap()));
        for (recorded, actual) in trace.iter().zip(&assessment.values.trace) {
            assert_eq!(recorded["name"], actual.name);
            assert_eq!(recorded["operator"], format!("{:?}", actual.operator));
            assert_eq!(
                recorded["dependencies"],
                json!(actual.dependencies.iter().map(|r| r.0).collect::<Vec<_>>())
            );
            assert!(recorded["dependencies"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| ids.contains(&r.as_u64().unwrap())));
        }
    }
}

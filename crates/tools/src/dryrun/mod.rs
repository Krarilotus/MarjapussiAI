//! Targeted comparisons on teacher-forced honest views and complete engine continuations.
//! Neither a loss nor a better realized continuation proves that a choice was mistaken.

pub mod fixture;
mod options;

use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    time::Instant,
};

use agent::{Agent, Decision, SeatView, Table};
use engine::{CardSet, GameAction, GamePhase, SUITS};
use eval::play_table_with;
use serde_json::{json, Value};

use crate::{
    cli::Cli,
    record::{action_json, game_record, today, Meta, Note},
    selfplay::{metrics_json, phase, AssessmentRecorder, Policy},
};
use fixture::Fixture;
use options::{Options, OPTIONS};

pub const SCHEMA: &str = "marjapussi.dryrun/1";
pub const POLICY_SEED: u64 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Teacher,
    Continue,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Replacement {
    Action,
    Seat,
    Partnership,
}

/// The compiled adapter may connect its opt-in profile feature here. A single reset
/// happens after fixture verification; the final snapshot covers all policy calls.
pub struct Profiling {
    pub reset: Box<dyn Fn()>,
    pub snapshot: Box<dyn Fn() -> Value>,
}

struct Once {
    first: Option<Box<dyn Agent>>,
    later: Box<dyn Agent>,
}

impl Agent for Once {
    fn act(&mut self, view: &SeatView, legal: &[GameAction]) -> Decision {
        match self.first.take() {
            Some(mut first) => first.act(view, legal),
            None => self.later.act(view, legal),
        }
    }
}

fn cards(cards: CardSet) -> Vec<String> {
    cards.cards().iter().map(ToString::to_string).collect()
}

/// Only exact visible knowledge goes in `known`; possible cards are uncertainty,
/// not a posterior. Policy explanation values have their own provenance below.
fn visible(table: &Table, seat: u8) -> Value {
    let view = table.view(seat);
    let k = view.knowledge;
    let seats: Vec<_> = (0..4).map(|s| json!({
        "seat": s, "certain": cards(k.certain(s)), "possible_not_certain": cards(k.possible(s)),
        "hand_len": k.hand_len(s), "rights": format!("{:?}", k.rights(s)),
        "uncalled_pairs": {"certain_suit_mask": k.pairs(s).certain, "possible_suit_mask": k.pairs(s).possible},
        "voids": SUITS.iter().filter(|&&suit| k.is_void(s, suit)).map(|suit| suit.code().to_string()).collect::<Vec<_>>(),
    })).collect();
    json!({
        "basis": "acting_seat_filtered_events_and_exact_knowledge",
        "seat": seat, "own_hand": cards(k.certain(seat)), "seats": seats,
        "events": view.events.iter().map(|e| format!("{e:?}")).collect::<Vec<_>>(),
        "constraints": k.constraints().iter().map(|c| json!({"seat": c.seat, "cards": cards(c.mask), "lo": c.lo, "hi": c.hi})).collect::<Vec<_>>(),
        "suit_mask_bits": SUITS.map(|s| json!({"suit": s.code().to_string(), "bit": 1u8 << s as u8})),
        "trump": k.trump().map(|s| s.code().to_string()),
        "called": SUITS.iter().filter(|&&s| k.is_called(s)).map(|s| s.code().to_string()).collect::<Vec<_>>(),
        "value": k.value(), "declarer": k.declarer(), "standing": k.winning(),
        "series": {"totals": view.series.team_totals, "games_left": view.series.games_left},
        "kinds": view.kinds.map(|kind| format!("{kind:?}")),
        "posterior": null,
        "posterior_note": "not captured; possible sets and explanation facts are not posterior distributions",
    })
}

fn decision_json(table: &Table, seat: u8, decision: &Decision) -> Value {
    let raising = table.game.state.phase == GamePhase::Raising;
    json!({
        "seat": seat, "phase": phase(&table.game.state.phase),
        "event_count": table.view(seat).events.len(),
        "action": action_json(&decision.action, raising), "action_debug": format!("{:?}", decision.action),
        "rule": decision.explanation.label,
        "facts": decision.explanation.facts.iter().map(|f| json!({
            "key": f.key, "value": f.value, "finite": f.value.is_finite(),
        })).collect::<Vec<_>>(),
        "facts_basis": "policy_explanation_not_hard_proof_or_calibrated_probability",
        "assessment_recorded": false, "value_assessment_present": false, "sequence_present": false,
        "assessment": null, "assessment_matches_chosen_action": null,
        "capture_ms": null, "assessment_serialization_ms": null,
        "view": visible(table, seat),
        "legal": table.to_act().map(|(_, legal)| legal.iter().map(|a| action_json(a, raising)).collect::<Vec<_>>()),
    })
}

fn target_seat(fixture: &Fixture, step: usize, options: &Options) -> u8 {
    match options.candidate_team.as_str() {
        "new" => (fixture.index % 2) as u8,
        "old" => (1 - fixture.index % 2) as u8,
        _ => fixture.actions[step].player.0,
    }
}

fn replaced(seat: u8, target: u8, replacement: Replacement, step: usize, prefix: usize) -> bool {
    match replacement {
        Replacement::Action => seat == target && step == prefix,
        Replacement::Seat => seat == target,
        Replacement::Partnership => seat % 2 == target % 2,
    }
}

fn players(
    fixture: &Fixture,
    step: usize,
    candidate: usize,
    policies: &[Policy],
    options: &Options,
) -> Vec<Box<dyn Agent>> {
    let target = target_seat(fixture, step, options);
    (0..4u8)
        .map(|seat| {
            let side = usize::from(u64::from(seat % 2) != fixture.index % 2);
            let original = options.baseline[side].expect("validated baseline registration");
            let make = |i: usize| (policies[i].make)(POLICY_SEED, seat);
            if options.replacement == Replacement::Action && seat == target {
                Box::new(Once {
                    first: Some(make(candidate)),
                    later: make(original),
                }) as Box<dyn Agent>
            } else if replaced(seat, target, options.replacement, step, step) {
                make(candidate)
            } else {
                make(original)
            }
        })
        .collect()
}

fn teacher(
    fixture: &Fixture,
    step: usize,
    policy: &Policy,
    recorder: Option<&AssessmentRecorder>,
) -> Result<Value, String> {
    if let Some(recorder) = recorder {
        recorder.ensure_empty()?;
    }
    let table = fixture.prefix(step)?;
    let (seat, legal) = table.to_act().ok_or("teacher prefix already ended")?;
    let mut candidate = (policy.make)(POLICY_SEED, seat);
    let t0 = Instant::now();
    let decision = candidate.act(&table.view(seat), legal);
    let nanos = t0.elapsed().as_nanos();
    if !legal.contains(&decision.action) {
        return Err(format!("{} returned illegal teacher action", policy.name));
    }
    let mut row = decision_json(&table, seat, &decision);
    row["step"] = json!(step);
    row["policy"] = json!(policy.name);
    row["ms"] = json!(nanos as f64 / 1_000_000.0);
    if let Some(recorder) = recorder {
        if let Some(evidence) = recorder.consume(
            &table.view(seat),
            &decision.action,
            table.game.state.phase == GamePhase::Raising,
        )? {
            evidence.attach(&mut row, nanos);
        }
        recorder.ensure_empty()?;
    }
    Ok(json!({
        "decisions": [row], "first_divergence": (decision.action != fixture.actions[step]).then_some(step),
        "outcome": null, "record": null,
    }))
}

fn continuation(
    fixture: &Fixture,
    step: usize,
    candidate: usize,
    policies: &[Policy],
    options: &Options,
    recorder: Option<&AssessmentRecorder>,
) -> Result<Value, String> {
    if let Some(recorder) = recorder {
        recorder.ensure_empty()?;
    }
    let (table, mut observer) = fixture.prefix_observed(step)?;
    let target = target_seat(fixture, step, options);
    let mut players = players(fixture, step, candidate, policies, options);
    let names: [String; 4] = std::array::from_fn(|s| {
        let side = usize::from(s as u64 % 2 != fixture.index % 2);
        let index = if options.replacement != Replacement::Action
            && replaced(s as u8, target, options.replacement, step, step)
        {
            candidate
        } else {
            options.baseline[side].expect("validated baseline")
        };
        policies[index].name.clone()
    });
    let mut rows = Vec::new();
    let mut policy_times = Vec::new();
    let mut capture_error = None;
    let (info, log) = play_table_with(table, &mut players, 128, |table, seat, decision| {
        observer.observe(table);
        let global_step = step + rows.len();
        let side = usize::from(u64::from(seat % 2) != fixture.index % 2);
        let policy = if replaced(seat, target, options.replacement, global_step, step) {
            candidate
        } else {
            options.baseline[side].expect("validated baseline")
        };
        let mut row = decision_json(table, seat, decision);
        row["step"] = json!(global_step);
        row["policy"] = json!(policies[policy].name);
        let mut policy_nanos = None;
        if capture_error.is_none() {
            if let Some(recorder) = recorder {
                match recorder.consume(
                    &table.view(seat),
                    &decision.action,
                    table.game.state.phase == GamePhase::Raising,
                ) {
                    Ok(Some(evidence)) => {
                        policy_nanos = Some(evidence.policy_nanos);
                        evidence.attach(&mut row, 0); // Outer act duration is supplied by the completed eval log.
                    }
                    Ok(None) => {}
                    Err(error) => capture_error = Some(format!("capture at step {global_step}: {error}")),
                }
            }
        }
        policy_times.push(policy_nanos);
        rows.push(row);
    })
    .map_err(|e| e.to_string())?;
    // The existing eval hook cannot return errors. Fail this bounded result
    // before emitting any of its rows; never attach a mismatched queued record.
    if let Some(error) = capture_error {
        return Err(error);
    }
    if let Some(recorder) = recorder {
        recorder.ensure_empty()?;
    }
    for ((row, logged), policy_nanos) in rows.iter_mut().zip(&log).zip(policy_times) {
        row["ms"] = json!(policy_nanos.unwrap_or(logged.nanos) as f64 / 1_000_000.0);
        if let Some(nanos) = policy_nanos {
            row["capture_ms"] = json!(logged.nanos.saturating_sub(nanos) as f64 / 1_000_000.0);
        }
    }
    let first = log
        .iter()
        .enumerate()
        .find_map(|(offset, logged)| {
            (fixture.actions.get(step + offset) != Some(&logged.decision.action)).then_some(step + offset)
        })
        .or_else(|| (step + log.len() != fixture.actions.len()).then_some(step + log.len()));
    let mut notes: Vec<_> = (0..step).map(|_| Note::default()).collect();
    notes.extend(log.iter().map(|l| Note {
        decision: Some(l.decision.clone()),
        ..Note::default()
    }));
    let meta = Meta {
        source: "dryrun",
        id: format!(
            "g{}-s{step}-{}-{:?}",
            fixture.index, policies[candidate].name, options.replacement
        ),
        seed: fixture.source["deal_seed"].as_u64(),
        params_hash: None,
        date: today(),
        players: names,
    };
    Ok(json!({
        "decisions": rows, "first_divergence": first,
        "outcome": metrics_json(&observer.finish(&info)), "record": game_record(&meta, &info, &notes),
        "record_players_note": "persistent factories; action-only override is identified in decisions",
        "candidate_seat": target, "candidate_team": target % 2,
    }))
}

fn emit(writer: &mut impl Write, value: &Value) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, value).map_err(|e| format!("write: {e}"))?;
    writeln!(writer)
        .and_then(|_| writer.flush())
        .map_err(|e| format!("flush: {e}"))
}

fn header(options: &Options, policies: &[Policy], plans: &[(Fixture, Vec<usize>)], recording: bool) -> Value {
    json!({
        "schema": SCHEMA, "type": "start", "mode": format!("{:?}", options.mode),
        "replacement": format!("{:?}", options.replacement), "policy_seed": POLICY_SEED,
        "suite_seed": options.suite.seed, "suite_deals": options.suite.deals,
        "input": options.input, "declared_input_sha256": options.declared_sha256,
        "input_hash_verified_by_harness": false,
        "input_guard": "complete ordered identity scan; selected games legal/outcome replay; length/mtime before/after",
        "frozen_recorded_sources": {"new": "b3abdaf1f5e47111904db442ce36db7dc4ae9e0b", "old": "1632af833199e5253271d493af8531badfac81c0"},
        "factories": policies.iter().map(|p| json!({"name": p.name, "parameters": p.parameters})).collect::<Vec<_>>(),
        "candidate_factories": options.candidates.iter().map(|&i| &policies[i].name).collect::<Vec<_>>(),
        "continuation_factories": {
            "recorded_new": options.baseline[0].map(|i| &policies[i].name),
            "recorded_old": options.baseline[1].map(|i| &policies[i].name),
        },
        "cases": plans.iter().map(|(f, steps)| json!({"index": f.index, "steps": steps})).collect::<Vec<_>>(),
        "agent_state": "fresh_at_prefix; recorded private cache/RNG state unavailable",
        "recording": {"enabled": recording,
            "scope": "only instrumented factories; absence for historical/plain agents is not a completed assessment",
            "evaluation": "same actual act call; checked honest context; serialization after policy timing"},
        "kinds": ["Heuristic", "Heuristic", "Heuristic", "Heuristic"], "max_runs": options.max_runs, "max_continuation_actions": 128,
    })
}

fn execute(
    options: &Options,
    policies: &[Policy],
    plans: &[(Fixture, Vec<usize>)],
    writer: &mut impl Write,
    completed: &mut usize,
    recorder: Option<&AssessmentRecorder>,
) -> Result<(), String> {
    for (fixture, steps) in plans {
        for &step in steps {
            for &candidate in &options.candidates {
                let evaluated = if options.mode == Mode::Teacher {
                    teacher(fixture, step, &policies[candidate], recorder)
                } else {
                    continuation(fixture, step, candidate, policies, options, recorder)
                };
                let mut result = evaluated.map_err(|e| {
                    format!(
                        "game {}, step {step}, candidate {}: {e}",
                        fixture.index, policies[candidate].name
                    )
                })?;
                result["schema"] = json!(SCHEMA);
                result["type"] = json!("result");
                result["index"] = json!(fixture.index);
                result["deal_index"] = fixture.source["deal"].clone();
                result["deal_seed"] = fixture.source["deal_seed"].clone();
                result["hands_turned"] = fixture.source["turned"].clone();
                result["recorded_new_team"] = fixture.source["new_team"].clone();
                result["prefix_step"] = json!(step);
                result["candidate"] = json!(policies[candidate].name);
                result["original_focal_decision"] = fixture.source["decisions"][step].clone();
                result["original_prefix_decisions"] =
                    json!(&fixture.source["decisions"].as_array().expect("verified decisions")[..step]);
                result["original_outcome"] = metrics_json(&fixture.original);
                result["comparison_note"] = json!("after first divergence, step numbers are positional, not equivalent visible states; realized outcomes do not establish choice quality");
                emit(writer, &result)?;
                *completed += 1;
            }
        }
    }
    Ok(())
}

pub fn run_with(policies: Vec<Policy>) -> Result<(), String> {
    run_with_profiling(policies, None)
}

/// Reuse the registry and historical compile-time adapters of the selfplay worker.
/// Output is create-new NDJSON, flushed per result, with an explicit completion footer.
pub fn run_with_profiling(policies: Vec<Policy>, profiling: Option<Profiling>) -> Result<(), String> {
    run_args_with_profiling(std::env::args().skip(1), policies, profiling)
}

/// Optional tools-only recording for current factories. Historical adapters can
/// keep the existing APIs and plain Agent factories unchanged.
pub fn run_with_recording(
    policies: Vec<Policy>,
    profiling: Option<Profiling>,
    recorder: Option<AssessmentRecorder>,
) -> Result<(), String> {
    run_args_with_recording(std::env::args().skip(1), policies, profiling, recorder)
}

/// The same CLI contract without process-global argv, useful to compiled adapters.
pub fn run_args_with_profiling(
    args: impl IntoIterator<Item = String>,
    policies: Vec<Policy>,
    profiling: Option<Profiling>,
) -> Result<(), String> {
    run_args_with_recording(args, policies, profiling, None)
}

/// Matching argv-independent entry point for compiled policy adapters.
pub fn run_args_with_recording(
    args: impl IntoIterator<Item = String>,
    policies: Vec<Policy>,
    profiling: Option<Profiling>,
    recorder: Option<AssessmentRecorder>,
) -> Result<(), String> {
    let cli = Cli::from_args(args, OPTIONS, &[]);
    let options = Options::parse(&cli, &policies)?;
    let input = File::open(&options.input).map_err(|e| format!("input: {e}"))?;
    let before = input.metadata().map_err(|e| e.to_string())?;
    let plans = options::load(&options)?
        .into_iter()
        .map(|f| {
            let steps = options.steps_for(&f)?;
            Ok((f, steps))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let expected = plans.iter().map(|(_, steps)| steps.len()).sum::<usize>() * options.candidates.len();
    if expected == 0 || expected > options.max_runs {
        return Err(format!(
            "selected {expected} comparisons; expected 1..{} (--steps/--phases narrow the selection)",
            options.max_runs
        ));
    }
    if let Some(recorder) = &recorder {
        recorder.ensure_empty()?;
    }
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&options.output)
        .map_err(|e| format!("output: {e}"))?;
    let mut writer = BufWriter::new(output);
    emit(&mut writer, &header(&options, &policies, &plans, recorder.is_some()))?;
    if let Some(profile) = &profiling {
        (profile.reset)();
    }
    let t0 = Instant::now();
    let mut completed = 0;
    let mut result = execute(
        &options,
        &policies,
        &plans,
        &mut writer,
        &mut completed,
        recorder.as_ref(),
    );
    if result.is_ok() && completed != expected {
        result = Err(format!("incomplete comparison count: {completed}/{expected}"));
    }
    // Retain the open input handle through execution; also detect path replacement.
    let unchanged =
        |after: std::fs::Metadata| before.len() == after.len() && before.modified().ok() == after.modified().ok();
    if !input.metadata().is_ok_and(&unchanged) || !std::fs::metadata(&options.input).is_ok_and(unchanged) {
        result = Err("input changed while running; results are not qualified".into());
    }
    emit(
        &mut writer,
        &summary(
            expected,
            completed,
            &result,
            profiling.as_ref(),
            t0.elapsed().as_secs_f64() * 1000.0,
        ),
    )?;
    result
}

fn summary(
    expected: usize,
    completed: usize,
    result: &Result<(), String>,
    profiling: Option<&Profiling>,
    elapsed_ms: f64,
) -> Value {
    json!({
        "schema": SCHEMA, "type": "summary", "status": if result.is_ok() { "complete" } else { "failed" },
        "expected": expected, "completed": completed, "elapsed_execution_ms": elapsed_ms,
        "error": result.as_ref().err(), "profile": profiling.map(|p| (p.snapshot)()),
        "hard_wall_limit": "caller process-tree wrapper; policy calls cannot be preempted here",
    })
}

#[cfg(test)]
#[path = "../../tests/internal/dryrun.rs"]
mod tests;

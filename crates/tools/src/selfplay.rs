//! Four fixed policy slots, whole-deal worker shards, and one complete NDJSON game at a time.

pub mod recording;

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
use heuristic::sequence_concepts::{Actions, Guard, Information, Node, ReadMeaning, Suffix};
use heuristic::sequence_evaluation::{ChoiceScope, Consumed, Selection};
use heuristic::value_concepts::{Assessment, CohortMethod, NodeValue};
use heuristic::{BeliefScope, HeuristicAgent, Params, PlanMode, PlanningAssessment, SequenceTrace};
use serde_json::{json, Value};

use crate::{
    cli::Cli,
    record::{action_json, game_record, today, Meta, Note},
};

pub const SCHEMA: &str = "marjapussi.selfplay/1";

/// Canonical response-solver counters do not measure the policy's total CPU work.
pub const SEQUENCE_SOLVER_EFFORT_MEANING: &str = "Canonical response-solver search nodes: actual_solver_nodes counts attempted callbacks including failures; represented_solver_nodes also charges completed memo hits. These are algorithm-specific effort, not wall time, total CPU, complete policy work or value-graph node counts.";

pub const SEQUENCE_WORK_ACCOUNTING_MEANING: &str = "sibling-tail-dag-v1 shares only one unchanged joint cohort's sibling alternatives resolving through guards or empty legal domains to the same Tail before any engine action. Original alternative order, values and per-edge consumed/interrupted evidence remain separate. Unique tail dependency effort is charged once with optional caches off/cold/warm; memo hits still charge represented dependency effort. This is versioned bounded-work accounting, not general DAG/CPU accounting, probability or concept quality. An absent model in an older capture remains unknown.";

/// Structured recorded stopping reason; this does not reconstruct the full Limits.
pub fn deferred_limit_json(limit: heuristic::sequence_evaluation::DeferredLimit) -> Value {
    use heuristic::sequence_evaluation::DeferredLimit;
    match limit {
        DeferredLimit::Deadline => json!({"kind": "deadline"}),
        DeferredLimit::RepresentedTransitions { required, remaining } =>
            json!({"kind": "represented_transitions", "required": required, "remaining": remaining}),
        DeferredLimit::RepresentedComparisons { required, remaining } =>
            json!({"kind": "represented_comparisons", "required": required, "remaining": remaining}),
        DeferredLimit::RepresentedSolverNodes { required, remaining } =>
            json!({"kind": "represented_solver_nodes", "required": required, "remaining": remaining}),
        DeferredLimit::SolverAccountingOverflow => json!({"kind": "solver_accounting_overflow"}),
        DeferredLimit::LeafUnavailable { actor } => json!({"kind": "leaf_unavailable", "actor": actor}),
        DeferredLimit::FiniteContinuation { max_steps } =>
            json!({"kind": "finite_continuation", "max_steps": max_steps}),
        DeferredLimit::Reconsideration { remaining } => json!({"kind": "reconsideration", "remaining": remaining}),
        DeferredLimit::AuthoredGate => json!({"kind": "authored_gate"}),
    }
}

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
    sequence: Option<SequenceTrace>,
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
            sequence: self.agent.last_sequence().cloned(),
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
/// Runtime refs are scoped to this decision; nested comparisons have distinct namespaces.
pub fn assessment_json(a: &PlanningAssessment, raising: bool) -> Value {
    let mut result = values_json(&a.values, Some(&a.actions), raising, "value");
    let context = &a.context;
    result["schema"] = json!("marjapussi.planning-assessment/1");
    result["value_assessment_present"] = json!(true);
    result["context"] = json!({"observer": context.observer, "event_count": context.events.len(),
        "belief_scope": belief_scope_json(&context.belief_scope),
        "events_debug": format!("{:?}", context.events), "knowledge_debug": format!("{:?}", context.knowledge),
        "kinds": format!("{:?}", context.kinds),
        "series": {"team_totals": context.series.team_totals, "games_left": context.series.games_left},
        "response_params": context.response_params.to_toml(), "response_beliefs_debug": format!("{:?}", context.response_params.beliefs),
        "seed": context.seed, "possible_allocations": context.possible_allocations,
        "requested_worlds": context.requested_worlds, "proposal_draws": context.proposal_draws});
    result["extensions"] = json!({});
    result
}

fn cohort_json(method: CohortMethod) -> Value {
    match method {
        CohortMethod::Enumerated => json!({"method": "enumerated"}),
        CohortMethod::ImportanceSampled { draws, proposal_ess } => {
            json!({"method": "importance_sampled", "draws": draws, "proposal_ess": proposal_ess})
        }
        CohortMethod::Resampled { draws, proposal_ess } => {
            json!({"method": "resampled", "draws": draws, "proposal_ess": proposal_ess})
        }
        CohortMethod::HistorySampled {
            current_draws,
            current_proposal_ess,
            current_resampled,
            model,
            proposals,
            slots,
        } => json!({"method": "conditional_history_sampled", "history_model": model,
            "history_proposals": proposals, "history_slots": slots,
            "current_allocations": {"method": if current_draws == 0 { "enumerated" }
                else if current_resampled { "resampled" } else { "importance_sampled" },
                "draws": current_draws, "proposal_ess": current_proposal_ess}}),
    }
}

fn values_json(v: &Assessment, actions: Option<&[engine::GameAction]>, raising: bool, namespace: &str) -> Value {
    let cohort = cohort_json(v.provenance.cohort);
    let trace: Vec<_> = v
        .trace
        .iter()
        .map(|n| {
            let value = match &n.value {
                NodeValue::Assessment => json!({"kind": "assessment"}),
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
            };
            json!({"ref": n.reference.0, "nodeId": format!("{namespace}/{}", n.reference.0),
            "name": n.name, "meaning": format!("{:?}", n.meaning),
            "operator": format!("{:?}", n.operator),
            "dependencies": n.dependencies.iter().map(|r| r.0).collect::<Vec<_>>(),
            "dependency_nodeIds": n.dependencies.iter().map(|r| format!("{namespace}/{}", r.0)).collect::<Vec<_>>(), "value": value})
        })
        .collect();
    let candidates: Vec<_> = v
        .candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            json!({
                "index": i, "action": actions.and_then(|a| a.get(i)).map(|a| action_json(a, raising)),
                "our_score": c.our_score, "opponent_score": c.opponent_score, "difference": c.difference,
                "declaring": c.declaring, "joint_made": c.joint_made, "made_given_declaring": c.made_given_declaring,
                "refs": {"scores": c.refs.scores.0.0, "our_score": c.refs.our_score.0.0,
                    "opponent_score": c.refs.opponent_score.0.0, "difference": c.refs.difference.0.0,
                    "declaring": c.refs.declaring.0.0, "joint_made": c.refs.joint_made.0.0,
                    "made_given_declaring": c.refs.made_given_declaring.0.0},
            })
        })
        .collect();
    json!({
        "root": v.root.0, "nodeId": format!("{namespace}/{}", v.root.0), "ranking": v.ranking.0.0,
        "selected": v.selected, "alternative": v.alternative, "root_team": v.root_team,
        "provenance": {"belief_model": v.provenance.belief_model, "response_model": v.provenance.response_model,
            "cohort": cohort, "limit": "numerical model integration, not proof or calibrated probability"},
        "definition": {"opponent_weight": v.definition.opponent_weight},
        "cohort_size": v.cohort_size, "cohort_weight": v.cohort_weight, "cohort_complete": v.cohort_complete,
        "evaluated_nodes": v.evaluated_nodes, "memo_hits": v.memo_hits,
        "candidates": candidates, "trace": trace,
    })
}

fn cards_json(cards: engine::CardSet) -> Value {
    json!(cards.iter().collect::<Vec<_>>())
}

fn rights_json(rights: engine::PlayerTrumpPossibilities) -> Value {
    json!(match rights {
        engine::PlayerTrumpPossibilities::Own => "own",
        engine::PlayerTrumpPossibilities::Yours => "yours",
        engine::PlayerTrumpPossibilities::Ours => "ours",
    })
}

fn event_json(event: &observation::SeatEvent) -> Value {
    use observation::SeatEvent::*;
    match event {
        Bid { seat, value } => json!({"kind": "bid", "seat": seat, "value": value}),
        StopBidding { seat } => json!({"kind": "stop_bidding", "seat": seat}),
        Pass { from, to, cards } => json!({"kind": "pass", "from": from, "to": to,
            "cards": cards.map(cards_json)}),
        CardPlayed { seat, card } => json!({"kind": "card_played", "seat": seat, "card": card}),
        AnnounceTrump { seat, suit } => {
            json!({"kind": "announce_trump", "seat": seat, "suit": suit.code().to_string()})
        }
        AskPair { seat } => json!({"kind": "ask_pair", "seat": seat}),
        AskHalf { seat, suit } => json!({"kind": "ask_half", "seat": seat, "suit": suit.code().to_string()}),
        YesPair { seat, suit } => json!({"kind": "yes_pair", "seat": seat, "suit": suit.code().to_string()}),
        NoPair { seat } => json!({"kind": "no_pair", "seat": seat}),
        YesHalf {
            seat,
            suit,
            asker_has_half,
        } => json!({"kind": "yes_half", "seat": seat,
            "suit": suit.code().to_string(), "asker_has_half": asker_has_half}),
        NoHalf { seat, suit } => json!({"kind": "no_half", "seat": seat, "suit": suit.code().to_string()}),
        Undone => json!({"kind": "undone"}),
    }
}

fn information_json(info: &Information) -> Value {
    let k = &info.knowledge;
    json!({"seat": info.seat, "event_count": info.events.len(),
        "events": info.events.iter().map(event_json).collect::<Vec<_>>(),
        "kinds": info.kinds.map(|kind| format!("{kind:?}")),
        "series": {"team_totals": info.series.team_totals, "games_left": info.series.games_left},
        "knowledge": {"observer": k.me(),
            "seats": (0..4).map(|seat| json!({"seat": seat,
                "certain": cards_json(k.certain(seat)), "possible": cards_json(k.possible(seat)),
                "hand_len": k.hand_len(seat), "rights": rights_json(k.rights(seat))})).collect::<Vec<_>>(),
            "played": cards_json(k.played()), "trick": k.trick(),
            "trump": k.trump().map(|s| s.code().to_string()),
            "called": engine::SUITS.into_iter().filter(|&s| k.is_called(s)).map(|s| s.code().to_string()).collect::<Vec<_>>(),
            "constraints": k.constraints().iter().map(|c| json!({"seat": c.seat,
                "cards": cards_json(c.mask), "min": c.lo, "max": c.hi})).collect::<Vec<_>>()}})
}

fn belief_scope_json(scope: &BeliefScope) -> Value {
    match scope {
        BeliefScope::CurrentInformation => json!({"kind": "current_information"}),
        BeliefScope::PreparedConditional { origin } => json!({"kind": "prepared_conditional",
            "origin": information_json(origin),
            "meaning": "original joint model conditioned on modeled observation; not a rebuilt live posterior"}),
    }
}

fn actions_json(actions: &Actions) -> Value {
    match actions {
        Actions::Exact(kind) => json!({"operator": "exact_action_type",
            "action_type": kind}),
        Actions::Raises => json!({"operator": "all_legal_raises"}),
        Actions::Cards(cards) => json!({"operator": "legal_cards", "cards": cards_json(*cards)}),
        Actions::Standing(cards) => json!({"operator": "known_team_winning_legal_cards",
            "cards": cards_json(*cards), "limit": "does not prove this seat keeps the lead"}),
    }
}

fn guard_json(guard: &Guard) -> Value {
    let query = |query: &beliefs::ConceptQuery| {
        let literal = |literal: &beliefs::Literal| {
            use beliefs::Concept;
            let predicate = match literal.concept {
                Concept::Card { seat, card } => json!({"operator": "card", "seat": seat, "card": card}),
                Concept::Void { seat, suit } => {
                    json!({"operator": "void", "seat": seat, "suit": suit.code().to_string()})
                }
                Concept::Pair { seat, suit } => {
                    json!({"operator": "pair", "seat": seat, "suit": suit.code().to_string()})
                }
                Concept::Count { seat, cards, min, max } => json!({"operator": "count", "seat": seat,
                    "cards": cards_json(cards), "min": min, "max": max}),
            };
            json!({"predicate": predicate, "value": literal.value})
        };
        json!({"event": query.event.iter().map(literal).collect::<Vec<_>>(),
            "given": query.given.iter().map(literal).collect::<Vec<_>>()})
    };
    match guard {
        Guard::PlayReady => json!({"operator": "play_ready",
            "meaning": "observer has an actual legal card, question, announcement or answer action"}),
        Guard::OwnLead => json!({"operator": "own_lead"}),
        Guard::Leader(seat) => json!({"operator": "leader", "seat": seat}),
        Guard::Trump(suit) => json!({"operator": "trump", "suit": suit.code().to_string()}),
        Guard::OwnCards(cards) => json!({"operator": "own_cards", "cards": cards_json(*cards)}),
        Guard::Known(q) => json!({"operator": "unconditional_hard_truth", "query": query(q),
            "unknown_is_distinct": true}),
    }
}

// Exact-equality interning is diagnostic only. It preserves shared semantic
// suffixes/information once, without creating a second evaluator or a digest key.
#[derive(Default)]
struct SequenceJson<'a> {
    information: Vec<&'a Information>,
    programs: Vec<&'a Suffix>,
}

impl<'a> SequenceJson<'a> {
    fn information(&mut self, info: &'a Information) -> String {
        let index = self.information.iter().position(|old| *old == info).unwrap_or_else(|| {
            self.information.push(info);
            self.information.len() - 1
        });
        format!("sequence/information/{index}")
    }

    fn program(&mut self, suffix: &'a Suffix) -> Value {
        let index = self.programs.iter().position(|old| *old == suffix).unwrap_or_else(|| {
            self.programs.push(suffix);
            self.programs.len() - 1
        });
        json!({"id": format!("sequence/program/{index}"),
            "nodeId": format!("sequence/program/{index}/{}", suffix.start.0)})
    }

    fn selection(&mut self, selection: &'a Selection) -> Value {
        match selection {
            Selection::Stage(suffix) => json!({"kind": "stage", "program": self.program(suffix)}),
            Selection::Offer { action, next } => json!({"kind": "offer", "action": action_json(action, false),
                "next": self.program(next)}),
        }
    }

    fn consumed(&mut self, read: &'a Consumed) -> Value {
        let information = self.information(&read.information);
        let program = self.program(&read.suffix);
        let reading = match &read.reading {
            ReadMeaning::Guard { predicate, truth } => json!({"kind": "guard", "predicate": guard_json(predicate),
                "truth": truth, "branch": match truth {Some(true) => "yes", Some(false) => "no", None => "unknown"}}),
            ReadMeaning::Binding { actions, legal_members } => {
                json!({"kind": "binding", "actions": actions_json(actions),
                "legal_members": legal_members.iter().map(|a| action_json(a, false)).collect::<Vec<_>>()})
            }
            ReadMeaning::Alternatives(stages) => json!({"kind": "alternatives",
                "alternative_nodeIds": match read.suffix.nodes.get(read.suffix.start.0) {
                    Some(Node::Choice(stages)) => Some(stages.iter().map(|s|
                        format!("{}/{}", program["id"].as_str().unwrap(), s.0)).collect::<Vec<_>>()),
                    _ => None,
                },
                "source_program_stages": stages.iter().map(|s| s.0).collect::<Vec<_>>(),
                "limit": "source stage indices are not remapped suffix node IDs"}),
            ReadMeaning::Response { next } => json!({"kind": "response_transition",
                "next_nodeId": match read.suffix.nodes.get(read.suffix.start.0) {
                    Some(Node::Response { next }) => Some(format!("{}/{}", program["id"].as_str().unwrap(), next.0)),
                    _ => None,
                },
                "next_source_stage": next.0,
                "limit": "source stage index is not a remapped suffix node ID; one modeled response, not a terminal tail"}),
            ReadMeaning::Deferred => json!({"kind": "deferred",
                "meaning": "authored bound exhausted; no response tail or completed value implied"}),
            ReadMeaning::ResponseLeaf => json!({"kind": "response_leaf"}),
            ReadMeaning::Reconsider { remaining, retained } => json!({
                "kind": "reconsider", "remaining": remaining,
                "retained_source_stage": retained.0,
            }),
        };
        json!({"information": information, "program": program, "nodeId": program["nodeId"], "reading": reading})
    }

    fn programs_json(&self) -> Vec<Value> {
        self.programs.iter().enumerate().map(|(i, program)| {
            let id = |stage: usize| format!("sequence/program/{i}/{stage}");
            let nodes: Vec<_> = program.nodes.iter().enumerate().map(|(stage, node)| {
                let definition = match node {
                    Node::Action { actions, next } => json!({"operator": "action_binding",
                        "actions": actions_json(actions), "next": id(next.0)}),
                    Node::Guard { predicate, yes, no, unknown } => json!({"operator": "guard",
                        "predicate": guard_json(predicate), "yes": id(yes.0), "no": id(no.0), "unknown": id(unknown.0)}),
                    Node::Choice(alternatives) => json!({"operator": "value_comparison",
                        "alternatives": alternatives.iter().map(|s| id(s.0)).collect::<Vec<_>>()}),
                    Node::Response { next } => json!({"operator": "response_transition", "next": id(next.0)}),
                    Node::Deferred => json!({"operator": "deferred"}),
                    Node::Tail => json!({"operator": "response_leaf"}),
                    Node::Reconsider { remaining, retained } => json!({
                        "operator": "reconsider", "remaining": remaining, "retained": id(retained.0),
                    }),
                };
                json!({"nodeId": id(stage), "definition": definition})
            }).collect();
            json!({"id": format!("sequence/program/{i}"), "root": id(program.start.0), "nodes": nodes,
                "edge_semantics": "conditional control flow; value dependencies are in assessment.trace"})
        }).collect()
    }
}

fn choice_scope_json(scope: ChoiceScope) -> &'static str {
    match scope {
        ChoiceScope::Parent => "parent",
        ChoiceScope::Global => "global",
        ChoiceScope::Local => "local",
        ChoiceScope::FixedAction => "fixed_action",
        ChoiceScope::Response => "response",
    }
}

// Consistency validation of a recorded authorization, not a second planner or
// a proof of reachability. The immutable evaluator-owned SelectedPlan owns that.
fn sequence_authorization_error(trace: &SequenceTrace) -> Option<&'static str> {
    let Some(authorization) = &trace.authorization else { return None };
    if trace.mode == PlanMode::Deferred || trace.failure.is_some() {
        return Some("deferred trace cannot authorize a selected child");
    }
    let parent = &authorization.parent;
    let Some(values) = &parent.assessment else { return Some("parent assessment is missing") };
    if parent.scope != ChoiceScope::Parent || !values.cohort_complete || parent.support.is_empty()
        || values.candidates.len() != parent.alternatives.len()
        || values.candidates.len() != parent.membership.programs.len()
        || parent.selected != values.selected
        || parent.membership.programs.get(parent.selected) != Some(&parent.semantic_suffix)
    {
        return Some("parent selection is incomplete or inconsistent");
    }
    if authorization.records.len() != trace.choices.len()
        || authorization.records.iter().collect::<std::collections::BTreeSet<_>>().len() != authorization.records.len()
    {
        return Some("authorization record indices do not uniquely align with trace choices");
    }
    for choice in &trace.choices {
        if choice.context != parent.context || choice.information.seat != parent.information.seat
            || choice.selected >= choice.alternatives.len() || choice.support.is_empty()
            || choice.support.iter().any(|pair| !parent.support.contains(pair))
        {
            return Some("authorized child context, selection or original support mismatch");
        }
        match choice.scope {
            ChoiceScope::Parent | ChoiceScope::Global | ChoiceScope::Local => {
                let Some(values) = &choice.assessment else { return Some("ranked child assessment is missing") };
                if !values.cohort_complete || values.candidates.len() != choice.alternatives.len()
                    || values.selected != choice.selected
                {
                    return Some("ranked child assessment is incomplete or inconsistent");
                }
            }
            ChoiceScope::FixedAction | ChoiceScope::Response => {
                if choice.assessment.is_some() || choice.alternatives.len() != 1 || choice.selected != 0 {
                    return Some("fixed child must be a singleton without a value assessment");
                }
            }
        }
    }
    None
}

/// Only serialize the actual trace. Terminal worlds are model hypotheses,
/// observer projections never expose another actor's private reasoning state.
pub fn sequence_json(trace: &SequenceTrace, value_assessment: Option<&PlanningAssessment>) -> Value {
    let mut refs = SequenceJson::default();
    let information = refs.information(&trace.information);
    let program = trace.program.as_ref().map(|p| refs.program(p));
    let scope = match &trace.scope {
        BeliefScope::CurrentInformation => json!({"kind": "current_information"}),
        BeliefScope::PreparedConditional { origin } => json!({"kind": "prepared_conditional",
            "origin": refs.information(origin), "meaning": "original joint model conditioned on modeled observation"}),
    };
    let authorization = trace.authorization.as_ref().map(|authorization| {
        let parent = &authorization.parent;
        let values = parent.assessment.as_ref();
        let selected = values.and_then(|v| v.candidates.get(v.selected));
        json!({"schema": "marjapussi.selection-authorization/1",
            "consistency_error": sequence_authorization_error(trace),
            "parent": {"id": "sequence/authorization/parent", "scope": choice_scope_json(parent.scope),
                "information": refs.information(&parent.information), "program": refs.program(&parent.semantic_suffix),
                "context": {"params": parent.context.params.to_toml(), "seed": parent.context.seed,
                    "posterior_revision": parent.context.posterior_revision,
                    "belief_model": parent.context.provenance.belief_model,
                    "response_model": parent.context.provenance.response_model,
                    "cohort": cohort_json(parent.context.provenance.cohort)},
                "selected": parent.selected, "alternative": values.and_then(|v| v.alternative),
                "candidate_count": values.map(|v| v.candidates.len()),
                "alternatives_count": parent.alternatives.len(),
                "program_count": parent.membership.programs.len(), "legal_count": parent.membership.legal.len(),
                "comparison_programs": parent.membership.programs.iter().map(|p| refs.program(p)).collect::<Vec<_>>(),
                "cohort_complete": values.map(|v| v.cohort_complete),
                "selected_estimate": selected.map(|v| json!({"our_score": v.our_score,
                    "opponent_score": v.opponent_score, "difference": v.difference,
                    "declaring": v.declaring, "joint_made": v.joint_made,
                    "made_given_declaring": v.made_given_declaring})),
                "support": parent.support.iter().map(|(origin, weight)| json!({"origin": origin, "weight": weight})).collect::<Vec<_>>()},
            "certificates": authorization.records.iter().enumerate().map(|(i, record)| json!({
                "choice": format!("sequence/choice/{i}"), "record_index": record})).collect::<Vec<_>>(),
            "meaning": "Evaluator-owned completed parent selection authorizes only its reachable selected-tree certificates. Record indices are provenance, not probabilities.",
            "projection": "Compact parent summary only: no parent terminal arrays, full value trace or future records copied. Parent refs are decision-local. This summary is not an exact equality key: cross-decision interning requires full producer semantic evidence and run/model provenance, not ids or this projection alone."})
    });
    let choices: Vec<_> = trace.choices.iter().enumerate().map(|(i, choice)| {
        let info = refs.information(&choice.information);
        let suffix = refs.program(&choice.semantic_suffix);
        let alternatives: Vec<_> = choice.alternatives.iter().map(|a| refs.selection(a)).collect();
        let consumed: Vec<_> = choice.consumed.iter().map(|r| refs.consumed(r)).collect();
        let programs: Vec<_> = choice.membership.programs.iter().map(|p| refs.program(p)).collect();
        let values = choice.assessment.as_ref().map(|a| values_json(a, None, false, &format!("sequence/choice/{i}/value")));
        json!({"id": format!("sequence/choice/{i}"), "scope": choice_scope_json(choice.scope), "information": info, "program": suffix,
            "context": {"params": choice.context.params.to_toml(), "seed": choice.context.seed,
                "posterior_revision": choice.context.posterior_revision,
                "belief_model": choice.context.provenance.belief_model, "response_model": choice.context.provenance.response_model,
                "cohort": cohort_json(choice.context.provenance.cohort)},
            "membership": {"legal": choice.membership.legal.iter().map(|a| action_json(a, false)).collect::<Vec<_>>(), "programs": programs},
            "alternatives": alternatives, "selected": choice.selected,
            "value_nodeId": values.as_ref().map(|v| v["nodeId"].clone()), "assessment": values,
            "consumed": consumed, "support": choice.support.iter().map(|(origin, weight)| json!({"origin": origin, "weight": weight})).collect::<Vec<_>>(),
            "support_meaning": "unnormalized original cohort mass; not hard impossibility evidence"})
    }).collect();
    let terminals: Vec<_> = trace.terminals.iter().map(|terminal| {
        let tricks: Vec<_> = terminal.tricks.iter().map(|trick| {
            let p = &trick.successor;
            json!({"leader": trick.leader, "winner": trick.winner, "cards": trick.cards, "points": trick.points,
                "information": refs.information(&trick.root),
                "successor": {"leader": p.leader, "trick": p.trick(), "trump": p.trump.map(|s| s.code().to_string()),
                    "called_mask": p.called, "rights": p.rights.map(rights_json),
                    "phase": match p.phase {
                        engine::PlayPhase::Cards => json!({"kind": "cards"}),
                        engine::PlayPhase::Start => json!({"kind": "start"}),
                        engine::PlayPhase::Answer(question) => json!({"kind": "answer", "question": question}),
                    },
                    "tricks_done": p.tricks_done, "team_tricks": p.tricks,
                    "card_points": p.card_points, "pair_points": p.pair_points}})
        }).collect();
        let consumed: Vec<_> = terminal.consumed.iter().map(|r| refs.consumed(r)).collect();
        // A final recorded observer snapshot permits mechanical resource accounting;
        // no hidden hands or replayed strategy are needed. Absence stays explicit.
        let final_info = terminal.tricks.last().map(|t| &t.root);
        let resources = final_info.filter(|info| info.events.starts_with(&trace.information.events)).map(|info| {
            let delta = &info.events[trace.information.events.len()..];
            json!({"from_information": information, "to_information": refs.information(info),
                "played_cards": delta.iter().filter_map(|e| match e {
                    observation::SeatEvent::CardPlayed {seat, card} => Some(json!({"seat": seat, "card": card})), _ => None,
                }).collect::<Vec<_>>(),
                "question_and_pair_events": delta.iter().filter(|e| matches!(e,
                    observation::SeatEvent::AskPair {..} | observation::SeatEvent::AskHalf {..}
                    | observation::SeatEvent::YesPair {..} | observation::SeatEvent::NoPair {..}
                    | observation::SeatEvent::YesHalf {..} | observation::SeatEvent::NoHalf {..}
                    | observation::SeatEvent::AnnounceTrump {..})).map(event_json).collect::<Vec<_>>(),
                "remaining_own_cards": cards_json(info.knowledge.certain(info.seat)),
                "rights_before": (0..4).map(|s| rights_json(trace.information.knowledge.rights(s))).collect::<Vec<_>>(),
                "rights_after": (0..4).map(|s| rights_json(info.knowledge.rights(s))).collect::<Vec<_>>()})
        });
        let o = &terminal.outcome;
        json!({"origin": terminal.origin, "weight": terminal.weight, "interrupted": terminal.interrupted,
            "first": terminal.first.as_ref().map(|offer| json!({"action": action_json(&offer.action, false), "source_program_next_stage": offer.next.0})),
            "outcome": {"playing_team": o.playing_team, "game_value": o.game_value,
                "team_points": o.team_points, "team_tricks": o.team_tricks,
                "card_points": terminal.tricks.last().map(|t| t.successor.card_points),
                "pair_points": terminal.tricks.last().map(|t| t.successor.pair_points),
                "engine_scores": engine::game_scores(o, engine::SCHWARZ_FACTOR_FIFTHS)},
            "tricks": tricks, "consumed": consumed, "consumed_resources": resources,
            "meaning": "conditionally exact engine terminal under a model hypothesis, not a live outcome"})
    }).collect();
    let work = &trace.work;
    json!({"schema": "marjapussi.sequence-trace/1",
        "mode": match trace.mode {PlanMode::Formation => "formation", PlanMode::Repair => "repair",
            PlanMode::Prepared => "prepared", PlanMode::Retry => "retry", PlanMode::Deferred => "deferred"},
        "information": information, "belief_scope": scope, "program": program,
        "value_nodeId": value_assessment.map(|a| format!("value/{}", a.values.root.0)),
        "authorization": authorization, "choices": choices, "terminals": terminals,
        "work": {"accounting": work.accounting(),
            "tail_aliases": work.tail_aliases, "tail_alias_transitions": work.tail_alias_transitions,
            "tail_alias_solver_nodes": work.tail_alias_solver_nodes,
            "transitions": work.transitions, "represented_transitions": work.represented_transitions,
            "comparisons": work.comparisons, "represented_comparisons": work.represented_comparisons, "response_calls": work.response_calls,
            "actual_solver_nodes": work.actual_solver_nodes, "represented_solver_nodes": work.represented_solver_nodes,
            "response_hits": work.response_hits, "suffix_hits": work.suffix_hits,
            "retained_bytes": work.retained_bytes, "completed_candidates": work.completed_candidates,
            "total_candidates": trace.total_candidates,
            "deferred_limit": trace.deferred_limit.map(|limit| format!("{limit:?}")),
            "deferred_limit_detail": trace.deferred_limit.map(deferred_limit_json)},
        "failure": trace.failure.map(|reason| json!({"kind": match reason {
            heuristic::sequence_concepts::SequenceError::InvalidProgram => "invalid_program",
            heuristic::sequence_concepts::SequenceError::HistoryChanged => "history_changed",
            heuristic::sequence_concepts::SequenceError::WrongContext => "wrong_context",
            heuristic::sequence_concepts::SequenceError::IllegalAction => "illegal_action",
            heuristic::sequence_concepts::SequenceError::Budget => "budget",
            heuristic::sequence_concepts::SequenceError::InvalidCohort => "invalid_cohort",
            heuristic::sequence_concepts::SequenceError::InvalidValue => "invalid_value",
        }})),
        "information_sets": refs.information.iter().enumerate().map(|(i, info)| json!({
            "id": format!("sequence/information/{i}"), "value": information_json(info)})).collect::<Vec<_>>(),
        "programs": refs.programs_json(),
        "limits": "only recorded observer projections and evaluated choices; absence is not proof; no live posterior recomputation"})
}

/// One serialization path for benchmark, replay and performance diagnostics.
/// Both inputs must be the results of the same already-completed policy call.
pub fn decision_assessment_json(
    value: Option<&PlanningAssessment>,
    sequence: Option<&SequenceTrace>,
    raising: bool,
) -> Option<Value> {
    let mut assessment = value.map(|a| assessment_json(a, raising));
    if let Some(sequence) = sequence {
        let envelope = assessment.get_or_insert_with(|| {
            json!({
                "schema": "marjapussi.planning-assessment/1", "value_assessment_present": false, "extensions": {},
            })
        });
        envelope["extensions"]["sequence"] = sequence_json(sequence, value);
    }
    assessment
}

fn captured_json(capture: &CapturedDecision, raising: bool) -> Option<Value> {
    let mut result = decision_assessment_json(capture.assessment.as_ref(), capture.sequence.as_ref(), raising)?;
    if capture.sequence.is_some() {
        result["extensions"]["sequence"]["identity"] = json!({
            "seat": capture.seat, "event_count": capture.event_count,
            "action": action_json(&capture.action, raising),
        });
    }
    Some(result)
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
        }) && captured
            .as_ref()
            .and_then(|c| c.sequence.as_ref())
            .is_none_or(|s| s.information.matches(&view));
        let selected_matches = captured
            .as_ref()
            .and_then(|c| c.assessment.as_ref())
            .map(|a| a.actions[a.values.selected] == decision.action);
        let serialization_started = Instant::now();
        let assessment = captured
            .as_ref()
            .and_then(|c| captured_json(c, table.game.state.phase == GamePhase::Raising));
        let serialization_nanos = if assessment.is_some() {
            serialization_started.elapsed().as_nanos()
        } else {
            0
        };
        // Retain one serialized result per decision, not a second raw assessment copy.
        if let Some(c) = &mut captured {
            c.assessment = None;
            c.sequence = None;
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
        "value_assessment_present": assessment.as_ref().is_some_and(|a| a["value_assessment_present"] == true),
        "sequence_present": assessment.as_ref().is_some_and(|a| a["extensions"]["sequence"].is_object()),
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
        assert!(capture.sequence.is_none());
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
                belief_scope: BeliefScope::CurrentInformation,
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
        assert_eq!(row["value_assessment_present"], true);
        assert_eq!(row["context"]["belief_scope"]["kind"], "current_information");
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
        let sequence = SequenceTrace {
            authorization: None,
            mode: PlanMode::Formation,
            information: Information::of(&view),
            scope: BeliefScope::CurrentInformation,
            program: None,
            choices: Vec::new(),
            terminals: Default::default(),
            work: Default::default(),
            failure: None,
            deferred_limit: None,
            total_candidates: None,
        };
        let sequence = sequence_json(&sequence, Some(&assessment));
        assert_eq!(sequence["value_nodeId"], row["nodeId"]);
        assert!(row["trace"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["nodeId"] == sequence["value_nodeId"]));
    }

    // A bounded engine-only fixture builder for serializer tests, not a second policy evaluator.
    fn sequence_fixture() -> (eval::Table, SequenceTrace, engine::GameAction) {
        use heuristic::sequence_concepts::Stage;
        use heuristic::sequence_evaluation::{Context, Membership, PreparedChoice};
        use heuristic::value_concepts::Provenance;
        let mut table = eval::Table::new(&eval::deal(5));
        let bid = *table
            .to_act()
            .unwrap()
            .1
            .iter()
            .find(|a| matches!(a.action_type, engine::ActionType::NewBid(_)))
            .unwrap();
        table.apply(bid);
        for _ in 0..32 {
            if table
                .to_act()
                .unwrap()
                .1
                .iter()
                .any(|a| matches!(a.action_type, engine::ActionType::CardPlayed(_)))
            {
                break;
            }
            let action = table.to_act().unwrap().1[0];
            table.apply(action);
        }
        let (seat, legal) = table.to_act().unwrap();
        let action = *legal
            .iter()
            .find(|a| matches!(a.action_type, engine::ActionType::CardPlayed(_)))
            .unwrap();
        let information = Information::of(&table.view(seat));
        let guard = Guard::Known(beliefs::ConceptQuery::new(beliefs::Concept::Pair {
            seat: engine::partner(seat),
            suit: engine::Suit::Bells,
        }));
        let program = Suffix {
            start: Stage(2),
            nodes: vec![
                Node::Tail,
                Node::Action {
                    actions: Actions::Exact(action.action_type),
                    next: Stage(0),
                },
                Node::Guard {
                    predicate: guard.clone(),
                    yes: Stage(1),
                    no: Stage(0),
                    unknown: Stage(0),
                },
            ],
        };
        let choice = PreparedChoice {
            scope: ChoiceScope::FixedAction,
            information: information.clone(),
            semantic_suffix: program.clone(),
            context: Context {
                params: Params::default(),
                seed: 0,
                posterior_revision: 7,
                provenance: Provenance {
                    belief_model: "serializer-fixture",
                    response_model: "fixture-only",
                    cohort: CohortMethod::Enumerated,
                },
            },
            membership: Membership {
                legal: legal.to_vec(),
                programs: vec![program.clone(), program.clone()],
            },
            alternatives: vec![Selection::Offer {
                action,
                next: Suffix {
                    start: Stage(0),
                    nodes: vec![Node::Tail],
                },
            }],
            selected: 0,
            assessment: None,
            // Explicit stored unknown tests serialization, not this fixture's inferred probability.
            consumed: vec![Consumed {
                information: information.clone(),
                suffix: program.clone(),
                reading: ReadMeaning::Guard {
                    predicate: guard,
                    truth: None,
                },
            }],
            support: vec![(0, 0.123456789012345)],
        };
        let trace = SequenceTrace {
            authorization: None,
            mode: PlanMode::Prepared,
            scope: BeliefScope::PreparedConditional {
                origin: information.clone(),
            },
            information,
            program: Some(program),
            choices: vec![choice],
            terminals: Default::default(),
            work: Default::default(),
            failure: None,
            deferred_limit: None,
            total_candidates: None,
        };
        (table, trace, action)
    }

    // Hand-built serializer fixture only. It does not claim SelectedPlan
    // execution or playing strength; production authorization is evaluator-owned.
    fn authorized_sequence_fixture() -> (agent::Table, SequenceTrace, engine::GameAction) {
        use heuristic::sequence_evaluation::SelectionAuthorization;
        use heuristic::value_concepts::ValueStore;
        let (table, mut trace, action) = sequence_fixture();
        let mut local = trace.choices[0].clone();
        let cohort = CohortMethod::HistorySampled {
            current_draws: 2048, current_proposal_ess: Some(1.25), current_resampled: true,
            model: "synthetic-history-model", proposals: 17, slots: 1,
        };
        local.context.provenance.cohort = cohort;
        let terminals = [(engine::GameOutcome {
            playing_team: Some(0), game_value: 120, team_points: [130, 10], team_tricks: [5, 4],
        }, local.support[0].1)];
        let values = ValueStore::new(&[&terminals], 0, local.context.provenance).unwrap().assess().unwrap();
        let mut parent = local.clone();
        parent.scope = ChoiceScope::Parent;
        parent.membership.programs = vec![parent.semantic_suffix.clone()];
        parent.alternatives = vec![Selection::Stage(parent.semantic_suffix.clone())];
        parent.assessment = Some(values.clone());
        parent.consumed.clear();
        let mut response = local.clone();
        response.scope = ChoiceScope::Response;
        local.scope = ChoiceScope::Local;
        local.alternatives = vec![Selection::Stage(local.semantic_suffix.clone())];
        local.assessment = Some(values);
        response.assessment = None;
        trace.choices = vec![local, response];
        trace.authorization = Some(SelectionAuthorization {
            parent: std::sync::Arc::new(parent), records: vec![3, 8],
        });
        (table, trace, action)
    }

    #[test]
    fn compact_authorization_preserves_scopes_support_and_history_provenance_without_future_tree() {
        let (_, mut trace, _) = authorized_sequence_fixture();
        assert_eq!(sequence_authorization_error(&trace), None);
        let row = sequence_json(&trace, None);
        assert!(row["authorization"]["consistency_error"].is_null());
        assert_eq!(decision_assessment_json(None, Some(&trace), false).unwrap()["value_assessment_present"], false,
            "parent provenance is not a current-call value ranking");
        assert_eq!(row["authorization"]["parent"]["scope"], "parent");
        assert_eq!(row["authorization"]["parent"]["cohort_complete"], true);
        assert_eq!(row["authorization"]["parent"]["candidate_count"], 1);
        assert_eq!(row["authorization"]["certificates"][1]["record_index"], 8);
        assert_eq!(row["choices"][0]["scope"], "local");
        assert_eq!(row["choices"][1]["scope"], "response");
        assert!(row["choices"][1]["assessment"].is_null());
        let parent = &row["authorization"]["parent"];
        assert_eq!(parent["support"][0]["weight"], json!(trace.choices[0].support[0].1));
        assert_eq!(parent["context"]["cohort"]["history_proposals"], 17);
        assert_eq!(parent["context"]["cohort"]["current_allocations"]["draws"], 2048);
        assert_eq!(parent["context"]["cohort"]["current_allocations"]["proposal_ess"], 1.25);
        for field in ["trace", "terminals", "records", "assessment"] {
            assert!(parent.get(field).is_none(), "compact parent must not copy {field}");
        }
        // Exact same information/suffix is interned once, not once per branch.
        assert_eq!(row["information_sets"].as_array().unwrap().len(), 1);
        assert_eq!(parent["program"], row["choices"][0]["program"]);
        trace.mode = PlanMode::Retry;
        let retry = sequence_json(&trace, None);
        assert_eq!(retry["mode"], "retry");
        assert_eq!(retry["authorization"], row["authorization"]);
        assert_eq!(retry["choices"][0]["scope"], "local");
        trace.choices.clear();
        trace.authorization.as_mut().unwrap().records.clear();
        assert_eq!(sequence_authorization_error(&trace), None, "completed parent without children is valid");
    }

    #[test]
    fn authorization_rejects_bad_alignment_incomplete_parent_and_changed_original_weight() {
        let (_, trace, _) = authorized_sequence_fixture();
        let mut wrong = trace.clone();
        wrong.authorization.as_mut().unwrap().records.pop();
        assert!(sequence_authorization_error(&wrong).unwrap().contains("align"));
        wrong = trace.clone();
        std::sync::Arc::make_mut(&mut wrong.authorization.as_mut().unwrap().parent)
            .assessment.as_mut().unwrap().cohort_complete = false;
        assert!(sequence_authorization_error(&wrong).unwrap().contains("incomplete"));
        wrong = trace.clone();
        wrong.choices[0].support[0].1 += 0.1;
        assert!(sequence_authorization_error(&wrong).unwrap().contains("support"));
        wrong = trace.clone();
        wrong.choices[1].assessment = wrong.choices[0].assessment.clone();
        assert!(sequence_authorization_error(&wrong).unwrap().contains("singleton"));
        wrong = trace.clone();
        wrong.mode = PlanMode::Deferred;
        assert!(sequence_authorization_error(&wrong).unwrap().contains("deferred"));
        // Recorder refuses the incorrect receipt before removal/serialization.
        let (table, _, action) = authorized_sequence_fixture();
        let recorder = AssessmentRecorder::default();
        recorder.0.borrow_mut().push_back(CapturedDecision {
            seat: action.player.0, event_count: wrong.information.events.len(), action,
            policy_nanos: 0, assessment: None, sequence: Some(wrong),
        });
        assert!(recorder.consume(&table.view(action.player.0), &action, false).unwrap_err().contains("authorization"));
        assert_eq!(recorder.0.borrow().len(), 1, "mismatch must not attach/remove evidence");
    }

    #[test]
    fn response_read_uses_remapped_suffix_ref_and_keeps_exhaustion_distinct() {
        use heuristic::sequence_concepts::{Program, Stage};
        let (_, trace, _) = sequence_fixture();
        let program = Program {
            name: "serializer-only-response-gate", version: 1, start: Stage(4),
            nodes: vec![
                Node::Tail, Node::Deferred,
                Node::Choice(vec![Stage(0), Stage(1)]), // Unreachable source node, removed from suffix.
                Node::Guard { predicate: Guard::PlayReady, yes: Stage(0), no: Stage(1), unknown: Stage(1) },
                Node::Response { next: Stage(3) },
            ],
        };
        let read = Consumed {
            information: trace.information, suffix: program.suffix(program.start).unwrap(),
            reading: ReadMeaning::Response { next: Stage(3) },
        };
        let mut refs = SequenceJson::default();
        let consumed = refs.consumed(&read);
        let programs = refs.programs_json();
        let nodes = programs[0]["nodes"].as_array().unwrap();
        let response = nodes.iter().find(|n| n["definition"]["operator"] == "response_transition").unwrap();
        let guard = nodes.iter().find(|n| n["definition"]["operator"] == "guard").unwrap();
        assert_eq!(consumed["nodeId"], response["nodeId"]);
        assert_eq!(consumed["reading"]["next_nodeId"], guard["nodeId"]);
        assert_eq!(consumed["reading"]["next_source_stage"], 3);
        assert_eq!(guard["nodeId"], "sequence/program/0/2");
        assert_eq!(guard["definition"]["predicate"]["operator"], "play_ready");
        assert_eq!(response["definition"]["next"], guard["nodeId"]);
        assert_eq!(nodes.iter().filter(|n| n["definition"]["operator"] == "deferred").count(), 1);
        assert_eq!(nodes.iter().filter(|n| n["definition"]["operator"] == "response_leaf").count(), 1);
        let exhausted = Consumed {
            information: read.information.clone(),
            suffix: program.suffix(Stage(1)).unwrap(), reading: ReadMeaning::Deferred,
        };
        assert_eq!(refs.consumed(&exhausted)["reading"]["kind"], "deferred");
    }

    #[test]
    fn exact_preplay_bindings_preserve_stop_and_all_four_pass_cards() {
        let cards = [0, 8, 17, 35].map(engine::Card::from_index);
        let pass = actions_json(&Actions::Exact(engine::ActionType::Pass(cards)));
        assert_eq!(pass["operator"], "exact_action_type");
        assert_eq!(pass["action_type"]["Pass"], json!(cards));
        assert_eq!(pass["action_type"]["Pass"].as_array().unwrap().len(), 4);
        assert_eq!(actions_json(&Actions::Exact(engine::ActionType::StopBidding))["action_type"], "StopBidding");
    }

    #[test]
    fn sequence_without_values_retains_unknown_guard_shared_nodes_scope_and_hidden_pass_privacy() {
        let (_, trace, action) = sequence_fixture();
        let captured = CapturedDecision {
            seat: trace.information.seat,
            event_count: trace.information.events.len(),
            action,
            policy_nanos: 17,
            assessment: None,
            sequence: Some(trace),
        };
        let envelope = captured_json(&captured, false).unwrap();
        assert_eq!(envelope["value_assessment_present"], false);
        assert!(envelope.get("candidates").is_none());
        let sequence = &envelope["extensions"]["sequence"];
        assert_eq!(sequence["mode"], "prepared");
        assert!(sequence["value_nodeId"].is_null());
        assert_eq!(sequence["belief_scope"]["kind"], "prepared_conditional");
        assert_eq!(sequence["information_sets"].as_array().unwrap().len(), 1);
        assert_eq!(sequence["programs"].as_array().unwrap().len(), 2);
        let choice = &sequence["choices"][0];
        assert_eq!(choice["membership"]["programs"][0], choice["membership"]["programs"][1]);
        assert_eq!(choice["support"][0]["weight"], json!(0.123456789012345));
        assert!(choice["consumed"][0]["reading"]["truth"].is_null());
        assert_eq!(choice["consumed"][0]["reading"]["branch"], "unknown");
        assert_eq!(choice["consumed"][0]["nodeId"], sequence["program"]["nodeId"]);
        let nodes = sequence["programs"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["nodes"].as_array().unwrap())
            .collect::<Vec<_>>();
        let ids = nodes
            .iter()
            .map(|n| n["nodeId"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids.len(), nodes.len());
        for n in nodes {
            let definition = &n["definition"];
            for edge in ["next", "yes", "no", "unknown"] {
                if let Some(target) = definition[edge].as_str() {
                    assert!(ids.contains(target));
                }
            }
        }
        let hidden = event_json(&observation::SeatEvent::Pass {
            from: 1,
            to: 3,
            cards: None,
        });
        assert!(hidden["cards"].is_null());
        assert_eq!(
            captured.policy_nanos, 17,
            "serialization does not rewrite the inner act measurement"
        );
    }

    #[test]
    fn solver_effort_serialization_retains_u64_and_typed_deferral_without_cpu_claims() {
        use heuristic::sequence_evaluation::DeferredLimit;
        let (_, mut trace, _) = sequence_fixture();
        trace.work.actual_solver_nodes = 3;
        trace.work.represented_solver_nodes = u64::MAX;
        trace.work.represented_comparisons = 5;
        trace.mode = PlanMode::Deferred;
        trace.deferred_limit = Some(DeferredLimit::RepresentedSolverNodes { required: u64::MAX, remaining: 7 });
        let row = sequence_json(&trace, None);
        assert_eq!(row["work"]["actual_solver_nodes"].as_u64(), Some(3));
        assert_eq!(row["work"]["represented_solver_nodes"].as_u64(), Some(u64::MAX));
        assert_eq!(row["work"]["accounting"], "sibling-tail-dag-v1");
        trace.work.tail_aliases = 3;
        trace.work.tail_alias_transitions = 17;
        trace.work.tail_alias_solver_nodes = u64::MAX;
        let aliases = sequence_json(&trace, None);
        assert_eq!(aliases["work"]["tail_aliases"], 3);
        assert_eq!(aliases["work"]["tail_alias_transitions"], 17);
        assert_eq!(aliases["work"]["tail_alias_solver_nodes"].as_u64(), Some(u64::MAX));
        assert_eq!(aliases["work"]["actual_solver_nodes"], row["work"]["actual_solver_nodes"]);
        assert_eq!(aliases["work"]["represented_solver_nodes"], row["work"]["represented_solver_nodes"]);
        assert_eq!(row["work"]["represented_comparisons"], 5);
        assert_eq!(row["work"]["deferred_limit_detail"]["required"].as_u64(), Some(u64::MAX));
        assert_eq!(row["work"]["deferred_limit_detail"]["remaining"], 7);
        assert!(row["work"].get("solver_node_limit").is_none(), "configured Limits are not in SequenceTrace");
        assert!(row["terminals"].as_array().unwrap().is_empty());
        assert_eq!(deferred_limit_json(DeferredLimit::SolverAccountingOverflow)["kind"], "solver_accounting_overflow");
        assert_eq!(deferred_limit_json(DeferredLimit::AuthoredGate)["kind"], "authored_gate");
        trace.work.actual_solver_nodes = 0;
        trace.deferred_limit = None;
        let row = sequence_json(&trace, None);
        assert_eq!(row["work"]["actual_solver_nodes"], 0);
        assert!(row["work"]["deferred_limit_detail"].is_null());
    }

    #[test]
    fn terminal_serialization_keeps_engine_tricks_points_resources_and_work_without_replanning() {
        use heuristic::sequence_evaluation::{Terminal, TrickOutcome};
        let (mut table, mut trace, action) = sequence_fixture();
        let seat = trace.information.seat;
        let mut tricks = Vec::new();
        for step in 0..96 {
            if table.game.ended() {
                break;
            }
            let (_, legal) = table.to_act().unwrap();
            let chosen = if step == 0 {
                action
            } else {
                *legal
                    .iter()
                    .find(|a| matches!(a.action_type, engine::ActionType::CardPlayed(_)))
                    .unwrap_or(&legal[0])
            };
            assert!(legal.contains(&chosen));
            let before = table.game.state.all_tricks.len();
            table.apply(chosen);
            if table.game.state.all_tricks.len() > before {
                let trick = table.game.state.all_tricks.last().unwrap();
                let (actor, phase) = table
                    .to_act()
                    .map(|(actor, legal)| (actor, engine::PlayPhase::from_actions(legal)))
                    .unwrap_or((seat, engine::PlayPhase::Cards));
                tricks.push(TrickOutcome {
                    leader: trick.leader.0,
                    winner: trick.winner.0,
                    cards: trick.cards.map(|c| c.index()),
                    points: trick.points.0,
                    successor: table.view(seat).knowledge.public_state(actor, phase),
                    root: Information::of(&table.view(seat)),
                });
            }
        }
        assert!(table.game.ended());
        let outcome = engine::GameFinishedInfo::from(table.game.clone()).outcome();
        trace.terminals = std::sync::Arc::new(vec![Terminal {
            origin: 0,
            weight: 1.0,
            outcome,
            tricks,
            consumed: Vec::new(),
            interrupted: false,
            first: None,
            first_program: None,
        }]);
        trace.work.transitions = 36;
        trace.work.represented_transitions = 72;
        trace.work.suffix_hits = 1;
        let row = sequence_json(&trace, None);
        let terminal = &row["terminals"][0];
        assert_eq!(
            terminal["outcome"]["engine_scores"],
            json!(engine::game_scores(&outcome, engine::SCHWARZ_FACTOR_FIFTHS))
        );
        assert_eq!(terminal["tricks"].as_array().unwrap().len(), 9);
        assert_eq!(
            terminal["consumed_resources"]["played_cards"].as_array().unwrap().len(),
            36
        );
        assert_eq!(terminal["consumed_resources"]["remaining_own_cards"], json!([]));
        for (recorded, actual) in terminal["tricks"]
            .as_array()
            .unwrap()
            .iter()
            .zip(&trace.terminals[0].tricks)
        {
            assert_eq!(recorded["winner"], actual.winner);
            assert_eq!(recorded["points"], actual.points);
            assert_eq!(
                recorded["successor"]["card_points"],
                json!(actual.successor.card_points)
            );
            assert_eq!(
                recorded["successor"]["pair_points"],
                json!(actual.successor.pair_points)
            );
        }
        assert_eq!(row["work"]["transitions"], 36);
        assert_eq!(row["work"]["represented_transitions"], 72);
        assert_eq!(row["work"]["suffix_hits"], 1);
    }
}

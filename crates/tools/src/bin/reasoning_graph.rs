//! Export the actual ValueStore trace as the portable review graph schema.
//! Default: one candidate with two weighted synthetic terminals (compact recipe).
//! --compare: include a second synthetic candidate and its complete dependencies.
//! Neither mode executes a policy or claims a live decision/continuation.
//! --capture PATH [--index GAME --step STEP] projects an existing runtime capture.

use std::collections::{HashMap, HashSet};
use std::io::{self, Write};

use engine::GameOutcome;
use heuristic::value_concepts::{
    Assessment, CohortMethod, EvaluatedNode, NodeId, NodeValue, Operator, Provenance, ResultMeaning, ValueStore,
};
use serde_json::{json, Value};

#[path = "reasoning_graph/capture.rs"]
mod capture;

fn node_id(reference: NodeId) -> String {
    format!("value-runtime-{}", reference.0)
}

fn readable(name: &str) -> String {
    let mut label = name.replace('_', " ");
    if let Some(first) = label.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    label
}

fn numerical_value(value: &NodeValue) -> Value {
    match value {
        NodeValue::ConditionalScores(scores) => json!({"conditional_engine_scores": scores}),
        NodeValue::Number(number) => json!({"number": number}),
        NodeValue::Ranking { selected, alternative } => {
            json!({"selected": selected, "alternative": alternative})
        }
        NodeValue::Weights(weights) => json!({"weights": weights}),
        // New input/result types retain their actual evaluator value, rather
        // than acquiring invented semantics from a separate graph catalogue.
        other => json!({"debug": format!("{other:?}")}),
    }
}

fn dependency_label(operator: &Operator, input: NodeId) -> String {
    match operator {
        Operator::Best { values, .. } => values
            .iter()
            .position(|r| r.0 == input)
            .map(|candidate| format!("candidate {candidate} value"))
            .unwrap_or_else(|| "input".into()),
        Operator::BranchDifference {
            scores,
            opponent_weight,
            ..
        } => {
            if scores.0 == input {
                "same-outcome scores".into()
            } else if opponent_weight.0 == input {
                "opponent coefficient".into()
            } else {
                "input".into()
            }
        }
        Operator::ExpectedDifference { values, weights, mass } => {
            if values.0 == input {
                "conditional differences".into()
            } else if weights.0 == input {
                "joint weights".into()
            } else if mass.0 == input {
                "normalizing mass".into()
            } else {
                "input".into()
            }
        }
        Operator::Assessment { ranking, .. } => {
            if ranking.0 == input {
                "selection".into()
            } else {
                "reported estimate".into()
            }
        }
        Operator::Ratio {
            numerator, denominator, ..
        } => {
            if numerator.0 == input {
                "numerator".into()
            } else if denominator.0 == input {
                "denominator".into()
            } else {
                "input".into()
            }
        }
        Operator::ExpectedScore {
            scores, weights, mass, ..
        } => {
            if scores.0 == input {
                "conditional scores".into()
            } else if weights.0 == input {
                "joint weights".into()
            } else if mass.0 == input {
                "normalizing mass".into()
            } else {
                "input".into()
            }
        }
        Operator::EngineScores { outcomes, .. } => {
            if outcomes.0 == input {
                "supplied terminal outcomes".into()
            } else {
                "scoring input".into()
            }
        }
        Operator::EventWeight { outcomes, weights, .. } => {
            if outcomes.0 == input {
                "joint outcome event".into()
            } else if weights.0 == input {
                "joint weights".into()
            } else {
                "event input".into()
            }
        }
        Operator::TotalWeight { .. } => "cohort weights".into(),
        _ => "input".into(),
    }
}

/// Candidate annotations come from evaluated refs and their dependencies; this
/// is instance binding information, not an independent strategic definition.
fn candidate_uses(assessment: &Assessment) -> HashMap<NodeId, Vec<usize>> {
    let definitions: HashMap<_, _> = assessment.trace.iter().map(|node| (node.reference, node)).collect();
    let mut uses: HashMap<NodeId, Vec<usize>> = HashMap::new();
    for (candidate, value) in assessment.candidates.iter().enumerate() {
        let refs = value.refs;
        let mut pending = vec![
            refs.scores.0,
            refs.our_score.0,
            refs.opponent_score.0,
            refs.difference.0,
            refs.declaring.0,
            refs.joint_made.0,
            refs.made_given_declaring.0,
        ];
        let mut seen = HashSet::new();
        while let Some(reference) = pending.pop() {
            if seen.insert(reference) {
                uses.entry(reference).or_default().push(candidate);
                if let Some(node) = definitions.get(&reference) {
                    pending.extend(&node.dependencies);
                }
            }
        }
    }
    uses
}

fn assessment_closure(assessment: &Assessment) -> HashSet<NodeId> {
    let definitions: HashMap<_, _> = assessment.trace.iter().map(|node| (node.reference, node)).collect();
    let mut reachable = HashSet::new();
    let mut pending = vec![assessment.root];
    while let Some(reference) = pending.pop() {
        if reachable.insert(reference) {
            if let Some(node) = definitions.get(&reference) {
                pending.extend(&node.dependencies);
            }
        }
    }
    reachable
}

fn exported_node(node: &EvaluatedNode, uses: &[usize]) -> Value {
    let (kind, epistemic) = if matches!(node.meaning, ResultMeaning::DefinitionConstant) {
        ("fact", "Declared definition coefficient; not evidence about play")
    } else if matches!(node.meaning, ResultMeaning::ConditionalExact) {
        (
            "fact",
            "Conditionally exact under the supplied synthetic terminal assumptions; not a proof about hidden hands",
        )
    } else if matches!(node.meaning, ResultMeaning::ModelEstimate) {
        ("belief", "Numerical model estimate; no calibration or hard-proof claim")
    } else if matches!(node.meaning, ResultMeaning::ModelSelection) {
        (
            "decision",
            "Ordering within the supplied candidate set; not probability of being optimal",
        )
    } else {
        (
            "future",
            "Result meaning supplied by the executable evaluator; inspect its runtime details",
        )
    };
    let mut label = readable(node.name);
    if uses.len() == 1
        && !matches!(
            node.operator,
            Operator::CohortWeights | Operator::TotalWeight { .. } | Operator::Constant(_)
        )
    {
        label.push_str(&format!(" · candidate {}", uses[0]));
    }
    let actual_value = numerical_value(&node.value);
    json!({
        "id": node_id(node.reference), "label": label, "kind": kind,
        "summary": format!("Executed {}. Numerical values are illustrative synthetic-fixture results, not live policy evidence.", node.name),
        "returns": actual_value.to_string(), "operator": format!("{:?}", node.operator),
        "epistemic": epistemic,
        "status": "Implemented evaluator · synthetic fixture",
        "inputs": node.dependencies.iter().map(|input| json!({
            "id": node_id(*input), "label": dependency_label(&node.operator, *input)
        })).collect::<Vec<_>>(),
        "detail": format!("Runtime ref {:?}; result meaning {:?}; used by candidates {:?}. Refs identify this immutable comparison session only. No actor behavior or legal continuation was simulated by this exporter.", node.reference, node.meaning, uses),
        "runtime_ref": node.reference.0, "runtime_value": actual_value,
        "candidate_uses": uses,
    })
}

fn terminal(team: Option<u8>, points: [i32; 2]) -> GameOutcome {
    GameOutcome {
        playing_team: team,
        game_value: 120,
        team_points: points,
        team_tricks: [5, 4],
    }
}

fn synthetic_graph(compare: bool) -> Result<Value, String> {
    let first = [
        (terminal(Some(0), [130, 10]), 0.25),
        (terminal(Some(1), [100, 40]), 0.75),
    ];
    let second = [(terminal(None, [160, 80]), 0.25), (terminal(None, [115, 25]), 0.75)];
    let mut candidates = vec![first.as_slice()];
    if compare {
        candidates.push(second.as_slice());
    }
    let provenance = Provenance {
        belief_model: "illustrative-joint-fixture-not-calibrated",
        response_model: "supplied-terminal-fixture-no-policy-execution",
        cohort: CohortMethod::Enumerated,
    };
    let assessment = ValueStore::new(&candidates, 0, provenance)
        .map_err(|error| format!("value-store input: {error:?}"))?
        .assess()
        .map_err(|error| format!("value-store evaluation: {error:?}"))?;
    let references: HashSet<_> = assessment.trace.iter().map(|node| node.reference).collect();
    if references.len() != assessment.trace.len()
        || !references.contains(&assessment.root)
        || assessment
            .trace
            .iter()
            .flat_map(|node| &node.dependencies)
            .any(|id| !references.contains(id))
    {
        return Err("runtime trace has duplicate refs or missing dependency definitions".into());
    }
    let uses = candidate_uses(&assessment);
    let reachable = assessment_closure(&assessment);
    let export =
        |node: &EvaluatedNode| exported_node(node, uses.get(&node.reference).map(Vec::as_slice).unwrap_or(&[]));
    Ok(json!({
        "schema": 1, "source_revision": "runtime definitions",
        "status": "Actual executable ValueStore trace; illustrative fixture, not a live policy trace",
        "views": [{
            "id": "value-store", "title": "Implemented · Score estimates",
            "root": node_id(assessment.root),
            "nodes": assessment.trace.iter().filter(|node| reachable.contains(&node.reference))
                .map(&export).collect::<Vec<_>>(),
            "diagnostic_nodes": assessment.trace.iter().filter(|node| !reachable.contains(&node.reference))
                .map(&export).collect::<Vec<_>>(),
            "projection": "Actual assessment dependency closure: selection and reported outcome estimates share the same evaluated inputs.",
            "fixture": {
                "illustrative": true, "candidate_count": candidates.len(), "root_team": assessment.root_team,
                "belief_model": provenance.belief_model, "response_model": provenance.response_model,
                "cohort_method": format!("{:?}", provenance.cohort),
                "coverage_limit": "Enumerates only the supplied two-terminal fixture, not all belief-consistent deals",
                "cohort_complete": assessment.cohort_complete,
                "source_boundary": "Supplied weighted GameOutcome terminals; engine::game_scores owns scoring. No holding inference, action selection policy or continuation simulation supplies this fixture.",
                "scoring_rules": {"function": "engine::game_scores", "schwarz_factor_fifths": engine::SCHWARZ_FACTOR_FIFTHS},
                "selected": assessment.selected, "alternative": assessment.alternative,
            }
        }]
    }))
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let graph =
        match args.as_slice() {
            [] => synthetic_graph(false)?,
            [flag] if flag == "--compare" => synthetic_graph(true)?,
            [flag, rest @ ..] if flag == "--capture" => capture::from_args(rest)?,
            _ => return Err(
                "usage: reasoning_graph [--compare] | --capture PATH [--index GAME] [--step STEP] (stdout JSON only)"
                    .into(),
            ),
        };
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    serde_json::to_writer(&mut stdout, &graph).map_err(|error| format!("JSON output: {error}"))?;
    writeln!(stdout)
        .and_then(|_| stdout.flush())
        .map_err(|error| format!("stdout: {error}"))
}

fn main() {
    if let Err(error) = run() {
        eprintln!("reasoning_graph: {error}");
        std::process::exit(1);
    }
}

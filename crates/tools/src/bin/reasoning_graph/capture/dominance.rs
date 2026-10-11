//! Check and project recorded evidence. Never compute an envelope or select a move.

use super::*;

fn number_bits(value: &Value, number: &str, bits: &str) -> Result<u64, String> {
    let number = value[number]
        .as_f64()
        .filter(|n| n.is_finite())
        .ok_or("missing finite bound value")?;
    let encoded = value[bits].as_str().ok_or("missing lossless IEEE bits")?;
    if encoded.len() != 16 {
        return Err("invalid IEEE bit encoding".into());
    }
    let bits = u64::from_str_radix(encoded, 16).map_err(|_| "invalid IEEE bit encoding")?;
    if bits != number.to_bits() {
        return Err("recorded number/IEEE bits mismatch".into());
    }
    Ok(bits)
}

pub(super) fn support_key(pair: &Value) -> Result<(u64, u64), String> {
    let origin = pair["origin"].as_u64().ok_or("missing original support origin")?;
    let weight = pair["weight"]
        .as_f64()
        .filter(|w| w.is_finite() && *w > 0.0)
        .ok_or("invalid original support weight")?;
    let bits = if pair.get("weight_bits").is_some() {
        number_bits(pair, "weight", "weight_bits")?
    } else {
        weight.to_bits() // Legacy complete captures did not carry the redundant bit field.
    };
    Ok((origin, bits))
}

pub(super) fn support_subsequence(child: &[Value], parent: &[Value]) -> Result<bool, String> {
    let child = child.iter().map(support_key).collect::<Result<Vec<_>, _>>()?;
    let parent = parent.iter().map(support_key).collect::<Result<Vec<_>, _>>()?;
    let mut remaining = parent.iter();
    Ok(child.iter().all(|pair| remaining.by_ref().any(|p| p == pair)))
}

fn interval_bits(interval: &Value) -> Result<(u64, u64), String> {
    let lower = number_bits(interval, "lower", "lower_bits")?;
    let upper = number_bits(interval, "upper", "upper_bits")?;
    if f64::from_bits(lower).total_cmp(&f64::from_bits(upper)).is_gt() {
        return Err("reversed completion interval".into());
    }
    Ok((lower, upper))
}

fn producer<'a>(
    nodes: &HashMap<String, &'a Value>,
    namespace: &str,
    reference: &Value,
    family: &str,
    operator: &str,
) -> Result<&'a Value, String> {
    let reference = reference.as_u64().ok_or("missing typed operand reference")?;
    let node = nodes
        .get(&format!("{namespace}/{reference}"))
        .copied()
        .ok_or("missing typed operand")?;
    if node["operator_family"] != family || node["operator"] != operator {
        return Err(format!("operand is not {family} {operator}"));
    }
    Ok(node)
}

fn exact_inputs(node: &Value, expected: &[Value]) -> Result<(), String> {
    if array(node, "dependencies")? != expected {
        return Err("typed arithmetic inputs differ from consumed dependencies".into());
    }
    Ok(())
}

// Follow the actual incumbent and EACH competitor's typed producer chain.
// Validate shared inputs and recorded output shapes, never rerun arithmetic.
fn arithmetic_inputs(proof: &Value, nodes: &HashMap<String, &Value>, namespace: &str) -> Result<(), String> {
    let scope = &proof["scope"];
    let support = array(scope, "support")?;
    let refs = &proof["selected_completed_estimate"]["refs"];
    let expected = producer(nodes, namespace, &refs["difference"], "value", "ExpectedDifference")?;
    let inputs = &expected["bindings"];
    exact_inputs(
        expected,
        &[
            inputs["values_ref"].clone(),
            inputs["weights_ref"].clone(),
            inputs["mass_ref"].clone(),
        ],
    )?;
    let weights = producer(nodes, namespace, &inputs["weights_ref"], "value", "CohortWeights")?;
    exact_inputs(weights, &[])?;
    let actual_weights = array(&weights["value"], "weights")?;
    if actual_weights.len() != support.len()
        || actual_weights
            .iter()
            .zip(support)
            .any(|(w, pair)| w.as_f64().map(f64::to_bits) != support_key(pair).ok().map(|(_, bits)| bits))
    {
        return Err("canonical cohort weights differ from original support bits/order".into());
    }
    let mass = producer(nodes, namespace, &inputs["mass_ref"], "value", "TotalWeight")?;
    exact_inputs(mass, &[inputs["weights_ref"].clone()])?;
    if mass["bindings"]["weights_ref"] != inputs["weights_ref"]
        || !mass["value"]["number"]
            .as_f64()
            .is_some_and(|x| x.is_finite() && x > 0.0)
    {
        return Err("mass is not the incumbent cohort's recorded TotalWeight".into());
    }
    let branches = producer(nodes, namespace, &inputs["values_ref"], "value", "BranchDifference")?;
    let branch = &branches["bindings"];
    exact_inputs(
        branches,
        &[branch["scores_ref"].clone(), branch["opponent_weight_ref"].clone()],
    )?;
    if branch["scores_ref"] != refs["scores"] || array(&branches["value"], "branch_values")?.len() != support.len() {
        return Err("incumbent branch/score support mismatch".into());
    }
    let scores = producer(nodes, namespace, &refs["scores"], "value", "EngineScores")?;
    let outcomes_ref = &scores["bindings"]["outcomes_ref"];
    exact_inputs(scores, std::slice::from_ref(outcomes_ref))?;
    let outcomes = producer(nodes, namespace, outcomes_ref, "value", "ConditionalOutcomes")?;
    exact_inputs(outcomes, &[])?;
    if array(&scores["value"], "conditional_engine_scores")?.len() != support.len()
        || array(&outcomes["value"], "conditional_outcomes")?.len() != support.len()
    {
        return Err("completed incumbent score/outcome count mismatch".into());
    }
    let coefficient = producer(nodes, namespace, &branch["opponent_weight_ref"], "value", "Constant")?;
    exact_inputs(coefficient, &[])?;
    let coefficient_bits = number_bits(&coefficient["bindings"], "number", "number_bits")?;
    if coefficient["value"]["number"].as_f64().map(f64::to_bits) != Some(coefficient_bits)
        || scope["opponent_weight_bits"] != format!("{coefficient_bits:016x}")
        || branch["team"].as_u64().is_none_or(|team| team >= 2)
    {
        return Err("incumbent coefficient/team differs from objective".into());
    }
    for competitor in array(proof, "competitors")? {
        let expected_bound = producer(
            nodes,
            namespace,
            &competitor["bound_ref"],
            "bound",
            "ExpectedDifferenceBounds",
        )?;
        let b = &expected_bound["bindings"];
        if b["weights_ref"] != inputs["weights_ref"] || b["mass_ref"] != inputs["mass_ref"] {
            return Err("bound does not share incumbent weights/mass refs".into());
        }
        let bounded_branches = producer(nodes, namespace, &b["values_ref"], "bound", "BranchDifferenceBounds")?;
        let bb = &bounded_branches["bindings"];
        if bb["team"] != branch["team"]
            || bb["opponent_weight_ref"] != branch["opponent_weight_ref"]
            || bounded_branches["value"]["kind"] != "branch_intervals"
        {
            return Err("bound does not share incumbent team/coefficient".into());
        }
        let branch_slots = array(&bounded_branches["value"], "slots")?;
        if branch_slots.len() != support.len() {
            return Err("bound branch count mismatch".into());
        }
        for slot in branch_slots {
            interval_bits(slot)?;
        }
        let envelope = producer(nodes, namespace, &bb["scores_ref"], "bound", "ScoreIntervals")?;
        let e = &envelope["bindings"];
        if envelope["meaning"] != "ConditionalPublicEnvelope"
            || envelope["value"]["kind"] != "score_intervals"
            || e["weights_ref"] != inputs["weights_ref"]
            || e["slots"] != envelope["value"]["slots"]
            || e["root_team"] != branch["team"]
            || e["opponent_weight_bits"] != scope["opponent_weight_bits"]
        {
            return Err("bound ancestry lacks its matching typed public envelope".into());
        }
        let slots = array(e, "slots")?;
        if slots.len() != support.len() {
            return Err("enclosure score count mismatch".into());
        }
        for slot in slots {
            let teams = slot
                .as_array()
                .filter(|v| v.len() == 2)
                .ok_or("invalid team score enclosure")?;
            for team in teams {
                let lower = team["lower"].as_i64().ok_or("invalid score lower endpoint")?;
                let upper = team["upper"].as_i64().ok_or("invalid score upper endpoint")?;
                if lower > upper {
                    return Err("reversed score enclosure".into());
                }
            }
        }
    }
    Ok(())
}

pub(super) fn validate(choice: &Value) -> Result<(), String> {
    let proof = &choice["completion_dominance"];
    if choice["scope"] != "local"
        || choice["proof_kind"] != "completion_dominance"
        || !choice["assessment"].is_null()
        || !choice["value_nodeId"].is_null()
        || proof["schema"] != "marjapussi.completion-dominance/1"
        || proof["kind"] != "completion_dominance"
    {
        return Err("completion dominance requires a Local proof, never a complete assessment".into());
    }
    for field in ["candidates", "alternative", "runner_up", "cohort_complete"] {
        if proof.get(field).is_some() {
            return Err("completion dominance has fabricated full-domain diagnostics".into());
        }
    }
    let scope = &proof["scope"];
    for field in [
        "information",
        "context",
        "program",
        "membership",
        "alternatives",
        "selected",
        "support",
    ] {
        if scope[field] != choice[field] {
            return Err(format!("completion dominance scope mismatch: {field}"));
        }
    }
    let support = array(scope, "support")?;
    if support.is_empty() {
        return Err("completion dominance original support is empty".into());
    }
    let mut origins = HashSet::new();
    for pair in support {
        if pair.get("weight_bits").is_none() || !origins.insert(support_key(pair)?.0) {
            return Err("completion dominance missing exact or unique original support".into());
        }
    }
    let count = array(choice, "alternatives")?.len();
    let selected = proof["selected"]
        .as_u64()
        .ok_or("completion dominance selected index missing")? as usize;
    if count < 2 || selected >= count || choice["selected"].as_u64() != Some(selected as u64) {
        return Err("completion dominance selected domain mismatch".into());
    }
    let root = identity(proof, "nodeId")?;
    let root_ref = proof["root"]
        .as_u64()
        .ok_or("completion dominance root reference missing")?;
    let root_suffix = format!("/{root_ref}");
    let namespace = root
        .strip_suffix(root_suffix.as_str())
        .ok_or("completion dominance root namespace mismatch")?;
    let trace = array(proof, "trace")?;
    let mut nodes = HashMap::new();
    for n in trace {
        let reference = n["ref"].as_u64().ok_or("missing completion trace reference")?;
        let id = identity(n, "nodeId")?;
        if id != format!("{namespace}/{reference}") || nodes.contains_key(&id) {
            return Err("completion trace reference/arena mismatch".into());
        }
        let dependencies = array(n, "dependencies")?;
        let ids = array(n, "dependency_nodeIds")?;
        if dependencies.len() != ids.len() {
            return Err("completion trace dependency count mismatch".into());
        }
        for (reference, id) in dependencies.iter().zip(ids) {
            let reference = reference.as_u64().ok_or("invalid completion dependency reference")?;
            let id = id.as_str().ok_or("invalid completion dependency nodeId")?;
            if id != format!("{namespace}/{reference}") || !nodes.contains_key(id) {
                return Err("completion dependency missing, reordered or outside this arena".into());
            }
        }
        if n["operator_family"] == "bound" {
            let b = &n["bindings"];
            let bound_dependencies = match n["operator"].as_str() {
                Some("ScoreIntervals") => vec![b["weights_ref"].clone()],
                Some("BranchDifferenceBounds") => vec![b["scores_ref"].clone(), b["opponent_weight_ref"].clone()],
                Some("ExpectedDifferenceBounds") => {
                    vec![b["values_ref"].clone(), b["weights_ref"].clone(), b["mass_ref"].clone()]
                }
                Some("CompletionOrdering") => std::iter::once(b["incumbent_ref"].clone())
                    .chain(array(b, "competitors")?.iter().map(|c| c["bound_ref"].clone()))
                    .collect(),
                _ => return Err("unknown bound operator".into()),
            };
            if bound_dependencies != dependencies {
                return Err("bound operator/consumed dependencies mismatch".into());
            }
        }
        nodes.insert(id, n);
    }
    let root_node = nodes.get(&root).ok_or("missing consumed completion root")?;
    if root_node["operator_family"] != "bound"
        || root_node["operator"] != "CompletionOrdering"
        || root_node["meaning"] != "CompletionOrdering"
        || root_node["value"]["kind"] != "completion_ordering"
        || root_node["value"]["selected"].as_u64() != Some(selected as u64)
        || root_node["bindings"]["selected"].as_u64() != Some(selected as u64)
    {
        return Err("completion ordering root is inconsistent".into());
    }
    let estimate = &proof["selected_completed_estimate"];
    let selected_bits = number_bits(estimate, "difference", "difference_bits")?;
    let value_refs = estimate["refs"].as_object().ok_or("missing selected completed refs")?;
    let ref_ids = estimate["ref_nodeIds"]
        .as_object()
        .ok_or("missing selected completed nodeIds")?;
    let roles = [
        "scores",
        "our_score",
        "opponent_score",
        "difference",
        "declaring",
        "joint_made",
        "made_given_declaring",
    ];
    if value_refs.len() != roles.len()
        || ref_ids.len() != roles.len()
        || roles
            .iter()
            .any(|role| !value_refs.contains_key(*role) || !ref_ids.contains_key(*role))
    {
        return Err("selected completed diagnostic domain mismatch".into());
    }
    for (role, reference) in value_refs {
        let reference = reference.as_u64().ok_or("invalid selected completed reference")?;
        let id = format!("{namespace}/{reference}");
        let Some(n) = nodes.get(&id) else {
            return Err("selected completed dependency missing".into());
        };
        if ref_ids.get(role).and_then(Value::as_str) != Some(id.as_str()) || n["operator_family"] != "value" {
            return Err("selected diagnostic is not its consumed completed value".into());
        }
        if role != "scores" && n["value"]["number"] != estimate[role.as_str()] {
            return Err("selected diagnostic/consumed value mismatch".into());
        }
        if role == "difference"
            && (root_node["bindings"]["incumbent_ref"].as_u64() != Some(reference)
                || n["value"]["number"].as_f64().map(f64::to_bits) != Some(selected_bits))
        {
            return Err("completion incumbent dependency mismatch".into());
        }
    }
    arithmetic_inputs(proof, &nodes, namespace)?;
    let competitors = array(proof, "competitors")?;
    let excluded = array(&root_node["value"], "excluded")?;
    let bound_refs = array(&root_node["bindings"], "competitors")?;
    if competitors.len() + 1 != count || excluded.len() != competitors.len() || bound_refs.len() != competitors.len() {
        return Err("completion dominance does not cover every original competitor".into());
    }
    for (((competitor, excluded), bound_ref), index) in competitors
        .iter()
        .zip(excluded)
        .zip(bound_refs)
        .zip((0..count).filter(|i| *i != selected))
    {
        if competitor["index"].as_u64() != Some(index as u64)
            || excluded["index"] != competitor["index"]
            || bound_ref["index"] != competitor["index"]
            || competitor["status"] != "bounded_unexecuted"
            || bound_ref["bound_ref"] != competitor["bound_ref"]
        {
            return Err("completion competitor order or status mismatch".into());
        }
        if competitor
            .as_object()
            .ok_or("invalid completion competitor")?
            .keys()
            .any(|k| !["index", "status", "bound_ref", "bound_nodeId", "interval"].contains(&k.as_str()))
        {
            return Err("unexecuted competitor carries non-bound diagnostics".into());
        }
        let reference = competitor["bound_ref"]
            .as_u64()
            .ok_or("missing competitor bound reference")?;
        let id = identity(competitor, "bound_nodeId")?;
        if id != format!("{namespace}/{reference}") {
            return Err("competitor bound arena mismatch".into());
        }
        let n = nodes.get(&id).ok_or("missing competitor bound dependency")?;
        let (_, upper) = interval_bits(&competitor["interval"])?;
        if n["operator_family"] != "bound"
            || n["meaning"] != "ComputedModelEnclosure"
            || n["value"]["kind"] != "interval"
            || n["value"]["interval"] != competitor["interval"]
            || number_bits(excluded, "upper", "upper_bits")? != upper
        {
            return Err("competitor bound/evaluated interval mismatch".into());
        }
        // Validate the recorded witness, never recompute scores or choose a move.
        let order = f64::from_bits(upper).total_cmp(&f64::from_bits(selected_bits));
        if order.is_gt() || index < selected && order.is_eq() {
            return Err("completion witness violates original ordered ties".into());
        }
    }
    let envelopes: Vec<_> = trace
        .iter()
        .filter(|n| n["meaning"] == "ConditionalPublicEnvelope")
        .collect();
    if envelopes.is_empty() {
        return Err("completion dominance lacks consumed public envelope evidence".into());
    }
    for n in envelopes {
        let b = &n["bindings"];
        if n["operator_family"] != "bound"
            || n["operator"] != "ScoreIntervals"
            || b["snapshot_matches_information"] != true
            || b["information"] != scope["information"]
            || b["support"] != scope["support"]
            || b["legal"] != scope["membership"]["legal"]
            || b["opponent_weight_bits"] != scope["opponent_weight_bits"]
            || ["belief_model", "response_model", "cohort"]
                .iter()
                .any(|field| b["provenance"][*field] != scope["context"][*field])
        {
            return Err("public enclosure scope or support mismatch".into());
        }
    }
    Ok(())
}

pub(super) fn project(choice: &Value, graph: &mut Graph) -> Result<String, String> {
    validate(choice)?;
    let proof = &choice["completion_dominance"];
    for n in array(proof, "trace")? {
        let id = identity(n, "nodeId")?;
        let (kind, meaning) = match n["meaning"].as_str() {
            Some("DefinitionConstant") => ("rule", "Declared objective coefficient, not evidence about play."),
            Some("ConditionalExact") => ("fact", "Exact arithmetic conditional on supplied terminal outcomes; engine score nodes apply the known scoring rule. This does not prove the incumbent's modeled trajectory."),
            Some("ModelEstimate") => ("belief", "Completed incumbent's joint-model estimate; not certainty about hidden cards or calibrated success."),
            Some("ModelSelection") => ("decision", "Incumbent-only value graph, local index 0. This is not a complete comparison of the original Local domain."),
            Some("ConditionalPublicEnvelope") => ("fact", "Conditional enclosure from the recorded honest public state and forward/no-Undo premises. Endpoints need not be reachable."),
            Some("ComputedModelEnclosure") => ("belief", "Certified enclosure of computed IEEE cohort expectation under the stated objective, ordered support and mass; not an observed score or competitor model estimate."),
            Some("CompletionOrdering") => ("decision", "Completed incumbent dominates the stated competitors' legal terminal enclosures with original tie order. No competitor completion, hidden-card certainty or best-play probability is claimed."),
            Some("Unresolved") => return Err("unresolved node cannot authorize completion dominance".into()),
            other => return Err(format!("unsupported completion meaning {other:?}")),
        };
        let mut inputs = array(n, "dependency_nodeIds")?
            .iter()
            .map(|id| {
                id.as_str()
                    .map(|id| edge(id, "consumed evaluator ref", "evaluated_dependency"))
                    .ok_or_else(|| "invalid completion dependency".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        if n["meaning"] == "ConditionalPublicEnvelope" {
            inputs.push(edge(
                &identity(&n["bindings"], "information")?,
                "consumed public snapshot",
                "captured_field",
            ));
        }
        let mut projected = node(
            &id,
            super::super::readable(n["name"].as_str().ok_or("missing completion node name")?),
            kind,
            n["operator"].as_str().ok_or("missing completion operator")?,
            &n["value"],
            inputs,
            meaning,
            "Captured consumed evaluator result",
        );
        projected["result_meaning"] = n["meaning"].clone();
        projected["operator_family"] = n["operator_family"].clone();
        projected["bindings"] = n["bindings"].clone();
        projected["runtime_ref"] = n["ref"].clone();
        if n["nodeId"] == proof["nodeId"] {
            projected["completion_dominance"] = json!({"scope": proof["scope"], "selected": proof["selected"],
                "selected_completed_estimate": proof["selected_completed_estimate"], "competitors": proof["competitors"],
                "scope_identity": proof["scope_identity"]});
        }
        graph.add(projected)?;
    }
    identity(proof, "nodeId")
}

#[cfg(test)]
#[path = "../../../../tests/fixtures/completion_dominance.rs"]
mod fixture;

#[cfg(test)]
mod tests {
    use super::*;

    fn captured(conditional: bool) -> Value {
        let (trace, assessment) = fixture::fixture(conditional);
        tools::selfplay::decision_assessment_json(assessment.as_ref(), Some(&trace), false).unwrap()
    }

    #[test]
    fn consumed_local_bound_graph_keeps_parent_evidence_and_distinct_meanings() {
        for conditional in [false, true] {
            let input = captured(conditional);
            let sequence = &input["extensions"]["sequence"];
            let choice = array(sequence, "choices")
                .unwrap()
                .iter()
                .find(|c| c["proof_kind"] == "completion_dominance")
                .unwrap();
            let proof = &choice["completion_dominance"];
            validate(choice).unwrap();
            let mut graph = Graph::default();
            super::super::sequence(sequence, &mut graph).unwrap();
            let root = identity(proof, "nodeId").unwrap();
            let selected = identity(&proof["selected_completed_estimate"]["ref_nodeIds"], "difference").unwrap();
            assert_eq!(graph.nodes[&root]["result_meaning"], "CompletionOrdering");
            assert_eq!(graph.nodes[&selected]["result_meaning"], "ModelEstimate");
            assert_eq!(
                graph.nodes["sequence/authorization/parent"]["runtime_value"]["cohort_complete"],
                true
            );
            let id = identity(choice, "id").unwrap();
            assert!(array(&graph.nodes[&id], "inputs")
                .unwrap()
                .iter()
                .any(|edge| edge["relation"] == "selected_parent_authorization"));
            assert!(array(&graph.nodes[&id], "inputs")
                .unwrap()
                .iter()
                .any(|edge| edge["id"] == root && edge["relation"] == "evaluated_dependency"));
            let trace = array(proof, "trace").unwrap();
            let trace_ids: HashSet<_> = trace.iter().map(|n| identity(n, "nodeId").unwrap()).collect();
            assert_eq!(
                trace.len(),
                graph.nodes.keys().filter(|id| trace_ids.contains(*id)).count()
            );
            let bound = identity(&proof["competitors"][0], "bound_nodeId").unwrap();
            assert_eq!(graph.nodes[&bound]["runtime_value"]["kind"], "interval");
            assert!(graph.nodes[&bound]["epistemic"]
                .as_str()
                .unwrap()
                .contains("not an observed score"));
            let exported = super::super::export(&input, None, None).unwrap();
            let work = &exported["views"][0]["operational_details"];
            assert_eq!(
                work["work_accounting_model"],
                "sibling-frontier-dag-v2+completion-dominance-local-v1"
            );
            assert!(work["work_accounting_meaning"]
                .as_str()
                .unwrap()
                .contains("not exhaustive-work parity"));
        }
    }

    #[test]
    fn rejects_forged_bound_domain_scope_indices_bits_and_completed_diagnostics() {
        let original = captured(false);
        let index = array(&original["extensions"]["sequence"], "choices")
            .unwrap()
            .iter()
            .position(|c| c["proof_kind"] == "completion_dominance")
            .unwrap();
        for lane in 0..12 {
            let mut input = original.clone();
            let choice = &mut input["extensions"]["sequence"]["choices"][index];
            match lane {
                0 => choice["scope"] = json!("global"),
                1 => choice["completion_dominance"]["scope"]["support"][0]["weight_bits"] = json!("3ff0000000000000"),
                2 => choice["completion_dominance"]["competitors"][0]["index"] = json!(2),
                3 => {
                    choice["completion_dominance"]["competitors"]
                        .as_array_mut()
                        .unwrap()
                        .pop();
                }
                4 => choice["completion_dominance"]["competitors"][0]["bound_nodeId"] = json!("value/0"),
                5 => choice["completion_dominance"]["scope"]["opponent_weight_bits"] = json!("8000000000000000"),
                6 => choice["completion_dominance"]["competitors"][0]["made_given_declaring"] = json!(1.0),
                7 => choice["assessment"] = json!({"cohort_complete": true}),
                8 => choice["completion_dominance"]["selected_completed_estimate"]["difference"] = json!(139.0),
                9 => choice["completion_dominance"]["scope"]["alternatives"]
                    .as_array_mut()
                    .unwrap()
                    .swap(0, 1),
                10 => {
                    choice["completion_dominance"]["trace"]
                        .as_array_mut()
                        .unwrap()
                        .last_mut()
                        .unwrap()["meaning"] = json!("Unresolved")
                }
                _ => {
                    choice["completion_dominance"]["trace"]
                        .as_array_mut()
                        .unwrap()
                        .last_mut()
                        .unwrap()["dependency_nodeIds"][0] = json!("value/0")
                }
            }
            assert!(
                super::super::export(&input, None, None).is_err(),
                "forged dominance lane {lane}"
            );
        }
        let mut input = captured(true);
        let choices = input["extensions"]["sequence"]["choices"].as_array_mut().unwrap();
        let choice = choices
            .iter_mut()
            .find(|c| c["proof_kind"] == "completion_dominance")
            .unwrap();
        choice["support"][0]["origin"] = json!(0);
        assert!(super::super::export(&input, None, None).is_err());
    }
    fn rewire(node: &mut Value, field: &str, slot: usize, target: &Value) {
        node["bindings"][field] = target["ref"].clone();
        node["dependencies"][slot] = target["ref"].clone();
        node["dependency_nodeIds"][slot] = target["nodeId"].clone();
    }

    #[test]
    fn rejects_wrong_typed_or_unshared_operands_and_detached_envelopes() {
        let original = captured(false);
        let index = array(&original["extensions"]["sequence"], "choices")
            .unwrap()
            .iter()
            .position(|c| c["proof_kind"] == "completion_dominance")
            .unwrap();
        for lane in 0..9 {
            let mut input = original.clone();
            let proof = &mut input["extensions"]["sequence"]["choices"][index]["completion_dominance"];
            let nodes = proof["trace"].as_array_mut().unwrap();
            let position = |op: &str| nodes.iter().position(|n| n["operator"] == op).unwrap();
            let scores = position("EngineScores");
            let weights = position("CohortWeights");
            let envelope = position("ScoreIntervals");
            let branches = position("BranchDifferenceBounds");
            let expected = position("ExpectedDifferenceBounds");
            let mass = position("TotalWeight");
            let coefficient = position("Constant");
            match lane {
                0 => {
                    // Topologically valid, but engine scores are not cohort weights.
                    let target = nodes[scores].clone();
                    rewire(&mut nodes[envelope], "weights_ref", 0, &target);
                }
                1 | 2 => {
                    // Same producer/value type and even same number, wrong identity.
                    let mut target = nodes[if lane == 1 { mass } else { coefficient }].clone();
                    let namespace = target["nodeId"]
                        .as_str()
                        .unwrap()
                        .rsplit_once('/')
                        .unwrap()
                        .0
                        .to_owned();
                    target["ref"] = json!(10000);
                    target["nodeId"] = json!(format!("{namespace}/10000"));
                    if lane == 1 {
                        rewire(&mut nodes[expected], "mass_ref", 2, &target);
                    } else {
                        rewire(&mut nodes[branches], "opponent_weight_ref", 1, &target);
                    }
                    nodes.insert(envelope, target);
                }
                3 => nodes[branches]["bindings"]["team"] = json!(1),
                4 => nodes[envelope]["bindings"]["slots"][0][0]["upper"] = json!(999),
                5 => {
                    nodes[envelope]["bindings"]["slots"].as_array_mut().unwrap().pop();
                }
                6 => nodes[weights]["value"]["weights"][0] = json!(0.125),
                7 => {
                    // A genuine envelope remains elsewhere, but this chain is detached.
                    let target = nodes[scores].clone();
                    rewire(&mut nodes[branches], "scores_ref", 0, &target);
                }
                _ => nodes[envelope]["bindings"]["opponent_weight_bits"] = json!("4000000000000000"),
            }
            assert!(
                super::super::export(&input, None, None).is_err(),
                "typed input corruption {lane}"
            );
        }
    }

    #[test]
    fn conditional_original_support_roundtrips_through_capture_file_parser() {
        let input = captured(true);
        let path = std::env::temp_dir().join(format!("marjapussi-local-dominance-{}.json", std::process::id()));
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        serde_json::to_writer(file, &input).unwrap();
        let result = super::super::from_args(&[path.to_string_lossy().into_owned()]);
        std::fs::remove_file(&path).unwrap();
        let exported = result.unwrap();
        let expected = super::super::export(&input, None, None).unwrap();
        assert_eq!(
            exported, expected,
            "file parser must preserve conditional [2,7] evidence and exact bits"
        );
    }
}

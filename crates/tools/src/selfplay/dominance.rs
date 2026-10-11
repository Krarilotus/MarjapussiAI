//! Serialization of evaluator-owned Local completion dominance; no scoring here.

use super::*;
use heuristic::sequence_evaluation::{CompletionDominance, PreparedChoice};
use heuristic::value_concepts::bounded::{BoundMeaning, BoundOperator, BoundSource, BoundValue, FloatInterval};
use heuristic::value_concepts::{CandidateEstimate, Operator, RuntimeOperator, RuntimeValue};

pub(super) fn support_subsequence(child: &[(usize, f64)], parent: &[(usize, f64)]) -> bool {
    let mut remaining = parent.iter();
    child.iter().all(|(origin, weight)| {
        remaining
            .by_ref()
            .any(|(id, w)| id == origin && w.to_bits() == weight.to_bits())
    })
}

pub(super) fn consistency_error(choice: &PreparedChoice) -> Option<&'static str> {
    let Some(SelectionProof::CompletionDominance(proof)) = &choice.proof else {
        return Some("missing completion dominance");
    };
    let scope = proof.scope();
    let values = proof.values();
    if choice.scope != ChoiceScope::Local
        || scope.information != choice.information
        || scope.context != choice.context
        || scope.semantic_suffix != choice.semantic_suffix
        || scope.membership != choice.membership
        || scope.alternatives != choice.alternatives
        || scope.selected != choice.selected
        || values.selected != choice.selected
        || choice.selected >= choice.alternatives.len()
        || scope.support.is_empty()
        || !scope
            .support
            .iter()
            .copied()
            .eq(choice.support.iter().map(|(i, w)| (*i, w.to_bits())))
        || !values
            .competitors
            .iter()
            .map(|(i, _, _)| *i)
            .eq((0..choice.alternatives.len()).filter(|i| *i != choice.selected))
    {
        return Some("completion dominance scope, domain or original support mismatch");
    }
    let mut consumed = std::collections::HashSet::new();
    for node in &values.trace {
        if node.dependencies.iter().any(|id| !consumed.contains(id)) || !consumed.insert(node.reference) {
            return Some("completion dominance trace dependencies are inconsistent");
        }
    }
    if !consumed.contains(&values.root.0) {
        return Some("completion dominance root missing from consumed trace");
    }
    None
}

fn bits(value: f64) -> String {
    format!("{:016x}", value.to_bits())
}
fn support(values: &[(usize, u64)]) -> Value {
    json!(values
        .iter()
        .map(|(origin, weight)| json!({
            "origin": origin, "weight": f64::from_bits(*weight), "weight_bits": format!("{weight:016x}")
        }))
        .collect::<Vec<_>>())
}
fn interval(value: &FloatInterval) -> Value {
    json!({"lower": value.lower, "upper": value.upper,
        "lower_bits": bits(value.lower), "upper_bits": bits(value.upper)})
}
fn score_intervals(values: &[knowledge::ScoreIntervals]) -> Value {
    json!(values
        .iter()
        .map(|scores| scores.teams.map(|v| json!({"lower": v.lower, "upper": v.upper})))
        .collect::<Vec<_>>())
}
fn estimate(value: &CandidateEstimate, namespace: &str) -> Value {
    let refs = value.refs;
    json!({"our_score": value.our_score, "opponent_score": value.opponent_score,
        "difference": value.difference, "difference_bits": bits(value.difference),
        "declaring": value.declaring, "joint_made": value.joint_made,
        "made_given_declaring": value.made_given_declaring,
        "refs": {"scores": refs.scores.0.0, "our_score": refs.our_score.0.0,
            "opponent_score": refs.opponent_score.0.0, "difference": refs.difference.0.0,
            "declaring": refs.declaring.0.0, "joint_made": refs.joint_made.0.0,
            "made_given_declaring": refs.made_given_declaring.0.0},
        "ref_nodeIds": {"scores": format!("{namespace}/{}", refs.scores.0.0),
            "our_score": format!("{namespace}/{}", refs.our_score.0.0),
            "opponent_score": format!("{namespace}/{}", refs.opponent_score.0.0),
            "difference": format!("{namespace}/{}", refs.difference.0.0),
            "declaring": format!("{namespace}/{}", refs.declaring.0.0),
            "joint_made": format!("{namespace}/{}", refs.joint_made.0.0),
            "made_given_declaring": format!("{namespace}/{}", refs.made_given_declaring.0.0)}})
}

pub(super) fn capture<'a>(proof: &'a CompletionDominance, refs: &mut SequenceJson<'a>, namespace: &str) -> Value {
    let scope = proof.scope();
    let values = proof.values();
    let information = refs.information(&scope.information);
    let program = refs.program(&scope.semantic_suffix);
    let trace: Vec<_> = values.trace.iter().map(|node| {
        let (family, operator, bindings) = match &node.operator {
            RuntimeOperator::Value(operator) => {
                let (kind, mut binding) = match operator {
                    Operator::CohortWeights => ("CohortWeights", json!({})),
                    Operator::ConditionalOutcomes { candidate } => ("ConditionalOutcomes", json!({"candidate": candidate})),
                    Operator::TotalWeight { weights } => ("TotalWeight", json!({"weights_ref": weights.0.0})),
                    Operator::Constant(value) => ("Constant", json!({"number": value, "number_bits": bits(*value)})),
                    Operator::EngineScores { outcomes } => ("EngineScores", json!({"outcomes_ref": outcomes.0.0})),
                    Operator::BranchDifference { scores, team, opponent_weight } => ("BranchDifference",
                        json!({"scores_ref": scores.0.0, "team": team, "opponent_weight_ref": opponent_weight.0.0})),
                    Operator::ExpectedDifference { values, weights, mass } => ("ExpectedDifference",
                        json!({"values_ref": values.0.0, "weights_ref": weights.0.0, "mass_ref": mass.0.0})),
                    Operator::ExpectedScore { scores, weights, team, mass } => ("ExpectedScore",
                        json!({"scores_ref": scores.0.0, "weights_ref": weights.0.0, "team": team, "mass_ref": mass.0.0})),
                    Operator::EventWeight { outcomes, weights, event } => ("EventWeight",
                        json!({"outcomes_ref": outcomes.0.0, "weights_ref": weights.0.0, "event": format!("{event:?}")})),
                    Operator::Ratio { numerator, denominator } => ("Ratio",
                        json!({"numerator_ref": numerator.0.0, "denominator_ref": denominator.0.0})),
                    Operator::Best { values } => ("Best", json!({"values_refs": values.iter().map(|r| r.0.0).collect::<Vec<_>>()})),
                    Operator::Assessment { ranking, diagnostics } => ("Assessment",
                        json!({"ranking_ref": ranking.0.0, "diagnostic_refs": diagnostics.iter().map(|r| r.0.0).collect::<Vec<_>>()})),
                };
                binding["candidate_index_scope"] = json!("incumbent_only");
                binding["original_selected_index"] = json!(values.selected);
                ("value", kind.to_owned(), binding)
            }
            RuntimeOperator::Bound(operator) => {
                let (name, bindings) = match operator {
                    BoundOperator::ScoreIntervals { source, slots, weights } => {
                        let BoundSource::PublicForwardEnvelope { snapshot, remaining_card_budget, future_pair_budget } = source;
                        ("ScoreIntervals", json!({"source": "public_forward_envelope", "information": information,
                            "snapshot_matches_information": snapshot.knowledge == scope.information.knowledge
                                && snapshot.events == scope.information.events,
                            "phase": format!("{:?}", snapshot.phase),
                            "legal": snapshot.legal.iter().map(|a| action_json(a, false)).collect::<Vec<_>>(),
                            "support": support(&snapshot.support), "root_team": snapshot.root_team,
                            "opponent_weight_bits": format!("{:016x}", snapshot.opponent_weight_bits),
                            "provenance": {"belief_model": snapshot.provenance.belief_model,
                                "response_model": snapshot.provenance.response_model, "cohort": cohort_json(snapshot.provenance.cohort)},
                            "remaining_card_budget": remaining_card_budget, "future_pair_budget": future_pair_budget,
                            "slots": score_intervals(slots), "weights_ref": weights.0.0,
                            "premises": "Trusted consistent forward public snapshot and full eligible domain; no Undo or exchange. Boxes enclose legal terminal completions, not reachable endpoints or competitor existence."}))
                    }
                    BoundOperator::BranchDifference { scores, team, opponent_weight } => ("BranchDifferenceBounds",
                        json!({"scores_ref": scores.0.0, "team": team, "opponent_weight_ref": opponent_weight.0.0})),
                    BoundOperator::ExpectedDifference { values, weights, mass } => ("ExpectedDifferenceBounds",
                        json!({"values_ref": values.0.0, "weights_ref": weights.0.0, "mass_ref": mass.0.0})),
                    BoundOperator::CertifiedBest { selected, incumbent, competitors } => ("CompletionOrdering",
                        json!({"selected": selected, "incumbent_ref": incumbent.0.0,
                            "competitors": competitors.iter().map(|(i, r)| json!({"index": i, "bound_ref": r.0.0})).collect::<Vec<_>>()})),
                };
                ("bound", name.into(), bindings)
            }
        };
        let meaning = match &node.meaning {
            BoundMeaning::CompleteValue(meaning) => format!("{meaning:?}"),
            other => format!("{other:?}"),
        };
        let value = match &node.value {
            RuntimeValue::Value(value) => node_value_json(value),
            RuntimeValue::Bound(value) => match value {
                BoundValue::Scores(v) => json!({"kind": "score_intervals", "slots": score_intervals(v)}),
                BoundValue::Branches(v) => json!({"kind": "branch_intervals", "slots": v.iter().map(interval).collect::<Vec<_>>()}),
                BoundValue::Interval(v) => json!({"kind": "interval", "interval": interval(v)}),
                BoundValue::Certified { selected, excluded } => json!({"kind": "completion_ordering", "selected": selected,
                    "excluded": excluded.iter().map(|(i, upper)| json!({"index": i, "upper": upper, "upper_bits": bits(*upper)})).collect::<Vec<_>>()}),
                BoundValue::Unresolved(reason) => json!({"kind": "unresolved", "reason": format!("{reason:?}")}),
            },
        };
        json!({"ref": node.reference.0, "nodeId": format!("{namespace}/{}", node.reference.0),
            "name": node.name, "operator_family": family, "operator": operator, "bindings": bindings,
            "meaning": meaning, "dependencies": node.dependencies.iter().map(|id| id.0).collect::<Vec<_>>(),
            "dependency_nodeIds": node.dependencies.iter().map(|id| format!("{namespace}/{}", id.0)).collect::<Vec<_>>(), "value": value})
    }).collect();
    json!({"schema": "marjapussi.completion-dominance/1", "kind": "completion_dominance",
        "root": values.root.0.0, "nodeId": format!("{namespace}/{}", values.root.0.0),
        "selected": values.selected, "selected_completed_estimate": estimate(&values.selected_value, namespace),
        "scope": {"information": information, "program": program, "selected": scope.selected,
            "context": {"params": scope.context.params.to_toml(), "seed": scope.context.seed,
                "posterior_revision": scope.context.posterior_revision, "belief_model": scope.context.provenance.belief_model,
                "response_model": scope.context.provenance.response_model, "cohort": cohort_json(scope.context.provenance.cohort)},
            "membership": {"legal": scope.membership.legal.iter().map(|a| action_json(a, false)).collect::<Vec<_>>(),
                "programs": scope.membership.programs.iter().map(|p| refs.program(p)).collect::<Vec<_>>()},
            "alternatives": scope.alternatives.iter().map(|a| refs.selection(a)).collect::<Vec<_>>(),
            "support": support(&scope.support), "opponent_weight_bits": format!("{:016x}", scope.opponent_weight_bits)},
        "competitors": values.competitors.iter().map(|(i, reference, bounds)| json!({
            "index": i, "status": "bounded_unexecuted", "bound_ref": reference.0.0,
            "bound_nodeId": format!("{namespace}/{}", reference.0.0), "interval": interval(bounds)
        })).collect::<Vec<_>>(), "trace": trace,
        "meaning": "The completed selected continuation dominates every legal terminal completion enclosed for the stated original competitors and joint support. This certifies Local ordering, not competitor model values, completion, hidden cards or probability of best play.",
        "scope_identity": "Construction-local evidence summary only; not a latent-cohort equality, memo or rebind key. Parent-selected reachability is separate.",
        "value_index_scope": "Public value nodes describe the singleton completed incumbent at local index 0; the selected field maps it to the original Local domain. No full-domain Assessment is present."})
}

#[cfg(test)]
#[path = "../../tests/fixtures/completion_dominance.rs"]
mod fixture;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_local_capture_preserves_bounds_parent_estimates_and_arena_refs() {
        let (trace, assessment) = fixture::fixture(false);
        assert_eq!(sequence_authorization_error(&trace), None);
        let value = decision_assessment_json(assessment.as_ref(), Some(&trace), false).unwrap();
        assert_eq!(value["cohort_complete"], true);
        assert_eq!(value["candidates"].as_array().unwrap().len(), 1);
        let sequence = &value["extensions"]["sequence"];
        assert_eq!(sequence["authorization"]["parent"]["cohort_complete"], true);
        assert_eq!(sequence["authorization"]["consistency_error"], Value::Null);
        let choice = sequence["choices"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["proof_kind"] == "completion_dominance")
            .unwrap();
        assert!(choice["assessment"].is_null());
        assert!(choice["value_nodeId"].is_null());
        let proof = &choice["completion_dominance"];
        assert_eq!(proof["scope"]["alternatives"].as_array().unwrap().len(), 3);
        assert_eq!(proof["scope"]["alternatives"][0], proof["scope"]["alternatives"][2]);
        assert_eq!(proof["selected_completed_estimate"]["difference"], 140.0);
        for (i, competitor) in proof["competitors"].as_array().unwrap().iter().enumerate() {
            assert_eq!(competitor["index"], i + 1);
            assert_eq!(competitor["status"], "bounded_unexecuted");
            assert_eq!(competitor["interval"]["upper"], 140.0);
            for field in [
                "difference",
                "our_score",
                "declaring",
                "joint_made",
                "made_given_declaring",
                "receipt",
            ] {
                assert!(
                    competitor.get(field).is_none(),
                    "unexecuted competitor must not carry {field}"
                );
            }
        }
        assert_eq!(
            proof["competitors"][0]["bound_nodeId"],
            proof["competitors"][1]["bound_nodeId"]
        );
        let nodes = proof["trace"].as_array().unwrap();
        assert!(nodes
            .iter()
            .any(|n| n["operator_family"] == "bound" && n["meaning"] == "ConditionalPublicEnvelope"));
        assert!(nodes.iter().any(|n| n["meaning"] == "CompletionOrdering"));
        let difference = &proof["selected_completed_estimate"]["ref_nodeIds"]["difference"];
        assert!(nodes
            .iter()
            .any(|n| n["nodeId"] == *difference && n["operator_family"] == "value"));
        assert!(nodes.iter().all(|n| !value["trace"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["nodeId"] == n["nodeId"])));
        assert!(proof.get("alternative").is_none());
        assert!(proof.get("cohort_complete").is_none());
    }

    #[test]
    fn actual_conditional_capture_keeps_noncontiguous_origin_bits_and_checks_scope() {
        let (trace, assessment) = fixture::fixture(true);
        assert!(assessment.is_none());
        assert_eq!(sequence_authorization_error(&trace), None);
        let index = trace
            .choices
            .iter()
            .position(|c| matches!(&c.proof, Some(SelectionProof::CompletionDominance(_))))
            .unwrap();
        let row = sequence_json(&trace, None);
        let support = &row["choices"][index]["completion_dominance"]["scope"]["support"];
        assert_eq!(support[0]["origin"], 2);
        assert_eq!(support[1]["origin"], 7);
        assert_eq!(
            support[0]["weight_bits"],
            format!("{:016x}", (3.0_f64 / 36.0).to_bits())
        );
        assert_eq!(
            support[1]["weight_bits"],
            format!("{:016x}", (8.0_f64 / 36.0).to_bits())
        );
        for mutation in 0..4 {
            let mut bad = trace.clone();
            match mutation {
                0 => bad.choices[index].scope = ChoiceScope::Global,
                1 => bad.choices[index].support.reverse(),
                2 => bad.choices[index].context.posterior_revision += 1,
                _ => bad.choices[index].alternatives.pop().map(|_| ()).unwrap(),
            }
            assert!(sequence_authorization_error(&bad).is_some());
        }
    }
}

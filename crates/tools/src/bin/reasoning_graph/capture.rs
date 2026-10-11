//! Projection of serialized runtime data. No policy, belief query or engine replay.
//! Value edges retain evaluator refs; program edges describe definition references.
//! Recorded continuation order is provenance, not a newly evaluated strategic DAG.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use serde_json::{json, Value};

const MAX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_NODES: usize = 4096;
const MAX_DEPTH: usize = 128;

fn array<'a>(value: &'a Value, field: &str) -> Result<&'a [Value], String> {
    value[field]
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("capture missing array: {field}"))
}

fn identity(value: &Value, field: &str) -> Result<String, String> {
    value[field]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("capture missing reference: {field}"))
}

fn edge(id: &str, label: &str, relation: &str) -> Value {
    json!({"id": id, "label": label, "relation": relation})
}

fn node(
    id: &str,
    label: String,
    kind: &str,
    operator: &str,
    value: &Value,
    inputs: Vec<Value>,
    scope: &str,
    status: &str,
) -> Value {
    json!({"id": id, "label": label, "kind": kind,
        "summary": format!("Recorded {operator}. {scope}"),
        "returns": concise(value), "operator": operator, "epistemic": scope,
        "status": status, "inputs": inputs, "runtime_value": value,
        "detail": "Projection of supplied runtime fields and references. The exporter performs no strategy evaluation.",
        "identity_role": "runtime instance"})
}

fn concise(value: &Value) -> String {
    for (field, name) in [
        ("conditional_outcomes", "conditional engine outcomes"),
        ("conditional_engine_scores", "team 0 / team 1 scores"),
        ("weights", "joint cohort weights"),
        ("branch_values", "conditional score differences"),
    ] {
        if let Some(items) = value[field].as_array() {
            return format!("{} {name}; full values in exported JSON", items.len());
        }
    }
    if value["events"].is_array() {
        return format!(
            "Observer {}; {} recorded observations; filtered knowledge snapshot",
            value["seat"], value["event_count"]
        );
    }
    value.to_string()
}

#[derive(Default)]
struct Graph {
    nodes: BTreeMap<String, Value>,
    reads: HashMap<String, String>,
}

impl Graph {
    fn add(&mut self, mut value: Value) -> Result<String, String> {
        let id = identity(&value, "id")?;
        // One drawn arrow per shared input; retain all authored branch roles
        // (e.g. yes/no/unknown may intentionally reference the same suffix).
        let inputs = value["inputs"].as_array_mut().ok_or("invalid graph inputs")?;
        let mut unique: Vec<Value> = Vec::new();
        for input in std::mem::take(inputs) {
            if let Some(previous) = unique.iter_mut().find(|e| e["id"] == input["id"]) {
                if previous["label"] != input["label"] {
                    previous["label"] = json!(format!(
                        "{}; {}",
                        previous["label"].as_str().unwrap_or(""),
                        input["label"].as_str().unwrap_or("")
                    ));
                }
            } else {
                unique.push(input);
            }
        }
        *inputs = unique;
        if self.nodes.len() >= MAX_NODES {
            return Err("capture graph exceeds 4096 nodes; select a smaller capture".into());
        }
        if self.nodes.insert(id.clone(), value).is_some() {
            return Err(format!("duplicate runtime reference {id}"));
        }
        Ok(id)
    }

    fn link(&mut self, consumer: &str, input: &str, label: &str, relation: &str) -> Result<(), String> {
        let consumer = self
            .nodes
            .get_mut(consumer)
            .ok_or_else(|| format!("missing consumer {consumer}"))?;
        let inputs = consumer["inputs"].as_array_mut().ok_or("invalid graph inputs")?;
        if !inputs.iter().any(|e| e["id"] == input) {
            inputs.push(edge(input, label, relation));
        }
        Ok(())
    }

    fn collection(
        &mut self,
        id: &str,
        label: &str,
        values: Vec<String>,
        operator: &str,
    ) -> Result<Option<String>, String> {
        if values.is_empty() {
            return Ok(None);
        }
        let inputs = values
            .iter()
            .map(|id| edge(id, "recorded member", "captured_field"))
            .collect();
        self.add(node(
            id,
            label.into(),
            "future",
            operator,
            &json!({"recorded_members": values.len()}),
            inputs,
            "Recorded conditional-model evidence collection; not another evaluator or a new quality measure.",
            "Captured runtime evidence",
        ))
        .map(Some)
    }

    fn visit(&self, root: &str, done: &mut HashSet<String>) -> Result<(), String> {
        fn visit(
            graph: &Graph,
            id: &str,
            active: &mut HashSet<String>,
            done: &mut HashSet<String>,
        ) -> Result<(), String> {
            if active.contains(id) {
                return Err(format!("captured dependency cycle at {id}"));
            }
            if done.contains(id) {
                return Ok(());
            }
            if active.len() >= MAX_DEPTH {
                return Err("capture dependency depth exceeds 128".into());
            }
            let node = graph
                .nodes
                .get(id)
                .ok_or_else(|| format!("missing captured dependency {id}"))?;
            active.insert(id.into());
            for input in array(node, "inputs")? {
                visit(graph, &identity(input, "id")?, active, done)?;
            }
            active.remove(id);
            done.insert(id.into());
            Ok(())
        }
        visit(self, root, &mut HashSet::new(), done)
    }

    fn reachable(&self, root: &str) -> Result<HashSet<String>, String> {
        let mut done = HashSet::new();
        self.visit(root, &mut done)?;
        Ok(done)
    }
}

fn authored_targets(definition: &Value) -> Result<Vec<(String, String)>, String> {
    let fields: &[&str] = match definition["operator"].as_str() {
        Some("action_binding") => &["next"],
        Some("response_transition") => &["next"],
        Some("reconsider") => &["retained"],
        Some("guard") => &["yes", "no", "unknown"],
        Some("value_comparison") => &[],
        Some("response_leaf" | "deferred") => &[],
        other => {
            return Err(format!(
                "unsupported authored operator {other:?}; no replacement definition invented"
            ))
        }
    };
    let mut refs = fields
        .iter()
        .map(|field| Ok((identity(definition, field)?, (*field).into())))
        .collect::<Result<Vec<_>, String>>()?;
    if definition["operator"] == "value_comparison" {
        for (i, target) in array(definition, "alternatives")?.iter().enumerate() {
            refs.push((
                target.as_str().ok_or("invalid authored alternative")?.into(),
                format!("alternative {i}"),
            ));
        }
    }
    Ok(refs)
}

// Canonicalize entire typed definitions with canonical child refs, not labels or
// winners. Runtime bindings remain separate. First source ID is the shared ID.
fn definitions(sequence: &Value, graph: &mut Graph) -> Result<HashMap<String, String>, String> {
    let mut source = BTreeMap::new();
    for program in array(sequence, "programs")? {
        for n in array(program, "nodes")? {
            let id = identity(n, "nodeId")?;
            if source.insert(id.clone(), n["definition"].clone()).is_some() {
                return Err(format!("duplicate authored ref {id}"));
            }
        }
    }
    if source.len() > MAX_NODES {
        return Err("too many authored definitions".into());
    }
    fn canonical(
        id: &str,
        source: &BTreeMap<String, Value>,
        aliases: &mut HashMap<String, String>,
        interned: &mut HashMap<String, String>,
        active: &mut HashSet<String>,
        graph: &mut Graph,
    ) -> Result<String, String> {
        if let Some(found) = aliases.get(id) {
            return Ok(found.clone());
        }
        if active.len() >= MAX_DEPTH {
            return Err("authored definition depth exceeds 128".into());
        }
        if !active.insert(id.into()) {
            return Err(format!("authored definition cycle at {id}"));
        }
        let original = source
            .get(id)
            .ok_or_else(|| format!("missing authored definition {id}"))?;
        let mut definition = original.clone();
        let mut inputs = Vec::new();
        for (target, role) in authored_targets(original)? {
            let shared = canonical(&target, source, aliases, interned, active, graph)?;
            if let Some(index) = role.strip_prefix("alternative ") {
                definition["alternatives"][index.parse::<usize>().map_err(|_| "invalid alternative ordinal")?] =
                    json!(shared);
            } else {
                definition[role.as_str()] = json!(shared);
            }
            inputs.push(edge(
                &shared,
                &format!("definition reference: {role} (not execution)"),
                "authored_definition_reference",
            ));
        }
        let key = definition.to_string();
        let shared = if let Some(shared) = interned.get(&key) {
            shared.clone()
        } else {
            let label = match definition["operator"].as_str().unwrap_or("") {
                "guard" => format!("Guard definition: {}", guard_name(&definition["predicate"])),
                "action_binding" => format!("Action definition: {}", actions_name(&definition["actions"])),
                "value_comparison" => "Compare authored continuations".into(),
                "reconsider" => format!("Reconsider continuations · remaining depth {}", definition["remaining"]),
                "response_transition" => "One modeled actor response, then next stage".into(),
                "deferred" => "Defer: authored bound exhausted".into(),
                _ => "Response-policy leaf definition".into(),
            };
            let mut value = node(id, label, "rule", "Authored continuation definition", &definition, inputs,
                "Authored data, not a fact about the deal or evidence that a route executed. Reference arrows describe definition parameters.", "Implemented authored definition");
            value["identity_role"] = json!("authored definition");
            graph.add(value)?;
            interned.insert(key, id.into());
            id.into()
        };
        aliases.insert(id.into(), shared.clone());
        active.remove(id);
        Ok(shared)
    }
    let mut aliases = HashMap::new();
    let mut interned = HashMap::new();
    for id in source.keys() {
        canonical(id, &source, &mut aliases, &mut interned, &mut HashSet::new(), graph)?;
    }
    for (source, shared) in &aliases {
        let node = graph.nodes.get_mut(shared).ok_or("missing canonical definition")?;
        if node["source_refs"].is_null() {
            node["source_refs"] = json!([]);
        }
        node["source_refs"].as_array_mut().unwrap().push(json!(source));
    }
    for n in graph.nodes.values_mut() {
        if let Some(refs) = n["source_refs"].as_array_mut() {
            refs.sort_by_key(Value::to_string);
        }
    }
    Ok(aliases)
}

fn guard_name(predicate: &Value) -> String {
    match predicate["operator"].as_str().unwrap_or("") {
        "play_ready" => "Observer has an actual legal play action".into(),
        "own_lead" => "Our seat has the lead".into(),
        "leader" => format!("Seat {} has the lead", predicate["seat"]),
        "trump" => format!("Trump is {}", predicate["suit"].as_str().unwrap_or("?")),
        "own_cards" => format!("Retain own cards {}", predicate["cards"]),
        "unconditional_hard_truth" => {
            let literals = |field| {
                predicate["query"][field]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|literal| {
                        let p = &literal["predicate"];
                        let condition = match p["operator"].as_str().unwrap_or("") {
                            "pair" => format!("seat {} holds {} pair", p["seat"], p["suit"]),
                            "void" => format!("seat {} is void in {}", p["seat"], p["suit"]),
                            "card" => format!("seat {} holds card {}", p["seat"], p["card"]),
                            "count" => format!("seat {} holds {}..{} of {}", p["seat"], p["min"], p["max"], p["cards"]),
                            _ => p.to_string(),
                        };
                        if literal["value"] == false {
                            format!("not ({condition})")
                        } else {
                            condition
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" AND ")
            };
            let event = literals("event");
            let given = literals("given");
            format!(
                "Hard query: {}{}",
                if event.is_empty() { "empty event" } else { &event },
                if given.is_empty() {
                    String::new()
                } else {
                    format!(" given {given}")
                }
            )
        }
        other => format!("Recorded {other} predicate"),
    }
}

fn actions_name(actions: &Value) -> String {
    match actions["operator"].as_str().unwrap_or("") {
        "exact_action_type" => {
            let action = &actions["action_type"];
            if action == "StopBidding" {
                "Stop bidding".into()
            } else if let Some(cards) = action.get("Pass") {
                format!("Pass four cards {cards}")
            } else {
                action.to_string()
            }
        }
        "legal_cards" => "Bind legal cards".into(),
        "all_legal_raises" => "Bind legal raises".into(),
        "known_team_winning_legal_cards" => "Bind known team-winning leads".into(),
        other => format!("Recorded {other} binding"),
    }
}

fn values(assessment: &Value, graph: &mut Graph) -> Result<String, String> {
    let root = identity(assessment, "nodeId")?;
    for n in array(assessment, "trace")? {
        let id = identity(n, "nodeId")?;
        let (kind, scope) = match n["meaning"].as_str() {
            Some("DefinitionConstant") => ("rule", "Declared objective coefficient, not evidence about play."),
            Some("ConditionalExact") => ("fact", "Conditionally exact arithmetic under the supplied model terminal assumptions; not actual hidden-hand certainty."),
            Some("ModelEstimate") => ("belief", "Numerical joint-model estimate in the recorded information scope; no calibration claim."),
            Some("ModelSelection") => ("decision", "Ordering within this recorded candidate set; not probability of optimality or proof of success."),
            other => return Err(format!("unsupported runtime value meaning {other:?}; inspect the raw capture")),
        };
        let inputs = array(n, "dependency_nodeIds")?
            .iter()
            .map(|r| {
                r.as_str()
                    .map(|id| edge(id, "consumed evaluator ref", "evaluated_dependency"))
                    .ok_or_else(|| "invalid value dependency reference".into())
            })
            .collect::<Result<Vec<_>, String>>()?;
        let name = n["name"].as_str().ok_or("missing runtime node name")?;
        let operator = n["operator"].as_str().ok_or("missing recorded runtime operator")?;
        let mut value = node(
            &id,
            super::readable(name),
            kind,
            operator,
            &n["value"],
            inputs,
            scope,
            "Captured retained evaluator result",
        );
        value["result_meaning"] = n["meaning"].clone();
        value["runtime_ref"] = n["ref"].clone();
        let uses: Vec<_> = assessment["candidates"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|candidate| {
                let roles: Vec<_> = candidate["refs"]
                    .as_object()?
                    .iter()
                    .filter(|(_, r)| *r == &n["ref"])
                    .map(|(role, _)| role.as_str())
                    .collect();
                if roles.is_empty() {
                    None
                } else {
                    Some(json!({"candidate": candidate["index"], "action": candidate["action"], "roles": roles}))
                }
            })
            .collect();
        value["candidate_uses"] = json!(uses);
        value["bindings"] = json!({"model": assessment["provenance"].to_string(),
            "information_scope": assessment["context"]["belief_scope"].to_string()});
        if id == root {
            value["assessment_metadata"] = json!({"selected": assessment["selected"], "alternative": assessment["alternative"],
                "candidates": assessment["candidates"], "cohort_complete": assessment["cohort_complete"],
                "cohort_size": assessment["cohort_size"], "provenance": assessment["provenance"], "context": assessment["context"]});
            value["returns"] = json!(format!(
                "Recorded candidate {} selected; alternative {}; {}",
                assessment["selected"],
                assessment["alternative"],
                concise(&n["value"])
            ));
        }
        graph.add(value)?;
    }
    if !graph.nodes.contains_key(&root) {
        return Err(format!("missing value assessment root {root}"));
    }
    Ok(root)
}

fn read(read: &Value, aliases: &HashMap<String, String>, graph: &mut Graph) -> Result<String, String> {
    let source = identity(read, "nodeId")?;
    let definition = aliases
        .get(&source)
        .ok_or_else(|| format!("missing read definition {source}"))?;
    let info = identity(read, "information")?;
    // Only complete equality of recorded binding + context + reading shares a
    // query. Equal labels/winners never merge different conditional outcomes.
    let key = json!([definition, info, read["reading"]]).to_string();
    if let Some(id) = graph.reads.get(&key) {
        return Ok(id.clone());
    }
    let reading = &read["reading"];
    let (label, kind) = match reading["kind"].as_str() {
        Some("guard") => (
            format!(
                "{}: {}",
                guard_name(&reading["predicate"]),
                match reading["truth"].as_bool() {
                    Some(true) => "known true",
                    Some(false) => "known false",
                    None => "unknown",
                }
            ),
            if reading["truth"].is_boolean() { "fact" } else { "rule" },
        ),
        Some("binding") => (format!("Legal binding: {}", actions_name(&reading["actions"])), "fact"),
        Some("alternatives") => ("Available continuation branches".into(), "rule"),
        Some("response_leaf") => ("Consumed response-policy boundary".into(), "future"),
        Some("response_transition") => ("Consumed one-step modeled response boundary".into(), "future"),
        Some("deferred") => (
            "Authored bound exhausted · no completed continuation value".into(),
            "rule",
        ),
        Some("reconsider") => (
            format!(
                "Reconsider legal continuations · remaining depth {}",
                reading["remaining"]
            ),
            "future",
        ),
        other => return Err(format!("unsupported consumed reading {other:?}")),
    };
    if reading["kind"] == "response_transition" {
        let next = identity(reading, "next_nodeId")?;
        let shared = aliases
            .get(&next)
            .ok_or_else(|| format!("missing response continuation {next}"))?;
        if graph.nodes[definition]["runtime_value"]["operator"] != "response_transition"
            || graph.nodes[definition]["runtime_value"]["next"].as_str() != Some(shared.as_str())
        {
            return Err("consumed response continuation does not match its authored definition".into());
        }
    }
    let id = format!("capture/query/{}", graph.reads.len());
    let mut value = node(&id, label, kind, "Consumed runtime query", reading,
        vec![edge(definition, "queried authored definition", "definition_binding"), edge(&info, "observer information binding", "captured_field")],
        "Reading in this precise observer projection. Hypothetical hard truth is not promoted to actual current knowledge; unknown is not false or a probability.", "Captured runtime reading");
    value["source_nodeId"] = json!(source);
    graph.add(value)?;
    graph.reads.insert(key, id.clone());
    Ok(id)
}

fn program_ref(value: &Value, aliases: &HashMap<String, String>) -> Result<String, String> {
    let id = identity(value, "nodeId")?;
    aliases
        .get(&id)
        .cloned()
        .ok_or_else(|| format!("unresolved authored reference {id}"))
}

// Authorization is provenance, not a score dependency or a reconstructed proof
// that the selected route is globally optimal.
fn authorization(sequence: &Value, aliases: &HashMap<String, String>, graph: &mut Graph) -> Result<Option<String>, String> {
    let authorization = &sequence["authorization"];
    if authorization.is_null() { return Ok(None); }
    if authorization["schema"] != "marjapussi.selection-authorization/1"
        || !authorization["consistency_error"].is_null()
        || sequence["mode"] == "deferred" || !sequence["failure"].is_null()
    {
        return Err("invalid or deferred selection authorization".into());
    }
    let parent = &authorization["parent"];
    let count = parent["candidate_count"].as_u64().ok_or("authorization parent candidate count missing")?;
    let selected = parent["selected"].as_u64().ok_or("authorization parent selection missing")?;
    if parent["scope"] != "parent" || parent["cohort_complete"] != true
        || count == 0 || selected >= count
        || parent["alternatives_count"].as_u64() != Some(count)
        || parent["program_count"].as_u64() != Some(count)
        || array(parent, "comparison_programs")?.len() as u64 != count
    {
        return Err("authorization parent is not a consistent completed comparison".into());
    }
    let choices = array(sequence, "choices")?;
    let certificates = array(authorization, "certificates")?;
    let support = array(parent, "support")?;
    if support.is_empty() { return Err("authorization original parent support is missing".into()); }
    for program in array(parent, "comparison_programs")? {
        program_ref(program, aliases)?;
    }
    if parent["comparison_programs"][selected as usize] != parent["program"] {
        return Err("authorization selected parent program mismatch".into());
    }
    if certificates.len() != choices.len() { return Err("authorization certificate count mismatch".into()); }
    let mut records = HashSet::new();
    for (certificate, choice) in certificates.iter().zip(choices) {
        let index = certificate["record_index"].as_u64().ok_or("authorization record index missing")?;
        if certificate["choice"] != choice["id"] || !records.insert(index)
            || choice["context"] != parent["context"]
            || array(choice, "support")?.is_empty()
            || !array(choice, "support")?.iter().all(|pair| support.contains(pair))
        {
            return Err("authorization child identity, context or original support mismatch".into());
        }
        let alternatives = array(choice, "alternatives")?;
        let selected = choice["selected"].as_u64().ok_or("authorized child selection missing")?;
        let values = &choice["assessment"];
        match choice["scope"].as_str() {
            Some("parent" | "global" | "local") => {
                if values["cohort_complete"] != true || values["selected"].as_u64() != Some(selected)
                    || array(values, "candidates")?.len() != alternatives.len() || selected as usize >= alternatives.len()
                {
                    return Err("authorized ranked child assessment incomplete or inconsistent".into());
                }
            }
            Some("response" | "fixed_action") => {
                if !values.is_null() || selected != 0 || alternatives.len() != 1 {
                    return Err("authorized fixed child must be a singleton without a value assessment".into());
                }
            }
            _ => return Err("authorized child evidence scope missing or unsupported".into()),
        }
    }
    let id = identity(parent, "id")?;
    graph.add(node(&id, "Completed parent selection".into(), "rule", "SelectedPlan authorization projection",
        parent, vec![edge(&identity(parent, "information")?, "original honest information", "authorization_provenance"),
            edge(&program_ref(&parent["program"], aliases)?, "parent-selected authored candidate", "definition_binding")],
        "Completed ranking of original nominated parent programs under its named joint model. Reachable certificates authorize reuse; this is not a fresh live-posterior ranking or proof of optimality.",
        "Compact completed-parent provenance; full original comparison omitted"))?;
    for certificate in certificates {
        let choice_id = identity(certificate, "choice")?;
        graph.link(&choice_id, &id, "reachable certificate in selected parent tree", "selected_parent_authorization")?;
        graph.nodes.get_mut(&choice_id).ok_or("missing authorized choice")?["authorization_record_index"] = certificate["record_index"].clone();
    }
    Ok(Some(id))
}

fn sequence(sequence: &Value, graph: &mut Graph) -> Result<Option<String>, String> {
    if sequence["schema"] != "marjapussi.sequence-trace/1" {
        return Err("unsupported sequence trace schema".into());
    }
    let actual = identity(sequence, "information")?;
    let origin = sequence["belief_scope"]["origin"].as_str();
    for info in array(sequence, "information_sets")? {
        let id = identity(info, "id")?;
        let value = &info["value"];
        let observed = id == actual || origin == Some(id.as_str())
            || sequence["authorization"]["parent"]["information"].as_str() == Some(id.as_str());
        let scope = if observed {
            "Honest observer knowledge in its recorded current or historical-origin context; no other actor's private view."
        } else {
            "Conditional observer projection inside a model continuation; not actual current knowledge."
        };
        graph.add(node(
            &id,
            format!(
                "{} · seat {}",
                if observed {
                    "Observer facts"
                } else {
                    "Conditional observation"
                },
                value["seat"]
            ),
            "fact",
            "Recorded Information",
            value,
            vec![],
            scope,
            "Captured observer projection",
        ))?;
    }
    let aliases = definitions(sequence, graph)?;
    let mut choices = Vec::new();
    for choice in array(sequence, "choices")? {
        let id = identity(choice, "id")?;
        let mut inputs = vec![
            edge(&identity(choice, "information")?, "information set", "captured_field"),
            edge(
                &program_ref(&choice["program"], &aliases)?,
                "bound authored suffix",
                "definition_binding",
            ),
        ];
        if choice["assessment"].is_object() {
            let root = values(&choice["assessment"], graph)?;
            if choice["value_nodeId"] != root {
                return Err("prepared-choice assessment reference mismatch".into());
            }
            inputs.push(edge(&root, "evaluated conditional ranking", "evaluated_dependency"));
        }
        for consumed in array(choice, "consumed")? {
            inputs.push(edge(
                &read(consumed, &aliases, graph)?,
                "recorded guard / binding",
                "recorded_consumption",
            ));
        }
        let (name, scope) = match choice["scope"].as_str() {
            Some("parent") => ("Parent candidate comparison", "Complete original nominated parent domain; model estimate, not proof of optimality."),
            Some("global") => ("Global authored boundary", "Complete declared horizon-adjusted authored boundary domain, not all conceivable strategies or a fresh live posterior."),
            Some("local") => ("Local authored alternatives", "Complete local bound alternatives inside the parent-selected tree; does not assert a new global Best."),
            Some("fixed_action") => ("Fixed authored action", "Fixed step on an authorized branch; no value ranking or success probability is invented."),
            Some("response") => ("Recorded response-model step", "Singleton offer of the declared response model; not a value comparison or observed probability."),
            None => ("Conditional suffix choice", "Evidence scope was not recorded by this older capture; do not infer global authorization from its mode or membership."),
            Some(other) => return Err(format!("unsupported prepared choice scope {other}")),
        };
        let mut value = node(&id, format!("{name}: alternative {}", choice["selected"]), "decision", "Recorded PreparedChoice",
            &json!({"scope": choice["scope"], "selected": choice["selected"], "alternatives": choice["alternatives"], "support": choice["support"]}), inputs,
            scope, "Captured scoped choice; authorization requires explicit parent provenance");
        value["bindings"] = json!({"context": choice["context"].to_string()});
        choices.push(graph.add(value)?);
    }
    let authorization = authorization(sequence, &aliases, graph)?;
    let mut terminals = Vec::new();
    let mut origins = HashSet::new();
    for terminal in array(sequence, "terminals")? {
        let origin = terminal["origin"].as_u64().ok_or("missing cohort origin")?;
        if !origins.insert(origin) {
            return Err("duplicate selected-cohort origin".into());
        }
        let branch = graph.add(node(&format!("capture/origin/{origin}"), format!("Joint cohort member {origin}"), "future", "Recorded conditional cohort binding",
            &json!({"origin": origin, "weight": terminal["weight"]}), vec![],
            "One original joint cohort member and unnormalized model mass; hidden allocation and response inputs are not exported here.", "Captured conditional binding"))?;
        let mut previous = actual.clone();
        let mut tricks = Vec::new();
        for (ordinal, trick) in array(terminal, "tricks")?.iter().enumerate() {
            let id = format!("capture/origin/{origin}/trick/{ordinal}");
            graph.add(node(&id, format!("Trick {}: seat {} wins · {} points", ordinal + 1, trick["winner"], trick["points"]), "fact",
                "Recorded joint TrickOutcome", &json!({"leader": trick["leader"], "winner": trick["winner"], "cards": trick["cards"], "points": trick["points"]}),
                vec![edge(&branch, "same joint hypothesis", "conditional_binding"), edge(&previous, "earlier context in this recorded continuation", "recorded_continuation_order")],
                "Conditionally exact joint winner, cards and points under this recorded continuation; not separate marginal winner probabilities.", "Captured conditional engine result"))?;
            let successor = format!("{id}/successor");
            let info = identity(trick, "information")?;
            graph.add(node(&successor, format!("Next leader {} · surviving resources", trick["successor"]["leader"]), "fact", "Recorded successor state",
                &trick["successor"], vec![edge(&id, "same joint trick result", "captured_field"), edge(&info, "filtered successor holdings / observations", "captured_field")],
                "Recorded next leader, rights, trump and credited card/pair points in this same conditional branch. No new continuation is simulated by the exporter.", "Captured conditional successor"))?;
            previous = successor;
            tricks.push(id);
        }
        let mut inputs = vec![
            edge(&branch, "conditional origin and weight", "conditional_binding"),
            edge(&previous, "last recorded successor", "recorded_continuation_order"),
        ];
        if terminal["consumed_resources"].is_object() {
            let resources = format!("capture/origin/{origin}/resources");
            let mut resource_inputs: Vec<_> = tricks
                .iter()
                .map(|id| edge(id, "recorded completed trick", "captured_field"))
                .collect();
            for field in ["from_information", "to_information"] {
                resource_inputs.push(edge(
                    &identity(&terminal["consumed_resources"], field)?,
                    field,
                    "captured_field",
                ));
            }
            graph.add(node(&resources, "Cards and question rights consumed".into(), "fact", "Recorded resource accounting",
                &terminal["consumed_resources"], resource_inputs,
                "Resource accounting from captured observer-event deltas and before/after rights. It preserves this conditional branch; no imagined strategic bonus.", "Captured conditional resources"))?;
            inputs.push(edge(
                &resources,
                "same continuation's resource record",
                "captured_field",
            ));
        }
        for consumed in array(terminal, "consumed")? {
            inputs.push(edge(
                &read(consumed, &aliases, graph)?,
                "recorded guard / binding",
                "recorded_consumption",
            ));
        }
        terminals.push(graph.add(node(&format!("capture/origin/{origin}/outcome"), format!("Conditional final engine outcome · {origin}"), "fact",
            "Recorded GameOutcome", &json!({"outcome": terminal["outcome"], "interrupted": terminal["interrupted"], "first": terminal["first"]}), inputs,
            "Exact engine outcome conditional on this modeled continuation, not a realized live score. Contract and Schwarz scoring remain engine-owned.", "Captured conditional terminal"))?);
    }
    let mut inputs = Vec::new();
    if sequence["program"].is_object() {
        inputs.push(edge(
            &program_ref(&sequence["program"], &aliases)?,
            "selected authored suffix definition",
            "definition_binding",
        ));
    }
    if let Some(parent) = authorization {
        inputs.push(edge(&parent, "completed parent selection provenance", "selected_parent_authorization"));
    }
    if let Some(id) = graph.collection(
        "capture/choices",
        "Recorded conditional suffix choices",
        choices,
        "Captured Vec<PreparedChoice>",
    )? {
        inputs.push(edge(&id, "recorded conditional choices", "captured_field"));
    }
    if let Some(id) = graph.collection(
        "capture/terminals",
        "Selected joint-cohort terminal outcomes",
        terminals,
        "Captured Vec<Terminal>",
    )? {
        inputs.push(edge(&id, "selected cohort's recorded continuations", "captured_field"));
    }
    if inputs.is_empty() {
        return Ok(None);
    } // Deferred work/status is operational metadata, not a quality concept.
    graph.add(node("capture/sequence", "Selected conditional continuation".into(), "future", "Recorded SequenceTrace projection",
        &json!({"belief_scope": sequence["belief_scope"], "recorded_terminal_count": sequence["terminals"].as_array().map(Vec::len)}), inputs,
        "Only the supplied selected-program and continuation evidence. No unrecorded candidate outcomes, response inputs or rejected plans are filled in.", "Captured continuation evidence")).map(Some)
}

/// Attach terminal provenance only when the explicit assessment root, selected
/// score ref, outcome values and cohort weights all agree. Equal labels or a
/// superficially equal win are never sufficient to claim an evaluated join.
fn join_terminals(assessment: &Value, sequence: &Value, graph: &mut Graph) -> Result<bool, String> {
    let terminals = array(sequence, "terminals")?;
    if terminals.is_empty() || assessment["value_assessment_present"] != true {
        return Ok(false);
    }
    if sequence["value_nodeId"] != assessment["nodeId"] {
        return Err("sequence/value assessment root mismatch".into());
    }
    let selected = assessment["selected"].as_u64().ok_or("missing selected candidate")? as usize;
    let score_ref = assessment["candidates"][selected]["refs"]["scores"]
        .as_u64()
        .ok_or("missing selected score ref")?;
    let trace = array(assessment, "trace")?;
    let score = trace
        .iter()
        .find(|n| n["ref"].as_u64() == Some(score_ref))
        .ok_or("selected score node missing")?;
    let dependencies = array(score, "dependencies")?;
    let outcomes = trace
        .iter()
        .find(|n| n["value"]["conditional_outcomes"].is_array() && dependencies.iter().any(|r| r == &n["ref"]))
        .ok_or("selected conditional outcomes dependency missing")?;
    let supplied = array(&outcomes["value"], "conditional_outcomes")?;
    let mut weight_nodes = trace.iter().filter(|n| n["value"]["weights"].is_array());
    let weights = weight_nodes.next().ok_or("joint weights missing")?;
    if weight_nodes.next().is_some() {
        return Err("ambiguous cohort weights; cannot align selected terminals".into());
    }
    let supplied_weights = array(&weights["value"], "weights")?;
    if supplied.len() != terminals.len() || supplied_weights.len() != terminals.len() {
        return Err("selected terminal/cohort count mismatch".into());
    }
    for (i, terminal) in terminals.iter().enumerate() {
        if terminal["weight"] != supplied_weights[i] {
            return Err("selected terminal weight alignment mismatch".into());
        }
        for field in ["playing_team", "game_value", "team_points", "team_tricks"] {
            if terminal["outcome"][field] != supplied[i][field] {
                return Err(format!("selected terminal value mismatch: {field}"));
            }
        }
    }
    graph.link(
        &identity(outcomes, "nodeId")?,
        "capture/terminals",
        "aligned selected-cohort continuation provenance",
        "verified_capture_alignment",
    )?;
    for terminal in terminals {
        let origin = terminal["origin"].as_u64().ok_or("missing cohort origin")?;
        graph.link(
            &identity(weights, "nodeId")?,
            &format!("capture/origin/{origin}"),
            "same recorded cohort origin / weight",
            "verified_capture_alignment",
        )?;
    }
    Ok(true)
}

pub(super) fn export(input: &Value, index: Option<u64>, step: Option<u64>) -> Result<Value, String> {
    let (decision, assessment, game_index) = if input["decisions"].is_array() {
        if index.is_some_and(|i| input["index"].as_u64() != Some(i)) {
            return Err("requested game index not found".into());
        }
        let step = step.ok_or("--step is required for a selfplay game envelope")?;
        let d = array(input, "decisions")?
            .iter()
            .find(|d| d["step"].as_u64() == Some(step))
            .ok_or("decision step not found")?;
        (Some(d), &d["assessment"], input["index"].as_u64())
    } else if input["assessment"].is_object() {
        if index.is_some() || step.is_some_and(|s| input["step"].as_u64() != Some(s)) {
            return Err("selection does not match standalone decision".into());
        }
        (Some(input), &input["assessment"], None)
    } else {
        if index.is_some() || step.is_some() {
            return Err("index/step cannot select a standalone assessment".into());
        }
        (None, input, None)
    };
    if assessment["schema"] != "marjapussi.planning-assessment/1" {
        return Err("missing supported captured assessment; full NDJSON is required, not compact report data".into());
    }
    let sequence_data = &assessment["extensions"]["sequence"];
    if let Some(d) = decision.filter(|_| sequence_data.is_object()) {
        for field in ["seat", "event_count", "action"] {
            if !d[field].is_null()
                && !sequence_data["identity"][field].is_null()
                && d[field] != sequence_data["identity"][field]
            {
                return Err(format!("sequence decision identity mismatch: {field}"));
            }
        }
    }
    let mut graph = Graph::default();
    let mut root_inputs = Vec::new();
    let value_root = if assessment["value_assessment_present"] == true {
        let root = values(assessment, &mut graph)?;
        root_inputs.push(edge(
            &root,
            "actual retained value assessment (action match recorded separately)",
            "captured_field",
        ));
        Some(root)
    } else {
        None
    };
    let mut joined = false;
    if sequence_data.is_object() {
        if let Some(root) = sequence(sequence_data, &mut graph)? {
            root_inputs.push(edge(&root, "actual selected continuation evidence", "captured_field"));
        }
        let info = identity(sequence_data, "information")?;
        root_inputs.push(edge(&info, "honest acting observer input", "captured_field"));
        if value_root.is_some() {
            joined = join_terminals(assessment, sequence_data, &mut graph)?;
        }
    }
    if root_inputs.is_empty() {
        return Err("capture contains no supported value or sequence evidence".into());
    }
    let action = decision
        .map(|d| &d["action"])
        .filter(|a| !a.is_null())
        .unwrap_or(&sequence_data["identity"]["action"]);
    let captured_identity = json!({"game_index": game_index,
        "step": decision.map(|d| &d["step"]), "seat": decision.map(|d| &d["seat"]).unwrap_or(&sequence_data["identity"]["seat"]),
        "action": action, "event_count": decision.map(|d| &d["event_count"]).unwrap_or(&sequence_data["identity"]["event_count"])});
    let operations = json!({"mode": sequence_data["mode"], "failure": sequence_data["failure"], "work": sequence_data["work"],
        "work_accounting_model": sequence_data["work"]["accounting"],
        "work_accounting_meaning": tools::selfplay::sequence_work_accounting_meaning(sequence_data["work"]["accounting"].as_str()),
        "solver_effort_meaning": tools::selfplay::SEQUENCE_SOLVER_EFFORT_MEANING,
        "solver_node_limit": {"status": "not recorded by SequenceTrace",
            "meaning": "Limits::solver_nodes is optional; neither a configured ceiling nor None/uncapped can be inferred from these counters. Recorded deferral details describe only the stopping lane."},
        "work_attribution": "Per-trace aggregate; not attributed to individual concept or continuation nodes.",
        "belief_scope": sequence_data["belief_scope"], "selected_terminal_value_alignment_verified": joined,
        "authorization_projection": sequence_data["authorization"]["projection"],
        "assessment_matches_chosen_action": decision.map(|d| &d["assessment_matches_chosen_action"]),
        "unsupported_or_not_exported": ["Unrecorded joint allocations and other actors' response inputs are not fabricated.",
            "Only selected-cohort continuation traces are retained; unrecorded rejected-candidate paths are not invented.",
            "No proof of optimality, calibration, or a complete strategic dependency graph is established by this projection.",
            "Mode, failure and work counters are operational evidence, not concept-quality nodes."]});
    let mut root = node("capture/decision", if action.is_null() {"Captured assessment (action not recorded)".into()} else {format!("Recorded action: {}", action_name(action))},
        "decision", "Recorded act output and retained evidence", &captured_identity, root_inputs,
        "An actual recorded choice is deterministic, not certain to succeed. This display retains supplied evidence without rerunning selection.", "Captured runtime projection");
    root["detail"] = json!(format!("Operational evidence (separate from concepts): {operations}"));
    graph.add(root)?;
    let reachable = graph.reachable("capture/decision")?;
    // Also validate unused captured definitions/diagnostics; an invalid graph is
    // rejected rather than silently presented as complete.
    let mut validated = HashSet::new();
    for id in graph.nodes.keys() {
        graph.visit(id, &mut validated)?;
    }
    let synthetic = input["synthetic_fixture"] == true;
    Ok(
        json!({"schema": 1, "source_revision": "serialized runtime capture; policy revision requires the run manifest",
        "evidence_kind": if synthetic {"synthetic_capture_fixture"} else {"captured_runtime_projection"},
        "status": if synthetic {"Synthetic serializer fixture; no policy execution evidence"} else {"Captured runtime evidence projection; no policy replay or new evaluator"},
        "views": [{"id": "captured-runtime", "title": if synthetic {"Fixture · Captured graph structure"} else {"Captured · Decision and continuation"},
            "root": "capture/decision", "default_depth": 1,
            "nodes": graph.nodes.iter().filter(|(id, _)| reachable.contains(*id)).map(|(_, n)| n).collect::<Vec<_>>(),
            "diagnostic_nodes": graph.nodes.iter().filter(|(id, _)| !reachable.contains(*id)).map(|(_, n)| n).collect::<Vec<_>>(),
            "capture_identity": captured_identity, "operational_details": operations,
            "projection": "Evaluator dependencies, authored-definition references and recorded continuation provenance retain distinct edge relations. No conceptual illustration is substituted for execution evidence."}]}),
    )
}

fn action_name(action: &Value) -> String {
    match action["type"].as_str().unwrap_or("") {
        "card" => format!("Play {}", action["card"].as_str().unwrap_or("?")),
        "announce" => format!("Announce {}", action["suit"].as_str().unwrap_or("?")),
        "ask_pair" => "Ask partner's pair".into(),
        "ask_half" => format!("Ask partner's {} half", action["suit"]),
        "bid" | "raise" => format!("{} {}", action["type"].as_str().unwrap_or(""), action["value"]),
        "pass_bid" => "Pass bidding".into(),
        other => super::readable(other),
    }
}

pub(super) fn from_args(args: &[String]) -> Result<Value, String> {
    let path = args
        .first()
        .ok_or("--capture requires a JSON or selfplay NDJSON path")?;
    let mut index = None;
    let mut step = None;
    for pair in args[1..].chunks(2) {
        if pair.len() != 2 {
            return Err("capture selection flags require an integer".into());
        }
        let destination = match pair[0].as_str() {
            "--index" => &mut index,
            "--step" => &mut step,
            _ => return Err(format!("unknown capture option {}", pair[0])),
        };
        if destination.is_some() {
            return Err(format!("duplicate capture option {}", pair[0]));
        }
        *destination = Some(pair[1].parse::<u64>().map_err(|_| "invalid capture index/step")?);
    }
    let path = Path::new(path);
    let file = File::open(path).map_err(|e| format!("capture open: {e}"))?;
    if file.metadata().map_err(|e| format!("capture metadata: {e}"))?.len() <= MAX_BYTES {
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("capture read: {e}"))?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("capture exceeds 64 MiB".into());
        }
        if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
            return export(&value, index, step);
        }
    } else if index.is_none() {
        return Err("capture exceeds 64 MiB; use --index/--step on NDJSON".into());
    }
    let index = index.ok_or("NDJSON requires --index GAME and --step STEP")?;
    let mut reader = BufReader::new(File::open(path).map_err(|e| format!("capture reopen: {e}"))?);
    loop {
        let mut line = Vec::new();
        let read = reader
            .by_ref()
            .take(MAX_BYTES + 1)
            .read_until(b'\n', &mut line)
            .map_err(|e| format!("capture NDJSON: {e}"))?;
        if read == 0 {
            return Err("requested game index not found".into());
        }
        if read as u64 > MAX_BYTES {
            return Err("NDJSON game exceeds 64 MiB; extract a single decision first".into());
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let value: Value = serde_json::from_slice(&line).map_err(|e| format!("capture NDJSON row: {e}"))?;
        if value["index"].as_u64() == Some(index) {
            return export(&value, Some(index), step);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Serializer-shape fixture only: no policy was executed and these numbers
    // are not a measured game. Tests verify identity/projection, not strength.
    fn fixture() -> Value {
        let leaf = "sequence/program/0/1";
        let guard = "sequence/program/0/0";
        let info = "sequence/information/0";
        let after = "sequence/information/1";
        let consumed = json!({"information": info, "nodeId": guard,
            "reading": {"kind": "guard", "predicate": {"operator": "own_lead"},
                "truth": null, "branch": "unknown"}});
        let terminals: Vec<_> = [0.25, 0.75]
            .iter()
            .enumerate()
            .map(|(i, weight)| {
                json!({"origin": i, "weight": weight, "interrupted": false, "first": null,
                "outcome": {"playing_team": 0, "game_value": 120,
                    "team_points": [140, 0], "team_tricks": [9, 0], "engine_scores": [140, -120]},
                "tricks": [{"leader": 0, "winner": 0, "cards": [0, 1, 2, 3],
                    "points": 7 + i, "information": after,
                    "successor": {"leader": 0, "rights": ["own", "own", "own", "own"],
                        "trump": null, "card_points": [7 + i, 0], "pair_points": [0, 0]}}],
                "consumed": [consumed],
                "consumed_resources": {"from_information": info, "to_information": after,
                    "played_cards": [{"seat": 0, "card": 0}], "remaining_own_cards": [4],
                    "question_and_pair_events": [], "rights_before": ["own", "own", "own", "own"],
                    "rights_after": ["own", "own", "own", "own"]}})
            })
            .collect();
        json!({"synthetic_fixture": true, "step": 13, "seat": 0, "event_count": 20,
            "action": {"type": "card", "card": "eA"}, "assessment": {
            "schema": "marjapussi.planning-assessment/1", "value_assessment_present": false,
            "extensions": {"sequence": {"schema": "marjapussi.sequence-trace/1", "mode": "formation",
                "information": info, "belief_scope": {"kind": "current_information"},
                "identity": {"seat": 0, "event_count": 20, "action": {"type": "card", "card": "eA"}},
                "work": {"represented_transitions": 8, "transitions": 4}, "failure": null,
                "program": {"nodeId": guard}, "value_nodeId": null,
                "programs": [{"nodes": [
                    {"nodeId": guard, "definition": {"operator": "guard", "predicate": {"operator": "own_lead"},
                        "yes": leaf, "no": leaf, "unknown": leaf}},
                    {"nodeId": leaf, "definition": {"operator": "response_leaf"}}]},
                    {"nodes": [{"nodeId": "sequence/program/1/0", "definition": {"operator": "response_leaf"}}]}],
                "information_sets": [{"id": info, "value": {"seat": 0, "event_count": 20, "events": []}},
                    {"id": after, "value": {"seat": 0, "event_count": 24, "events": []}}],
                "choices": [{"id": "sequence/choice/0", "information": info, "program": {"nodeId": guard},
                    "assessment": null, "selected": 0, "alternatives": [], "support": [],
                    "context": {"seed": 0}, "consumed": [consumed]}], "terminals": terminals}}}})
    }

    fn with_values() -> Value {
        let mut fixture = fixture();
        let sequence = &fixture["assessment"]["extensions"]["sequence"];
        let outcomes: Vec<_> = sequence["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| {
                let o = &t["outcome"];
                json!({"playing_team": o["playing_team"], "game_value": o["game_value"],
                "team_points": o["team_points"], "team_tricks": o["team_tricks"]})
            })
            .collect();
        let weights: Vec<_> = sequence["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["weight"].clone())
            .collect();
        let assessment = &mut fixture["assessment"];
        assessment["value_assessment_present"] = json!(true);
        assessment["nodeId"] = json!("value/3");
        assessment["selected"] = json!(0);
        assessment["candidates"] = json!([{"refs": {"scores": 1}}]);
        let entries = [
            (
                "conditional_outcomes",
                "ConditionalExact",
                "TerminalOutcomes",
                json!({"conditional_outcomes": outcomes}),
                vec![],
            ),
            (
                "engine_scores",
                "ConditionalExact",
                "EngineScores",
                json!({"conditional_engine_scores": [[140, -120], [140, -120]]}),
                vec![0],
            ),
            (
                "joint_weights",
                "ModelEstimate",
                "CohortWeights",
                json!({"weights": weights}),
                vec![],
            ),
            (
                "best_candidate",
                "ModelSelection",
                "Ranking",
                json!({"selected": 0, "alternative": null}),
                vec![1, 2],
            ),
        ];
        assessment["trace"] = json!(entries
            .iter()
            .enumerate()
            .map(|(i, (name, meaning, op, value, deps))| {
                json!({"ref": i, "nodeId": format!("value/{i}"), "name": name, "meaning": meaning,
                "operator": op, "value": value, "dependencies": deps,
                "dependency_nodeIds": deps.iter().map(|d| format!("value/{d}")).collect::<Vec<_>>()})
            })
            .collect::<Vec<_>>());
        assessment["extensions"]["sequence"]["value_nodeId"] = json!("value/3");
        fixture
    }

    fn nodes(output: &Value) -> BTreeMap<&str, &Value> {
        output["views"][0]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| (n["id"].as_str().unwrap(), n))
            .collect()
    }

    #[test]
    fn shares_complete_definitions_and_queries_without_merging_joint_outcomes() {
        let output = export(&fixture(), None, None).unwrap();
        let nodes = nodes(&output);
        assert_eq!(output["evidence_kind"], "synthetic_capture_fixture");
        assert_eq!(nodes.len(), output["views"][0]["nodes"].as_array().unwrap().len());
        assert_eq!(
            nodes
                .values()
                .filter(|n| n["identity_role"] == "authored definition")
                .count(),
            2
        );
        assert_eq!(
            nodes["sequence/program/0/1"]["source_refs"].as_array().unwrap().len(),
            2
        );
        assert_eq!(
            nodes
                .values()
                .filter(|n| n["operator"] == "Consumed runtime query")
                .count(),
            1
        );
        let guard_edges = nodes["sequence/program/0/0"]["inputs"].as_array().unwrap();
        assert_eq!(guard_edges.len(), 1);
        for branch in ["yes", "no", "unknown"] {
            assert!(guard_edges[0]["label"].as_str().unwrap().contains(branch));
        }
        assert_eq!(nodes["capture/query/0"]["runtime_value"]["truth"], Value::Null);
        assert!(nodes["capture/query/0"]["label"].as_str().unwrap().ends_with("unknown"));
        assert_eq!(nodes["capture/origin/0/trick/0"]["runtime_value"]["points"], 7);
        assert_eq!(nodes["capture/origin/1/trick/0"]["runtime_value"]["points"], 8);
        assert!(nodes["sequence/information/1"]["epistemic"]
            .as_str()
            .unwrap()
            .contains("not actual current knowledge"));
        let relations: HashSet<_> = nodes
            .values()
            .flat_map(|n| n["inputs"].as_array().unwrap())
            .map(|e| {
                assert!(nodes.contains_key(e["id"].as_str().unwrap()));
                e["relation"].as_str().unwrap()
            })
            .collect();
        assert!(relations.contains("authored_definition_reference"));
        assert!(relations.contains("recorded_continuation_order"));
    }

    #[test]
    fn aligns_selected_value_cohort_and_rejects_wrong_weight_or_decision_identity() {
        let output = export(&with_values(), None, None).unwrap();
        assert_eq!(
            output["views"][0]["operational_details"]["selected_terminal_value_alignment_verified"],
            true
        );
        let projected = nodes(&output);
        assert_eq!(
            projected["value/0"]["inputs"][0]["relation"],
            "verified_capture_alignment"
        );
        // A conditioned cohort can retain noncontiguous original identities.
        let mut conditioned = with_values();
        conditioned["assessment"]["extensions"]["sequence"]["terminals"][0]["origin"] = json!(17);
        conditioned["assessment"]["extensions"]["sequence"]["terminals"][1]["origin"] = json!(23);
        let conditioned = export(&conditioned, None, None).unwrap();
        assert!(nodes(&conditioned).contains_key("capture/origin/23"));
        let mut wrong = with_values();
        wrong["assessment"]["extensions"]["sequence"]["terminals"][1]["weight"] = json!(0.5);
        assert!(export(&wrong, None, None).unwrap_err().contains("weight alignment"));
        wrong = fixture();
        wrong["seat"] = json!(2);
        assert!(export(&wrong, None, None).unwrap_err().contains("identity mismatch"));
    }

    // Synthetic serializer-shape evidence only; no SelectedPlan was executed.
    fn authorized_fixture() -> Value {
        let mut input = with_values();
        let mut local = input["assessment"].clone();
        local.as_object_mut().unwrap().remove("extensions");
        local["nodeId"] = json!("sequence/choice/0/value/3");
        local["cohort_complete"] = json!(true);
        for value in local["trace"].as_array_mut().unwrap() {
            value["nodeId"] = json!(format!("sequence/choice/0/value/{}", value["ref"]));
            value["dependency_nodeIds"] = json!(value["dependencies"].as_array().unwrap().iter()
                .map(|id| format!("sequence/choice/0/value/{id}")).collect::<Vec<_>>());
        }
        let sequence = &mut input["assessment"]["extensions"]["sequence"];
        let parent_support = json!([{"origin": 0, "weight": 0.25}, {"origin": 1, "weight": 0.75}]);
        let choice = &mut sequence["choices"][0];
        choice["scope"] = json!("local");
        choice["support"] = json!([{"origin": 1, "weight": 0.75}]);
        choice["alternatives"] = json!([{"kind": "stage", "program": choice["program"]}]);
        choice["assessment"] = local;
        choice["value_nodeId"] = json!("sequence/choice/0/value/3");
        let mut response = choice.clone();
        response["id"] = json!("sequence/choice/1");
        response["scope"] = json!("response");
        response["assessment"] = Value::Null;
        response["value_nodeId"] = Value::Null;
        response["alternatives"] = json!([{"kind": "offer", "action": {"type": "card", "card": "eA"}}]);
        sequence["choices"].as_array_mut().unwrap().push(response);
        sequence["authorization"] = json!({"schema": "marjapussi.selection-authorization/1", "consistency_error": null,
            "parent": {"id": "sequence/authorization/parent", "scope": "parent", "cohort_complete": true,
                "information": sequence["information"], "program": sequence["program"],
                "comparison_programs": [sequence["program"]],
                "context": sequence["choices"][0]["context"], "candidate_count": 1,
                "alternatives_count": 1, "program_count": 1, "legal_count": 2, "selected": 0,
                "support": parent_support, "selected_estimate": {"difference": 260.0}},
            "certificates": [{"choice": "sequence/choice/0", "record_index": 3},
                {"choice": "sequence/choice/1", "record_index": 8}], "projection": "synthetic compact parent"});
        input
    }

    #[test]
    fn shares_completed_parent_and_distinguishes_local_global_and_retry_without_probability_claim() {
        let input = authorized_fixture();
        let output = export(&input, None, None).unwrap();
        let n = nodes(&output);
        assert_eq!(n.keys().filter(|id| **id == "sequence/authorization/parent").count(), 1);
        assert!(n["sequence/choice/0"]["label"].as_str().unwrap().starts_with("Local authored"));
        assert!(n["sequence/choice/0"]["epistemic"].as_str().unwrap().contains("not assert a new global"));
        assert_eq!(n["sequence/choice/1"]["runtime_value"]["scope"], "response");
        for id in ["sequence/choice/0", "sequence/choice/1"] {
            let inputs = n[id]["inputs"].as_array().unwrap();
            let parent = inputs.iter().find(|edge| edge["id"] == "sequence/authorization/parent").unwrap();
            assert_eq!(parent["relation"], "selected_parent_authorization");
        }
        assert_eq!(n["sequence/choice/1"]["authorization_record_index"], 8);
        assert!(n["sequence/authorization/parent"]["runtime_value"].get("probability").is_none());
        let mut retry = input.clone();
        retry["assessment"]["extensions"]["sequence"]["mode"] = json!("retry");
        let retry = export(&retry, None, None).unwrap();
        assert_eq!(nodes(&retry)["sequence/choice/0"], n["sequence/choice/0"]);
        let mut global = input.clone();
        global["assessment"]["extensions"]["sequence"]["choices"][0]["scope"] = json!("global");
        let global = export(&global, None, None).unwrap();
        assert!(nodes(&global)["sequence/choice/0"]["epistemic"].as_str().unwrap().contains("horizon-adjusted"));
        let legacy = export(&fixture(), None, None).unwrap();
        assert!(nodes(&legacy)["sequence/choice/0"]["epistemic"].as_str().unwrap().contains("not recorded"));
    }

    #[test]
    fn rejects_forged_authorization_alignment_scope_mass_and_incomplete_evidence() {
        let input = authorized_fixture();
        for lane in 0..6 {
            let mut wrong = input.clone();
            let sequence = &mut wrong["assessment"]["extensions"]["sequence"];
            match lane {
                0 => sequence["authorization"]["certificates"][1]["record_index"] = json!(3),
                1 => sequence["authorization"]["parent"]["cohort_complete"] = json!(false),
                2 => sequence["choices"][0]["support"][0]["weight"] = json!(1.0),
                3 => sequence["choices"][0]["assessment"]["cohort_complete"] = json!(false),
                4 => sequence["choices"][1]["scope"] = json!("local"),
                _ => sequence["mode"] = json!("deferred"),
            }
            assert!(export(&wrong, None, None).is_err(), "invalid authorization lane {lane}");
        }
        let mut no_children = input.clone();
        let sequence = &mut no_children["assessment"]["extensions"]["sequence"];
        sequence["choices"] = json!([]);
        sequence["authorization"]["certificates"] = json!([]);
        assert!(nodes(&export(&no_children, None, None).unwrap()).contains_key("sequence/authorization/parent"));
    }

    #[test]
    fn rejects_missing_refs_cycles_and_unequal_definitions_are_not_shared() {
        let mut input = fixture();
        input["assessment"]["extensions"]["sequence"]["programs"][0]["nodes"][0]["definition"]["unknown"] =
            json!("missing");
        assert!(export(&input, None, None).unwrap_err().contains("missing authored"));
        input = fixture();
        input["assessment"]["extensions"]["sequence"]["programs"][0]["nodes"][0]["definition"]["yes"] =
            json!("sequence/program/0/0");
        assert!(export(&input, None, None).unwrap_err().contains("cycle"));
        input = fixture();
        input["assessment"]["extensions"]["sequence"]["programs"][1]["nodes"][0]["definition"] = json!({
            "operator": "action_binding", "actions": {"operator": "legal_cards", "cards": [7]}, "next": "sequence/program/0/1"});
        let output = export(&input, None, None).unwrap();
        assert_eq!(
            output["views"][0]["diagnostic_nodes"][0]["identity_role"],
            "authored definition"
        );
    }

    #[test]
    fn deferred_status_is_operational_and_selects_step_identity_not_array_position() {
        let mut input = fixture();
        let sequence = &mut input["assessment"]["extensions"]["sequence"];
        for field in ["programs", "choices", "terminals"] {
            sequence[field] = json!([]);
        }
        sequence["program"] = Value::Null;
        sequence["mode"] = json!("deferred");
        sequence["failure"] = json!({"kind": "budget"});
        let output = export(&input, None, None).unwrap();
        let nodes = nodes(&output);
        assert_eq!(nodes.len(), 2); // Recorded action and acting information only.
        assert!(!nodes.contains_key("capture/sequence"));
        assert_eq!(output["views"][0]["operational_details"]["failure"]["kind"], "budget");
        let game = json!({"synthetic_fixture": true, "index": 17, "decisions": [input]});
        assert!(export(&game, Some(17), Some(0)).is_err());
        assert_eq!(
            export(&game, Some(17), Some(13)).unwrap()["views"][0]["capture_identity"]["step"],
            13
        );
    }

    #[test]
    fn retains_reconsideration_and_raise_binding_definitions_without_fake_value_nodes() {
        let mut input = fixture();
        let sequence = &mut input["assessment"]["extensions"]["sequence"];
        sequence["programs"][0]["nodes"].as_array_mut().unwrap().extend([
            json!({"nodeId": "sequence/program/0/2", "definition": {
                "operator": "reconsider", "remaining": 1, "retained": "sequence/program/0/0"}}),
            json!({"nodeId": "sequence/program/0/3", "definition": {
                "operator": "action_binding", "actions": {"operator": "all_legal_raises"}, "next": "sequence/program/0/2"}}),
        ]);
        sequence["program"]["nodeId"] = json!("sequence/program/0/3");
        sequence["choices"][0]["consumed"].as_array_mut().unwrap().push(json!({
            "information": "sequence/information/0", "nodeId": "sequence/program/0/2",
            "reading": {"kind": "reconsider", "remaining": 1, "retained_source_stage": 0}}));
        let output = export(&input, None, None).unwrap();
        let nodes = nodes(&output);
        assert!(nodes["sequence/program/0/3"]["label"]
            .as_str()
            .unwrap()
            .contains("legal raises"));
        assert!(nodes["sequence/program/0/2"]["inputs"][0]["label"]
            .as_str()
            .unwrap()
            .contains("retained"));
        assert!(nodes.values().any(|n| n["runtime_value"]["kind"] == "reconsider"));
        assert!(!nodes.keys().any(|id| id.starts_with("value/")));
    }

    #[test]
    fn solver_effort_remains_aggregate_operational_data_without_new_concept_nodes() {
        let mut input = fixture();
        let original = export(&input, None, None).unwrap();
        input["assessment"]["extensions"]["sequence"]["work"] = json!({
            "actual_solver_nodes": 0, "represented_solver_nodes": u64::MAX,
            "accounting": "sibling-tail-dag-v1", "tail_aliases": 3, "tail_alias_transitions": 17, "tail_alias_solver_nodes": u64::MAX,
            "deferred_limit_detail": {"kind": "represented_solver_nodes", "required": u64::MAX, "remaining": 0}});
        let output = export(&input, None, None).unwrap();
        let semantic_nodes = |value: &Value| {
            nodes(value)
                .into_iter()
                .map(|(id, node)| {
                    let mut node = node.clone();
                    if id == "capture/decision" {
                        // Only this inspector field contains aggregate operational
                        // metadata. All identities, values and dependency edges
                        // must remain identical when the work counters change.
                        node.as_object_mut().unwrap().remove("detail");
                    }
                    (id.to_owned(), node)
                })
                .collect::<BTreeMap<_, _>>()
        };
        assert_eq!(
            semantic_nodes(&output),
            semantic_nodes(&original),
            "cost counters must not become strategic dependencies"
        );
        let details = &output["views"][0]["operational_details"];
        assert_eq!(details["work"]["actual_solver_nodes"], 0);
        assert_eq!(details["work_accounting_model"], "sibling-tail-dag-v1");
        assert_eq!(details["work"]["tail_alias_transitions"], 17);
        assert!(original["views"][0]["operational_details"]["work_accounting_model"].is_null(),
            "older captures must not inherit the current model label");
        assert!(original["views"][0]["operational_details"]["work_accounting_meaning"]
            .as_str().unwrap().contains("not recorded"));
        assert!(details["work_accounting_meaning"].as_str().unwrap().contains("same Tail"));
        let mut v2_input = input.clone();
        v2_input["assessment"]["extensions"]["sequence"]["work"]["accounting"] = json!("sibling-frontier-dag-v2");
        let v2_work = &mut v2_input["assessment"]["extensions"]["sequence"]["work"];
        v2_work["frontier_aliases"] = json!(2);
        v2_work["frontier_alias_transitions"] = json!(19);
        v2_work["frontier_alias_comparisons"] = json!(7);
        v2_work["frontier_alias_solver_nodes"] = json!(u64::MAX);
        let v2 = export(&v2_input, None, None).unwrap();
        assert_eq!(semantic_nodes(&v2), semantic_nodes(&output));
        let v2_details = &v2["views"][0]["operational_details"];
        assert_eq!(v2_details["work_accounting_model"], "sibling-frontier-dag-v2");
        assert_eq!(v2_details["work"]["frontier_aliases"], 2);
        assert_eq!(v2_details["work"]["frontier_alias_transitions"], 19);
        assert_eq!(v2_details["work"]["frontier_alias_comparisons"], 7);
        assert_eq!(v2_details["work"]["frontier_alias_solver_nodes"].as_u64(), Some(u64::MAX));
        assert!(details["work"].get("frontier_aliases").is_none(),
            "V1 capture must not acquire V2 counters");
        assert!(v2_details["work_accounting_meaning"].as_str().unwrap().contains("Singleton Actions stay on the ordinary path"));
        let mut unknown_input = input.clone();
        unknown_input["assessment"]["extensions"]["sequence"]["work"]["accounting"] = json!("future-model");
        let unknown = export(&unknown_input, None, None).unwrap();
        assert_eq!(semantic_nodes(&unknown), semantic_nodes(&output));
        assert_eq!(unknown["views"][0]["operational_details"]["work_accounting_model"], "future-model");
        assert!(unknown["views"][0]["operational_details"]["work_accounting_meaning"]
            .as_str().unwrap().contains("Unrecognized"));
        assert_eq!(details["work"]["represented_solver_nodes"].as_u64(), Some(u64::MAX));
        assert_eq!(details["solver_node_limit"]["status"], "not recorded by SequenceTrace");
        assert!(details["solver_effort_meaning"]
            .as_str()
            .unwrap()
            .contains("not wall time, total CPU"));
        assert!(details["work_attribution"].as_str().unwrap().contains("aggregate"));
    }

    #[test]
    fn response_gate_is_shared_and_deferred_is_never_a_terminal_tail() {
        let mut input = fixture();
        let sequence = &mut input["assessment"]["extensions"]["sequence"];
        let response = "sequence/program/0/3";
        let guard = "sequence/program/0/0";
        let deferred = "sequence/program/0/2";
        sequence["programs"][0]["nodes"][0]["definition"] = json!({"operator": "guard",
            "predicate": {"operator": "play_ready"}, "yes": "sequence/program/0/1",
            "no": deferred, "unknown": deferred});
        sequence["programs"][0]["nodes"].as_array_mut().unwrap().extend([
            json!({"nodeId": deferred, "definition": {"operator": "deferred"}}),
            json!({"nodeId": response, "definition": {"operator": "response_transition", "next": guard}}),
            json!({"nodeId": "sequence/program/0/4", "definition": {"operator": "action_binding",
                "actions": {"operator": "exact_action_type", "action_type": "StopBidding"}, "next": response}}),
            json!({"nodeId": "sequence/program/0/5", "definition": {"operator": "action_binding",
                "actions": {"operator": "exact_action_type", "action_type": {"Pass": [0, 1, 2, 3]}}, "next": response}}),
            json!({"nodeId": "sequence/program/0/6", "definition": {"operator": "value_comparison",
                "alternatives": ["sequence/program/0/4", "sequence/program/0/5"]}}),
        ]);
        sequence["programs"][1]["nodes"][0]["definition"] = json!({"operator": "response_transition", "next": guard});
        sequence["program"]["nodeId"] = json!("sequence/program/0/6");
        let consumed = json!([
            {"information": "sequence/information/0", "nodeId": response,
                "reading": {"kind": "response_transition", "next_nodeId": guard, "next_source_stage": 9}},
            {"information": "sequence/information/0", "nodeId": guard,
                "reading": {"kind": "guard", "predicate": {"operator": "play_ready"}, "truth": false}},
            {"information": "sequence/information/0", "nodeId": deferred, "reading": {"kind": "deferred"}}
        ]);
        sequence["choices"][0]["consumed"] = consumed;
        sequence["terminals"] = json!([]); // Exhaustion supplies no outcome or value.
        let output = export(&input, None, None).unwrap();
        let projected = nodes(&output);
        assert_eq!(projected[response]["source_refs"].as_array().unwrap().len(), 2);
        assert_eq!(projected[response]["inputs"][0]["id"], guard);
        assert_eq!(
            projected[response]["inputs"][0]["relation"],
            "authored_definition_reference"
        );
        assert_eq!(projected[deferred]["runtime_value"]["operator"], "deferred");
        assert!(projected[deferred]["inputs"].as_array().unwrap().is_empty());
        assert_eq!(
            projected["sequence/program/0/1"]["runtime_value"]["operator"],
            "response_leaf"
        );
        assert!(projected["sequence/program/0/4"]["label"]
            .as_str()
            .unwrap()
            .contains("Stop bidding"));
        assert!(projected["sequence/program/0/5"]["label"]
            .as_str()
            .unwrap()
            .contains("Pass four cards"));
        assert!(projected.values().any(|n| n["runtime_value"]["kind"] == "deferred"));
        assert!(!projected
            .keys()
            .any(|id| id.starts_with("value/") || id.starts_with("capture/origin/")));

        input["assessment"]["extensions"]["sequence"]["choices"][0]["consumed"][0]["reading"]["next_nodeId"] =
            json!(deferred);
        assert!(export(&input, None, None).unwrap_err().contains("does not match"));
        input["assessment"]["extensions"]["sequence"]["programs"][0]["nodes"][3]["definition"]["next"] =
            json!("missing");
        assert!(export(&input, None, None).unwrap_err().contains("missing authored"));
    }
}

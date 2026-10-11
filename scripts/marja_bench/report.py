"""Aggregate completed game records. Team scores and rule facts come from Rust."""

from collections import Counter, defaultdict
import csv
import html
import json
import math
from pathlib import Path
import statistics


SEQUENCE_MODES = ("formation", "repair", "prepared", "retry", "deferred")
SEQUENCE_COUNTERS = ("transitions", "represented_transitions", "comparisons", "represented_comparisons",
                     "actual_solver_nodes", "represented_solver_nodes", "response_calls",
                     "response_hits", "suffix_hits", "retained_bytes", "completed_candidates",
                     "attempted_candidates", "total_candidates")

SOLVER_EFFORT_MEANING = (
    "Canonical response-solver search nodes: actual counts attempted callbacks including failures; "
    "represented also charges completed memo hits. Algorithm-specific effort, not wall time, total CPU, "
    "complete policy work or value-graph node counts. The configured optional solver-node ceiling is not recorded."
)


def value_assessment_presence(decision):
    """Classify actual value data; an extension-only envelope is not a ranking."""
    assessment = decision.get("assessment")
    for source in (decision, assessment if isinstance(assessment, dict) else {}):
        if "value_assessment_present" in source:
            value = source["value_assessment_present"]
            return value if type(value) is bool else None
    # Legacy full value envelopes expose a real ranking/candidate binding. Old
    # compact presence flags alone cannot distinguish an extension-only capture.
    if isinstance(assessment, dict):
        if (type(assessment.get("selected")) is int and type(assessment.get("ranking")) is int
                and isinstance(assessment.get("candidates"), list)):
            return True
        return None
    if "assessment" in decision and assessment is None:
        return False
    return None


def value_rank_complete(decision):
    present = value_assessment_presence(decision)
    if present is not True:
        return False if present is False else None
    if "value_rank_complete" in decision:
        value = decision["value_rank_complete"]
        return value if type(value) is bool else None
    assessment = decision.get("assessment")
    if not isinstance(assessment, dict):
        return None
    if assessment.get("cohort_complete") is False:
        return False
    if assessment.get("cohort_complete") is not True:
        return None
    selected, ranking, candidates = (assessment.get(k) for k in ("selected", "ranking", "candidates"))
    if type(selected) is int and type(ranking) is int and isinstance(candidates, list):
        return 0 <= selected < len(candidates) and ranking >= 0
    return None


def compact_sequence(decision):
    """Bounded scalar projection only. Do not copy programs, worlds or views."""
    if "sequence" in decision:  # Already compacted: preserve exact report inputs.
        return decision["sequence"]
    assessment = decision.get("assessment")
    extensions = assessment.get("extensions", {}) if isinstance(assessment, dict) else {}
    sequence = extensions.get("sequence") if isinstance(extensions, dict) else None
    if not isinstance(sequence, dict):
        return None
    work = sequence.get("work")
    work = work if isinstance(work, dict) else {}
    counters, invalid = {}, []
    for key in SEQUENCE_COUNTERS:
        # Optional future count fields must be explicit, never guessed from a
        # mode, partial completed count, candidate catalogue or list of choices.
        source = work if key in work else sequence
        value = source.get(key)
        valid = type(value) is int and value >= 0
        counters[key] = value if valid else None
        if key in source and value is not None and not valid:
            invalid.append(key)
    failure = sequence.get("failure")
    scope = sequence.get("belief_scope")
    failure_kind = failure.get("kind") if isinstance(failure, dict) else failure
    scope_kind = scope.get("kind") if isinstance(scope, dict) else None
    return {"mode": sequence.get("mode") if sequence.get("mode") in SEQUENCE_MODES else None,
            "belief_scope": scope_kind if scope_kind in ("current_information", "prepared_conditional") else None,
            "failure_present": failure is not None if "failure" in sequence else None,
            "failure": failure_kind if isinstance(failure_kind, str) else None,
            "work": counters, "invalid_work_fields": invalid}


def compact_decision(decision):
    sequence = compact_sequence(decision)
    return {**{k: decision[k] for k in ("seat", "ms", "phase", "rule")},
            "assessment_recorded": decision.get("assessment_recorded"),
            "assessment_present": decision.get("assessment") is not None or decision.get("assessment_present", False),
            "value_assessment_present": value_assessment_presence(decision),
            "value_rank_complete": value_rank_complete(decision),
            "assessment_matches_chosen_action": decision.get("assessment_matches_chosen_action"),
            "sequence_present": sequence is not None or decision.get("sequence_present") is True,
            "sequence": sequence,
            "capture_ms": decision.get("capture_ms"),
            "assessment_serialization_ms": decision.get("assessment_serialization_ms"),
            "facts": [f for f in decision["facts"] if f["key"] in
                      ("forecast_complete", "forecast_decisions", "forecast_reused_decisions")]}


def compact_game(game):
    """Keep report inputs only; full hands, actions and explanations stay on disk."""
    row = {k: game[k] for k in ("index", "deal_index", "deal_seed", "rotation", "lineup", "metrics")}
    row.update({k: game[k] for k in ("rotation_mode", "games_per_deal", "hands_turned", "policy_seeds") if k in game})
    if "policies" in game:
        row["policy_seeds"] = [p.get("policy_seed", p.get("stream_seed")) for p in game["policies"]]
    row["decisions"] = [compact_decision(d) for d in game["decisions"]]
    return row


def mean(values):
    return statistics.fmean(values) if values else None


def ratio(numerator, denominator):
    return numerator / denominator if denominator else None


def latency(values):
    values = sorted(values)
    if not values:
        return {"count": 0, "mean_ms": None, "median_ms": None, "p95_ms": None, "max_ms": None}
    return {"count": len(values), "mean_ms": mean(values), "median_ms": statistics.median(values),
            "p95_ms": values[max(0, math.ceil(.95 * len(values)) - 1)], "max_ms": values[-1],
            "total_seconds": sum(values) / 1000}


def clustered_mean(games, value):
    """Game-weighted mean with a deal-clustered, approximate normal 95% interval.

    Unequal final blocks retain their actual game counts; cluster residual sums,
    rather than means of unequal blocks, determine the standard error.
    """
    groups = defaultdict(list)
    for game in games:
        groups[game["deal_index"]].append(value(game))
    n, g = len(games), len(groups)
    estimate = mean([v for values in groups.values() for v in values])
    if g < 2:
        return {"mean": estimate, "ci95": None, "deal_clusters": g}
    variance = g / (g - 1) * sum(sum(v - estimate for v in values) ** 2 for values in groups.values()) / n**2
    error = 1.96 * math.sqrt(variance)
    return {"mean": estimate, "ci95": [estimate - error, estimate + error], "deal_clusters": g}


def team_index(game, physical_team):
    return game["lineup"][physical_team] % 2


def team_value(game, values, team):
    return values[next(t for t in (0, 1) if team_index(game, t) == team)]


def sequence_metrics(decisions):
    records = [compact_sequence(d) for d in decisions]
    records = [r for r in records if r is not None]
    present = sum(compact_sequence(d) is not None or d.get("sequence_present") is True for d in decisions)

    def counter(key, memory=False):
        # Older compact records predate new counters: absence remains missing,
        # rather than zero or a reconstruction from hits/elapsed time.
        values = [r["work"][key] for r in records if r["work"].get(key) is not None]
        result = {"recorded_decisions": len(values), "missing_decisions": present - len(values),
                  "invalid_decisions": sum(key in r["invalid_work_fields"] for r in records)}
        if memory:
            result.update(max_bytes=max(values) if values else None, mean_bytes=mean(values))
        else:
            result["total"] = sum(values) if values else None
        return result

    paired = [r["work"] for r in records if r["work"]["completed_candidates"] is not None
              and r["work"]["attempted_candidates"] is not None]
    completed = sum(w["completed_candidates"] for w in paired) if paired else None
    attempted = sum(w["attempted_candidates"] for w in paired) if paired else None
    consistent = all(w["completed_candidates"] <= w["attempted_candidates"] for w in paired)
    return {"recorded_decisions": present, "total_decisions": len(decisions),
            "modes": {mode: sum(r["mode"] == mode for r in records) for mode in SEQUENCE_MODES},
            "mode_unknown_decisions": present - sum(r["mode"] is not None for r in records),
            "belief_scopes": dict(Counter(r["belief_scope"] for r in records if r["belief_scope"] is not None)),
            "belief_scope_unknown_decisions": present - sum(r["belief_scope"] is not None for r in records),
            "failures": dict(Counter(r["failure"] for r in records if r["failure"] is not None)),
            "failure_present_decisions": sum(r["failure_present"] is True for r in records),
            "failure_kind_unknown_decisions": sum(r["failure_present"] is True and r["failure"] is None for r in records),
            "failure_status_unknown_decisions": present - sum(r["failure_present"] is not None for r in records),
            "candidate_counts": {key: counter(key) for key in
                                 ("completed_candidates", "attempted_candidates", "total_candidates")},
            "paired_completed_attempted": {"recorded_decisions": len(paired), "completed": completed,
                                           "attempted": attempted, "consistent": consistent if paired else None,
                                           "completion_rate": ratio(completed, attempted) if consistent else None},
            "actual_work": {**{key: counter(key) for key in ("transitions", "comparisons", "response_calls")},
                            "solver_nodes": counter("actual_solver_nodes")},
            "represented_work": {"transitions": counter("represented_transitions"),
                                 "comparisons": counter("represented_comparisons"),
                                 "solver_nodes": counter("represented_solver_nodes")},
            "solver_effort_meaning": SOLVER_EFFORT_MEANING,
            "reuse": {key: counter(key) for key in ("response_hits", "suffix_hits")},
            "retained_memory": counter("retained_bytes", memory=True)}


def recorded_timing(decisions, key):
    values = [d[key] for d in decisions if type(d.get(key)) in (int, float)
              and math.isfinite(d[key]) and d[key] >= 0]
    return {"recorded_decisions": len(values), "missing_decisions": len(decisions) - len(values),
            "seconds": sum(values) / 1000 if values else None}


def summarize(games, manifest):
    if not games:
        raise ValueError("No finished games to report")
    if len({g["index"] for g in games}) != len(games):
        raise ValueError("Duplicate game indices in input")
    mode = manifest.get("rotation_mode", "teams-and-seats")
    if mode not in ("teams", "teams-and-seats"):
        raise ValueError("Unknown rotation mode")
    size = 2 if mode == "teams" else 4
    if manifest.get("games_per_deal", size) != size:
        raise ValueError("Manifest rotation block size mismatch")
    for g in games:
        if not 0 <= g["index"] < manifest["games"]:
            raise ValueError("Game index outside requested sample")
        if g["index"] != size * g["deal_index"] + g["rotation"] or not 0 <= g["rotation"] < size:
            raise ValueError("Invalid deal/rotation identity")
        if g.get("rotation_mode", "teams-and-seats") != mode or g.get("games_per_deal", size) != size:
            raise ValueError("Worker/manifest rotation mismatch")
        if "hands_turned" in g and g["hands_turned"] != (mode == "teams-and-seats" and g["rotation"] >= 2):
            raise ValueError("Incorrect hand rotation")
        if manifest.get("policy_seed") is not None:
            seeds = g.get("policy_seeds") or [p.get("policy_seed") for p in g.get("policies", [])]
            if seeds != [manifest["policy_seed"]] * 4:
                raise ValueError("Worker/manifest policy seed mismatch")
        if g["lineup"] != [(s + 4 - g["rotation"]) % 4 for s in range(4)]:
            raise ValueError("Incorrect policy-slot rotation")
        m = g["metrics"]
        if sum(m["card_points"]) != m["card_point_ceiling"]:
            raise ValueError("Card points do not reconcile with their ceiling")
        if [c + p for c, p in zip(m["card_points"], m["pair_points"])] != m["team_points"]:
            raise ValueError("Card and pair points do not reconcile with team points")
    n = len(games)
    teams = []
    for team in (0, 1):
        team_decisions = [d for g in games for d in g["decisions"] if g["lineup"][d["seat"]] % 2 == team]
        score = lambda g: team_value(g, g["metrics"]["scores"], team)
        outcome = lambda g: (score(g) > team_value(g, g["metrics"]["scores"], 1 - team)) - (score(g) < team_value(g, g["metrics"]["scores"], 1 - team))
        declared = [g for g in games if (c := g["metrics"]["contract"]) and team_index(g, c["team"]) == team]
        raised = [g for g in declared if g["metrics"]["contract"]["raise_amount"] > 0]
        readings = [r for g in games for r in g["metrics"]["communication"]["readings"]
                    if g["lineup"][r["seat"]] % 2 == team]
        recognized = [r for r in readings if r["supported"] is not None]
        cards = sum(team_value(g, g["metrics"]["card_points"], team) for g in games)
        pairs = sum(team_value(g, g["metrics"]["pair_points"], team) for g in games)
        counters = Counter()
        for g in games:
            for seat, counts in enumerate(g["metrics"]["seats"]):
                if g["lineup"][seat] % 2 == team:
                    counters.update(counts)
        value_buckets = defaultdict(lambda: {"declared": 0, "made": 0})
        for g in declared:
            c = g["metrics"]["contract"]
            value_buckets[str(c["final_value"])]["declared"] += 1
            value_buckets[str(c["final_value"])]["made"] += int(c["made"])
        teams.append({
            "team": team, "slots": [team, team + 2],
            "policies": [manifest["policies"][team], manifest["policies"][team + 2]],
            "score_per_game": mean([score(g) for g in games]),
            "wins": sum(outcome(g) > 0 for g in games), "losses": sum(outcome(g) < 0 for g in games),
            "draws": sum(outcome(g) == 0 for g in games),
            "win_rate": sum(outcome(g) > 0 for g in games) / n,
            "decision_time": latency([d["ms"] for d in team_decisions]),
            "card_points_per_game": cards / n, "pair_points_per_game": pairs / n,
            "card_ceiling_share": cards / sum(g["metrics"]["card_point_ceiling"] for g in games),
            "pair_ceiling_share": pairs / sum(g["metrics"]["pair_point_ceiling"] for g in games),
            "total_ceiling_share": (cards + pairs) / sum(g["metrics"]["card_point_ceiling"] + g["metrics"]["pair_point_ceiling"] for g in games),
            "schwarz_suffered": sum(team_value(g, g["metrics"]["schwarz_teams"], team) for g in games),
            "schwarz_inflicted": sum(team_value(g, g["metrics"]["schwarz_teams"], 1 - team) for g in games),
            "contracts": len(declared), "contracts_made": sum(g["metrics"]["contract"]["made"] for g in declared),
            "contract_success_rate": ratio(sum(g["metrics"]["contract"]["made"] for g in declared), len(declared)),
            "mean_contract": mean([g["metrics"]["contract"]["final_value"] for g in declared]),
            "mean_contract_margin": mean([team_value(g, g["metrics"]["team_points"], team) - g["metrics"]["contract"]["final_value"] for g in declared]),
            "raised_games": len(raised), "raised_contracts_made": sum(g["metrics"]["contract"]["made"] for g in raised),
            "raise_success_rate": ratio(sum(g["metrics"]["contract"]["made"] for g in raised), len(raised)),
            "mean_raise": mean([g["metrics"]["contract"]["raise_amount"] for g in raised]),
            "actions": dict(counters), "contract_distribution": dict(sorted(value_buckets.items(), key=lambda x: int(x[0]))),
            "communication": {
                "recognizable": len(recognized), "supported": sum(r["supported"] for r in recognized),
                "support_rate": ratio(sum(r["supported"] for r in recognized), len(recognized)),
                "ambiguous_bids": sum(r["ambiguous"] for r in readings),
                "unrecognized_bids": sum(r["target"] is not None and r["message"] is None for r in readings),
                "passes_with_available_message": sum(bool(r["available_at_pass"]) for r in readings),
                "message_counts": dict(Counter(r["message"] for r in recognized)),
            },
        })
    slots = []
    for slot in range(4):
        decisions = [d for g in games for d in g["decisions"] if g["lineup"][d["seat"]] == slot]
        phases, rules = defaultdict(list), defaultdict(list)
        complete = incomplete = evaluated = reused = recorded = 0
        for d in decisions:
            phases[d["phase"]].append(d["ms"])
            rules[d["rule"]].append(d["ms"])
            facts = {f["key"]: f["value"] for f in d["facts"]}
            complete += facts.get("forecast_complete") == 1
            incomplete += facts.get("forecast_complete") == 0
            if "forecast_decisions" in facts:
                recorded += 1
                evaluated += facts["forecast_decisions"]
                reused += facts.get("forecast_reused_decisions", 0)
        capture_timing = recorded_timing(decisions, "capture_ms")
        serialization_timing = recorded_timing(decisions, "assessment_serialization_ms")
        slots.append({"slot": slot, "policy": manifest["policies"][slot],
                      "all_decisions": latency([d["ms"] for d in decisions]),
                      "nonforced_decisions": latency([d["ms"] for d in decisions if d["rule"] != "HX-1"]),
                      "phases": {p: latency(v) for p, v in phases.items()},
                      "rules": {p: latency(v) for p, v in rules.items()},
                      "complete_forecasts": complete, "incomplete_forecasts": incomplete,
                      "assessments": {
                          "instrumented_decisions": sum(d.get("assessment_recorded") is True for d in decisions),
                          "value_assessments_present": sum(value_assessment_presence(d) is True for d in decisions),
                          "value_presence_unknown_decisions": sum(value_assessment_presence(d) is None for d in decisions),
                          "completed_assessments": sum(value_rank_complete(d) is True for d in decisions),
                          "completion_unknown_decisions": sum(value_rank_complete(d) is None for d in decisions),
                          "selected_action_matches": sum(value_rank_complete(d) is True and d.get("assessment_matches_chosen_action") is True for d in decisions),
                          "selected_action_differs": sum(value_rank_complete(d) is True and d.get("assessment_matches_chosen_action") is False for d in decisions),
                          "capture_seconds": capture_timing["seconds"],
                          "serialization_seconds": serialization_timing["seconds"],
                          "capture_timing": capture_timing, "serialization_timing": serialization_timing},
                      "sequences": sequence_metrics(decisions),
                      "forecast_reuse": {"recorded_forecasts": recorded,
                                         "evaluated_decisions": evaluated, "reused_decisions": reused,
                                         "hit_rate": ratio(reused, evaluated + reused)}})
    delta = clustered_mean(games, lambda g: team_value(g, g["metrics"]["scores"], 0) - team_value(g, g["metrics"]["scores"], 1))
    complete = n == manifest["games"] and manifest.get("status") == "complete"
    # Whole-sample selection, independent of changed decisions. These are review
    # priorities and favorable/close controls, never assertions of bad/good play.
    groups = defaultdict(list)
    for g in games:
        groups[g["deal_index"]].append(g)
    blocks = [{"deal_index": d, "indices": sorted(g["index"] for g in rows),
               "score_difference_per_game": mean([team_value(g, g["metrics"]["scores"], 0) -
                                                  team_value(g, g["metrics"]["scores"], 1) for g in rows])}
              for d, rows in groups.items() if len(rows) == size] if complete else []
    review = {"selection": "complete blocks; engine margin only, not decision quality",
              "lowest_margin": sorted(blocks, key=lambda b: (b["score_difference_per_game"], b["deal_index"]))[:6],
              "highest_margin": sorted(blocks, key=lambda b: (-b["score_difference_per_game"], b["deal_index"]))[:6],
              "close_controls": sorted(blocks, key=lambda b: (abs(b["score_difference_per_game"]), b["deal_index"]))[:6]}
    return {"schema": "marjapussi.benchmark-report/1", "games": n,
            "requested_games": manifest["games"], "complete": complete, "review_deals": review,
            "distinct_deals": len({g["deal_index"] for g in games}),
            "all_pass_games": sum(g["metrics"]["contract"] is None for g in games),
            "schwarz_games": sum(g["metrics"]["schwarz"] for g in games),
            "score_difference_per_game": delta, "teams": teams, "slots": slots,
            "manifest": manifest}


def number(value):
    return "—" if value is None else f"{value:,.2f}"


def percent(value):
    return "—" if value is None else f"{100 * value:.1f}%"


def markdown(report):
    a, b = report["teams"]
    delta = report["score_difference_per_game"]
    interval = delta["ci95"]
    lines = ["# Marja policy comparison", "",
             f"{report['games']:,}/{report['requested_games']:,} finished games · {report['distinct_deals']:,} distinct deals · "
             f"{report['manifest']['workers']} worker(s)", "",
             f"Team A: **{' + '.join(a['policies'])}** (slots 0/2). Team B: **{' + '.join(b['policies'])}** (slots 1/3).", "",
             f"**A − B: {number(delta['mean'])} engine score points/game**. " +
             (f"Approximate deal-clustered 95% interval: [{number(interval[0])}, {number(interval[1])}]." if interval else "Too few deals for an interval."), "",
             "| Measure | Team A | Team B |", "|---|---:|---:|"]
    specs = [("Engine score / game", "score_per_game", number), ("Wins", "wins", str),
             ("Losses", "losses", str), ("Draws", "draws", str), ("Win rate (draws separate)", "win_rate", percent),
             ("Card + last-trick points / game", "card_points_per_game", number),
             ("Share of 140 card points", "card_ceiling_share", percent),
             ("Pair points / game", "pair_points_per_game", number), ("Share of 280 pair points", "pair_ceiling_share", percent),
             ("Share of 420 total points", "total_ceiling_share", percent),
             ("Schwarz suffered", "schwarz_suffered", str), ("Schwarz inflicted", "schwarz_inflicted", str),
             ("Contracts taken", "contracts", str), ("Contracts made", "contracts_made", str),
             ("Contract success", "contract_success_rate", percent), ("Average contract", "mean_contract", number),
             ("Average points above/below contract", "mean_contract_margin", number),
             ("Games raised after exchange", "raised_games", str), ("Raised contracts made", "raised_contracts_made", str),
             ("Success after raising", "raise_success_rate", percent), ("Average raise when raised", "mean_raise", number)]
    lines += [f"| {label} | {fmt(a[key])} | {fmt(b[key])} |" for label, key, fmt in specs]
    for label, key, fmt in [("Decision count (both teammates)", "count", str),
                            ("Average decision ms", "mean_ms", number), ("Decision p95 ms", "p95_ms", number)]:
        lines.append(f"| {label} | {fmt(a['decision_time'][key])} | {fmt(b['decision_time'][key])} |")
    lines += ["", "## Decision time", "", "| Slot / policy | Decisions | Average ms | Median ms | p95 ms | Maximum ms | Incomplete forecasts |",
              "|---|---:|---:|---:|---:|---:|---:|"]
    for slot in report["slots"]:
        t = slot["all_decisions"]
        lines.append(f"| {slot['slot']}: {slot['policy']} | {t['count']} | {number(t['mean_ms'])} | {number(t['median_ms'])} | {number(t['p95_ms'])} | {number(t['max_ms'])} | {slot['incomplete_forecasts']} |")
    lines += ["", "Times measure Agent::act wall time, including forced actions; setup and posthoc diagnostics are excluded. "
              "Nonforced and per-phase timings are in summary.json. Parallel CPU contention can affect deadline-limited decisions.", "",
              "| Slot / policy | Forecasts recording reuse | Evaluated decisions | Reused decisions | Reuse share |",
              "|---|---:|---:|---:|---:|"]
    for slot in report["slots"]:
        reuse = slot["forecast_reuse"]
        lines.append(f"| {slot['slot']}: {slot['policy']} | {reuse['recorded_forecasts']} | {number(reuse['evaluated_decisions'])} | {number(reuse['reused_decisions'])} | {percent(reuse['hit_rate'])} |")
    lines += ["", "Reuse counts include completed and interrupted comparisons when the policy records them. Older policies may omit these counters.", "",
              "## Recorded value rankings and sequence execution", "",
              "| Slot / policy | Complete value ranks | Value completion unknown | Sequence traces | Formation | Repair | Prepared | Retry | Deferred | Mode unknown |",
              "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|"]
    for slot in report["slots"]:
        sequence, assessment = slot["sequences"], slot["assessments"]
        modes = sequence["modes"]
        lines.append(f"| {slot['slot']}: {slot['policy']} | {assessment['completed_assessments']} | {assessment['completion_unknown_decisions']} | "
                     f"{sequence['recorded_decisions']}/{sequence['total_decisions']} | " +
                     " | ".join(str(modes[m]) for m in SEQUENCE_MODES) + f" | {sequence['mode_unknown_decisions']} |")
    lines += ["", "A sequence-only prepared/retry step does not count as a newly completed value ranking. "
              "Counts describe exported evidence, not absence of internal reasoning in uninstrumented policies.", "",
              "| Slot / policy | Completed candidates | Attempted candidates | Total candidates | Executed transitions | Represented transitions | Actual canonical solver nodes | Represented canonical solver nodes |",
              "|---|---:|---:|---:|---:|---:|---:|---:|"]

    def observed(counter):
        value = counter["total"]
        if value is None:
            return "— (not recorded)"
        return f"{value:,} ({counter['recorded_decisions']} recorded, {counter['missing_decisions']} missing)"

    for slot in report["slots"]:
        sequence = slot["sequences"]
        counts = sequence["candidate_counts"]
        cells = [counts[k] for k in ("completed_candidates", "attempted_candidates", "total_candidates")]
        cells += [sequence["actual_work"]["transitions"], sequence["represented_work"]["transitions"]]
        cells += [sequence["actual_work"]["solver_nodes"], sequence["represented_work"]["solver_nodes"]]
        lines.append(f"| {slot['slot']}: {slot['policy']} | " + " | ".join(observed(c) for c in cells) + " |")
    lines += ["", "Work totals use recorded counters only: executed transitions and represented logical transitions are separate. "
              "Missing attempted/total counts stay unknown; neither completed candidates nor available programs establish those counts. "
              "A completion rate uses only decisions recording both completed and attempted counts. "
              f"{SOLVER_EFFORT_MEANING} "
              "Response calls/hits, suffix hits, comparison calls and maximum reported cache-retention bytes are in summary.json. "
              "Retention bytes are a per-trace cache ledger, not process peak memory. Full traces remain in games NDJSON.", "",
              "## Bidding communication", "",
              "These are common-convention content checks against the speaker’s hand. They do not measure whether the receiver understood, "
              "or whether a strategic overbid was good. Ambiguous and unrecognized bids remain separate.", "",
              "| Measure | Team A | Team B |", "|---|---:|---:|"]
    for label, key, fmt in [("Unambiguous readings", "recognizable", str), ("Supported by hand", "supported", str),
                            ("Content support rate", "support_rate", percent), ("Ambiguous bids", "ambiguous_bids", str),
                            ("Unrecognized bids", "unrecognized_bids", str), ("Passes with live message content", "passes_with_available_message", str)]:
        lines.append(f"| {label} | {fmt(a['communication'][key])} | {fmt(b['communication'][key])} |")
    lines += ["", "## Interpretation", "",
              "- Engine score follows the contract and Schwarz rules; it differs from raw card/pair points.",
              "- The 140/280/420 denominators are scoring ceilings, not points provably attainable from these hands.",
              "- Contract margins describe the played outcomes; surplus is not proof a higher bid would have been safe.",
              f"- {report['manifest'].get('games_per_deal', 4)} games of the same deal form one uncertainty cluster. A final partial block has less seat balance.",
              "- Current assessment traces are retained from actual act calls; historical policies lack that instrumentation. No second strategy evaluator fills the gap.",
              "- With recording enabled, decision ms measures inner policy act; capture/serialization costs are separate and included in total job time.",
              "- Forecast completion reports the policy’s recorded flag; a completed forecast is not a proof of correct play.",
              "- Results compare these lineups and resource settings; they do not establish absolute playing strength.", ""]
    if not report["complete"]:
        lines += ["**Partial run: do not treat this report as the fixed-sample qualification result.**", ""]
    return "\n".join(lines)


def write_reports(directory, games, manifest):
    directory = Path(directory)
    games = sorted(games, key=lambda g: g["index"])
    result = summarize(games, manifest)
    (directory / "summary.json").write_text(json.dumps(result, indent=2, ensure_ascii=False), encoding="utf-8")
    text = markdown(result)
    (directory / "report.md").write_text(text, encoding="utf-8")
    # A standalone, dependency-free readable view. Keep raw data beside it for deeper analysis.
    sections = []
    for slot in result["slots"]:
        rows = "".join(f"<tr><td>{html.escape(name)}</td><td>{t['count']}</td><td>{number(t['mean_ms'])}</td><td>{number(t['total_seconds'])}</td></tr>"
                       for name, t in sorted(slot["rules"].items(), key=lambda x: -x[1]["total_seconds"]))
        sections.append(f"<details><summary>Slot {slot['slot']} · {html.escape(slot['policy'])}: time by rule</summary><table><tr><th>Rule</th><th>Decisions</th><th>Mean ms</th><th>Total seconds</th></tr>{rows}</table></details>")
    # Convert the intentionally small report vocabulary; all content is escaped first.
    body, in_table = [], False
    for line in text.splitlines():
        if line.startswith("|"):
            if line.startswith("|---"):
                continue
            if not in_table:
                body.append("<table>")
                in_table = True
            body.append("<tr>" + "".join(f"<td>{html.escape(c.strip())}</td>" for c in line.strip("|").split("|")) + "</tr>")
            continue
        if in_table:
            body.append("</table>")
            in_table = False
        escaped = html.escape(line).replace("**", "")
        if line.startswith("# "):
            body.append(f"<h1>{escaped[2:]}</h1>")
        elif line.startswith("## "):
            body.append(f"<h2>{escaped[3:]}</h2>")
        elif line:
            body.append(f"<p>{escaped}</p>")
    if in_table:
        body.append("</table>")
    page = '<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Marja policy comparison</title><style>body{font:16px/1.5 system-ui,sans-serif;max-width:1050px;margin:3rem auto;padding:0 1.5rem;color:#17233b;background:#f6f8fb}table{border-collapse:collapse;background:white;width:100%;margin:1rem 0}td,th{padding:.5rem .8rem;border-bottom:1px solid #e3e8f0;text-align:right}td:first-child,th:first-child{text-align:left}tr:first-child{font-weight:650}h1{color:#4937ad}h2{margin-top:2rem}details{padding:1rem;background:white;margin:.7rem 0}summary{cursor:pointer}p{max-width:95ch}</style>'
    (directory / "report.html").write_text(page + "".join(body + sections) + "</html>", encoding="utf-8")
    with (directory / "games.csv").open("w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f)
        writer.writerow(["index", "deal_index", "deal_seed", "rotation", "score_A", "score_B", "cards_A", "cards_B", "pairs_A", "pairs_B", "contract", "made", "schwarz"])
        for game in games:
            m = game["metrics"]
            writer.writerow([game[k] for k in ("index", "deal_index", "deal_seed", "rotation")] +
                            [team_value(game, m[key], team) for key in ("scores", "card_points", "pair_points") for team in (0, 1)] +
                            [m["contract"]["final_value"] if m["contract"] else None, m["contract"]["made"] if m["contract"] else None, m["schwarz"]])
    return result

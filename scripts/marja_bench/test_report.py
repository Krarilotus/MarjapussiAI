import copy
import math
import unittest

from report import clustered_mean, compact_game, markdown, summarize, value_assessment_presence, value_rank_complete


def game(index, delta=20, raised=False, size=4):
    rotation = index % size
    lineup = [(s + 4 - rotation) % 4 for s in range(4)]
    # Team A always wins, although its physical seat parity alternates.
    a_team = next(t for t in (0, 1) if lineup[t] % 2 == 0)
    ordered = lambda a, b: [a, b] if a_team == 0 else [b, a]
    seats = [{"auction_bids": 1, "auction_passes": 1, "raise_opportunities": 0,
              "raises": 0, "raise_amount": 0, "questions": 0, "melds": 0, "negative_answers": 0} for _ in range(4)]
    return {"index": index, "deal_index": index // size, "deal_seed": 17 + index // size,
            **({"rotation_mode": "teams", "games_per_deal": 2, "hands_turned": False} if size == 2 else {}),
            "rotation": rotation, "lineup": lineup, "decisions": [
                {"seat": s, "ms": lineup[s] + 1.0, "phase": "play", "rule": "test",
                 "facts": [{"key": "forecast_complete", "value": int(lineup[s] != 0)}]} for s in range(4)],
            "metrics": {"scores": ordered(100 + delta, 100), "card_points": ordered(80, 60),
                        "pair_points": ordered(100, 0), "team_points": ordered(180, 60),
                        "card_point_ceiling": 140, "pair_point_ceiling": 280,
                        "schwarz": False, "schwarz_teams": [False, False], "seats": seats,
                        "contract": {"team": a_team, "final_value": 130 if raised else 120,
                                     "auction_value": 120, "raise_amount": 10 if raised else 0, "made": True},
                        "communication": {"readings": [
                            {"seat": a_team, "target": 120, "message": "ace", "supported": True,
                             "ambiguous": False, "available_at_pass": []},
                            {"seat": 1 - a_team, "target": 140, "message": "small_or_halves", "supported": None,
                             "ambiguous": True, "available_at_pass": []}]}}}


class ReportTest(unittest.TestCase):
    def test_two_game_blocks_preserve_denominators_and_cluster_uncertainty(self):
        rows = [game(i, delta=(i // 2 % 2) * 40 - 20, size=2) for i in range(256)]
        for row in rows:
            row["policy_seeds"] = [0] * 4
        manifest = {"policies": ["new", "previous", "new", "previous"], "games": 256,
                    "rotation_mode": "teams", "games_per_deal": 2, "policy_seed": 0,
                    "status": "complete", "workers": 2}
        report = summarize(rows, manifest)
        self.assertTrue(report["complete"])
        self.assertEqual(report["distinct_deals"], 128)
        self.assertEqual(report["score_difference_per_game"]["deal_clusters"], 128)
        self.assertEqual(report["score_difference_per_game"]["mean"], 0)
        self.assertEqual(report["teams"][0]["wins"], 128)
        self.assertEqual(report["teams"][0]["losses"], 128)
        self.assertEqual(report["teams"][0]["contracts"], 256)
        independent = [{"deal_index": i, "x": (i % 2) * 40 - 20} for i in range(128)]
        self.assertEqual(report["score_difference_per_game"], clustered_mean(independent, lambda g: g["x"]))
        self.assertEqual(report["review_deals"]["lowest_margin"][0]["indices"], [0, 1])
        self.assertEqual(report["review_deals"]["highest_margin"][0]["indices"], [2, 3])
        partial = summarize(rows[:2], dict(manifest, status="interrupted_or_failed"))
        self.assertEqual(partial["review_deals"]["lowest_margin"], [])
        for field, value in (("hands_turned", True), ("rotation_mode", "teams-and-seats"),
                             ("policy_seeds", [1] * 4)):
            bad = copy.deepcopy(rows)
            bad[0][field] = value
            with self.assertRaises(ValueError):
                summarize(bad, manifest)

    def test_assessment_presence_survives_compaction_without_retaining_large_traces(self):
        row = game(0)
        row["decisions"][0].update(assessment_recorded=True, assessment={"trace": ["actual"],
                                   "selected": 0, "ranking": 3, "candidates": [{}], "cohort_complete": True},
                                   assessment_matches_chosen_action=True, capture_ms=2,
                                   assessment_serialization_ms=3)
        compact = compact_game(row)
        self.assertNotIn("assessment", compact["decisions"][0])
        report = summarize([compact], {"policies": ["a", "b", "c", "d"], "games": 1})
        a = report["slots"][0]["assessments"]
        self.assertEqual((a["instrumented_decisions"], a["completed_assessments"], a["selected_action_matches"]), (1, 1, 1))
        self.assertEqual(a["capture_seconds"], .002)
        self.assertEqual(a["serialization_seconds"], .003)

    def test_sequence_only_capture_is_not_a_complete_value_rank_even_with_stale_presence_flags(self):
        row = game(0)
        row["policy_seeds"] = [0] * 4
        row["decisions"][0].update(
            assessment_recorded=True, assessment_present=True, value_assessment_present=False,
            assessment_matches_chosen_action=True, sequence_present=True,
            assessment={"value_assessment_present": False, "extensions": {"sequence": {
                "mode": "prepared", "belief_scope": {"kind": "prepared_conditional"},
                "failure": None, "work": {"transitions": 0, "represented_transitions": 0,
                    "completed_candidates": 0}, "information_sets": ["private-large-view"],
                "programs": ["large-program"], "terminals": ["large-terminal"]}}})
        original = copy.deepcopy(row)
        compact = compact_game(row)
        d = compact["decisions"][0]
        self.assertFalse(d["value_assessment_present"])
        self.assertFalse(d["value_rank_complete"])
        self.assertTrue(d["sequence_present"])
        self.assertNotIn("assessment", d)
        self.assertNotIn("information_sets", d["sequence"])
        self.assertNotIn("programs", d["sequence"])
        self.assertNotIn("terminals", d["sequence"])
        self.assertIsNone(d["sequence"]["work"]["attempted_candidates"])
        self.assertEqual(compact_game(compact), compact)
        self.assertEqual(row, original, "compaction must leave retained evidence unchanged")
        manifest = {"policies": ["a", "b", "c", "d"], "games": 1}
        raw_report = summarize([row], manifest)
        report = summarize([compact], manifest)
        self.assertEqual(raw_report, report)
        assessments = report["slots"][0]["assessments"]
        self.assertEqual(assessments["instrumented_decisions"], 1)
        self.assertEqual(assessments["completed_assessments"], 0)
        self.assertEqual(assessments["selected_action_matches"], 0)
        sequence = report["slots"][0]["sequences"]
        self.assertEqual(sequence["modes"]["prepared"], 1)
        self.assertEqual(sequence["candidate_counts"]["completed_candidates"]["total"], 0)
        self.assertIsNone(sequence["candidate_counts"]["attempted_candidates"]["total"])
        self.assertIsNone(sequence["paired_completed_attempted"]["completion_rate"])

    def test_sequence_modes_work_and_candidate_completion_use_only_recorded_denominators(self):
        row = game(0)
        base = row["decisions"][0]
        decisions = []
        for mode, executed, represented, completed, retained in (
                ("formation", 10, 20, 2, 100), ("repair", 6, 12, 1, 80),
                ("prepared", 0, 0, 0, 0), ("retry", 0, 0, 0, 0), ("deferred", 4, 4, 0, 40)):
            decision = copy.deepcopy(base)
            work = {"transitions": executed, "represented_transitions": represented,
                    "comparisons": int(completed > 0), "response_calls": executed,
                    "response_hits": represented - executed, "suffix_hits": int(represented > executed),
                    "retained_bytes": retained, "completed_candidates": completed}
            sequence = {"mode": mode, "work": work,
                        "belief_scope": {"kind": "prepared_conditional" if mode in ("prepared", "retry") else "current_information"},
                        "failure": {"kind": "budget"} if mode == "deferred" else None}
            if mode in ("formation", "repair"):
                sequence.update(attempted_candidates=2, total_candidates=2 if mode == "formation" else 3)
            value = mode in ("formation", "repair")
            assessment = {"value_assessment_present": value, "extensions": {"sequence": sequence}}
            if value:
                assessment.update(selected=0, ranking=3, candidates=[{}], cohort_complete=True)
            decision.update(assessment=assessment, value_assessment_present=value,
                            assessment_recorded=True, assessment_matches_chosen_action=True)
            decisions.append(decision)
        row["decisions"] = decisions + row["decisions"][1:]
        manifest = {"policies": ["a", "b", "c", "d"], "games": 1, "workers": 1}
        raw_report = summarize([row], manifest)
        report = summarize([compact_game(row)], manifest)
        self.assertEqual(raw_report, report)
        slot = report["slots"][0]
        self.assertEqual(slot["assessments"]["completed_assessments"], 2)
        sequence = slot["sequences"]
        self.assertEqual(sequence["modes"], dict.fromkeys(("formation", "repair", "prepared", "retry", "deferred"), 1))
        self.assertEqual(sequence["actual_work"]["transitions"]["total"], 20)
        self.assertEqual(sequence["represented_work"]["transitions"]["total"], 36)
        self.assertEqual(sequence["candidate_counts"]["completed_candidates"]["total"], 3)
        attempted = sequence["candidate_counts"]["attempted_candidates"]
        self.assertEqual((attempted["total"], attempted["recorded_decisions"], attempted["missing_decisions"]), (4, 2, 3))
        self.assertEqual(sequence["paired_completed_attempted"]["completion_rate"], .75)
        self.assertEqual(sequence["candidate_counts"]["total_candidates"]["total"], 5)
        self.assertEqual(sequence["retained_memory"]["max_bytes"], 100)
        self.assertNotIn("total", sequence["retained_memory"], "cache footprints are not summed into process memory")
        self.assertEqual(sequence["failures"], {"budget": 1})
        text = markdown(report)
        self.assertIn("A sequence-only prepared/retry step does not count", text)
        self.assertIn("Executed transitions | Represented transitions", text)
        self.assertIn("not recorded", text)

    def test_missing_invalid_or_legacy_unclassified_value_data_remains_unknown(self):
        self.assertFalse(value_assessment_presence({"value_assessment_present": False,
                         "assessment": {"value_assessment_present": True}}))
        self.assertIsNone(value_assessment_presence({"value_assessment_present": "true"}))
        self.assertIsNone(value_rank_complete({"assessment_present": True}))
        self.assertIsNone(value_rank_complete({"assessment": {"trace": []}}))
        self.assertIsNone(value_rank_complete({"value_assessment_present": True,
                         "assessment": {"selected": 0, "ranking": 1, "candidates": [{}]}}))
        self.assertFalse(value_rank_complete({"value_assessment_present": True,
                         "assessment": {"cohort_complete": False}}))
        row = game(0)
        row["decisions"][0].update(assessment_present=True, assessment_recorded=True)
        report = summarize([compact_game(row)], {"policies": ["a", "b", "c", "d"], "games": 1})
        self.assertEqual(report["slots"][0]["assessments"]["completed_assessments"], 0)
        self.assertEqual(report["slots"][0]["assessments"]["completion_unknown_decisions"], 1)
        self.assertIsNone(report["slots"][0]["assessments"]["capture_seconds"])
        self.assertEqual(report["slots"][0]["assessments"]["capture_timing"]["missing_decisions"], 1)

    def test_sequence_bad_counter_types_and_missing_metadata_are_explicit(self):
        row = game(0)
        row["decisions"][0]["assessment"] = {"value_assessment_present": False, "extensions": {"sequence": {
            "mode": "future-mode", "work": {"transitions": 1.5, "represented_transitions": 12,
                "response_calls": True, "completed_candidates": -1, "total_candidates": "3"},
            "failure": {"kind": []}}}}
        compact = compact_game(row)
        work = compact["decisions"][0]["sequence"]["work"]
        for key in ("transitions", "response_calls", "completed_candidates", "total_candidates", "attempted_candidates"):
            self.assertIsNone(work[key])
        report = summarize([compact], {"policies": ["a", "b", "c", "d"], "games": 1})
        sequence = report["slots"][0]["sequences"]
        self.assertEqual(sequence["mode_unknown_decisions"], 1)
        self.assertEqual(sequence["belief_scope_unknown_decisions"], 1)
        self.assertEqual(sequence["failure_kind_unknown_decisions"], 1)
        self.assertEqual(sequence["actual_work"]["transitions"]["invalid_decisions"], 1)
        self.assertIsNone(sequence["actual_work"]["transitions"]["total"])
        self.assertEqual(sequence["represented_work"]["transitions"]["total"], 12)
        self.assertIsNone(sequence["paired_completed_attempted"]["consistent"])

    def test_impossible_completed_attempted_counts_do_not_produce_a_completion_rate(self):
        row = game(0)
        row["decisions"][0]["assessment"] = {"value_assessment_present": False, "extensions": {"sequence": {
            "mode": "deferred", "failure": None,
            "work": {"completed_candidates": 3, "attempted_candidates": 2}}}}
        report = summarize([compact_game(row)], {"policies": ["a", "b", "c", "d"], "games": 1})
        paired = report["slots"][0]["sequences"]["paired_completed_attempted"]
        self.assertFalse(paired["consistent"])
        self.assertIsNone(paired["completion_rate"])
        self.assertEqual(paired["recorded_decisions"], 1)

    def test_rotations_preserve_team_and_individual_attribution(self):
        rows = [game(i, raised=True) for i in range(8)]
        report = summarize(rows, {"policies": ["a", "b", "c", "d"], "games": 8, "workers": 2})
        a, b = report["teams"]
        self.assertEqual((a["wins"], b["wins"], a["contracts"], b["contracts"]), (8, 0, 8, 0))
        self.assertEqual(a["mean_raise"], 10)
        self.assertEqual(a["card_points_per_game"], 80)
        self.assertAlmostEqual(a["pair_ceiling_share"], 100 / 280)
        self.assertEqual(a["communication"]["supported"], 8)
        self.assertEqual(b["communication"]["recognizable"], 0)
        self.assertEqual(b["communication"]["ambiguous_bids"], 8)
        for slot in range(4):
            self.assertEqual(report["slots"][slot]["all_decisions"]["mean_ms"], slot + 1)
        self.assertEqual(report["slots"][0]["incomplete_forecasts"], 8)

    def test_repeating_each_deal_does_not_falsely_narrow_interval(self):
        single = [{"deal_index": i, "x": x} for i, x in enumerate((10, -10, 20, -20))]
        repeated = [g for g in single for _ in range(4)]
        first, second = [clustered_mean(rows, lambda g: g["x"]) for rows in (single, repeated)]
        self.assertEqual(first, second)
        expected_se = math.sqrt((100 + 100 + 400 + 400) / 3 / 4)
        self.assertAlmostEqual(first["ci95"][1], 1.96 * expected_se)

    def test_reuse_survives_compaction_and_includes_interrupted_comparisons(self):
        rows = [game(i) for i in range(4)]
        for row in rows:
            for d in row["decisions"]:
                if row["lineup"][d["seat"]] == 0:
                    d["facts"] += [{"key": "forecast_decisions", "value": 6},
                                   {"key": "forecast_reused_decisions", "value": 4}]
        report = summarize([compact_game(row) for row in rows],
                           {"policies": ["a", "b", "c", "d"], "games": 4, "status": "complete"})
        first = report["slots"][0]
        self.assertEqual(first["incomplete_forecasts"], 4)
        self.assertEqual(first["forecast_reuse"], {"recorded_forecasts": 4,
                         "evaluated_decisions": 24, "reused_decisions": 16, "hit_rate": .4})
        self.assertIsNone(report["slots"][1]["forecast_reuse"]["hit_rate"])

    def test_unequal_final_block_uses_game_weighted_mean(self):
        rows = [{"deal_index": 0, "x": 10}] * 4 + [{"deal_index": 1, "x": 30}]
        self.assertEqual(clustered_mean(rows, lambda g: g["x"])["mean"], 14)

    def test_missing_and_duplicate_records_are_not_complete_results(self):
        manifest = {"policies": ["a", "b", "c", "d"], "games": 8, "workers": 1}
        self.assertFalse(summarize([game(i) for i in range(4)], manifest)["complete"])
        with self.assertRaises(ValueError):
            summarize([game(0), game(0)], manifest)
        invalid = copy.deepcopy(game(0))
        invalid["metrics"]["card_points"][0] += 1
        with self.assertRaises(ValueError):
            summarize([invalid], manifest)
        full = [game(i) for i in range(8)]
        self.assertFalse(summarize(full, dict(manifest, status="interrupted_or_failed"))["complete"])
        self.assertTrue(summarize(full, dict(manifest, status="complete"))["complete"])


if __name__ == "__main__":
    unittest.main()

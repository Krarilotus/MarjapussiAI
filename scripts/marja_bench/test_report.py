import copy
import math
import unittest

from report import clustered_mean, compact_game, summarize


def game(index, delta=20, raised=False):
    rotation = index % 4
    lineup = [(s + 4 - rotation) % 4 for s in range(4)]
    # Team A always wins, although its physical seat parity alternates.
    a_team = next(t for t in (0, 1) if lineup[t] % 2 == 0)
    ordered = lambda a, b: [a, b] if a_team == 0 else [b, a]
    seats = [{"auction_bids": 1, "auction_passes": 1, "raise_opportunities": 0,
              "raises": 0, "raise_amount": 0, "questions": 0, "melds": 0, "negative_answers": 0} for _ in range(4)]
    return {"index": index, "deal_index": index // 4, "deal_seed": 17 + index // 4,
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

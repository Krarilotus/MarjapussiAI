import unittest

from changes import change, comparable, margin, our_rules, series_differs


def game(lineup, team, made, scores=(100, -100), value=140, rules=(), totals=(0, 0)):
    return {"lineup": lineup, "metrics": {"scores": list(scores), "contract": {
                "team": team, "final_value": value, "made": made}},
            "series": {"totals_before": list(totals)},
            "decisions": [{"seat": s, "phase": p, "rule": r} for s, p, r in rules]}


def run(**changes):
    m = {"games": 5000, "seed": 1, "policy_seed": 0, "rotation_mode": "series", "deadlines": "off",
         "policies": ["hp1", "c2", "hp1", "c2"]}
    m.update(changes)
    return m


class ChangesTest(unittest.TestCase):
    def test_results_are_read_by_slot_team(self):
        turned = [1, 0, 3, 2]  # Physical seat 0 holds slot 1: physical team 1 is slot team 0.
        self.assertEqual(margin(game(turned, 1, True, scores=(-40, 140))), 180)
        self.assertEqual(change(game(turned, 1, False), game(turned, 1, True)), "ours made")
        self.assertEqual(change(game(turned, 0, True), game(turned, 0, False)), "theirs failed")
        self.assertEqual(change(game(turned, 1, True), game(turned, 1, True, value=160)), "other value")
        self.assertEqual(change(game(turned, 1, True), game(turned, 0, True)), "other declarer")
        self.assertEqual(change(game(turned, 1, True), game(turned, 1, True)), "same result")

    def test_only_the_slot_zero_team_exchange_rules_count(self):
        rules = [(0, "passing_forth", "P-route"), (1, "passing_back", "P-route"), (3, "trick", "HE-1")]
        self.assertEqual(our_rules(game([1, 0, 3, 2], 1, True, rules=rules)), {("passing_back", "P-route")})

    def test_series_carry_over_is_detected(self):
        seats = [0, 1, 2, 3]
        self.assertFalse(series_differs(game(seats, 0, True), game(seats, 0, False)))
        self.assertTrue(series_differs(game(seats, 0, True), game(seats, 0, True, totals=(140, -60))))

    def test_only_paired_runs_compare(self):
        self.assertIsNone(comparable(run(), run(policies=["hp1@x.toml", "c2", "hp1@x.toml", "c2"])))
        self.assertIn("slots 1 and 3", comparable(run(), run(policies=["hp1", "base", "hp1", "base"])))
        self.assertIn("seed", comparable(run(), run(seed=2)))
        self.assertIn("clock", comparable(run(deadlines="wall"), run(deadlines="wall")))


if __name__ == "__main__":
    unittest.main()

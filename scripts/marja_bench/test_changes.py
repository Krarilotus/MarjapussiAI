import unittest

from changes import change, margin, our_rules


def game(lineup, team, made, scores=(100, -100), value=140, rules=()):
    return {"lineup": lineup, "metrics": {"scores": list(scores), "contract": {
                "team": team, "final_value": value, "made": made}},
            "decisions": [{"seat": s, "phase": p, "rule": r} for s, p, r in rules]}


class ChangesTest(unittest.TestCase):
    def test_results_are_read_by_slot_team(self):
        turned = [1, 0, 3, 2]  # Physical seat 0 holds slot 1: physical team 1 is slot team 0.
        self.assertEqual(margin(game(turned, 1, True, scores=(-40, 140))), 180)
        self.assertEqual(change(game(turned, 1, False), game(turned, 1, True)), "ours made")
        self.assertEqual(change(game(turned, 0, True), game(turned, 0, False)), "theirs failed")
        self.assertEqual(change(game(turned, 1, True), game(turned, 1, True, value=160)), "other contract")
        self.assertEqual(change(game(turned, 1, True), game(turned, 1, True)), "same result")

    def test_only_the_slot_zero_team_exchange_rules_count(self):
        rules = [(0, "passing_forth", "P-route"), (1, "passing_back", "P-route"), (3, "trick", "HE-1")]
        self.assertEqual(our_rules(game([1, 0, 3, 2], 1, True, rules=rules)), {("passing_back", "P-route")})


if __name__ == "__main__":
    unittest.main()

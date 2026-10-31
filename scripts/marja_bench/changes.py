"""Attribute a candidate's per-game changes against a reference run of the same games.

Both runs play the same deals, seats, rotation and seeds without deadlines, with the same
policy in slots 1 and 3, so a game's change is the slot-0 team's doing: its engine margin in
the candidate run minus its margin in the reference run. Games are grouped by what changed in
the contract's result (or only in the points) and by the team's exchange rules that differ
from the reference game's. A changed game also changes the series totals the later games of
its series start from, which series-aware decisions read: those games are counted apart.

    python3 scripts/marja_bench/changes.py target/bench-results/<reference> target/bench-results/<candidate>
"""

from collections import defaultdict
import json
from pathlib import Path
import sys

from report import team_index, team_value

WORST = 8  # Most negative games listed for review.
PHASES = ("passing_forth", "passing_back")  # Where the rules that may explain a change act.
SAME_RUN = ("games", "seed", "policy_seed", "rotation_mode", "deadlines")  # Manifest keys that must agree.


def games(run):
    rows = {}
    for path in sorted(Path(run).glob("worker-*.ndjson")):
        with path.open(encoding="utf-8") as f:
            for line in f:
                game = json.loads(line)
                rows[game["index"]] = game
    return rows


def manifest(run):
    with (Path(run) / "manifest.json").open(encoding="utf-8") as f:
        return json.load(f)


def comparable(ref, cand):
    """Why two runs' games are not paired by the slot-0 team alone (None if they are)."""
    for key in SAME_RUN:
        if ref.get(key) != cand.get(key):
            return f"{key} differs: {ref.get(key)} vs {cand.get(key)}"
    if ref.get("deadlines") != "off":
        return "runs with deadlines depend on the clock"
    if [ref["policies"][s] for s in (1, 3)] != [cand["policies"][s] for s in (1, 3)]:
        return "the opponents in slots 1 and 3 differ"
    return None


def margin(game):
    scores = game["metrics"]["scores"]
    return team_value(game, scores, 0) - team_value(game, scores, 1)


def result(game):
    """The contract as the slot teams see it: (declaring slot team, final value, made)."""
    c = game["metrics"]["contract"]
    return None if c is None else (team_index(game, c["team"]), c["final_value"], c["made"])


def our_rules(game):
    """(phase, rule) of the slot-0 team's decisions in `PHASES`."""
    return {(d["phase"], d["rule"]) for d in game["decisions"]
            if d["phase"] in PHASES and game["lineup"][d["seat"]] % 2 == 0}


def change(ref, cand):
    a, b = result(ref), result(cand)
    if a == b:
        return "same result"
    if a is None or b is None or a[0] != b[0]:
        return "other declarer"
    if a[1] != b[1]:
        return "other value"
    ours = b[0] == 0
    gained = b[2] if ours else not b[2]
    return "ours made" if ours and gained else "ours failed" if ours else "theirs failed" if gained else "theirs made"


def series_differs(ref, cand):
    return ref.get("series", {}).get("totals_before") != cand.get("series", {}).get("totals_before")


def main(reference, candidate):
    reason = comparable(manifest(reference), manifest(candidate))
    if reason:
        sys.exit(f"not a paired comparison: {reason}")
    ref, cand = games(reference), games(candidate)
    shared = sorted(ref.keys() & cand.keys())
    for i in shared:
        if (ref[i]["deal_seed"], ref[i]["lineup"]) != (cand[i]["deal_seed"], cand[i]["lineup"]):
            sys.exit(f"game {i}: the runs differ in deal or seats")
    groups = defaultdict(list)
    worst = []
    for i in shared:
        delta = margin(cand[i]) - margin(ref[i])
        if delta == 0:
            continue
        kind = change(ref[i], cand[i])
        routes = ",".join(sorted(f"{p}:{r}" for p, r in our_rules(cand[i]) - our_rules(ref[i]))) or "-"
        groups[(kind, routes)].append((delta, series_differs(ref[i], cand[i])))
        worst.append((delta, i, cand[i]["deal_index"], kind, routes))
    n = len(shared)
    deltas = [d for v in groups.values() for d, _ in v]
    print(f"{n} paired games; {len(deltas)} changed; {sum(deltas) / n:+.2f} points/game")
    print(f"{'change':<16}{'new rules':<44}{'games':>6}{'series':>7}{'worse':>7}{'per game':>10}{'from worse':>12}")
    for (kind, routes), v in sorted(groups.items(), key=lambda kv: sum(d for d, _ in kv[1])):
        d = [x for x, _ in v]
        bad = [x for x in d if x < 0]
        print(f"{kind:<16}{routes:<44}{len(v):>6}{sum(s for _, s in v):>7}{len(bad):>7}"
              f"{sum(d) / n:>+10.2f}{sum(bad) / n:>+12.2f}")
    print("(series: games whose series totals already differ before them)")
    print("\nmost negative games (delta, index, deal, change, new rules):")
    for row in sorted(worst)[:WORST]:
        print("  ", *row)


if __name__ == "__main__":
    main(*sys.argv[1:3])

"""Attribute a candidate's per-game changes against a reference run of the same games.

Both runs play the same deals, seats and rotation with the same policy in slots 1 and 3, so
a game's change is the slot-0 team's doing: its engine margin in the candidate run minus its
margin in the reference run. Games are grouped by what changed in the contract's result (or
only in the points) and by the team's exchange rules that differ from the reference game's.

    python3 scripts/marja_bench/changes.py target/bench-results/<reference> target/bench-results/<candidate>
"""

from collections import defaultdict
import json
from pathlib import Path
import sys

from report import team_index, team_value

WORST = 8  # Most negative games listed for review.
PHASES = ("passing_forth", "passing_back")  # Where the rules that may explain a change act.


def games(run):
    rows = {}
    for path in sorted(Path(run).glob("worker-*.ndjson")):
        with path.open(encoding="utf-8") as f:
            for line in f:
                game = json.loads(line)
                rows[game["index"]] = game
    return rows


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
    if a is None or b is None or a[:2] != b[:2]:
        return "other contract"
    ours = b[0] == 0
    gained = b[2] if ours else not b[2]
    return "ours made" if ours and gained else "ours failed" if ours else "theirs failed" if gained else "theirs made"


def main(reference, candidate):
    ref, cand = games(reference), games(candidate)
    shared = sorted(ref.keys() & cand.keys())
    for i in shared:
        r, c = ref[i], cand[i]
        if (r["deal_seed"], r["lineup"]) != (c["deal_seed"], c["lineup"]):
            sys.exit(f"game {i}: the runs differ in deal or seats")
    groups = defaultdict(list)
    worst = []
    for i in shared:
        delta = margin(cand[i]) - margin(ref[i])
        if delta == 0:
            continue
        routes = ",".join(sorted(f"{p}:{r}" for p, r in our_rules(cand[i]) - our_rules(ref[i]))) or "-"
        groups[(change(ref[i], cand[i]), routes)].append(delta)
        worst.append((delta, i, cand[i]["deal_index"], change(ref[i], cand[i]), routes))
    n = len(shared)
    total = sum(sum(v) for v in groups.values())
    print(f"{n} paired games; {sum(map(len, groups.values()))} changed; {total / n:+.2f} points/game")
    print(f"{'change':<16}{'new rules':<44}{'games':>6}{'worse':>7}{'per game':>10}{'from worse':>12}")
    for (kind, routes), v in sorted(groups.items(), key=lambda kv: sum(kv[1])):
        bad = [x for x in v if x < 0]
        print(f"{kind:<16}{routes:<44}{len(v):>6}{len(bad):>7}{sum(v) / n:>+10.2f}{sum(bad) / n:>+12.2f}")
    print(f"\nmost negative games (delta, index, deal, change, new rules):")
    for row in sorted(worst)[:WORST]:
        print("  ", *row)


if __name__ == "__main__":
    main(*sys.argv[1:3])

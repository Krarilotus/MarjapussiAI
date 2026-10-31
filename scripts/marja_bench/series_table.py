"""Print the per-game score table of `heuristic::series::GAME_SCORES` from a benchmark run.

Each game's engine scores (team A, team B), rounded to 20 points, counted in both team
orders (the table is symmetric), most frequent first, as Rust tuple literals.

    ./marja-bench base base base base 5000 --rotation series --policy-seed 0 --seed 1 --compact
    python3 scripts/marja_bench/series_table.py target/bench-results/<run>
"""

from collections import Counter
import json
from pathlib import Path
import sys

BIN = 20  # Points per bin; the bonus race's grid step must stay a multiple of it.
LINE = 116  # Rust source width for the printed rows.


def table(run):
    cells = Counter()
    for path in sorted(Path(run).glob("worker-*.ndjson")):
        with path.open(encoding="utf-8") as f:
            for line in f:
                a, b = json.loads(line)["metrics"]["scores"]
                for x, y in ((a, b), (b, a)):
                    cells[(round(x / BIN) * BIN, round(y / BIN) * BIN)] += 1
    return cells


def main(run):
    cells = table(run)
    rows = sorted(cells.items(), key=lambda kv: (-kv[1], kv[0]))
    print(f"// {sum(cells.values())} team-game samples, {len(rows)} cells")
    line = "    "
    for (a, b), n in rows:
        item = f"({a}, {b}, {n}), "
        if len(line) + len(item) > LINE:
            print(line.rstrip())
            line = "    "
        line += item
    print(line.rstrip())


if __name__ == "__main__":
    main(sys.argv[1])

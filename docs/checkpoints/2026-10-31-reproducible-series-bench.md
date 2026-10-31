# Checkpoint: reproducible series benchmark

- Date: 2026-10-31
- Status: verified (benchmark infrastructure; no strategy change)
- Tag: `checkpoint/2026-10-31-reproducible-series-bench`
- Branch: `hillclimb/frontier`
- Base checkpoint and commit: `v1/main` `4a1c51c` (`checkpoint/2026-10-14-v1-strength-attribution`)

## Result

The hill climb needs a benchmark that is fast, measures the site's series objective,
and does not depend on machine load. Before this checkpoint the runner was
Windows-only, measured single games only, and decisions depended on wall-clock
deadlines: the identical policy against its frozen copy gave −0.21 points/game
[−0.53, +0.11] on a loaded 30-worker run, and 2 card plays exceeded 100 ms.

- **Series rotation** (`selfplay --rotation series`): four consecutive deals form the
  site's tournament round (`engine::SeriesSettings`), played in order with the running
  series score in every seat's view. Openers rotate through slots 0, 3, 2, 1; the whole
  series is replayed on the same hands with the partnerships swapped. Reports add
  series points per series (game scores, ±1/5 difference, 500-point bonus), clustered
  by series.
- **Deadline switch** (`Params::deadlines`, `--deadlines off`): the owner's limits stay
  wall-clock deadlines in live play; evaluation runs only the logical work limits, so a
  decision is a function of its view. Every decision a deadline cut carries the new
  `deadline_hit` fact, counted per class in the report.
- **Owner's hard limits (2026-10-31)**: each exchange step 5 s (first pass was 1 s),
  each bid 200 ms (was 1 s), each card play, question or answer 100 ms; the
  raise-or-lead choice is both a bid and a card play, so 100 ms (was 5 s). The
  report's hard-limit table checks the maximum wall time per class.
- **POSIX runner**: CPU affinity per physical core, nice 10, per-worker address-space
  cap; `--compact` worker output (6.6 KB instead of 26 KB per game); the frozen `v1`
  policy (`4a1c51c`) as a comparison baseline; the harness lock is seeded from the
  workspace lock for offline resolution.

The policy factory receives `selfplay::Setup { seed, seat, deadlines }` instead of a
seed and seat. Frozen policies predating the switch get unreachable phase budgets
under `--deadlines off`; their endgame search keeps its own 45 ms limit.

## Evidence

| Command or review | Result | Scope / limitation |
|---|---|---|
| `./marja-bench new v1 new v1 5000 --rotation series --policy-seed 0 --workers 16 --cpus 32 --compact --seed 1 --deadlines off` | **0.00 points/game [0.00, 0.00], 0.00 points/series [0.00, 0.00]**; 37.6 s | 16 workers pinned one per physical core (Ryzen 9 9950X3D, WSL2) |
| Same at 30 workers, two runs | 0.00 / 0.00 and +0.06 [−0.06, +0.18] | the second differs only through frozen `v1`'s own 45 ms endgame limit under SMT load; `new` is load-independent |
| Same, wall deadlines (pre-switch), 30 workers | −0.21 [−0.53, +0.11]; 2 moves > 100 ms | the problem being fixed |
| Hard limits, 16 pinned workers, deadlines off | move max 64.98 ms, bid 1.71 ms, pass 7.54 ms; 0 over, 0 deadline hits | raw work without deadline cuts |
| `cargo test -p marjapussi-heuristic -p marjapussi-tools` | all passed (55 lib + integration targets) | test profile |
| `python3 -m unittest discover -p 'test_*.py'` in `scripts/marja_bench` | 36 passed | includes the new series/hard-limit report test |

Fixtures: deal suite `seed1`, 2,500 deals in 625 series, each series played twice
(5,000 games), policy seed 0. Identical policies give exactly zero on both margins.

## Remaining work

The frozen `v1` cannot drop its 45 ms endgame limit; deterministic comparisons use
this checkpoint as the `base` policy. Raw work above 100 ms appears only under SMT
oversubscription (30 workers); timing qualification uses one worker per physical core.

## Cleanup

No background jobs remain. Result folders under `target/bench-results` and the
harness cache under `target/marja-bench` are retained for reuse.

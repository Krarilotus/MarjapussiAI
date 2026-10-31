# Checkpoint: deep endgame

- Date: 2026-10-31
- Status: verified (strength on two deal seeds, hard limits, tests)
- Tag: `checkpoint/2026-10-31-deep-endgame`
- Branch: `hillclimb/frontier`
- Base checkpoint and commit: `checkpoint/2026-10-31-reproducible-series-bench` (`afe2ebb`)
- Frozen as benchmark policy `c2` (`1b24987`)

## Result

The exact endgame over believed worlds (PIMC) was the strongest lever found: starting
it at 5, 6 or 7 tricks left instead of 4 gained +5.4, +10.0 and +16.0 points/game, but
took up to 0.4, 3.2 and 94 seconds per card. This checkpoint makes deep search fit the
100 ms limit, with the decision a deterministic function of the view:

- **Faster exact search, same values** (golden hash of 300 fixed evaluations unchanged):
  conserved card points in the bound, principal-variation search, zero-point rank packing
  in the transposition key, reach criteria read from the same team's points search, and
  `FastState::question_activates` instead of simulated answers. 64-world contract decisions
  need 950k instead of 1.28M nodes at 6 cards and 3.63M instead of 5.93M at 7.
- **Budgeted deep endgame** (defaults): from seven tricks left, one million solver nodes per
  decision, at least 16 completed worlds unless every consistent deal was solved, racing
  (`solver::Racing`) drops moves two paired standard errors behind. `solver::Effort` bundles
  the work; a search that projects past its node limit before the required worlds stops and
  leaves the time to the rules.
- **Exact work sharing in the rollouts**: a level-0 trajectory answers every activation plan
  sharing the steps it consumed (outlook.route 7.8 s to 3.5 s over 100 games, identical
  decisions); the endgame's root policy filter is applied before searching.
- **Series objective (HS-1)**: `heuristic::series::SeriesStakes` values a game outcome by the
  expected final series margin (exact last game via `SeriesTally`, a first-to-500 bonus race
  before it). HZ-1 raises by it; partner raises are read with the same stakes.
- **One reader of the partner's raise** (`routes::RaiseEvidence`) for the route worlds and
  the endgame's weights.
- Rejected and removed: exchange by double-dummy search (no gain), raising by open-hand
  team points (−30 points/game), defender P(contract) objectives (−3.5 points/game). See the
  [hill-climb log](2026-10-31-hillclimb-log.md).

Core changes are additive (`SeriesScore::bonus_team`, `SeriesTally::resume`,
`CARD_POINTS_TOTAL`, `FastState::question_activates`) and pinned as compatible for the
frozen benchmark policies.

## Evidence

All runs: 2,500 deals in 625 four-game series, each replayed with the partnerships swapped
(5,000 games), policy seed 0, `--rotation series`. Qualification runs used 16 workers pinned
one per physical core on an otherwise idle Ryzen 9 9950X3D (WSL2).

| Command or review | Result | Scope / limitation |
|---|---|---|
| `hp1` vs `base`, seed 1 (sweep winner) | **+9.55 [+6.83, +12.27] points/game, +77.50 [+56.28, +98.73] points/series** | the sweep's seed; chosen there |
| `hp1` vs `base`, fresh seed 2, `--deadlines off` (`1bceec7`) | **+6.97 [+3.94, +10.00], +63.52 [+40.53, +86.51]**; bonus rate 0.130 vs 0.090 | confirmation |
| Final `1b24987`, seeds 1 / 2 | +8.43 [+5.79, +11.08] / +7.53 [+4.60, +10.45] points/game; +71.22 / +66.15 points/series | with the early stop |
| Same, `--deadlines wall` (seed 2, `589c69c`) | +6.99 [+3.95, +10.03], +64.60 [+41.65, +87.54]; 142 of ~105k candidate moves cut by the clock | live behaviour matches the deterministic lane |
| Partnership with the current AI: `hp1 base base base`, seed 2 | +3.04 [+0.60, +5.48] points/game, +38.60 [+19.14, +58.06] points/series | one new player partnered with `base` helps its team |
| Hard limits, deadlines off, seed 2 (`1b24987`) | move max 95.6 ms (33 above 80 ms of ~105k), bid 1.4 ms, pass 7.1 ms; no overrun | raw work |
| Hard limits, deadlines wall, seed 2 | move max 91.6 ms, bid 1.7 ms, pass 7.3 ms; no overrun | live |
| `cargo test --release --workspace` | all passed | release profile |
| Python benchmark tests | 36 passed | |

Fixtures: deal suites `seed1` and `seed2`; parameter variants are in the log.

## Remaining work

The move limit has a thin margin (worst observed 95.6 ms raw); the tail is the rules'
fallback after a budget-starved search. Deeper search still pays (unbounded seven tricks
+16 points/game): faster nodes, better ordering and cheaper fallbacks are the next levers,
followed by the research roadmap (exchange candidates, calibrated raising, pass inference,
series-aware bidding).

## Cleanup

Result folders under both benchmark worktrees and the frozen policy caches are retained.
No benchmark or build jobs remain running.

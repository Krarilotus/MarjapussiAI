# Compare four policies

Run from Git Bash on Windows or a POSIX shell (Linux, WSL), with Python 3.11+ and Rust installed:

```sh
bash marja-bench --install
source ~/.bashrc
marja-bench new old new old 4000
```

The four names are player slots. **0 and 2 are partners; 1 and 3 are partners.**
The last argument is the total number of finished games. The launcher builds a
release worker, reuses it while its inputs stay unchanged, and prints progress
and the report location. Its first build uses the locally cached Cargo dependencies.

```sh
marja-bench --list-policies
marja-bench hp1-fast random hp1-facts random 40 --seed 123
marja-bench new previous new previous 4000 --seed 20261012
```

| Name | Policy |
|---|---|
| `new`, `hp1` | Current checkout's default HP1. |
| `previous` | `b3abdaf`: verified live HP1, including its frozen solver. |
| `old` | `1632af8`: the earlier deployed policy. |
| `v1` | `4a1c51c`: the published `v1/main` head, the 2026-10-31 hill-climb baseline. |
| `base` | `afe2ebb`: `v1` with the deadline switch, reproducible under `--deadlines off`. |
| `c2` | `1b24987`: checkpoint 2 (budgeted seven-trick endgame, faster solver, series raise). |
| `c3` | `bb2f36c`: checkpoint 3 (auction thresholds, exchange routes over 512 worlds). |
| `c4` | `2b19c6f`: checkpoint 4 (B-take, faster exact solver, 1.2M endgame nodes). |
| `rev:COMMIT` | The heuristic, beliefs and solver of any local commit (shared core compatible). |
| `hp1@FILE` | The current HP1 with the flat TOML overrides of `FILE`. |
| `hp1-fast` | Current HP1 with return, continuation and endgame search disabled. |
| `hp1-facts` | Current HP1 with convention signals disabled. |
| `random` | Uniform random legal actions. |

Historical heuristic, belief and solver sources are extracted from their exact Git
revisions into a separate versioned build cache. Their package names are isolated;
the old heuristic imports its own frozen solver, including its search deadlines.
Only engine, observation, knowledge and agent are shared and must pass the existing
compatibility guard. No new shared-core anchor replaces the historical solver.
Run manifests retain per-file historical source proofs, compiler, dependency-lock,
generated-harness and binary hashes, parameters and effective policy seeds.
Keep the checkout unchanged during a build and retain the cache for reproduction.

## Series: points per game and per series

`--rotation series` plays the site's four-game tournament round (`SeriesSettings`):
four consecutive deals form one series, played in order, and every seat sees the
running series score (`SeatView::series`). The opening seat holds slots 0, 3, 2, 1,
so each player opens once and the opening partnership alternates. The whole series
is then replayed on the same hands with the partnerships swapped (openers 3, 0, 1, 2).
`--games` counts both plays: 5,000 games are 2,500 deals in 625 series, each played
twice. The report adds **series points per series** (game scores, ±1/5 of the
difference and the 500-point bonus), clustered by series, beside the per-game margin.

```sh
./marja-bench new v1 new v1 5000 --rotation series --policy-seed 0 \
  --workers 30 --cpus 32 --memory-gib 24 --compact
```

`--compact` drops the full game records and decision facts the report does not read
(about 6.6 KB instead of 26 KB per game). Leave it out to keep complete records for
explaining individual games. Identical policies give exactly zero on both margins:
each replay mirrors its game.

On POSIX the job pins workers to physical cores when there are enough, runs at
nice 10 and caps each worker's address space at the memory budget divided by the
workers (no whole-tree commit limit without cgroups). The report's **hard limits**
table gives each slot's maximum wall time per decision class against the owner's
limits: 100 ms per card play, question or answer (including the raise-or-lead choice
after the exchange), 200 ms per bid and 5 s per exchange step.

## Games and reports

By default every deal has four arrangements (`--rotation teams-and-seats`). Partnerships switch seat parity, and the last
two arrangements turn the hands by two seats. A team of identical bots plays
both hand pairs and both opening-hand roles. Different teammate policies receive
two starting hands each. A count divisible by four gives complete blocks.

`--rotation teams` plays each original deal twice, switching policy partnerships
between seats 0/2 and 1/3 without turning the hands. Thus 256 games means 128
deals in this mode, compared with 64 deals in the compatible four-game mode.
Workers receive whole deal blocks; output identity is `index = games_per_deal *
deal_index + rotation`. The second lineup is `[3,0,1,2]`; `policies[]` is always in
slot order, so use `lineup[physical_seat]` for attribution and effective seed lookup.

The output folder contains:

- `report.html`: readable tables with expandable time breakdowns.
- `summary.json`: aggregate scores, contracts, raises, communication and timings.
- `games.csv`: per-game team scores, card/pair points and outcomes.
- `worker-*.ndjson`: full deals, actions, explanations, decision timings and actual
  current-policy assessment traces. Historical policies have no typed assessment trace.
- `manifest.json`, `build.log`: configuration, provenance and completion status.

The main comparison is **engine score difference per game**. The approximate
95% interval clusters all arrangements by their original deal. Draws stay
separate from wins. Contract success and raise success are conditional on taking
those actions. Timing measures the agent call, including forced choices;
nonforced, phase and rule breakdowns explain where time goes.
Forecast reuse counters include interrupted comparisons when the policy records
them. They show how many decisions were evaluated and how many came from an
identical visible state; older policies may omit these counters.

Current recording wraps `HeuristicAgent::act` in tools, captures its existing
`last_assessment()` immediately, and checks the acting seat, honest context,
event count and returned action against the pre-apply engine hook. `step` indexes
non-START record actions. No diagnostic evaluator or extra policy call supplies
the trace. A recorded null assessment means no completed assessment was retained
for that act; `assessment_recorded=false` means the policy was not instrumented.
`assessment_matches_chosen_action` exposes any difference between the assessment
winner and the returned action instead of silently claiming they match.

Decision `ms` measures inner policy act. `capture_ms` includes copying its result;
`assessment_serialization_ms` records serialization outside the policy call.
Total job time includes these costs. Only one game's serialized diagnostics is
retained by a worker; Python compacts report inputs and keeps full NDJSON on disk.
Trace refs are decision-local, with actual operators, dependencies, values,
candidates, provenance and observer context. The assessment `extensions` object
is the integration point for actual future sequence receipts/traces; it is empty
at this checkpoint. Do not fill it with reconstructed or imagined reasoning.

For a complete run, `summary.json.review_deals` deterministically ranks full
blocks by lowest/highest engine margin and closest-to-zero controls. It considers
all completed decisions, not only changed choices. Rankings prioritize inspection;
they do not establish that a loss was a bad choice or a favorable result an improvement.
Partial reports leave these lists empty.

Captured card points use a ceiling of 140, including the last trick. Pair points
use 280; their combined ceiling is 420. These describe scoring capacity. Contract
margins describe the outcome under the bid actually played.

Bidding communication checks the content of unambiguous bids against the
speaker's hand using the common convention model. Ambiguous readings and bids
outside that convention are reported separately. Receiver understanding and
strategic bidding quality require reviewing the decisions and outcomes together.

Ctrl+C stops owned workers and retains finished games with a partial report.
Regenerate a report with:

```sh
marja-bench --report-only /path/to/result-folder
```

## Resources and reproducibility

The default is one worker, two logical CPUs, below-normal priority and a 4 GiB
commit limit for the complete process tree. The job owns and stops its child
processes. Result files and reusable build caches remain on disk.

Use the same shared `--lock` for all validation jobs. The builder additionally
holds `<cache>/.build.lock` from historical-source verification through binary
publication; do not select that path as the outer resource lock. On a final
binary-cache miss it resolves the offline Cargo graph, cleans only its local
packages, and rebuilds with the resulting dependency lock fixed. Registry
artifacts remain cached. Every selected local package must report fresh
compilation, and the copied worker must match Cargo's executable artifact event.
The build record retains commands, target, artifact events and the log hash.
Verified final-binary cache hits skip resolution, cleaning and compilation.

## Prepared live-baseline smoke (not run)

Only run after the required strategy, targeted correctness and performance
validation is complete and the candidate source/config is frozen. The requested
Windows job uses at most two workers within one shared two-CPU/4-GiB resource job:

```sh
python -X utf8 scripts/marja_bench new previous new previous 256 \
  --rotation teams --policy-seed 0 --seed 20261013 \
  --workers 2 --cpus 2 --memory-gib 4 \
  --lock /path/to/shared/validation.lock \
  --out ../benchmark-data/live-baseline-smoke-20261013
```

This is 256 complete games, 128 original deals and 128 uncertainty clusters;
each policy partnership plays 256 games, on each physical team 128 times.
There are 1,024 fresh seat agents (512 per policy). All agents receive effective
seed zero, preserving live defaults. Without `--policy-seed`, the original
deal/slot-derived seed behavior remains available; raw records retain both
`stream_seed` and actual `policy_seed`.

Read-only deployment checks on 2026-10-11 identified live AI
`b3abdaf1f5e47111904db442ce36db7dc4ae9e0b`, testbed
`cd553cc0beb03887c319aa3c9348b1190151b32f` and native module SHA256
`e2f9c1ab90afe3082a599d098bd792a7f342f7f75bb59a627e4c986a4a17c960`.
The running module version and image agree, and the live site adapter matches
its Git source byte-for-byte: newly created HP1 seats use empty/default params.
Existing individual-game overrides were not inspected. `old` is the earlier
`1632af8` rollback, not this live baseline. The Windows worker reproduces frozen
policy source/config; it is not the identical Linux native deployment artifact.
Live defaults use seed 0, plan/return worlds 8/8, endgame 4 tricks/64 worlds,
argmax, full signals, uniform exchange and no fitted calibration.

Both policy teams see the same original hands in each pair. Fixed seeds determine
deals/streams, but wall-limited historical searches can still vary with contention.
Freeze worker count, affinity, compiler and resource configuration in the receipt.
This preparation has no build/test/smoke evidence; root owns those checks.

For an expressly allocated eight-core run:

```sh
marja-bench new previous new previous 4000 --workers 8 --cpus 16
```

Workers receive complete deal blocks and separate physical cores when available.
Changing concurrency can change deadline-limited decisions, so record and keep
resource settings fixed within a comparison. Report generation loads compact
statistics; complete game histories remain in the NDJSON files.

Use one shared lock across AI and testbed worktrees. The installer saves the
selected path in its launcher:

```sh
bash marja-bench --install --lock /path/to/shared/validation.lock
```

`--out` chooses a fresh result directory; `--cache` chooses a reusable build
cache. `--prepare` builds the selected lineup without playing. For qualification,
freeze policies and sample size before starting and use fresh deal seeds after
changing strategy. The [evaluation boundary](EVALUATION.md) explains the evidence
needed for a playing-strength claim.

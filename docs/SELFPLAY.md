# Compare four policies

Run from Git Bash on Windows, with Python 3.11+ and Rust installed:

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
| `previous` | `b3abdaf`: the full-game continuation policy before conditional concepts. |
| `old` | `1632af8`: the earlier deployed policy. |
| `hp1-fast` | Current HP1 with return, continuation and endgame search disabled. |
| `hp1-facts` | Current HP1 with convention signals disabled. |
| `random` | Uniform random legal actions. |

Historical heuristic and belief sources are extracted from their exact Git
revisions into the build cache. Their shared core must pass the compatibility
guard. Run manifests record source, compiler, dependency-lock and binary hashes,
parameters and per-slot random seeds. Keep the checkout unchanged during a build.

## Games and reports

Every deal has four arrangements. Partnerships switch seat parity, and the last
two arrangements turn the hands by two seats. A team of identical bots plays
both hand pairs and both opening-hand roles. Different teammate policies receive
two starting hands each. A count divisible by four gives complete blocks.

The output folder contains:

- `report.html`: readable tables with expandable time breakdowns.
- `summary.json`: aggregate scores, contracts, raises, communication and timings.
- `games.csv`: per-game team scores, card/pair points and outcomes.
- `worker-*.ndjson`: full deals, actions, explanations and decision timings.
- `manifest.json`, `build.log`: configuration, provenance and completion status.

The main comparison is **engine score difference per game**. The approximate
95% interval clusters all four arrangements by their original deal. Draws stay
separate from wins. Contract success and raise success are conditional on taking
those actions. Timing measures the agent call, including forced choices;
nonforced, phase and rule breakdowns explain where time goes.
Forecast reuse counters include interrupted comparisons when the policy records
them. They show how many decisions were evaluated and how many came from an
identical visible state; older policies may omit these counters.

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

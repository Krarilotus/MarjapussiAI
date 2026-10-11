# Targeted dryrun and decision comparison

Owner: `tools::dryrun` (selection, source validation, output); `eval::play_table_with`
(bounded honest-view continuation); engine (all legality and outcomes).
This is diagnostic measurement, not a policy change or a strength qualification.

## Contract and invariants

`tools::dryrun::run_with(Vec<tools::selfplay::Policy>)` lets a caller register current,
proposed and frozen factories. The stock `dryrun` binary has only the existing builtins;
historical opponents require the existing compile-time Cargo adapter. No new FFI or
library loader is introduced. Root supplies factory/source/binary provenance and the
existing process-tree resource wrapper. A factory receives only policy seed and seat.

Register `baseline` from clean **8ca35c9** independently of `candidate` from the proposed
checkout, `previous` from frozen b3 and `old` from frozen 1632. Use
`--candidate baseline,candidate` for attribution on identical visible prefixes.
`--baseline-new previous --baseline-old old` retains original frozen opponents;
`--baseline-new baseline --baseline-old old` is a different, explicitly labelled
continuation experiment. Registry descriptors should include actual source/library
fingerprints as well as parameters. The input's frozen labels do not make an evaluated
current policy into b3, nor is the pre-change 8ca attribution baseline b3.

The input is the complete ordered original qualification NDJSON, with implicit index
`4 * deal + rotation`, suite seed 20261011 and 1000 deals by default. Each deal is played
four times, new team is `index % 2`, and hands turn by two seats for rotations 2/3.
All factories get **policy seed 0**, not a derived selfplay slot seed. Starting kinds
remain all-Heuristic and series score remains the original default. Selecting random
or another-kind candidates does not silently change this fixture metadata.

Every row's identity and the exact dataset length are checked. Selected games are
replayed completely, matching recorded actors/sides/steps and Debug actions against
the engine's legal list, then checking original final scores, team points, contract,
declarer, made, Schwarz flag and trick count. No candidate is called until this passes.
`Table` applies the original legal prefix through seat filters and exact knowledge;
candidate agents receive **only the acting SeatView and legal actions**. Full deals
appear only in posthoc game records, never as policy input.

`--input-sha256` is a **declared external guard value**, not a hash computed by this
worker. The caller must verify it before/after execution and retain recovery/segment
provenance. The worker also retains an input handle and checks length/mtime at the end.
For the verified recovered dataset the SHA-256 is
`9a1eb1b4e7a5118f6eefb90924c6da60767629f49b5ebfb0a4767777219699b7`.
Recorded policies are frozen b3abdaf (new) versus 1632af8 (old), distinct from the later
256-game pilot and from current/proposed policy factories. Those revision labels
identify this qualification input format; they do not verify caller-supplied factories.

## Modes and replacement boundaries

| Mode | Policy input and execution | Default selection |
|---|---|---|
| `teacher` | Fresh candidate at each original prefix; no subsequent play/outcome | All recorded steps, including unchanged decisions |
| `continue` | Original prefix, then fresh factories run a complete legal game | First start-trick step (`--steps focal`), or explicit fixture steps |
| `full` | Original dealt hands, fresh factories from before the first bid | Step 0 |

For continuation, both `--baseline-new` and `--baseline-old` are required registry names.
Physical seat parity and the fixture's recorded new team select the correct baseline;
there is no slot-order ambiguity. Replacement is separately recorded:

| `--replacement` | Substitution |
|---|---|
| `action` | Candidate chooses the focal action once; all subsequent actors use baseline factories |
| `seat` | Candidate controls the focal physical seat for the remaining game |
| `partnership` | Candidate controls both seats of the focal actor's team |

Full-start mode needs `seat` or `partnership`. `--candidate-team new` (default), `old`,
or `focal` chooses the replaced team; for single-seat full starts it chooses the team's
lowest-numbered physical seat, and `focal` means seat 0. Prefix modes always replace
the focal actor/team, including case 162 whose focal actor is the recorded old policy.
These are separate experiments; one-action and partnership outcomes must not be pooled.

Prefix continuations use **fresh agents at the prefix**. The records do not preserve
mutable RNG state, caches or deadlines. Honest observations are reconstructed exactly;
private policy state and wall-time decisions are not claimed to reproduce the original.
Include frozen-factory continuation controls before attributing a later divergence to
the candidate. First divergence compares the original action stream; after it, equal
step numbers are positional and need not denote equal states.

## Fixture selection before implementation

Requested cases: 125, 2549, 2403, 1113, 2649, 3390, 3545, 162. A committed **ID-only**
selection file adds focal prefixes and controls; no private/raw games are committed.
For coverage across all phases use `--cases` plus `--steps all`, not that focal file.

Four additional controls were selected from **all 4000 completed games before source
changes**, excluding the requested cases' deal blocks: made contract, score margin
1..10, sorted by absolute score margin then index, earliest distinct deal per stratum
of declaring policy × contract-team final score win/loss. No chosen-action, changed-
decision, rule or forecast filtering was used. All four selected margins are five.

| Index | Deal | Rotation | Declaring policy | New/old scores | Contract | Made | Contract team won |
|---|---|---|---|---|---|---|---|
| 812 | 203 | 0 | b3/new | 145 / 140 | 145 | yes | yes |
| 57 | 14 | 1 | 1632/old | 135 / 140 | 140 | yes | yes |
| 664 | 166 | 0 | b3/new | 150 / 155 | 150 | yes | no |
| 307 | 76 | 3 | 1632/old | 145 / 140 | 140 | yes | no |

All phases can be selected by `--phases bidding,passing_forth,passing_back,raising,start_trick,trick,answering_pair,answering_half`.
`--cases 125:13+14,812` supplies per-case steps; `--steps all`, `focal` or a comma-separated
step list supplies the default for cases lacking explicit steps. `--fixtures PATH`
accepts an array of `{ "index": N, "steps": [S,...] }`; omit `steps` to use the default.
Full mode deliberately ignores prefix step selections and phase filtering.

## Output and resource lifecycle

Output is create-new NDJSON (`marjapussi.dryrun/1`): a start envelope, one result per
case/step/candidate, and a summary with expected/completed counts and complete/failed
status. Flush after every envelope. Existing files are never replaced. A missing
complete summary or nonzero process exit means unfinished evidence; panics/forced
termination may leave a valid diagnostic prefix but never a completed run.

Each result includes the recorded prefix/focal decision, chosen action/rule,
full-precision explanation facts, decision time, phase, honest input snapshot and
legal actions. Known cards/voids/rights come from exact Knowledge; possible sets are
uncertainty, **not probabilities**. Policy facts are explanations, not hard proofs or
calibrated forecasts. Posterior distributions are explicitly absent; no extra Mind
reconstruction is mixed into decision timing. Nonfinite fact values carry `finite=false`.
Continuations include a complete existing game record and existing MatchStats JSON
(card/pair points and ceilings, contract auction/final value, raises, tricks, actual
contract Schwarz and convention support). Prefix game-record notes omit explanation
annotations; original prefix explanations remain in `original_prefix_decisions`.

Select at most 32 cases, 8 candidates and 512 comparisons (`--max-runs`, default 128).
Each continuation permits at most 128 policy calls. Oversized selections fail before
any policy calls. Input rows are capped at 1 MiB and selection files at 64 KiB. Policy
calls are synchronous and cannot be preempted by this library; root's existing bounded
process-tree wrapper supplies hard time, CPU, memory and teardown limits.

`run_with_profiling(policies, Some(Profiling { reset, snapshot }))` supports root's
compile-time opt-in profile feature. Reset after fixture validation, snapshot in the
final success/failure summary. The adapter owns its feature propagation and profile
crate identity; no overhead or feature dependency is added to unprofiled policies.

## Owner validation and invocation

No builds/tests/runs were authorized for this implementation thread. Root runs affected
tests sequentially through the existing shared limiter; these are suggested commands,
not claimed results (Git Bash syntax, existing target cache):

```sh
CARGO_BUILD_JOBS=1 cargo test --offline -p marjapussi-tools --lib dryrun::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo test --offline -p marjapussi-tools --lib selfplay::tests -- --test-threads=1
```

Use the compiled root adapter for historical factories (`previous`=b3, `old`=1632):

```sh
"$DRYRUN_EXE" --input "$ORIGINAL4000" --output "$NEW_OUTPUT" \
  --input-sha256 9a1eb1b4e7a5118f6eefb90924c6da60767629f49b5ebfb0a4767777219699b7 \
  --fixtures docs/design/targeted-dryrun-fixtures.json --steps focal \
  --candidate current,proposed --mode teacher
"$DRYRUN_EXE" --input "$ORIGINAL4000" --output "$ACTION_OUTPUT" \
  --cases 125:13,2549:13,2403:7,1113:13,2649:20,3390:7,3545:18,162:33 \
  --candidate current,proposed --mode continue --replacement action \
  --baseline-new previous --baseline-old old
```

Repeat the second invocation with `--replacement partnership` into a separate new
output. For all-phase coverage, narrow cases or raise the explicit comparison bound;
for example `--cases 125,812 --steps all --candidate current,proposed --max-runs 256`.
Run controls/alternatives on identical inputs and report realized outcomes separately
from uncertain reasons. None of these cases establishes that every lost game must win.

# Checkpoint: team-swapped live-baseline test preparation

- Date: 2026-10-11
- Status: **incomplete; static-only validation**
- Tag: `checkpoint/2026-10-11-selfplay-teams-preparation`
- Branch: `feature/selfplay-teams-preparation`
- Base: `7767df213cd8dd81933f49b57b52dede080ae334`, validated value-store slice;
  `checkpoint/2026-10-11-executable-value-store` remains an incomplete strategic migration.
- Frozen live baseline: AI `b3abdaf1f5e47111904db442ce36db7dc4ae9e0b`,
  testbed `cd553cc0beb03887c319aa3c9348b1190151b32f`.

## Result

The existing worker/launcher supports `--rotation teams`: each original deal is
played twice, with opposite policy partnerships and unchanged hands. The default
four-game arrangement remains compatible. `--policy-seed 0` preserves live HP1
default seeds while omitted seeds retain derived deal/slot streams. Whole-block
worker sharding and reports use the selected block size.

Historical heuristic, belief and solver crates are extracted from frozen Git
sources into isolated package names. The baseline no longer uses the current
solver. Existing shared engine/observation/knowledge/agent compatibility guards
remain; no new shared-core anchor was introduced. Historical source integrity
is checked before reuse and after compilation. Generated harness and binary
provenance is retained by the builder.

A tools-only wrapper captures the current agent's actual `last_assessment` from
each act call. It verifies seat, action, event count and full honest context,
then writes consumed evaluator refs, operators, dependencies, values and model
provenance. It does not call a second strategy evaluator. Policy act timing is
separate from capture/serialization overhead. Historical assessment absence stays
explicit. One game is retained by the worker; report aggregation compacts inputs
while preserving full NDJSON on disk. Future sequence trace extensions are empty
at this base and require actual sequence API integration.

## Evidence

| Command/review | Result | Scope/limitation |
|---|---|---|
| `rustfmt --edition 2021 crates/tools/src/selfplay.rs` | Passed | Rust parsing/formatting only; no type checking or compilation. |
| `ast.parse` over seven `scripts/marja_bench/*.py` files | Passed | Python source parsing only; no module execution or tests. |
| `git diff --check` | Passed | Whitespace only. |
| `git apply --reverse --check` on the exported eight-file patch | Passed | Patch matches this working tree; no application to root. |
| Source review of rotation, capture, seed and frozen-solver paths | Completed | Not runtime validation. |
| Added Rust/Python regression tests | **Not run** | Root owns serialized validation after sequence integration. |
| Build, performance measurement, 256-game smoke | **Not run** | Explicitly deferred; no new strength evidence. |

Prepared fixture specification, not a produced dataset: suite seed `20261013`,
128 fresh deals, two team assignments, 256 games, effective policy seed zero,
two workers within one two-logical-CPU/4-GiB below-normal Windows job and shared
validation lock. No sequential stopping. Complete output must contain 128 paired
deal clusters. No dataset digest or tested candidate binary exists for this job.

Written tests cover 128 unchanged-hand pairs, whole-block shards, zero-seed
forwarding, action-note alignment, team attribution and uncertainty denominators,
assessment capture/serialization, frozen solver isolation and cache tampering.
No private records or generated benchmark results are included in this commit.

## Remaining work

Root must complete required policy/sequence gaps, integrate this patch, run
serialized affected tests and performance checks, freeze candidate source/config,
and only then launch the prepared smoke. The command and metric interpretation
are in `docs/SELFPLAY.md`. Fixed seeds reproduce deals/streams; historical timed
searches can still vary with resource contention.

Read-only inspection of root's incomplete retained-plan draft identified these
integration requirements, not validated implementations:

- Root added `PlanningContext.belief_scope`. Serialize its actual scope and update
  the serializer's test fixture. Prepared-cohort conditioning must not be labeled
  as a fresh current-information posterior. Keep acting context and origin distinct.
- View-only forecast memo hits currently return a `Decision` without accepting
  observations or installing receipt after-state. Before retained intent is used
  by forecast actors, extend semantic keys/after-state handling or disable reuse
  for those actors. Existing stateless equivalence evidence does not qualify this
  new semantic state.
- Forced-sync precedes forced-action returns in the draft. Receipt synchronization
  still needs tests proving accepted actions advance exactly once, rejected
  proposals spend nothing, and changed information triggers adaptation rather than
  reuse solely because the old action remains legal. The required sequence API was
  unavailable during this read-only inspection.

## Cleanup

No owned builds, workers, benchmarks or containers were started. No disposable
build outputs were created or removed; reclaimed space: 0 bytes. Existing caches,
frozen evidence and the exported patch remain available for root integration.

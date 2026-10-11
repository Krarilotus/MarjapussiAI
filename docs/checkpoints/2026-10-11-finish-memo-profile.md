# Exact finish-memo profile events

Status: **incomplete overall; profile assertions passed, measurements recorded**.
Source `5129eb7`, tagged `checkpoint/2026-10-11-finish-memo-profile`.
Root integration base `be8badf71365178d5c6dd7a5c1d759b29317e608`;
integration tag `checkpoint/2026-10-11-finish-memo-profile-integration`.

Owner approved fixed existing-profiler labels only. No exact-key registry, hash,
extra equality scan, private payload, Work field, output schema, model, allowance
or cache-policy change. The profiler's existing `calls` and time fields export
these fixed labels without tools changes. Profiling overhead can shift timed
prefixes; measurements are not standalone performance/strength evidence.

## Events and interpretation

- `sequence.finish.lookup_hit` / `lookup_miss`: one event after the existing exact
  finish lookup. A hit can still be rejected by logical charging.
- `sequence.finish.hit_served`: only after successful charging, copying and prefix
  reattachment, immediately before returning the completed finish result.
- `sequence.finish.stored` / `not_retained.disabled` / `not_retained.entry_cap` /
  `not_retained.byte_cap`: mutually exclusive outcomes of a successfully completed
  miss. Their sum counts completed misses; misses minus that sum counts incomplete
  or unwound miss invocations, not lost complete results.
- `sequence.finish.hit_copy` / `store_copy`: replaces the two finish uses of the
  mixed `sequence.memo_copy` label. The latter still describes Tail copies; old
  captures retain their original meaning and must not be compared as identical
  aggregate copy populations.
- `sequence.eviction.reason.entries` / `bytes` / `both`: mutually exclusive reason
  for the existing response-pressure eviction. `sequence.eviction.memo_nonempty`
  counts events which actually clear a nonempty finish memo, NOT keys removed or
  later repeat requests. Oversized/disabled response admission still returns
  before the eviction path as before.

Short event-scope durations are instrumentation overhead, not useful operation
timings; use their `calls`. Existing joint_key, memo_lookup, run_ledger, trace and
eviction timings remain. Two short events per completed finish and up to two
additional events per eviction add constant-label clock/map work in profile
builds, with no per-key data retained. Disabled builds use existing zero-sized
profile scopes. Extra pressure-reason classification is cfg(profile) only.

Original admission order is unchanged: suffix reuse enabled, entry capacity,
then exactly one run_bytes/retain attempt. Disabled/full-entry cases still skip
the ledger. Charge_work calls, cached scalar costs, actual work counters,
result/prefix ownership and memo eviction behavior are unchanged.

Counts establish resident exact repeats and retention pressure. They do not
establish whether a rejected/evicted complete dependency was requested again,
whether repeats crossed different lexical parents, or whether misses represent
new histories rather than prior results outside retention. No broader logical
sharing decision is justified from those unknowns alone.

## Validation and remaining gates

The existing completed-parent sibling-tail fixture gains cfg(profile) assertions
for disabled admission, one stored dependency plus one served parent hit, entry
versus byte-cap rejection with original short-circuit ledger counts, and a found
but unaffordable parent hit (not copied/served). The same fixture verifies full
values/executions/receipt equality on the capacity variants. No new game fixture.
Existing stage-scan eviction coverage additionally checks byte-pressure events,
zero-cap early return, and that canonical-only eviction does not count as a
nonempty finish memo. Entry/both/nonempty eviction cases have source review only.
E independently reviewed the production boundaries and fixture assertions and
found no blocking source issue. No runtime validation was performed by E.

Rustfmt parses the changed files; owned added hunks conform. Whole-file checks
still report pre-existing unrelated formatting, deliberately untouched. Diff
whitespace and focused patch applicability checks are required for handoff.
The source author and reviewer ran no builds. Root used the serialized two-CPU,
below-normal, 4 GiB runner, optimized test profile, with receipts/logs in private
`tmp/2026-10-09-v1`:

- `finish-memo-profile-policy`: **118 passed with `--features profile`**, including
  both augmented fixtures. 86.91 s including compilation, 1.15 s tests, peak
  607.2 MiB. SHA256
  `e1e25277e574ca66a8986f3c8b53c1d2bc0c9ca5740ab8bee7c996dd3f6d5ea7`.
  Two unchanged expensive gates excluded: genuine depth-two auction/return
  (previously passed) and six-card final-global (previously failed its unchanged
  500k represented-transition cap). Their earlier evidence remains unchanged.
- `finish-memo-profile-build`: passed, 14.06 s, peak 310.0 MiB. SHA256
  `5c20ff2f09d3cf1a4ab19ff165ed4289c0eb9cf4b0e03733f8959484c17a6f7e`.
- `finish-memo-profile-two`: 20.93 s, peak 101.6 MiB. SHA256
  `892a1ca241d5746a796c31b97acff71dbbbdfe76ad20b51ec0e1dff166c0566c`.
  Same 105 actions/rules; all **23 attempts Deferred**. Actual/represented
  transitions 166,179/237,338, 14 individual candidates completed, zero frontier
  aliases. The capture and exact source snapshot remain in private
  `tmp/2026-10-11-finish-memo-profile`.

The matched diagnostic recorded 12,556 finish lookups: 67 found and served hits,
12,489 misses. Completed misses: 10,909 stored, 650 rejected by entry capacity,
731 by byte capacity; 199 miss invocations did not finish. There were 189 pressure
evictions (167 byte-only, 22 entry-only), 187 clearing a nonempty finish memo.
These are event counts, not counts of distinct dependencies or lost repeat hits.
The finite runtime diagnostic exercised entry-only/nonempty events; combined
pressure remains unobserved here, and positive deterministic assertions for these
cases remain absent as described above.

Finish hit copies took 7.972 ms, store copies 307.359 ms, lookup 9.405 ms. Suffix
construction still took 4.421 s over 162,719 calls within 20.656 s of planning.
Those measurements motivate reviewing cache lifetimes and decision expansion;
they do not establish that retaining more finish results would improve completion.
Profiler overhead and deadline prefixes prevent a controlled speed comparison.

Whole-plan completion and the six-card gate remain unresolved. No 256-game smoke,
deployment or strength claim.

Owned files: sequence.rs profile points/existing eviction assertion, existing
sequence_normalization.rs cache fixture assertions, this checkpoint. D Reach
and the residual/score-bound designs are separate. Validation processes ended;
no owned containers. Reclaimed space zero bytes; evidence and the planned reusable
build cache remain.

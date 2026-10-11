# Local completion-dominance capture companion

Status: **incomplete; source-only checkpoint, builds and tests not run**.

Base: `ad6e99ef1114f9a5efab8724ac789a283b033ad3`.
Isolated branch/worktree: `review/completion-dominance-capture`,
`wt/completion-dominance-capture`. Root owns integration and serialized validation.
Dependency: knowledge prerequisite `201ded1`, then frozen B core
`ebf4df0f3901395bd7db137b6ed2a1b2b4ba8236`. This companion includes neither.

## Owned change

Tools consume `PreparedChoice.proof` and the Complete-only `assessment()` accessor.
Complete parent/global/local captures retain their genuine completed estimates.
Local completion dominance has its own `marjapussi.completion-dominance/1` object:
full original ordered alternatives including aliases, original support IDs and
lossless IEEE weight/objective bits, completed incumbent diagnostics, ordered
bounded-unexecuted competitors, and the actually consumed shared ValueStore trace.
Every reference is namespaced to its own choice/arena. A singleton incumbent's
value graph is explicitly labeled incumbent-only; it is never exported as a
complete assessment of the Local domain. Losers have no fabricated probability,
runner-up, completion, receipt, or model-value estimate.

The graph exporter checks recorded scope/parent consistency, typed producers and
same arithmetic inputs, then projects the recorded dependency edges. Every bound
must reach its own consumed public envelope; an unrelated envelope elsewhere is
insufficient. Weight, mass, coefficient and team must match the completed
incumbent's typed pipeline. Enclosure score slots match their recorded result and
support count. No scores, envelopes, expectations or strategy are recomputed.
Existing selected-parent authorization remains a separate edge; summaries are
neither private-cohort identity nor memo/rebind authorization. The exporter is a
record-consistency check, not a cryptographic attestation or a live planner.

Operational metadata interprets the recorded
`sibling-frontier-dag-v2+completion-dominance-local-v1` Work label. It does not
upgrade older V1/V2/absent labels or imply exhaustive represented-work parity.
No Work fields, policy budgets, worlds, horizon, rule, belief, or cache changed.

## Concept ownership review of frozen core

- Knowledge remains owner of the checked forward/no-Undo public score enclosure.
- The common ValueStore owns executed score/difference/expectation arithmetic and
  typed interval/certified-ordering nodes with shared IDs, memo and consumed trace.
  The bound leaf reconsumes the knowledge rule rather than trusting a diagram label.
- Sequence privately constructs Local evidence only after candidate 0 genuinely
  completes on the full original joint group. Competitor enclosures assert no
  callback totality or modeled value. All alternatives must pass narrow card to
  literal-Tail eligibility; gates are not discarded. Error/Budget propagates.
- Failed numerical separation reuses candidate 0 on the ordinary full-domain path.
  Original tie order/support bits remain bound to the proof. Full BranchKey keeps
  cache identity; SelectedPlan owns selected-parent reachability. Global coverage
  remains Complete-only; frontier alias projection declines the new proof variant.
- No remaining blocker found in that narrow source path. This is not validation
  of all authored catalogue gates or an assertion of improved playing strength.

## Validation and unresolved gates

Passed here: rustfmt parsing on owned Rust sources; new-file formatting; whitespace
and focused diff inspection; `git apply --check` against frozen core `ebf4df0`. Source-only API/caller search found no other tools
PreparedChoice field users needing migration. No cargo build, test, clippy,
benchmark, deployment or smoke was launched.

Added six source tests using a shared engine-legal real-evaluator fixture:
two serializer tests and four graph tests. They cover the alias domain, genuine
completed parent, original conditional `[2,7]` support/bit patterns, separate
arena references/meanings, corrupted domain/scope/diagnostics, typed operand
rewiring including cloned same-kind mass/coefficient, detached envelope ancestry,
and serialize-to-file/from_args/export roundtrip. Existing fixture literals and
Work-label assertion migrate with the production API. All tests are **unrun**.

E found a pre-freeze gap in typed operand checks; the companion now follows each
bound's producer ancestry and validates exact shared arithmetic refs. E re-reviewed the fix and found no further blocker within recorded-consistency/export
scope. This source review does not replace fixture execution or file parser
roundtrip validation.

Root validation targets (not executed here; use existing resource wrapper,
`CARGO_BUILD_JOBS=1`, two CPUs/below-normal/4 GiB, one job at a time):

```text
cargo test -p marjapussi-tools --lib selfplay -- --test-threads=1
cargo test -p marjapussi-tools --bin reasoning_graph -- --test-threads=1
```

Explicit `--bin reasoning_graph` is required for its inline tests because the
manifest disables its default test target. Root should then run required combined
checks. The existing expensive catalogue gates and all-Deferred profile remain
unresolved; this patch makes no runtime or strength claim.

No root files edited. No build outputs, processes or containers created; reclaimed
space: 0 bytes. Source worktree and shared fixtures retained for root validation.

# Single completed leaf contract and bounded reuse

Status: **incomplete; static-only handoff**. Base owned checkpoint
`b5ceb82a7d9c4054413d4b303be925b5cb9d049a`. Intended annotated tag:
`checkpoint/2026-10-11-sequence-leaf-effort-static`.

## Contract and ownership

Only `crates/heuristic/src/planning/sequence.rs`, its inline tests, and
`planning/sequence/reuse.rs` change. Existing `evaluate` and
`evaluate_with_cache` now take one callback returning
`Result<LeafResponse, LeafFailure>`. Both carry `canonical_solver_nodes: u64`;
success carries `action: GameAction`, failure `reason: SequenceError`.
`LeafResponse::unit(action)` and `LeafFailure::unit(reason)` cover completed
zero-solver primitives/tests and zero-work failures. They are not substitutes
for reporting work performed by the strict solver wrapper.

`LeafBudget.remaining_solver_nodes: Option<u64>` exposes the remaining logical
lane. `Limits.solver_nodes` defaults to None; no production node cap is invented.
`Work.actual_solver_nodes` records callbacks, including failed attempts;
`represented_solver_nodes` also debits completed response/tail/suffix hits.
Cached aggregate costs are deltas of represented work, so nested cache hits
are included once in their parent cost and are not charged again on parent reuse.
Actual callback effort is counted before deadline/error handling. A failed,
illegal or late response is never admitted. Overflow and unaffordable completed
units defer explicitly; no partial comparison becomes a ranked result.

The callback must remain semantically completed and deterministic for the exact
actor Information, ordered legal domain and configured model. It must report
canonical solver work under the same versioned effort algorithm. This type does
not prove callback correctness. Root's completed_response_with_effort_before
wrapper supplies that contract; E's common adapter patch already targets these
fields. The local caller search found only unreleased private evaluator/test
users; there is no dual legacy mode, scope bit or second evaluator.
Root adapts external `tests/internal/sequence_execution.rs` and integrations.
That included local fixture still has the old callback signature until root's
adaptation, so this isolated checkout is not claimed independently compilable.

## Cache semantics

Completed responses are optional acceleration. Zero entries/bytes and oversized
entries compute uncached within the same logical/deadline allowance. Response
admission preflights the payload before copying it; optional trace/dependency
capacities are reclaimed first. If responses still fill the allowance, a bounded
batch eviction drops their storage before admitting the new completed result.
Recomputation is sound only under the strict deterministic completion contract.
The obsolete ResponseCache completion failure is removed. No lookup-index,
world-count, solver-strength, candidate-order/tie, horizon or ordinary time-lane
change is made.

`DeferredLimit` gains RepresentedSolverNodes { required, remaining } and
SolverAccountingOverflow. A partial leaf failure reports its attempted effort
in Work and LeafUnavailable rather than inventing the full required node count.
A complete cached unit can report its actual known required cost. Warm and cold
partial progress need not match on a deferred call; complete logical cost and
completed comparisons must. Wall deadlines remain soft and may differ with
scheduling or cache hit rate.

Persistent keys/scopes retain exact input/configuration comparisons. Current
cohort weights/support rebuild ancestor comparisons; weighted optima/prepared
rankings do not become persistent. Bounds changes reset cache allocation scope;
solver allowance is not semantic response input and is deliberately not a cache
scope key. Actor caches remain private; no real peer Mind or receipt is read.
The memory ledger covers retained caches, not recursive transient output or
caller solver memory. Batch eviction can reduce actual performance; no speed
claim is made before owner measurement.

## Source regressions and validation

New inline scenarios compare completed values, actions/receipts, traces/prepared
records and represented transition/comparison/solver work with cold, warm,
uncached and bounded evicting caches. They exercise nested suffix/tail effort
without double charging, an exact complete node allowance, tight warm/cold
rejection, deterministic eviction/recomputation, zero-capacity/oversized-entry
uncached work, and partial failure accounting/retry without failure admission.
The obsolete call-order-changing response test is replaced by deterministic
responses with a call counter. The existing boundary-membership test now injects
a synthetic UndoRequest: Table::to_act already filters it, so this is an identity
normalization guard, not evidence of a live engine/policy mismatch.

All changed/new tests are **unexecuted locally**. Only rustfmt parsing/formatting,
caller search, staged source review and git diff whitespace checks ran.
No build, test, benchmark, smoke, deployment, process or disposable output was
created; 0 bytes reclaimed. Root owns fixture adaptation, compilation, bounded
correctness/privacy checks and profiling. This checkpoint is not qualified for
playing strength, timing or readiness.

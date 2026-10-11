# Shared model and benchmark integration: source checkpoint

Status: **incomplete — executable validation pending**.
Base: `44ec5da`, integrating the conditional-concept and benchmark source checkpoints.
Tag: `checkpoint/2026-10-11-model-reuse-source`.

Belief queries and heuristic continuations share a lazily constructed deal model.
Its fact DP survives signal-only changes; play, transfer and undo invalidate
affected state. Weights-only access leaves the DP unallocated. Model and query
caches have separate borrow owners, preserving `Mind: Send` without locks or
shared pointers.

Zero-work queries return cheap hard proofs or explicit unresolved bounds before
constructing the model. A root-private fact remains separate from proof that
another player knows it. Impossible conditions retain undefined probability.
Source regressions cover allocation avoidance, shared/private facts, retained
space across bids, play/undo invalidation, independent cache borrowing and
agreement with the existing weight model.

The benchmark runner accepts the reviewed additive knowledge revision as an
exact shared-core compatibility anchor for historical policies. Policy source
revisions remain unchanged. Reports retain forecast decision-reuse counters for
completed and interrupted comparisons, with a rotation/compaction regression.
Writer teardown confirms shutdown before rereading files. Report generation
stages artifacts, publishes completion last and records failures truthfully;
report-only execution takes the shared and per-run locks. Mock regressions cover
writer failure, nonzero exits, interrupted reports and publication failures.

Rust formatting, Python syntax parsing and whitespace checks passed. Independent
source review found no blocking privacy, borrow-lifetime or model-reuse issue.
The pre-existing entry estimator lacks a finite-positive total-weight guard for
nondefault zero-mass models; no such case is established for frozen defaults.
This remains a robustness limit, separate from executable validation below.
The original frozen 4,000-game b3abdaf-versus-1632af8 run continues unchanged.
Focused compilation, correctness/privacy checks and runtime qualification follow
its completion under the shared two-CPU/4-GiB job limit. The new policy's later
comparison uses fresh deals against b3abdaf. No speed or strength gain is claimed
at this source checkpoint.

No production deployment or Docker resources changed. Benchmark evidence and
build caches remain in use; cleanup is deferred until their final use.

# Checkpoint: tools canonical solver-effort capture

- Date: 2026-10-11
- Status: incomplete; source-only handoff, root integration and validation pending
- Root snapshot: `f50464a6d9f9a2e3657560c1672e78414bc1003b` plus captured working files
- Evaluator API: `5ff466b906da08816979e5a436a2b97de376860a`
- Commit/tag: pending root's focused integration checkpoint; none created by this handoff

## Result

Actual and represented `u64` response-solver nodes are exported by the existing
same-call sequence serializer and perf observations. Actual includes attempted
callback work on failures; compatible completed response/tail/suffix hits debit
represented effort. These counts describe the configured canonical algorithm,
not elapsed time, total CPU, complete policy work, or value-evaluator node counts.

Structured `DeferredLimit` details supplement the existing debug-text field.
`RepresentedSolverNodes { required, remaining }`, `SolverAccountingOverflow`
and `AuthoredGate` remain distinct; there is no obsolete `ResponseCache` arm.
`SequenceTrace` does not contain `Limits::solver_nodes`: the configured ceiling
and an uncapped `None` are not inferred from counters or stopping reasons.

The graph inspector retains costs as aggregate operational evidence, without
adding concept nodes or attributing aggregate work to individual dependencies.
Compact reports retain actual/represented solver and comparison counts with
recorded/missing/invalid denominators. Older compact records remain readable;
absent counters are unknown, recorded zero is zero, integer totals stay exact.
Full traces continue to reside in the retained NDJSON.

## Evidence and limits

- Static source review against the frozen typed evaluator API.
- Rustfmt parser-only checks and Python syntax checks; no builds or tests run.
- Patch applicability checked against captured current dirty-root file baselines.
- Three tests added, unrun: typed/u64 capture and deferrals; graph cost separation;
  reporting failures, hits, zeros, missing/invalid and legacy compact records.
- Root owns external fixture/caller adaptation and serialized qualification.
  This handoff establishes no runtime, performance or playing-strength result.

## Cleanup

No processes, containers, builds or benchmarks started. Baseline snapshots and
patch retained for integration; no disposable build output created (0 bytes reclaimed).

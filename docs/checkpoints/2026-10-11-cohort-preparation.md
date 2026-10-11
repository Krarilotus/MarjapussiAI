# Checkpoint: cooperative forecast cohort preparation

- Date: 2026-10-11
- Status: incomplete; source-only patch, tests and builds unrun
- Root base: `31860a8c8a8d589bd9b9d182d97df4a61d38ca85`
- Scope: `planning.rs`, its preparation helper/tests, this record
- Commit/tag: root integration checkpoint pending; no commit/tag from this handoff

## Result

Forecast preparation checks the caller's deadline between existing ordered
draws, likelihood calls, reductions, resampling selections and complete table
replays. Only a complete pool/cohort is returned. An interrupted helper returns
`PreparationError::Deferred` and drops its unpublished prefix; there is no prefix
normalization, replacement draw, resampling-count reduction or partial forecast.

The never-interrupted path retains `n * 256`, the same ordered draws and weights,
RNG consumption, floating reduction/resampling/merge order and legacy merged
world count. Duplicate selections are additionally available as ordered slots
for C's separate conditional-history integration. No history model is added here,
and the existing hidden-pass guard remains strict until C supplies that path.

`Forecast::new` now returns `Result<Forecast, PreparationError>` instead of
conflating all failures into None. `PreparationError::Deferred.reason()` is
`SequenceError::Budget`; `InvalidCohort.reason()` is `InvalidCohort`. Root must
adapt the two caller-owned Option matches in preplay/strategic before compiling.
Use their existing `common::defer` and
`Failure::before_evaluation(error.reason(), candidates)` on Err. Deadline or
history proposal exhaustion must not become InvalidCohort. The bool cooperative
checker carries no finer stop-lane proof; leave that optional lane unknown unless
C/root explicitly supplies it. No common/preplay/retained changes are in this patch.

Opt-in profile scopes separate proposal draws/weights, reduction and table replay.
Disabled-profile scopes remain the existing zero-cost stub. Individual DP builds,
likelihood calls and table replays are atomic at this layer; it is not a hard
wall-time bound. The small exact-enumeration walker has no early-stop API: once
interrupted it stops weighting but finishes walking its at-most-n allocations.

## Evidence

Static parser/diff/applicability checks only. Six unrun tests cover original
ordered sample/weight/resampling/RNG parity, cancellation before another weight,
duplicate-slot retention, engine/private-view replay parity and interrupted replay,
small exact enumeration parity/interruption, every reduction-check interruption
and invalid/zero mass. Reference reduction is the frozen original implementation.

No new game results, runtime measurements, tests, builds or strength claims.
Root owns serialized validation and external callers; C owns checked hidden-pass
construction, the declared conditional history model and Forecast integration.

## Cleanup

No jobs or containers started. No disposable build outputs created (0 bytes
reclaimed). Source baselines and patch retained for coordinated integration.

# Completed sequence dependencies and joint reconsideration

Status: **incomplete; source-only integration handoff**. No qualification or
playing-strength claim. No builds, tests, smoke, profiling or benchmarks run.

Starting owned checkpoint: `78a3832e3f2d2463be785f41dc645a8ebd3ee776`.
C's concept APIs imported separately: `c6180f13b16060fc75a7d8a72c99fb9122ef6e00`
and `18de60a13b4ca3c215f8db60885c5855c198aa29` (local `1dec927`, `8d1a147`).
Root's integrated ValueStore, strict response wrapper, retained policy,
serialization and corrected external fixtures remain root-owned. An attempted
unrelated integration cherry-pick was aborted; no outside owned paths remain
changed by that attempt.

## Integration API and behavior

- `planning::sequence::CompletedCache::default()/clear()/retained_bytes()` and
  `evaluate_with_cache(..., &mut CompletedCache, leaf)` retain bounded exact-input
  completed dependencies and leaves between comparisons. Existing `evaluate`
  remains a cold wrapper. Cache is private, not a serialized semantic receipt.
- Scope checks root, actual response Params (including RNG/configuration), belief
  and response model identities, entry/byte bounds. Full actor Information and
  ordered legal actions identify responses; all four hypothetical actor views
  and ordered legal actions identify choice-free terminal tails. Hard proof
  reads use full root Information plus exact query. Canonical programs use full
  immutable equality and demanded stages; authored membership uses full root
  Information and ordered legal domain.
- Posterior generation, current cohort ids/order/weights and cohort-generation
  seed are not cached optimum inputs. Weighted suffix memo, PreparedChoice and
  assessments are comparison-local. Every call rebuilds conditional support and
  ancestor ValueStore comparisons. Root must clear on model/fork boundaries and
  keep response-model identity truthful if callback behavior/configuration changes.
- Reconsideration first groups full root Information and canonical boundary.
  C's pure binder produces current authored routes plus retained suffix; the
  evaluator installs that program at the consumed boundary, compares all bound
  alternatives on the same group's support, records actual comparison membership
  and propagates its selected descendants before ancestor selection. No
  individual-world optimum or procedural actor-selection callback is added.
- `prepared_lookup` conservatively returns NeedsReview for unbound Reconsider.
  Root owns temporary warm installation and global coverage certification.
- `Terminal.first_program` explicitly owns its canonical `first.next`. The
  returned Comparison receipt comes from that actual owner, including first
  actions selected after composition. Consumers must not pair composed stage ids
  with the original Candidate.program. Candidate remains source provenance;
  runtime owning program/trace is the receipt/first_program.
- `Deferred.limit: Option<DeferredLimit>` adds typed failed-lane information:
  deadline, represented transition/comparison allowance, mandatory response cache,
  unavailable leaf, finite continuation and reconsideration. SequenceError stays
  unchanged. Root must wire these diagnostics and any serializer/struct literals.

## Admission, accounting and completion contract

Optional suffix trace, tail, proof, canonical and membership caches are dropped
with their capacities when they would block mandatory response admission. Only
completed responses survive this reclaim. New response payload is copied only
after preflight admission and a valid completed callback. Failed/illegal/expired
callbacks have no entry or retained reservation. Logical engine transition and
comparison costs are charged on hits before exposing cached results. Tail reuse
can be disabled without bypassing response identity checks.

The callback now explicitly MUST return a deterministic semantically completed
response under Context's actual model/configuration. Incomplete eligible solver
work must return an error, not a timed world prefix or weaker fallback. This
bare GameAction callback cannot verify completion: root's new strict wrapper is
required. Old timed-first-result freezing is **not** an exact acceleration.

Canonical solver/leaf effort is not returned by this API yet. Thus represented
engine transition/comparison parity is implemented, but solver-node budget parity
is still unresolved. The next additive action-plus-effort API must debit canonical
leaf effort on hits too; no new solver cap is invented here. Responses remain
mandatory until that follow-up, so a genuinely full response cache still defers.
Under the strict deterministic completion contract later eviction/recomputation
can be sound, but was not added before this integration handoff.

The byte ledger bounds retained caches, not transient recursive results,
branch/Program clones, root agent state, leaf solver allocations, or total process
memory. Cache admission/deadlines may change completion, never quietly prune the
compared candidate catalogue or support. Callback preemption remains impossible;
the wall deadline is soft. No worlds, strategy bindings or ordinary-play budget
were reduced/extended by this iteration.

## Source regressions and unverified limits

New inline regressions cover nondyadic reweight/reorder with rebuilt assessments
and support; configuration invalidation and Send; typed deadline/logical/cache/
leaf failures; joint reconsideration's actual domain, common support and first
program ownership; and optional-capacity reclamation preserving completed
responses. Existing external execution fixtures remain included without editing
root's fixture file. Existing logical-budget regressions remain unchanged.

Rustfmt parsing/formatting and diff-whitespace checks are the only local checks.
All tests above are **unexecuted**, compilation/integration/serialization and
cache-enabled/disabled strict-solver parity remain root validation tasks. The
isolated branch does not contain root's ValueStore integration and is not claimed
independently buildable. New C catalogue expansion remains a separate checkpoint;
bounded depth/fanout and response approximation are not exhaustive strategic play.
No prepared global ranking transfers to a fresh posterior, no actual other
agent's backend/receipt is read, and no readiness/smoke approval follows from this
static checkpoint. No disposable processes/build outputs created; 0 bytes reclaimed.

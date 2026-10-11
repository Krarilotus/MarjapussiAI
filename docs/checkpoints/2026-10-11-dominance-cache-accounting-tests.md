# Completion-dominance cache accounting companion

Status: **incomplete, source-only**. Base is B's frozen
`ebf4df0f3901395bd7db137b6ed2a1b2b4ba8236`, on root `ad6e99e` plus the validated
knowledge prerequisite. Branch `test/dominance-cache-accounting`; intended tag
`checkpoint/2026-10-11-dominance-cache-accounting-tests`.

Only a new child test module and its declaration are added. No production,
fixture domain, shared-core guard or policy changes. The genuine seven-trick
fixture and Local candidate domain come from the parent's existing helpers.
Two original support slots retain IDs 2/7 and weights .25/.75.

Three focused tests cover:

- Completed parent `finish` without caches versus a cold cache and a served
  within-session parent memo hit: identical terminals and prepared proof,
  represented transitions/comparisons/nonzero solver effort, no repeated leaf or
  proof evaluation. A comparison allowance one below the cached cost rejects
  before exposing records or charging the unit.
- A genuine dominance-bearing parent under response entry-only, byte-only and
  combined pressure: exact ledger subtraction, proof memo survival and subsequent
  hit for entry-only; optional caches cleared under byte pressure. Returned
  caller-owned proof remains valid independently of cache eviction.
- Exact/one-less byte admission for `run_bytes` of a real proof-bearing payload,
  and the full `finish` path using its measured final retained-byte threshold.
  The latter checks that only the final optional parent admission differs;
  execution, evidence and represented work remain equal. It does not assume
  cloned vectors retain their original capacities.

The deterministic test leaf reports seven canonical nodes per callback. This is
an accounting fixture, not execution or validation of the solver algorithm. The
leaf honors an insufficient supplied node allowance with typed failure/effort.
Eviction probes use distinct legal Start states; they do not alter the proof
fixture or reduce its continuation coverage.

Validation performed: rustfmt on the new test source; staged diff/whitespace and
scope inspection. **No compilation, tests, Clippy, benchmark or games run.**
Root owns the serialized bounded runner. Suggested narrow test filter:
`planning::sequence::dominance::tests::cache_accounting` with one test thread.
Runtime assertions and compilation remain unverified. E's optional source-audit
request was not delivered because its thread was running; no independent review
of this companion is claimed.

No processes, containers or build artifacts created. Reclaimed zero bytes;
retain this small source worktree until root integration/validation is complete.

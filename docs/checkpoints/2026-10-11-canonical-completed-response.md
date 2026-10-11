# Checkpoint: canonical completed solver response

- Date: 2026-10-11
- Status: **incomplete** — implementation and tests authored; builds/tests intentionally not run while root owns validation.
- Tag: `checkpoint/2026-10-11-canonical-completed-response`
- Branch: `review/sequence-scenario-gates`
- Base commit: `e32b98982de190d135e6e5f9f70d749394b2bf2d`
- Paired repositories: unchanged; no testbed work or publication.

## Result

`solver::evaluate_complete` takes the same arguments as `evaluate_limited`.
It clears the supplied solver's reusable TT and resets its node counter once
before the entire evaluation, retaining normal within-evaluation TT reuse.
Only the unchanged existing `Decision` with `complete == true` may succeed;
a completion-time deadline gate also rejects work finishing after the supplied
safety deadline. Errors and incomplete world/secondary-criteria results return
typed failures with consumed canonical node effort, never a fallback action.

The accounting convention is named `empty-tt-whole-evaluation-nodes-v1`.
It is deterministic for fixed solver TT capacity, semantic inputs, belief,
initial RNG state and search implementation when the work completes. A cache
hit must charge this represented effort; raw warm-TT node counts are not an
equivalent metric. This does not account for all belief/sampling work, provide
a hard wall-time guarantee, or equate completed runs with deadline-aborted ones.

The wrapper invents no node limit and changes no world count, objective,
criterion, solver capacity or search strength. Existing `evaluate` and
`evaluate_limited` APIs are unchanged. Root owns strict endgame integration,
shared ledger charging, and moving applicability checks before deadline checks.

## Evidence

| Command or review | Result | Scope / limitation |
|---|---|---|
| Source/API review against current solver and endgame | Completed | Confirms partial-world and secondary-criteria completion distinction; not runtime validation |
| `rustfmt --edition 2021 crates/solver/src/completed.rs crates/solver/tests/completed.rs` | Completed | Formatting only; no compilation |
| Staged diff and whitespace review | Completed before commit | Only solver wrapper/export, focused tests and this checkpoint staged |
| Build, solver tests, lint, benchmark, selfplay | **Not run** | Root owns the validation lane; no new success/performance claim |

Authored tests cover complete semantic/node parity against a fresh ordinary
evaluation, reused TT/counter contamination, exact node-cap completion, zero
budget/expired deadline, original zero-weight errors, interrupted world-prefix
rejection, and interrupted secondary contract criteria after all first-pass
worlds complete. They reuse the synthetic seed-41 legal-prefix construction
already used by `solver/tests/worlds.rs`; no private datasets are involved.
The new assertions remain unverified until root executes them.

## Remaining work

Root should integrate the wrapper and run the named solver `completed` test
target plus affected strict-response checks under the shared resource limits.
No builds/tests were attempted, so compilation and test failures remain unknown.
The previous verified strategic baseline identified by the existing dryrun
record remains `1632af8`, `checkpoint/2026-10-10-claim-established-suits`;
this incomplete checkpoint does not replace or promote that baseline.

Full-response caching must retain the same RNG/cohort, TT configuration and
semantic model identity, and debit recorded effort on hits. Preserve the
eight-world/two-trick response configuration; no performance-driven weakening
is part of this change. Policy tie-breaking outside the solver and phase
consumer integration remain root-owned.

## Cleanup

No builds, tests, jobs, servers or Docker resources started. No disposable
outputs created; **0 bytes reclaimed**. Shared caches/evidence retained.
Local commit/tag only; no push, merge or deployment.

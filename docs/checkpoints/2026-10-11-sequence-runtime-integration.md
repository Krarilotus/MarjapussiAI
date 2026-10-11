# Checkpoint: sequence runtime integration

- Date: 2026-10-11
- Status: incomplete; semantic checks pass, strategic/performance gate fails
- Base: `28e6039`
- Tag: `checkpoint/2026-10-11-sequence-runtime-integration`

Finite authored continuations participate in open-lead selection. Retained programs, current evidence and conditional estimates have separate identities. The policy compares fresh authored routes and its retained suffix without a sunk-cost bonus. Unfinished comparisons record Deferred; their predictions are invalidated while semantic intent remains available for repair. Auction, return allocation and raising still use the existing continuation implementation at this checkpoint.

The actual evaluated alternative domain gates prepared selection. An availability catalogue cannot certify a ranking. A validated covering record and its conditional support authorize the consumed path. Raw history detects forks and new undo events; semantic receipts use canonical accepted events. Forced accepted actions advance once. A separate transaction snapshot preserves identical retries, including deferred fallbacks, without advancing the program. Clones retain private intent and retry state while rebuilding private readers and solver scratch. Hypothetical response actors omit the redundant transaction snapshot and never receive actual partner intent.

Runtime recording captures the same value assessments, program nodes, consumed guards, joint trick outcomes, resources and conditioning scope used by the policy. Sequence-only records do not claim a fresh value ranking. Compact reports distinguish recorded modes and actual versus represented work; absent attempted-candidate counts remain unknown. Full traces stay in external NDJSON.

## Validation

All jobs used the shared validation lock, one build/test job, two logical CPUs, below-normal priority, 4 GiB process-tree limit and optimized test profile.

| Evidence | Result |
|---|---|
| `sequence-runtime-unit3`: heuristic library | 42 passed; includes cache work equivalence, action-dependent activation, joint information-set choice, coverage, forced/retry/clone/model invalidation and undo-repair-retry-acceptance. 19.47 s including compilation, 360.5 MiB peak. |
| `sequence-tools-recording-tests`: selfplay recording tests | 7 passed at the earlier integration snapshot; structured scope, shared refs, trick resources, privacy and 128 two-game blocks. Later runtime changes still require final tools validation. |
| `sequence-report-tests`: Python runner/report/build guards | 26 passed; 0.51 s and 34.0 MiB. |
| Initial heuristic integration | 36/38 passed. Corrected two fixtures: the retry fixture began on a forced move; the hidden-swap fixture changed the visible first trick. Reconstruction now preserves the observer prefix through Table::from_view. No production rule was relaxed. |
| Two development performance games, seeds 0 and 1 | Completed in 4.32 s, 40.8 MiB. All 12 sequence attempts deferred. Eleven ordinary attempts exhausted their soft 50 ms lane; one initial formation exhausted approximately 8192 represented transitions before completing the catalogue. This is failed useful-coverage validation, not a playing-strength result. |

Evidence logs and JSON live outside Git under the shared validation directory. Heuristic final log SHA-256: `19fa45526d2cd67cd3480c4d6a12f76648a3ea46f2457660111eee0fe616d713`. Python report log: `28700222f58d2475380758b13ee51d13511a8f8cbc52e0e5b287353779f1e925`. The pre-optimization performance executable and outputs are separately fingerprinted in the private work area; its early working-tree patch alone is not a complete source archive.

## Remaining gate

The 256-game comparison has not started. Initial formation must actually complete, ordinary follow-through must remain useful under current evidence, and the integrated policy must pass the targeted loss/close/control cases and performance checks. Planned corrections remove redundant trace copying, canonicalization and catalogue rebuilding, then address formation entry and dependency work coverage. Soft deadlines cannot preempt an individual Rust primitive; no hard 50 ms guarantee is claimed. Complete global prepared coverage remains rare and conservatively requests repair.

The portable HTML has not yet been refreshed with a completed sequence decision, and no production update occurred. No owned container or background server is running. Shared validation caches and evidence are retained for the next iteration; reclaimed space: 0 bytes.

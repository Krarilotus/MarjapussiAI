# Retained domains, canonical actions and direct hard reads

Status: **incomplete**. Base: `872bc75`. Intended tag:
`checkpoint/2026-10-11-retained-domain-integration`.

The live policy and modeled actors share one canonical eligible action order.
Exact pass payloads and authored candidate order remain intact. A retained
proposal stores its original complete legal domain, including Tail proposals,
and clears it on acceptance or invalidation. Depth-two preplay reuse checks the
actual bound continuation domain. Hard concept guards use the existing exact
Knowledge proof kernel directly, avoiding construction and replay of a Mind.
Unused private response wrappers are removed; strict response callers use the
typed completed-effort API.

Integrated sources: A `1e7ff18`, E `19b9ba3`, D `acf2923` and profile-only
`1e3b29a`, plus A's focused response-wrapper cleanup. Focused patches preserve
concurrent root changes. The accompanying source checkpoint notes describe the
individual contracts and source-only handoffs.

## Evidence

- `direct-hard-unit`: 32 belief checks passed (21 library, 11 conditional).
- `canonical-retained-unit`: compilation exposed an implicit test import of
  `BeliefParams`; the fixture now imports it explicitly.
- `canonical-retained-unit2`: 83 of 85 heuristic checks passed. Test execution
  took 27.52 s, total build/run 75.37 s, peak process-tree commit 447.3 MiB.
  The reconsideration fixture expected raw action order; its binding input now
  uses the canonical helper, pending rerun. The genuine full-catalogue two-world
  auction/return fixture exhausted 2,000,000 represented transitions before its
  first complete candidate: 1,123,959 actual transitions and 44,651 represented
  comparisons. This remains a failing integration gate, not a latency guarantee.
- `structural-profile-build` passed. `structural-profile-two` ran the same two
  development deals as the earlier diagnostic, 105 decisions. All 23 planning
  attempts still deferred: 13 deadlines, 8 inherited comparison-limit stops,
  2 unsupported hidden-history preparations. Action and action/rule hashes
  matched the earlier diagnostic; no playing-strength inference follows.
- Detailed instrumentation attributed 4,599 ms of 12,721 ms evaluator time to
  447,330 suffix requests, including 451 ms of cached suffix copies. These are
  instrumented diagnostic timings on a shared machine, not release qualification.

Runner receipts and private source/profile artifacts are under the shared
`tmp/2026-10-09-v1` and `tmp/2026-10-11-structural-profile` folders. Every build/run
used the serialized two-CPU, below-normal-priority, 4-GiB process-tree lane.

## Remaining gates

Parent selection must explicitly authorize finite local continuation execution;
full catalogue availability alone cannot certify a local comparison. Hidden
exchange histories need checked hypothetical reconstruction. Complete useful
formation and repair, efficient suffix reuse, the strong-contract regression,
and changed-world/privacy scenarios remain required before the 256-game smoke.
The smoke, deployment and public strength evaluation have not run.

All retained worktrees, build caches and evidence are still needed. No owned
Docker resources were created; reclaimed space: 0 bytes.

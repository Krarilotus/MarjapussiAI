# Checkpoint: conditional-sequence design

- Date: 2026-10-11
- Status: **review-only**
- Tag: `checkpoint/2026-10-11-conditional-sequence-design`
- Branch: `design/conditional-sequences`
- Base: `1907407a3ef680a8ed14eaa4a97aba91fe9c989e`, following
  `checkpoint/2026-10-11-conditional-plan-dryrun` (`4acefbb`).
- Testbed source inspected: `cd553cc0beb03887c319aa3c9348b1190151b32f`,
  `feature/takeback-claim`; unchanged by this iteration.

## Result

The [design](../design/conditional-sequences.md) connects resource preservation
to conditional execution across turns. It specifies typed sequences, adaptive
opportunity checks and repair, joint evidence, private semantic receipts,
all-seat preparation, transactional persistence, shared forecast/live execution,
bounded lazy work, cache keys/invalidation and staged removal of superseded code.
Two self-contained [research prompts](../design/sequence-research-prompts.md)
cover adaptive planning and efficient incremental reasoning.

Plan feasibility and current value are separate. New beliefs can replace a
still-feasible sequence. Cached estimates from an older posterior cannot settle
the new comparison. Fixed logical work is the default serving and validation
contract; faster caches do not silently increase search effort. Safety deadline
aborts remain explicit incomplete results.

## Evidence

| Check or review | Result | Scope / limitation |
|---|---|---|
| Existing heuristic, planning/cache, concepts, perspective and dryrun sources | Inspected | Design grounded in existing ownership; no production edits. |
| Testbed runtime/service/native sources at the revision above | Inspected | Four cached agents, eviction, shared PyO3 service and pre-commit mutation identified; no live deployment verification. |
| Independent sequence/solver design review | No remaining design blocker | Opportunity-driven adaptation, current evidence stamps, answer choice and execution-vs-selection distinctions checked. |
| Independent runtime/cache design review | No remaining design blocker | Preparation barrier, atomic receipts, honest cold mode and fixed logical serving work; final clarification rechecked by reviewer. |
| Static Markdown validation | Passed | Changed document links resolve; code fences balance; no replacement characters. Mermaid diagrams inspected as source, not rendered. |
| `git diff --check` and staged diff review | Passed | Documentation only. |
| Builds, automated gameplay tests, benchmarks | Not run | No executable code changed; no new strength or speed claim. |

The prior 4,000-game evidence and targeted dryruns are reused, not rerun. Dataset
fingerprint: `9a1eb1b4e7a5118f6eefb90924c6da60767629f49b5ebfb0a4767777219699b7`.
The [previous checkpoint](2026-10-11-conditional-plan-dryrun.md) remains
strategically incomplete. Known loss cases are diagnostics, not guaranteed wins.

## Remaining work

Implement and validate the first vertical slice. Receipt persistence, execution
parity, privacy, adaptive reassessment, cache neutrality and measured ordinary-play
latency are gates before promotion. Earlier allocation/contract errors, actor
response-model mismatch, hidden transfer histories and probability calibration
remain explicit limits. Research has been scoped, not conducted in this iteration.

This checkpoint changes documentation only. It is local; nothing was pushed,
merged or deployed, and no new policy is claimed ready for play.

## Cleanup

No build/test job, local server or Docker container was started. Review agents
finished. No disposable build output was created; reclaimed space: **0 bytes**.
Retained the shared benchmark evidence, existing target caches, review notes and
isolated design worktree for implementation and audit.

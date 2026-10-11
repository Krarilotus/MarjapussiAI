# Common authored continuation runtime

Status: **incomplete strategic qualification**. The integrated policy unit checks pass; the runtime reuse defects listed below still prevent benchmark readiness. Base `f50464a`; intended annotated tag `checkpoint/2026-10-11-common-runtime-integration`.

## Implementation

Auction/return interventions and play formation/repair share one evaluation, strict actor-response adapter, expected-score comparison, receipt installation, and deferral path. The unused scalar rollout and old planning cache are removed. Preplay transitions retain their original joint cohort through a visible phase gate into the authored play catalogue; the initial binding preserves the play reconsideration horizon. Response nodes represent a fixed modeled actor response, never an assessed strategic choice. Exhausted phase coverage defers explicitly.

Canonical solver work travels through the single typed leaf result on success and failure. Completed response, tail and suffix hits charge the same represented work as their fresh computation; actual work is reported separately. Cache capacity affects acceleration, not semantic completion: an oversized or evicted completed response can be recomputed. No default solver-node cap, reduced world count, reduced solver strength or shorter candidate horizon was introduced by this integration.

Strict responses reject enabled recursive forecasts or retained intent before synchronization, with a typed failure and zero solver effort. Hypothetical actors use only their own filtered information. Actual live decisions retain their ordinary bounded policy. Formation allowance is not replenished by cloning or retrying a transaction. Accepted history, forks and undo determine when private receipts and disposable caches can be reused.

Sources: response dispatch `b5ceb82a`; typed evaluator `5ff466b`; common path `968a4af`; typed adapter `e975f3b`; strict-response isolation patch checkpoint `e51f976`. Earlier solver and catalogue changes are in the base ancestry. The focused handoff patches were integrated into the existing root work, including external fixture adaptations.

## Evidence

All runs use the shared runner: one build job, two logical CPUs, below-normal priority, 4 GiB process-tree memory, one test thread, optimized standard test profile.

- `common-leaf-unit`: 77 policy library tests passed, none failed. Total 50.74 s including compilation; test execution 1.04 s; peak committed memory 497.4 MiB. Log SHA256 `525fca8cd42e493cc8e85fcea6118f90efa193fb32bca13652685fcacf5b28d8`.
- `logical-clear-unit`: 7 solver library, 4 completed-response and 6 exact/oracle tests passed. Total 224.6 s; peak 267.5 MiB. Log SHA256 `514986a1662215936ba3b882596055f50f7a5f77fd1c433c29a0ac4ca80f9a14`.
- The preceding 72/73 policy run used an invalid post-exchange fixture with a forced opening. Its repaired fixture has two legal opening leads, identical actor information across hypotheses, and the original joint-weight assertion; it passes in the 77-test run.
- Cache tests cover matched represented work with cold/warm/uncached execution, nested hits, eviction, incomplete-response rejection, tight budgets, changed weights and contexts. These are correctness checks, not playing-strength or timing evidence.

## Remaining acceptance work

Independent source review found three real receipt integration defects: depth-two preplay binding is compared with a plain catalogue during live coverage checks; simulated sorted hand order can differ from shuffled live legal order; the inner pending retry only checks that the old action remains legal. Corrections and true preplay-to-live prepared integration tests are required. No successful production Prepared claim is established by this checkpoint.

Forecast formation still rejects observer-hidden historical exchanges, excluding defenders after exchange. Hypothetical history support needs explicit modeling rather than access to real private cards. Cohort construction has nonpreemptible stages before the evaluator; ordinary latency is a soft limit. Hard-read replay and structural copying need current-run profiling. Finite authored routes still terminate in an explicitly modeled response tail; this is not complete-game optimal imperfect-information search.

Tools/captured graph changes have not yet been validated with this integration and are outside this commit. Next gates are real formation/prepared/repair traces, equal-coverage profiling, and targeted loss/close/control replay. Only then run the fixed 256-game paired comparison against the frozen live policy. No smoke games, deployment or restart occurred.

No disposable Docker resources were created. Shared build caches, worktrees and evidence remain needed for the next checks; reclaimed space: 0 bytes.

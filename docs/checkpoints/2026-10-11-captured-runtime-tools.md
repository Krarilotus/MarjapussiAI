# Captured continuation evidence and reporting

Status: **tools checks passed; policy qualification incomplete**. Base `31860a8`; intended annotated tag `checkpoint/2026-10-11-captured-runtime-tools`.

## Contract

Perf and recorded-game dryruns capture the exact same policy call's assessment and sequence trace after decision timing. Recorder consumption checks seat, event count, action, complete observer context and metadata; missing historical evidence stays missing. Capture does not rerun the policy. A continuation-hook capture failure is returned before emitting the bounded result.

The reasoning-graph exporter accepts `--capture`, `--index` and `--step`, checks references and selected terminal/value alignment, and shares equal authored definitions without merging distinct runtime contexts or joint outcomes. Response transitions, phase guards, explicit deferral and response tails retain distinct meanings. Operational limits and effort remain inspector metadata rather than strategic concepts. The existing HTML layout can consume this runtime export through `--runtime-graph`.

Reports expose actual and represented canonical solver nodes separately, retaining exact integer counts, zeros, missing data and invalid input distinctions. A configured node ceiling cannot be inferred from the counters and remains unrecorded. The metric measures the canonical response-solver algorithm, not total CPU work or confidence in a move. Candidate completion, value-rank completion and prepared execution remain separate observations.

## Validation

Shared one-job/two-CPU/4-GiB runner, optimized test profile:

- Tools library: 28 tests passed in `capture-tools-unit2`; log SHA256 `1974ad4a17afa8cc0a736508476abf5270e2f8045c4b24abebcdaff2cae5a30d`. That combined command also exposed a graph fixture expecting operational inspector text to remain unchanged when counters changed.
- Graph exporter: all 7 tests passed in `capture-graph-unit3`, 10.38 s including compilation, peak 268.9 MiB; log SHA256 `f0688a86c3ecb3e9de596c03f9731b1f8a632449888ace61d008f7582fc52bb4`. The corrected check excludes only the operational inspector text; all node identities, values and dependency edges must remain identical.
- Report: all 13 tests passed in `capture-report-unit2`, 0.14 s, peak 28.4 MiB; log SHA256 `e4f25b818708a8e4c9c89f11ae2921b8ea3c4be5b30633c323c1deb05255d494`. Its first run found a missing workers field in the new render fixture, now supplied.
- Initial tools compilation caught a stale Terminal fixture lacking first_program; it now explicitly has no first action/program. No production behavior changed for these fixture repairs.

Focused source handoffs: captured-graph-export, dryrun-runtime-capture, sequence-operator-capture and typed-leaf-effort-capture. The last patch SHA256 is `ed352165b7b5b39da8598cf8fb314631cbccf588175c9e8c484762b981ac3bcd`.

The earlier two-development-game profile is diagnostic evidence only: 105 decisions, 23 sequence attempts all Deferred, no Formation or Prepared. Seven hit the represented-comparison allowance, fourteen hit deadlines, two failed cohort construction. Its source identity and private captures are retained outside Git; log SHA256 `b903a6a65980d5517d4b551a24164d6d13094fcbabc43c588c0ea970684366e6`. It precedes the new effort-field export and subsequent reuse/performance fixes. No playing-strength inference is made.

No 256-game smoke, HTML delivery, deployment or restart occurred. Current implementation/latency/coverage gates remain unresolved. Private datasets and generated captures are excluded. No containers or disposable build trees were created; shared caches and evidence are retained for planned validation, 0 bytes reclaimed.

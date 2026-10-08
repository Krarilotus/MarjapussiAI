# Checkpoint: implementation review and continuation plan

- Date: 2026-10-08
- Status: **review-only**; not a release or a declaration that all tests pass
- Tag: `checkpoint/2026-10-08-review-plan`
- Branch: `planning/ai-review-next-steps`
- Base: `kb/batch1`, `4782c65a41b7d2c8dcb50f3efac54a1bcc67aadd`
- Compared AI revisions: core `9563626`, preview `1c077ae`
- Testbed inspected: `066581872d3f0181ecf377487f2ddcddcfae38e8` (static review)

## Result

Added a current overview, evidence-based code/AI review, ordered project plan,
checkpoint workflow, checkpoint template, and persistent repository/editor rules.
No production code changed. Retained two bug reproductions outside Cargo test
discovery. The original AI/testbed checkouts and outer workspace changes remain
untouched. No push, merge, or pull request was performed.

## Evidence

Windows; Rust/Cargo 1.91.1; two build jobs and two test threads; an isolated target
under `Documents/MarjaAI/tmp/2026-10-08-ai-review/target`. The shared CPU was busy,
so these are correctness results, not latency benchmarks.

| Check | Result | Scope / limitation |
|---|---|---|
| `cargo fmt --all -- --check` | Passed | Reviewed workspace and final documentation state. |
| `cargo metadata --offline --locked --no-deps --format-version 1` | Passed | All nine AI crates resolve. |
| `cargo clippy --offline --locked --workspace --all-targets -j 2 -- -D warnings` | Passed | All normal targets; no production edits. |
| `cargo test --workspace --offline --locked -j 2 -- --test-threads=2` | **Incomplete; all completed tests passed** | Stopped the long 2,000-game self-play audit explicitly. This command did not exit successfully; full suite/doc tests are not certified. |
| Completed engine checks | Passed | Rule/scoring/legality, allocation guard, fast-core comparison, golden traces, recorded-game replay. |
| Completed knowledge/observation/belief checks | Passed | Soundness, exact sampling/count comparisons, public/private filtering and hidden-card swaps. |
| Heuristic scenario tests | 67 passed, 1 ignored | Bidding 24; passing 17; play 26 plus one pre-existing ignored owner scenario. |
| Completed solver checks | Passed | Brute-force/engine minimax comparisons, transposition fields, weighted worlds and privacy. |
| Completed tools checks | Passed | Policy hidden-card swaps, record output, deterministic self-play (50 deals played twice). |
| Review probes (separate target) | **2 failures reproduced** | Cached auction value 120 vs fresh 125; `trust_partner = NaN` accepted. |
| Testbed Cargo metadata | Failed to resolve sibling agent dependency | Existing sibling is on core-only `main`. Server/frontend runtime checks were not run in this review. |

Ignored semantic checks remain the owner card-choice case and pass-read calibration.
Ignored speed experiments were not run. No new strength tournament, training run,
benchmark, or UI/browser acceptance test was performed. The T3 reviewer thread
could not run because its provider quota was exhausted; findings are from direct
source inspection and the stated checks, not a completed independent-agent review.

Private replay fixture: 1,579 NDJSON records, SHA-256
`e41a69d6f81819dc3619655c9a35ca39c906a8ebe647e3aac869bd65b8823865`.
The engine replay guard passed with its documented legacy normalisation/exclusions,
expected digest `cb8cf66a3bd29d91`, and expected aggregate scores. The data was
read in place, not copied into this repository. This identifies a verified reusable
fixture for later reviews; the private records and player identifiers remain outside Git.

## Reproducing the new findings

At this checkpoint the standalone probes used synthetic seed 42 and invalid
configuration text. They failed at the reviewed base for the reasons above.
The original artifact remains in this checkpoint's Git history. Later fixes
promoted their coverage into the normal tests and removed the duplicate probe:

```sh
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=<owned-target> \
  cargo test --offline --locked -j 2 -p marjapussi-heuristic --test history --test params -- --test-threads=2
```

See the [history](2026-10-08-history-cache.md) and
[configuration](2026-10-08-config-validation.md) checkpoints for the fixes.

## Remaining work

Follow `docs/NEXT_STEPS.md`, starting with baseline consolidation and F3/F4.
The branch has known behaviour/evaluation limitations and inherited publication
safeguards to reconcile. No strategic changes have been approved or implemented
by this documentation checkpoint.

## Cleanup

All owned review jobs are stopped; no containers or servers were started.
The generated target contains **1,525,933,400 bytes (about 1.53 GB)**. Automatic
approval review rejected deletion, including a literal-path retry, with the reason
`blocked by policy`; it remains at the path above. **Build space reclaimed: 0 bytes.**
Global Cargo caches and the private dataset are retained. The temporary scaffold
review worktree was unregistered and its temporary branch removed; the initial
T3 scaffold was restored to `master`. Its empty temporary folder remains under
`.t3/worktrees/marjapussi-ai-plan-review-20261008` because Git reported permission
denied when removing the directory. The continuation worktree is intentionally retained.

# Checkpoint: reuse hidden-exchange likelihood sums

- Date: 2026-10-14
- Status: **incomplete — compilation, correctness and performance validation pending**
- Tag: `checkpoint/2026-10-14-exchange-sum-reuse`
- Branch: `work/2026-10-14-v1-performance`
- Base commit: `af55bedcebe982644b937573ee739dada254b9fd`
- Paired repository revisions: no testbed changes or deployment.

## Result

`Mind::deal_model` already retains its model within an epoch and shares the fact
DP with exact marginals. However, every model refresh discarded its hidden-exchange
terms. Repeated card plays could reconstruct the dense signature index and repeat
the same sums over possible dealt feature splits. The separate feature-posterior
cache already retains its own exchange terms; this patch addresses the joint deal
weights used by concept queries and sampled-world consumers.

A refreshed model now takes the previous model's exchange terms only when the
ordered hidden passers, their two complete dealt-class likelihood tables, and the
exchange kernel compare equal. Reuse stays within the same Mind; observer, private
history owner, parameters and seat kinds are never shared between Minds. The new
model still constructs its own current facts, dealt/played card mappings, factors
and likelihoods. Its fact DP and all query caches retain their existing epoch keys.

Each retained term depends only on its held-feature-count signature and the checked
inputs. `weight` still computes that signature from current holdings plus played
cards. Rights, trump, other seats' likelihoods and card factors do not enter the
exchange sum; their ordinary owners still refresh. No arithmetic, traversal order,
world count, query budget or policy rule changes. The cache is moved, not copied;
its existing finite signature vocabulary is unchanged, but terms live longer.

Owned files: `crates/beliefs/src/deals.rs`, `crates/beliefs/src/features.rs`,
`crates/beliefs/tests/internal/exchange_reuse.rs`, and this record. No heuristic,
solver, bidding, lead, outlook or passing files changed.

## Evidence

| Command or review | Result | Scope / limitation |
|---|---|---|
| Repository workflow, existing model/term owners and callers | Inspected | Static dependency review only |
| Shared `fresh2000-keep2/summary.json` and four worker NDJSON files | Read existing evidence | No new games or benchmark run |
| `rustfmt --config skip_children=true` on the three owned Rust files | Passed | Formatting/parser check, not type checking |
| `git diff --check` | Passed | Whitespace only |
| Builds, tests, benchmark, independent review | **Not run** | Explicitly delegated to root's serialized bounded validation lane |

Shared evidence: `C:/Users/Johannes/Documents/MarjaAI/tmp/v1main-work/bench/fresh2000-keep2`.
Summary SHA256: `bda9d1d7643750bd3102b6996a8cd20b87c59ae39d528cb1dd1a92d673bb5a33`.
Existing comparison: 2,000 games / 1,000 deals, seed 20261101, teams rotation,
current `af55bed` versus prior live `b3abdaf`. Current mean 3.6493 ms, p95 21.6256 ms,
maximum 337.3167 ms. Open leads consume 108.0250 s and trick plays 61.7051 s of
177.8716 s total current-policy decision time. Prior planner mean 87.6120 ms is
not the optimization baseline.

Read-only aggregation of recorded decisions using the report's physical-seat to
lineup mapping found 42 current decisions above 100 ms: 27 start-trick and 15 trick.
Seventeen were by the team opposing the declarer (the hidden-exchange observers).
Example existing controls: deal 26/game 52/step 21, 299.6094 ms, hidden exchange;
deal 984/game 1969/step 12, 337.3167 ms, visible exchange. These timings do not
attribute all latency to exchange work, and this patch cannot explain all outliers.
No raw deals, player identifiers or generated benchmark data are committed.

Added tests compare bit-identical independent/reused weights and complete query
results (including bounds and work accounting) after play, undo and rights changes;
assert actual term transfer before new weight reads; and exercise invalidation for
both likelihoods, ordered/changed/absent passers and kernel changes. Other-seat
likelihood and card-factor changes must retain sums while matching fresh weights.
A separate Mind must start empty. Existing model-reuse tests cover fact/public
epochs and seat/parameter isolation.

The optional `belief.exchange_sum` timing scope counts actual term computations.
An ignored profile microfixture compares 16 independent model refreshes with/without
term transfer and expects 16 versus 1 actual sum calls. It is a reuse diagnostic,
not a claim about whole-game latency or playing strength.

## Remaining work

Root must run the following serially under the shared two-CPU, below-normal,
4-GiB process-tree bound, with `CARGO_BUILD_JOBS=1` and the retained target cache:

```text
cargo test -p marjapussi-beliefs --lib exchange_reuse -- --test-threads=1
cargo test -p marjapussi-beliefs --lib deals::tests -- --test-threads=1
cargo test -p marjapussi-beliefs --test conditional_concepts -- --test-threads=1
cargo test -p marjapussi-beliefs --features profile --lib exchange_reuse_microfixture -- --ignored --test-threads=1 --nocapture
```

For a minimal whole-policy check, compare base and candidate with the same
parameters and seeds using the existing `perf` executable and distinct output paths:

```text
cargo run --release -p marjapussi-tools --bin perf -- --games 8 --repeat 1 --out <decision-log> --timings-out <timings-json>
```

Require identical action/rule/explanation logs, then measure the shared slow cases
and phase distributions with the unchanged workload. An optional `--features profile`
run with `--profile-out <profile-json>` identifies sum calls; compare final latency
without profiling overhead. Measure retained cache memory as well as time.

No new verified checkpoint or release baseline is established here. Root retains
the accepted live `b3abdaf` and supplied candidate `af55bed` until validation; their
last verified checkpoint tags were not independently requalified in this worktree.
No performance or playing-strength improvement is claimed from static inspection.

## Cleanup

No build/test/benchmark jobs, services, containers, subagents or disposable build
outputs created. No deployment or push. Reclaimed space: **0 bytes**. Shared
datasets and target caches retained for root's planned validation.

# Checkpoint: shared fact-DP construction

- Date: 2026-10-11
- Status: **incomplete; source-only, executable validation pending**
- Tag: `checkpoint/2026-10-11-fact-dp-reuse-source`
- Branch: `review/concept-runtime-efficiency`
- Base: `7767df213cd8dd81933f49b57b52dede080ae334`, executable value-store checkpoint
- Source ownership: beliefs only; no heuristic, solver, engine or knowledge changes

## Result

Exact card marginals previously constructed a temporary `Knowledge::deals()`
while the same Mind's joint model built its own copy. Both now consume one lazy
`FactSpace`, shared privately inside that Mind. `Arc` avoids copying the DP;
`OnceLock` retains the existing `Mind: Send` contract. No global cache, new
dependency, allocation grouping or probability calculation is introduced.

The owner is keyed by the existing own/public fact epochs. These are local
identities, never keys shared across Minds. Signal-only model refreshes retain
the DP but reconstruct weights under the existing complete model key. Card
events and undo invalidate it through fact epochs. Parameters and kinds remain
immutable constructor inputs; replacement Minds have independent owners even
when epoch numbers match. Exact base reads do not initialize weights/signals;
weights-only reads do not construct the DP. Zero-budget proof paths remain lazy.

For a context read through both exact marginals and the joint model, construction
falls structurally from two DPs to one. Counts, holder marginals, sampling order,
uniform prior and joint likelihoods are unchanged. A model from the previous
epoch can retain its old DP until refreshed/dropped, as before; the current
exact-read owner may coexist briefly. There is no unbounded context collection.
Contexts using only exact marginals now retain their DP until invalidation or
Mind eviction instead of immediately dropping it. Runtime and memory benefit
must be measured; no speed claim is made from source inspection.

## Evidence

| Check | Result |
|---|---|
| Direct rustfmt on the five owned Rust files | Passed |
| `git diff --check` | Passed |
| Source inspection of callers, fact epochs, model keys and borrow owners | Completed |
| Compilation, unit/integration tests | Not run: root owns resource-limited validation |
| Microfixture / performance qualification | Not run |

Added tests compare cached results with independently constructed spaces:
counts, holder counts, seeded samples and per-world weights. They cover both
read orders, fresh versus reused history after bids/plays/undo/forks, separate
seat/hand/kind/parameter contexts, and retained no-eager-weight/zero-budget
contracts. Existing tests continue to cover public-fact-only invalidation and
the separate query/model borrow owners.

Root commands, in its existing resource wrapper (two logical CPUs,
below-normal priority, 4 GiB committed-memory cap, `CARGO_BUILD_JOBS=1`, one job
at a time, retained target cache):

```text
cargo test -p marjapussi-beliefs --lib -- --test-threads=1
cargo test -p marjapussi-beliefs --test conditional_concepts -- --test-threads=1
cargo test -p marjapussi-beliefs --test deals -- --test-threads=1
cargo test -p marjapussi-beliefs --features profile --lib deals::tests::fact_dp_reuse_microfixture -- --exact --ignored --nocapture --test-threads=1
```

The last command is a targeted construction microfixture, not a policy smoke or
strength benchmark. It compares 16 fresh contexts with shared versus separately
constructed base/model DPs, using identical counts/marginals/seeded samples.
The existing `belief.deal_space` profile scope reports actual construction calls:
expected 32 unshared versus 16 shared. Elapsed output is a small diagnostic,
not statistically qualified throughput. Profiling remains opt-in; no new counter
framework was added. Correctness fixtures are synthetic, with sample seeds 37/91.

## Remaining work

Root must run the selected checks and measure before marking this optimization
verified. Any timing-dependent completed-work differences remain a planner
qualification concern; this patch does not establish policy/cache parity under
wall-clock interruption. Solver DP reuse and conditioned perspective DP reuse
are intentionally outside this narrow patch. All strategic sequence, receipt,
adaptation and qualification gates from the independent review remain open
unless separately closed by root's integration evidence. No 256-game smoke,
deployment, push or merge was performed.

## Cleanup

No builds, processes or Docker resources were created. Shared caches and evidence
were retained; reclaimed space: 0 bytes. The full review is saved outside Git at
`tmp/2026-10-11-graph-contract/runtime-review.txt` under the outer MarjaAI workspace.

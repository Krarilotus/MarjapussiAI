# Fact-DP reuse validation

Status: **validated narrow cache refactor; strategic migration incomplete**.
Integrated source `8a27334` and legal-fixture repair `83dbae1` (review worktree
originals `04b97f7` and `ee1a021`). Tag:
`checkpoint/2026-10-11-fact-dp-reuse-validation`.

One immutable, lazy exact allocation model is shared by a Mind's exact marginal
and joint-belief reads. Fact changes and undo invalidate it. Signal-only changes
refresh weights and retain identical allocation facts. Exact-only reads do not
construct belief weights; there is no sharing between seats or process-global
cache. The existing Send contract is preserved.

The first validation rejected an illegal test opening. The replacement uses the
engine's legal all-pass and 125-contract histories, passing, accepted undo and a
legal alternative opening; rule enforcement was retained.

- `sequence-fact-dp-unit2`: 19 library tests passed; one manual timing test ignored.
  Checks include exact/current/fresh agreement, identical sampled deals and
  weights, both read orders, undo, changed facts/configuration and privacy.
- `sequence-fact-dp-micro`: the ignored fixture passed separately. Sixteen rounds
  performed 32 constructions without sharing versus 16 with sharing. Observed
  elapsed totals were 125.1 microseconds versus 62.1 microseconds on this small
  fixture. Peak process-tree commitment was 27.2 MiB.
- This proves reduced duplicate work for this access pattern, not a whole-game
  speedup. Exact-only consumers now retain the allocation model until context
  invalidation; real planning memory and latency remain qualification checks.

Both runs used the optimized test profile, one build job, two logical CPUs,
below-normal priority and a 4 GiB process-tree cap. Evidence and reusable caches
remain outside Git. No Docker resources were created; reclaimed space: 0 bytes.
The requested 256-game policy comparison has not started.

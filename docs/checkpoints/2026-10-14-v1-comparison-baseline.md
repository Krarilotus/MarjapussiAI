# Frozen v1 handoff comparison

Status: preparation; strength and runtime qualification pending.

Base: `af55bedcebe982644b937573ee739dada254b9fd`.
Tag: `checkpoint/2026-10-14-v1-comparison-baseline`.

The benchmark now accepts `v1base`, freezing the heuristic, beliefs and solver
from Opus's final v1 handoff. The existing `previous` policy remains the public
live policy at `b3abdaf`. This permits separating improvements over the current
v1 policy from comparisons against the earlier public planner.

Both use the existing verified historical-source adapter and shared-core
compatibility check; no decision code or parameters change. The source hashes,
policy revisions, seed and seat rotation are recorded by the ordinary runner.

Validation: Python source parsing and policy registration inspection. Worker
compilation and paired matches are pending the implementation checkpoints. No
strength claim follows from adding a comparator.

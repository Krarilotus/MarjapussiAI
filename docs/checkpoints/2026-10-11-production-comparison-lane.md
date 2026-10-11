# Production comparison allowance

Status: **incomplete**. Base `df83f27`; intended tag
`checkpoint/2026-10-11-production-comparison-lane`.

The common production adapter no longer inherits the old 256-comparison stop.
The caller's existing deadline, represented transition allowance and finite
authored horizon still apply. Explicit evaluator comparison limits and accounting
remain available. A comparison is charged after its alternatives finish, so the
old stop did not bound the expensive initial expansion. No world, candidate or
horizon is removed, and no solver-node default is introduced.

The independent source review found no unbounded zero-transition path: programs
are acyclic, reconsideration depth decreases, and evaluation checks its deadline
between steps. Individual operations can overrun that soft deadline; transient
allocation is separate from retained-cache accounting.

`comparison-lane-build` passed. On the identical two development games,
`comparison-lane-two` still deferred every planning attempt, with unchanged
actions and action/rule hashes. Removing the earlier stop let preparation phases
use their roughly two-second wall allowance; total elapsed time increased from
12.99 to 20.86 seconds, peak commit 105.3 MiB. This is an explicit scheduling
change, **not a speedup or completion improvement**. The source and measured
artifacts are in shared private `tmp/2026-10-11-comparison-lane`.

Useful completion and suffix-cost optimization remain gates before the paired
smoke. No deployment, benchmark or resource cleanup was performed.

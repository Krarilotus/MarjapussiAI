# Return suit allocations ? 2026-10-09

Base: `1bfd8a5`; tag: `checkpoint/2026-10-09-return-suit-allocations`.
Status: coverage and integration checked; strategic qualification inconclusive.
This is a bounded shortlist improvement, not completion of the shared suit-control
planner or v1 release. See [audit and dependency map](../design/live-planning-audit.md).

## Change

Represent feasible retained suit lengths and each legal returned-Ace entry in
the existing return forecast. Preserve C42 weights, engine rules, sampling,
seat-local continuation and the two-second comparison budget/fallback.

## Validation

- Synthetic allocation coverage fails against the original shortlist, passes
  against this change. The same fixture checks legal candidates and preserves
  C42 when the deadline expires without complete outcomes.
- 8 library, 9 passing and 5 history tests pass (22 total); affected-library
  Clippy passes with warnings denied.
- All 155 selected-player return games replay. 67 return choices changed;
  exact human agreement decreased from 22 to 17, which is not a quality verdict.
  Completed forecasts: 142/155 versus 153/155 in the earlier baseline run.
  New return latency: median 980 ms, p95 2005 ms, maximum 2502 ms on the bounded
  shared Windows host. The budget is soft; calls in progress can overrun it.
- Predeclared fixed comparison: `check`, seed 2, 32 deals, both teams and both
  partner orientations, 128 games, no sequential stopping; all games complete.
  Score difference: 19.69 per duplicate deal pair, approximate 95% interval
  [-3.69, 43.06]. This small development comparison does not
  establish stronger play or noninferiority to good humans.
- In that comparison, completed return forecasts were 60/64 new and 61/64 base;
  median return latency 914 ms versus 523 ms. No new broad resource budget:
  one job, two logical CPUs, below-normal priority, 4 GiB cap; peak run commit
  about 235 MiB. Retained the pinned dependency versions.
- Private receipts: `return-length-negative`, `return-length-checks-fresh`,
  `return-length-human-replay`, `return-length-match-check`.
- A first check command named a nonexistent integration target; it did not run.
  A subsequent cross-worktree shared-target run reused the old unit artifact.
  Refreshing the changed crate's source timestamps forced recompilation and
  produced the passing result above. Do not alternate same-named worktrees in
  one Cargo target; the comparison uses a separately named baseline crate.
- Independent T3 review remains unavailable: task threads accept messages but
  have no model session. The shared agents window was not restarted.

## Remaining

The additional candidates cost time and can increase deadline fallback. They
still rely on a small sample and an imperfect continuation policy. Full suit
promotion reasoning, pass-intent inference, auction continuation, raising and
series-aware strategy remain open. Public multiplayer deployment is separate
and is not complete at this checkpoint. No server was replaced by this change.

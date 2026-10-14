# v1 test-release qualification — 2026-10-14

Status: correctness checks passed; exploratory live trial, strength inconclusive.
Frozen AI candidate: 13d2015192ef04a27b21920ae314bd0853d35278. This checkpoint
adds report coverage and evidence only; it does not change the frozen policy.

Changes since Opus af55bed: reuse compatible hidden-exchange sums across fact
refreshes; align ownership-offer reading/counting; add run-preserving return
candidates and resource-aware question timing; keep simulated plans actor-local;
reuse completed engine-scored continuation concepts within a single decision.
No new fitted threshold or private partner-plan sharing is introduced. The old
fitted B-take, route-take, route-raise and pass-forth rollout remain disabled.

Validation: 52 beliefs tests; full heuristic run 141 passed plus the added
pending-answer cache regression passed with the other three cache tests.
Cached/uncached values and chosen plans agree bitwise on those fixtures.
35 report/runner/build tests passed. Missing forecast flags now display unknown;
HE-1 completion is recorded separately. Reports were regenerated from raw data.

Predeclared paired smoke runs, one worker/two logical CPUs/4 GiB, policy seed 0:

| Comparison | Games / deals | Engine difference per game | Deal-clustered 95% interval | Mean decision ms (candidate / comparator) |
|---|---|---|---|---|
| af55bed, seed 2026101401 | 256 / 128 | -5.92 | [-13.50, 1.67] | 2.94 / 3.14 |
| live b3abdaf, seed 2026101402 | 512 / 256 | +12.40 | [-4.51, 29.32] | 2.91 / 68.46 |

Against live: 274 wins, 237 losses, one draw; contracts made 64.3% versus 53.8%.
Candidate p95 16.48 ms versus 467.72 ms. There were 16 candidate actions above
100 ms, one during raising; the worst play action was 412.20 ms. No hard 100 ms
guarantee. Overall performance versus Opus is not demonstrated improved; cache
microfixtures alone do not establish whole-policy speedup. The 512-game run
compares the whole v1 candidate with live, not just this iteration's changes.
Neither comparison proves statistical superiority or non-inferiority.

All 15 selected recorded-prefix replays from historical deals 576/864/972 retain
the prior actions. They are not fixed. In the 256-game comparison, the largest
losses first diverge at HO1/HO2 choices that delay activation or lose an entry.
Only 22/128 deals differ in actions. This is localization, not causal attribution.
Known limits: short own-activation plans, cheap partner responses, current-contract
return valuation without a later raise, and the existing lexicographic endgame
objective. A broader strength claim needs more untouched deals and those cases.

Private evidence: tmp/2026-10-14-v1-completion/{bench-v1base256,bench-live512,
qualification-review.md,final-replay.log,final-perf.log,report-tests.log}.
No private records or player identifiers are committed. Public API tests and
release safeguards are recorded separately in the paired testbed checkpoint.

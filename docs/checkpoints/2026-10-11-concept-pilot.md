# Conditional concepts: development comparison

Status: **incomplete — correctness checks pass; strength and performance acceptance remain open**.
Base: `160c043`, with test-fixture corrections through `5e9df2e`.
Tag: `checkpoint/2026-10-11-concept-pilot`.

The owner authorized four parallel workers for an early comparison over 64 fresh
deals, each played in four arrangements. The 256-game fixed sample used seed
20261013, current policy `5e9df2e` against frozen `b3abdaf`, default parameters,
and derived per-deal/slot seeds. All games and writers completed successfully.
The worker binary SHA-256 was
`6362ac16565b14b1ae59feaf7e2760201f840df0941c711e5dff4824c57cb12a`.

The current policy's engine score advantage was **9.65 points/game**, with an
approximate deal-clustered 95% interval **[-13.76, 33.06]**. It won 125 games,
lost 130 and drew one. This sample does not establish a playing-strength gain.
It raised in 55 games and made 39 of those contracts; the previous policy raised
in 47 and made 31. Pair points averaged 42.81 versus 47.42. These are observations
on different chosen continuations, not proofs about optimal bids or pair timing.

Decision time averaged **63.67 ms** for current and **60.62 ms** for previous,
including forced actions. The full play phase took 208.44 seconds. Recorded
forecast decision-cache hits were 55,804 out of 1,989,882 requests, **2.80%**.
This counter excludes other caches and measures reuse, not saved total runtime.
There is no demonstrated speed improvement; same-state cost comparisons and
the low reuse rate need investigation before performance acceptance.

The pilot used eight logical CPUs/four physical cores, below-normal priority,
and a 4 GiB job-wide commit cap; peak committed memory was 474.28 MiB. A live
read-back found that Windows' common job affinity prevented narrower per-worker
pinning. All four workers shared mask 255 throughout the pilot. Its settings
were left unchanged. The launcher now permits subset affinity and verifies each
worker's effective mask. A separate Windows smoke with two nested jobs verified
four disjoint masks (3, 12, 48, 192), then reaped all four child processes. This
launcher correction followed the pilot and does not change its recorded evidence.

Focused checks passed for knowledge allocation counts, conditional beliefs and
privacy, heuristic unit/play/passing/history/bidding/raising/contract planning,
evaluation statistics, the Rust worker and 16 Python report/lifecycle tests.
Initial failures were corrected fixtures: tens use the engine's `Z` code; the
forecast reuse test constructs a legal raising position; the isolated retained-
lead rule test disables the full planner like the other rule fixtures. Expected
rule behavior remains asserted. The cache test verifies that repeated completed
outcomes increase neither evaluated calls nor decision-cache hits, and that a
raised contract has its own outcome. Production policy defaults were unchanged.

Evidence is retained outside Git under
`tmp/2026-10-11-parallel-pilot/`: per-check logs/receipts, source/binary manifest,
four raw game files, JSON/CSV statistics and the standalone HTML report. The
source review accepted the fixture corrections and the per-player store boundary.

The earlier 4,000-game b3abdaf-versus-1632af8 process stopped without completion
metadata after 3,259 complete records. Its shutdown cause is unknown. Those
records remain unchanged; 741 missing games require a separately documented
recovery with the original policies, seeds and resource limits. Review the full
recovered evidence and fix justified dependency failures before freezing the
next 4,000-game comparison. The small pilot is development evidence; fresh seed
20261012 remains reserved for that later comparison.

No production deployment or Docker resources changed. Raw evidence and build
caches remain in planned use; no storage reclamation is claimed.

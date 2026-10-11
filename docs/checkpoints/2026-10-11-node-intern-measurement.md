# Local node index: measured and not adopted

Status: **incomplete overall; experiment rejected for adoption**.
Base `7f80e7f88bf721ca5f34ea67b966e6ec5d9e6e68`.
Tag: `checkpoint/2026-10-11-node-intern-measurement`.

A's source experiment `57a34f4f44823e0f6271749b0ea8300af1954ea1`
(`checkpoint/2026-10-11-local-node-intern-source`) replaced the per-suffix linear
node-intern scan with a local fingerprint-to-index-bucket map and full collision
equality. Traversal, canonical ordering and output stayed unchanged. The root
applied the focused patch for measurement, then reversed that exact patch after
the results below. No alternative hash implementation or extra derives remain;
the existing shared-core compatibility anchor is unchanged.

All jobs used the serialized two-CPU, below-normal, 4 GiB runner, optimized test
profile. Private receipt/log folder: `tmp/2026-10-09-v1`.

- `local-node-intern-focused`: **4 passed**, benchmark ignored, 78.85 s including
  compilation, 0.32 s tests, peak 669.0 MiB. SHA256
  `7e1002838d8aaf4085ae057cdf4f954a2e02fd87c93ddbea4ab6e6446424df0d`.
  Compares every fixture stage to the independent old scan; forced constant
  fingerprints, duplicate/reindexed graphs, ordering and invalid data pass.
- `local-node-intern-construction`: benchmark passed, 0.80 s, peak 27.6 MiB.
  SHA256 `849c2612c5518c183f03fb3c07096656b0c6d395f8256402eee37c12de0e4488`.
  Four alternating-order rounds: tiny two-node workload (only 8 calls/round)
  took 0.9–1.1 microseconds linear versus 2.5–4.1 indexed; 346-node resource
  workload took 37.3–40.7 ms versus 35.5–43.7; 349-node composed workload took
  39.1–39.6 ms versus 35.3–39.1. Tiny totals are noisy. The larger cases suggest
  modest gains, insufficient to establish a general hot-path improvement.
- `local-node-intern-profile-build`: passed, 43.38 s, peak 374.0 MiB. SHA256
  `167091e8b0c23641347c567a8013407349fb9b6cab386e0b3c4b41b2e8f518d9`.
- `local-node-intern-profile-two`: 21.65 s, peak 96.6 MiB. SHA256
  `09528ce2f573f756ce02f68efdf9def8b15f4433a384b2d9f3c87ecc74a6f487`.
  Same 105 actions/rules; **all 23 attempts Deferred**. Actual transitions
  111,369 versus 159,555 in the preceding V2 run; represented 163,249 versus
  229,248; completed individual candidates 13 versus 12. Suffix construction
  was 3.334 s / 140,907 calls versus 4.274 s / 162,142. Shared-machine timing and
  different deadline-limited prefixes do not prove a general regression or gain.
  No useful completed-plan or strength improvement was established.

The experiment adds tiny-path overhead and complexity without enough measured
benefit to justify adoption. Its source tag, focused patch, independent reference,
microbenchmark and actual source/profile snapshots remain available in private
`2026-10-11-node-intern-experiment` and `2026-10-11-node-intern-profile`.
No parameter, allowance or equivalence rule was tuned to rescue the experiment.

Current strategic/completion failures remain unchanged. Continue with lazy
frontier preparation and measured response work. No 256-game smoke or deployment
has run. Shared caches and evidence are retained; reclaimed zero bytes.

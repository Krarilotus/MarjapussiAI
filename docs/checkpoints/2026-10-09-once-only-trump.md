# Checkpoint: a suit becomes trump only once

- Status: verified for the rule correction and affected layers; no new strength claim.
- Tag: `checkpoint/2026-10-09-once-only-trump`.
- Branch: `fix/once-only-trump`; previous checkpoint `checkpoint/2026-10-09-portable-v1` (`54d1b51`).
- Implementation: `686de70` plus the exporter correction and legal regression fixture in this checkpoint.
- Paired testbed: `checkpoint/2026-10-09-configurable-series` (base `48481d0`).

The owner confirmed that only an unannounced suit may become trump. Repeating a half
question remains legal in the engine, but cannot restore or rescore an old trump.
Game, FastState, knowledge, solver planning and exported records now agree on this.
HP1 excludes already-called half questions and known unproductive repeats while
retaining genuinely unknown questions. A positive half answer without the asker's
half conveys information only. Rights remain per player.

## Evidence

- 75 affected integration checks pass: engine/FastState legal traces on both team
  parities; observation privacy; knowledge rules and random-game soundness;
  heuristic question filtering; solver exact/world/trick cases; record serialization.
- Solver results equal independent engine minimax and brute force in the existing
  randomized suites, including questions, answers and changing public rights.
- Clippy passes with warnings denied for affected engine, knowledge, heuristic,
  solver and tools libraries/tests. Formatting follows the existing 120-column style.
- GPT-6.1-Sol source review found the stale `StillTrump` exporter field; fixed by
  emitting `trump` only for `NewTrump`. Review was source-only; orchestrator ran checks.
- Initial new-test failure was an illegal fixture: its helper discarded the very half
  later asserted present. The corrected fixture retains that half using lower cards.
- Bounded Windows verification: one job, two logical CPUs, 4 GiB process-tree cap.
  Main 55-check run: 257.14 s, peak committed memory 281.2 MiB. Layer checks add 20.

Local evidence: `tmp/2026-10-09-v1/once-only-trump-{ai-tests3,layer-tests,clippy2}`
logs and SHA-256 receipts. These are validation timings, not live decision latency.

## Upstream engine

The same two-file correction is pushed to the three existing fork branches behind
SamuelLess PRs [3](https://github.com/SamuelLess/marjapussi-rs/pull/3),
[4](https://github.com/SamuelLess/marjapussi-rs/pull/4), and
[5](https://github.com/SamuelLess/marjapussi-rs/pull/5): `69e7c3e`, `d609191`, `8bb3193`.
A shared legal trace fails on upstream `3a1fbe4` and passes with all three changes,
including both default and lean builds of PR5 (20 checks). No AI implementation or
extra test framework was added to the external engine. Earlier benchmark numbers
predate this intentional rule correction and are not presented as fresh timings.

## Limits and cleanup

No Ace-preservation policy change, bidding convention change, series strategy or
strength improvement is claimed. Current HP1 ignores `SeriesScore`; the design
record now says so. The existing 50 ms search target remains soft.
Private live-game evidence stays outside Git. Shared build caches and regression
fixtures are retained for the next public-server iteration; no disposable Docker
resources were created for this checkpoint.

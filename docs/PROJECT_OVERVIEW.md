# Project overview

Reviewed 2026-10-08. **A substantial heuristic-AI prototype exists. Its foundations
are sound enough to continue; its strategy, evaluation, and release workflow still
need consolidation.** No new playing-strength claim is established by this review.

## Versions and locations

The real workspace is `Documents/MarjaAI`; the T3 project at
`.t3/projects/marjapussi-ai` is only a README/icon scaffold.

| Component | Reviewed revision | Meaning |
|---|---|---|
| AI `main` | `9563626` | Engine, observation, knowledge; current published overview documents. |
| AI `v1-preview` | `1c077ae` | Published agent snapshot, including beliefs, heuristic, solver, and evaluation. |
| AI `kb/batch1` | `4782c65` | Newer local bidding/passing work; the base of this continuation branch. |
| Testbed `main` | `0665818` | Separate Rust API and Next.js play/debug UI. Reviewed statically. |

The table records the original review inputs. Current playable v1 continues on
`integration/reviewed-ai`, paired with testbed `feature/playable-v1`. Both repositories
pin the portable setup with `checkpoint/2026-10-09-portable-v1`; `v1-preview` remains
the older snapshot. Original checkpoints remain immutable. See the
[return-planning record](checkpoints/2026-10-09-return-planning.md) for current evidence
and the [publication record](checkpoints/2026-10-08-publication.md) for ancestry.

The [testbed](https://github.com/Krarilotus/MarjapussiAI-testbed/tree/feature/playable-v1#run-it) documents
the sibling checkout setup and tested revision pair. All nine AI crates resolve
inside this repository; its engine is included source derived from
[marjapussi-rs](https://github.com/Krarilotus/marjapussi-rs), not an external Cargo
dependency. `NOTICE` records its provenance.

## Implementation map

| Crate | Responsibility | Assessment |
|---|---|---|
| `engine` | Legal actions, scoring, complete simulation, compact card/play representation | Useful single rule owner; differential and replay tests provide meaningful safeguards. |
| `observation` | Own hand and seat-filtered public/private events | Clear fairness boundary; hidden exchanges are redacted. |
| `knowledge` | Certain/possible cards, constraints, exact deal counts and samples | Appropriate separation of deductions from conventions; soundness and hidden-card-swap tests exist. |
| `agent` | `SeatView`, shared `Table` simulation, actions, explanations, seat metadata | One owner for filtered observations; policies receive only their own view. |
| `beliefs` | Uncertain card/hand features, bidding evidence, exchange bookkeeping, lazy caches | Useful fact-constrained model; several joint probabilities remain approximations. Pass-choice intent is not currently inferred. |
| `heuristic` | Bidding, exchange, entries, trump/lead decisions, return forecasts | Bounded continuations reuse existing concepts and engine transitions; raising remains disabled. |
| `solver` | Known-deal play search and aggregation over possible deals | Sensible bounded aid. Exact solving within a deal is distinct from optimal imperfect-information play. |
| `eval` | Duplicate deals, statistics, detectors; re-exports shared Table | Fixed-size comparisons; human agreement stays separate from playing strength. |
| `tools` | Arena, recorded-game studies, profiling and record adapters | Useful tooling; experimental scripts and provenance are spread across the outer workspace. |

The separate testbed owns HTTP/SQLite and the UI. It reuses the engine and agent
contracts rather than owning another copy of the game rules.

## What makes sense

- Keeping legality and hard deductions exact, while treating conventions as fallible evidence.
- A transparent heuristic agent before whole-game search or self-play learning.
- Compact state for repeated simulation, with differential tests against the fuller engine.
- Duplicate deals, deterministic seeds, hidden-card swaps, and decision explanations.
- A local play/debug interface for reviewing concrete disagreements with the owner.

Keep these boundaries. A rewrite or a generic runtime concept-graph framework is
not justified by this review. The concept catalogue can continue to describe typed
Rust helpers; a diagram is not evidence that a complete planner already exists.

## Current limits

- C42 selects the first pass and orders return candidates. Bounded return forecasts compare their completed outcomes, falling back to C42 when unfinished. The unused HP/HK reader and scorer have been removed; the active beliefs never called that reader. Inferring intent from chosen passes remains separate capability work. See [passing models](PASSING.md).
- Raising is deliberately disabled. Return planning is a policy forecast over a shortlist and sampled deals, not a full-game optimal solver.
- Fitted human-choice weights measure imitation, not necessarily good play. [Provenance](EVALUATION.md) records their in-sample limitations; no new weights were fitted.
- The local [question-aware endgame checkpoint](checkpoints/2026-10-09-solver-questions.md) keeps the four-card/64-world default, includes questions and answers, and adds a 45 ms search deadline. Independent source review found no blocking defect; hidden-information values remain estimates.
- The local [entry checkpoint](checkpoints/2026-10-09-entry-conditions.md) checks winning ruffs, the actual next leader and surviving announceable pairs against human replays. Bounded return rollouts now compose these transitions. The [return-planning checkpoint](checkpoints/2026-10-09-return-planning.md) records a completed 512-game comparison with no clear strength difference; live acceptance follows.
- Published continuation preserves the preview documentation, assets and private-data safeguards.
- Core-only `main` still lacks agent dependencies; use the paired setup documented in the testbed.

See the [review](REVIEW-2026-10-08.md) for concrete defects and the
[plan](NEXT_STEPS.md) for the order of work.

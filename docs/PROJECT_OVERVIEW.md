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

Continuation branch: `planning/ai-review-next-steps`.
Worktree: `Documents/MarjaAI/wt/ai-review-next-steps`.
Existing AI/testbed checkouts and the outer workspace's uncommitted work are preserved.
Remote branch references were inspected locally; they were not fetched or pushed.

## Implementation map

| Crate | Responsibility | Assessment |
|---|---|---|
| `engine` | Legal actions, scoring, complete simulation, compact card/play representation | Useful single rule owner; differential and replay tests provide meaningful safeguards. |
| `observation` | Own hand and seat-filtered public/private events | Clear fairness boundary; hidden exchanges are redacted. |
| `knowledge` | Certain/possible cards, constraints, exact deal counts and samples | Appropriate separation of deductions from conventions; soundness and hidden-card-swap tests exist. |
| `agent` | `SeatView`, actions, explanations, seat metadata | Small shared contract; does not supply the full game state to the policy. |
| `beliefs` | Uncertain card/hand features, bidding evidence, exchange bookkeeping, lazy caches | Useful fact-constrained model; several joint probabilities remain approximations. Pass-choice intent is not currently inferred. |
| `heuristic` | Bidding, exchange, trump/lead decisions, card valuation | Explainable prototype with local owner-informed corrections and empirically fitted choice weights. No complete coordinated game planner. |
| `solver` | Perfect-information endgame solving and aggregation over possible deals | Sensible bounded aid. Exact solving within a deal is distinct from optimal imperfect-information play. |
| `eval` | Table runner, duplicate deals, statistics, detectors | Good reusable evaluation boundary; statistical claims need tightening. |
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

- Pass choice uses the C42 feature score. The unused HP/HK reader and scorer have been removed; the active beliefs never called that reader. Inferring intent from chosen passes remains separate capability work. See [passing models](PASSING.md).
- Raising is deliberately disabled; coordinated entries/trump/contract planning remains incomplete.
- Fitted human-choice weights measure imitation, not necessarily good play. Their inspected fit scripts report in-sample agreement.
- The endgame uses at most 64 worlds by default with a four-card trigger; questions/activation can keep decisions on the heuristic path.
- Two behaviour-related tests are ignored, alongside deliberate performance tests.
- This local branch predates preview documentation and private-data safeguards. Reconcile those selectively before publishing.
- The existing testbed's sibling AI is on core-only `main`, so that pairing currently cannot resolve its agent dependencies.

See the [review](REVIEW-2026-10-08.md) for concrete defects and the
[plan](NEXT_STEPS.md) for the order of work.

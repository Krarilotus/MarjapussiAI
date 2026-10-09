# Bounded return forecasts

Continuation authorised 2026-10-09: finish the coordinated v1 slice, then live-test.

The existing play policy supplies the continuation. Compare legal returns on the
same sampled deals and play each to completion through the engine. Each simulated
seat receives only its own observation stream. Never optimise future actions by
looking at the sampled opponents' hands. Engine outcomes count distinct tricks,
actual pair activations, changed trumps and the last-trick bonus exactly once.
The forecast is conditional on these policies and approximate belief weights;
it is not a calibrated chance of optimal play.

Reuse the existing table runner by moving its observation/seat-view machinery
from `eval` to `agent`. Evaluation continues to re-export it. This avoids either
copying privacy logic or making production decisions depend on evaluation.
That relocation is a separate behaviour-preserving checkpoint.

Reconstruct a hypothetical original deal by reversing the observed first pass
from a fact-consistent pre-return deal. Replay the public auction and visible
exchange through that table. All candidates share that history and the partner's
same five unknown cards. Only the proposed four-card return changes.

Cheap C42 features remain a candidate ordering, with unchanged feature meanings.
The bounded shortlist includes the current best return and distinct retained,
transferred and split-pair allocations. Complete the same worlds for every
candidate before comparing them. Score using actual engine contract outcomes;
retain the existing choice when the comparison cannot finish within its budget.
No fitted coefficients are repurposed as score probabilities.

Live continuation reuses the same existing entry, announcement and play owners;
it observes actual events and recomputes from them. No hidden shared team plan,
fixed nine-trick script or independent graph/cache is introduced. Raising stays
disabled. Exact rollout budgets and qualification evidence must be recorded
before enabling the return choice; budget exhaustion must be visible.

Acceptance: legal replay, correct nine-card allocations, own-view privacy,
surviving/consumed halves, forced ace/green opening, entries interrupted by
opponents, trump changes, last-trick scoring, fresh/reused agreement. Use the
already reviewed human examples as development evidence, never a holdout. Freeze
parameters before the fixed-size duplicate qualification; report agreement and
playing strength separately. Measure return latency separately from the 50 ms
small-endgame target. No v1 completion claim without those results.

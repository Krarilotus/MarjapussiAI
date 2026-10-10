# Next steps

Goal: a fair, understandable agent that is enjoyable to play with, developed in
small reviewable iterations. Continue from `fix/live-planning-review`. The portable-v1 tag
remains an older reproducible baseline. Every iteration follows the
[checkpoint workflow](WORKFLOW.md).
The order below is proposed engineering work, not approval of new strategic rules.

Owner priority, 2026-10-08: condense the pass implementation first. Remove deprecated
and dead code rather than retaining dormant alternatives. The first checkpoint
preserves play; it does not introduce pass-choice inference.

| Order | Iteration | Done when | Checkpoint suffix |
|---|---|---|---|
| 0, completed | Condense pass choice and remove the unused reader (corrected F1) | One active C42 scorer; explicit giver inputs; obsolete code/config/tests removed; original choices and explanations preserved. See [passing models](PASSING.md). | `exchange-cleanup` |
| 1a, completed | Fix history and belief-parameter reuse (F3) | Complete raw-prefix and belief-parameter comparison; fresh/reused agents agree on forks, extensions, rewinds, undo, seat/hand/kind and parameter changes. | `history-cache` |
| 1b, completed | Validate configuration values (F4 input domains) | Nonfinite/out-of-range values and invalid joint settings rejected by the loader; direct Rust callers have explicit validation. Signed coefficients and deliberate zeros preserved. Runtime budgets remain in F7. | `config-validation` |
| 1c, implemented and reviewed | Complete the small-endgame rules and bound search | Questions, truthful answers, full per-player rights and trump changes are searched; interruption preserves only completed comparisons and cache bounds. Correctness/oracle checks and independent source review pass. Timing is a soft target on the shared machine; see the return-planning checkpoint. | [solver-questions](checkpoints/2026-10-09-solver-questions.md) |
| 1d, implemented and reviewed | Ground immediate entries and pair activation in human replays | Joint legal trick continuations distinguish team wins, seat entries and surviving announceable pairs. Safe own runs bound announcement delays; impossible activation no longer protects halves. | [entry-conditions](checkpoints/2026-10-09-entry-conditions.md) |
| 2, provenance recorded | Qualify evaluation and weight provenance (F2/F5) | Record each fit's source, data fingerprint, feature order and command. Split by complete game/session before fitting. Freeze weights before held-out evaluation. Use predeclared fixed-size duplicate comparisons and label approximate intervals; defer unsupported anytime-valid claims. | `evaluation` |
| 3 | Add pass-choice inference (remaining F1 capability) | Define the modelled actor and pre-pass information; reuse the current scorer from hypothetical giver views; validate synthetic same-policy likelihoods/void predictions and privacy before testing held-out human behaviour. Bound runtime and measure calibration on separate games. | `exchange-reading` |
| 4, partially completed | Consolidate a runnable baseline (F6) | Preview docs/data safeguards reconciled and sibling AI/testbed pair verified in the publication checkpoint. Public-fixture CI and supported toolchain policy remain. | `publication`, then `baseline-ci` |
| 5, bounded slice implemented | Compose continuation planning | Return selection, contract decisions and open leads share joint weighted deals and legal full-game continuations. Guaranteed raises reuse claim proofs; forecasted raises replay the proposed contract. Fixed 64-game qualification is inconclusive on strength. | [joint-continuations](checkpoints/2026-10-10-joint-continuations.md) |
| 6, verified for local live use | Make play/replay responsive (F7) | Bounded background work, revision-checked commits and cached agents pass concurrency/privacy checks and the paired browser/API smoke. Testbed `3d1aba4` runs AI `febba17`. | `playable-v1` |

The [return-pass planning proposal](design/return-pass-plan.md) breaks the
coordinated planning slice into six checkpoints with domain scenarios. Begin
with candidate allocations and legal activation routes; review the comparisons
before changing selection. Pass-choice inference is independent of that first
slice and need not delay it.

Evaluation preparation and checkout pairing can proceed independently. Each major
implementation/refactor still receives its own checkpoint. The validation suite
should grow from concrete failure modes, not tests that restate implementation.

## Shared evidence for all iterations

- Preserve a compact synthetic scenario set for auction, exchange, pair activation,
  legal card play, hidden-card swaps, and the two reproduced state/configuration bugs.
- Reuse `eval::DealSuite`: `dev` seed 1, `check` seed 2; reserve `holdout` seed 3
  for a declared qualification run. Pin deal count, rotations, seed-generation
  version, agent parameters, AI commit, and dataset digest in the run record.
- Do not repeatedly tune on a consumed holdout. Version a new holdout when needed.
- Private human records remain outside Git. Store only non-identifying aggregates
  and fingerprints in checkpoints; split by whole game/session before fitting.
- Compare heuristic-only, facts-only beliefs, full beliefs, and endgame-enabled
  variants where relevant. Use equal workloads and report failures/fallbacks.
- Report score and uncertainty, contract/all-pass/schwarz patterns, owner-reviewed
  mistakes, and decision latency. Human agreement is a diagnostic, not the release criterion.

## Keep later work separate

Midgame information-set search, neural policies, PPO/GRPO, difficulty styles, and
upstream-site integration remain later work. The separate public testbed is live.
Choose an algorithm from measured benefit and constraints; this review does not establish GRPO as preferable.
No GPU training or new runtime graph framework is needed for the next iterations.

## Next implementation task

The paired live Debug smoke passes; use concrete owner-played mistakes to
choose the next small iteration. The [bounded return checkpoint](checkpoints/2026-10-09-return-planning.md)
records the fixed comparison, human development sample, deadline limitations and
resource trim. Reuse its evidence; do not rerun broad exhaustive suites without a
changed dependency or new failure.

First diagnose changed returns on identical deals: distinguish a missing candidate,
an inaccurate forecast and poor continuation play before adding rules or search.
The current comparison is inconclusive, not evidence that the planner has no effect.
Next candidates: improve observed return/entry mistakes, calibrate forecast shares
on genuinely separate games, and investigate broader information-set planning.
Before exposing arbitrary transposition-table sizes, handle the existing zero-bit
TT edge. GPU work remains a separate decision. Every major iteration
gets its own checkpoint; no speculative second planner or duplicated rule logic.

## Live feedback: next dependency work

The [live audit](design/live-planning-audit.md) separates human agreement from
playing quality and maps the dependencies for developing suit control. The
[allocation checkpoint](checkpoints/2026-10-09-return-suit-allocations.md) broadens
return candidates. The [joint-continuation checkpoint](checkpoints/2026-10-10-joint-continuations.md)
shares promotion/entry/trump continuations with bidding, raising and play, using
the joint auction likelihood rather than multiplying card marginals.

Next: model hidden exchange histories and pass-choice intent, test affordable
high-bid interpretations, and improve continuation policy on recorded failure
branches. The approved human examples still expose cautious bids; a completed
forecast is evidence about this policy model, not proof that passing is best.
Calibrate on separate games before calling forecast shares reliable probabilities.
Series-aware selection around 500 points remains a separate objective above the
single-game score comparison.

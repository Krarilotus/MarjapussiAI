# Next steps

Goal: a fair, understandable agent that is enjoyable to play with, developed in
small reviewable iterations. Start from `planning/ai-review-next-steps`, based on
local AI `4782c65`. Every iteration follows the [checkpoint workflow](WORKFLOW.md).
The order below is proposed engineering work, not approval of new strategic rules.

Owner priority, 2026-10-08: condense the pass implementation first. Remove deprecated
and dead code rather than retaining dormant alternatives. The first checkpoint
preserves play; it does not introduce pass-choice inference.

| Order | Iteration | Done when | Checkpoint suffix |
|---|---|---|---|
| 0, completed | Condense pass choice and remove the unused reader (corrected F1) | One active C42 scorer; explicit giver inputs; obsolete code/config/tests removed; original choices and explanations preserved. See [passing models](PASSING.md). | `exchange-cleanup` |
| 1 | Fix history and configuration contracts (F3/F4) | Validate the complete event prefix before incremental reuse (or add an explicit history identity/extension contract). Include belief parameters in invalidation. Fresh/reused agents agree on extension, rewind, equal-length branches, new games and parameter changes. Validate finite/ranged values at configuration boundaries. | `state-config` |
| 2 | Qualify evaluation and weight provenance (F2/F5) | Record each fit's source, data fingerprint, feature order and command. Split by complete game/session before fitting. Freeze weights before held-out evaluation. Use predeclared fixed-size duplicate comparisons and label approximate intervals; defer unsupported anytime-valid claims. | `evaluation` |
| 3 | Add pass-choice inference (remaining F1 capability) | Define the modelled actor and pre-pass information; reuse the current scorer from hypothetical giver views; validate synthetic same-policy likelihoods/void predictions and privacy before testing held-out human behaviour. Bound runtime and disposition the ignored historical calibration check. | `exchange-reading` |
| 4 | Consolidate a runnable baseline (F6) | Selectively reconcile preview docs/data safeguards; pin AI/testbed revisions in a dedicated sibling setup. Add public-fixture CI and a tested Rust version; report required checks, skipped lanes and deferred checks separately. | `baseline` |
| 5 | Add one coordinated planning slice | Compose existing exchange, entry, trump activation and first-lead concepts in named owner scenarios. Then separately review contract estimates/raising. Avoid new abstractions without a concrete consumer. | `plan-entries`, then `plan-raising` |
| 6 | Make play/replay responsive (F7) | Snapshot state under the store lock, compute on a bounded worker, then revision-check before saving under a short lock. Test concurrent/stale-step handling. Reuse agent state only after F3 is fixed. | `playable-v1` |

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
site integration follow the baseline/planning review. Choose an algorithm from
measured benefit and constraints; this review does not establish GRPO as preferable.
No GPU training or new runtime graph framework is needed for the next iterations.

## Next implementation task

Promote the existing F3/F4 probes into normal regression tests and fix their
owning contracts. Start with complete-prefix comparison and parameter comparison
unless profiling justifies adding a new identity/revision API. Validate parameters
once at construction/loading, preserving deliberate zeros such as disabling the
endgame. Use the frozen exchange trace plus targeted reuse/configuration scenarios
to distinguish fixes from accidental policy changes.

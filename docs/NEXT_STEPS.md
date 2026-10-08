# Next steps

Goal: a fair, understandable agent that is enjoyable to play with, developed in
small reviewable iterations. Start from `planning/ai-review-next-steps`, based on
local AI `4782c65`. Every iteration follows the [checkpoint workflow](WORKFLOW.md).
The order below is proposed engineering work, not approval of new strategic rules.

| Order | Iteration | Done when | Checkpoint suffix |
|---|---|---|---|
| 0 | Consolidate the baseline | Published docs/data safeguards are reconciled with local policy work; the agent/testbed revisions are recorded in a runnable sibling setup; a required check command distinguishes passed, skipped, and deferred checks. Add CI for public fixtures and a tested Rust version. | `baseline` |
| 1 | Fix history and configuration contracts (F3/F4) | Fresh and reused agents agree across extension, rewind, same-length branch, new game, and parameter change. Invalid/nonfinite configuration fails with typed errors. Default decisions remain unchanged on frozen fixtures. | `state-config` |
| 2 | Align exchange policy and belief reading (F1) | One documented model/version contract covers acting and reading. Replay remains seat-limited; pass/void predictions and independent owner examples are reviewed. Resolve the ignored calibration case with an explicit outcome. | `exchange-model` |
| 3 | Qualify evaluation and weight provenance (F2/F5) | Each empirical weight has source revision, data fingerprint, feature order, fit command and split. Fit/check games are disjoint. Fixed-sample duplicate comparisons report score, uncertainty and strategic failure counts; sequential claims are corrected or deferred. | `evaluation` |
| 4 | Add one coordinated planning slice | Start with exchange → entry → trump activation → first lead. Demonstrate named owner scenarios and counterexamples with visible inputs and alternatives. Then separately add contract estimate/raising; do not silently enable all planned behaviour at once. | `plan-entries`, then `plan-raising` |
| 5 | Make play/replay responsive and complete the v1 review | Bound per-decision work, shorten store locks, verify stale-step handling, and test play/debug/replay with the exact checkpointed agent. Review representative games with the owner and disposition remaining strategic disagreements. | `playable-v1` |

Evaluation design can be prepared alongside iteration 1; use it when reviewing
iteration 2. The validation suite should grow from concrete failure modes, not
from tests that restate the implementation.

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

## First implementation task

Make F3/F4 regression probes into normal tests, fix their owning contracts, and
freeze before/after decision fixtures. Record any observable behaviour differences.
Complete the `state-config` checkpoint before starting a separate behaviour change.
Baseline documentation/data-safety work can precede that fix without changing play.

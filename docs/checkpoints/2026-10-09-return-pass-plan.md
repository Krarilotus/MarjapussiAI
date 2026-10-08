# Checkpoint: return-pass planning proposal

- Date: 2026-10-09 (Europe/Berlin)
- Status: review-only; no new strategy implemented
- Tag: `checkpoint/2026-10-09-return-pass-plan`
- Branch: `integration/reviewed-ai`
- Base AI: `3a1c14ce0fb10d82ef7bb92a23959886f151e133`, tagged
  `checkpoint/2026-10-08-config-validation`.
- Paired testbed: `353c786aad5ee64d683fe489ff6bdcc10d1fcbd2`, also tagged
  `checkpoint/2026-10-08-config-validation` in that repository.

## Result

The [proposal](../design/return-pass-plan.md) explains the current limitation:
`back_plan` chooses a suit before assessing each return's resulting allocation.
It sequences six iterations: allocation, activation, entries, joint choice,
follow-through/replanning and v1 qualification. Each names existing owners and
concrete scenario gates. The first stages preserve choices; strategic selection
and raising need their own reviewed cases.

No alternative pass scorer, belief framework, weights or runtime plan store were
added. Known card transfer and uncertain partner intent remain distinct. The
engine remains the owner of legality, rights and scoring. In particular, opening
lead restrictions and the absence of a pre-first-trick announcement constrain
plans; there is no invented increasing-trump-value rule.

## Evidence and review

- Checked `passing::decide`/features, `trump_plan::Reach`, lead helpers, knowledge
  transfer, and engine legal actions, play levels and rights transitions.
- Configuration implementation received an independent read-only T3 approval at
  AI `3a1c14c` and testbed `353c786`. Both annotated tags resolve to those hashes.
  Root ran the checks; the reviewer inspected code/evidence, not a second test run.
- The configuration checkpoint records 99 passing AI checks and 17 server tests,
  one inherited ignored heuristic scenario, clippy, the frozen 128-exchange trace
  and 500 hidden-card swaps. This documentation iteration does not repeat builds.
- Independent read-only T3 proposal review found no material blocker. Incorporated
  its clarifications: distinguish hand-only standing from current ruff-safe facts;
  enable changed return selection together with executable follow-through; cover
  restoring an already-called trump without scoring the pair again.
- Local links resolve in all six documents changed across these iterations;
  staged diff hygiene passes.

## Remaining work

Owner scenario review and measured acceptance thresholds precede strategy
changes. Evaluation provenance and server responsiveness remain v1 work. This is
a plan, not evidence of increased strength or a completed v1. These new
continuation commits/tags are local; the prior published revision pair remains
unchanged.

## Cleanup

No new containers or servers. Reusable build caches and the shared synthetic
fixture are retained. Removed the clean owned detached worktree
`tmp/2026-10-08-v1-planning-review` after review: **1,398,677 file bytes reclaimed**
(about 1.4 MB). All owned test/build jobs have finished; no cache deletion needed.

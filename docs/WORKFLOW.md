# Iterations and checkpoints

Owner requirement, 2026-10-08: keep checkpoints after every major implementation
iteration over new features or code refactors. This applies to all future work
on this branch and its descendants, through `AGENTS.md` and the always-on editor rule.

## One iteration

1. Choose one feature/refactor and its acceptance criteria from `NEXT_STEPS.md`.
   Record the starting checkpoint/commit. Use an isolated branch/worktree for a
   parallel or risky experiment; do not mix unrelated work into the iteration.
2. Capture the baseline fixtures and relevant checks before changing behaviour.
   Reuse the same verified deals between implementer and reviewer. Record IDs,
   hashes, parameters and commands, not private raw games.
3. Implement, inspect the diff, and run the relevant correctness/privacy checks.
   Refactors compare decisions before/after; hot-path changes need an appropriate
   benchmark on a sufficiently quiet machine. Behaviour changes include scenario
   explanations and owner review of the proposed strategy.
4. Copy `docs/checkpoints/TEMPLATE.md` to a dated iteration record. State what
   passed, failed, was skipped, or remains uncertain. Save independent review
   findings and unresolved items. Classify it as `verified`, `incomplete`, or
   `review-only`; a green build alone is not strategic acceptance.
5. Stage only iteration-owned files, inspect the staged diff, and commit the
   implementation and its record together. An incomplete attempt can have a WIP
   commit and an explicitly incomplete checkpoint; it cannot replace the last
   verified baseline.
6. Create an **annotated, immutable tag** `checkpoint/YYYY-MM-DD-<iteration>` on
   that commit, then verify it and record the tag/commit in the handoff. Use an
   additional suffix for another checkpoint that day; never move an existing tag.
7. Stop owned jobs and remove disposable build/scratch outputs after final use.
   Retain planned reusable caches, fixtures and evidence. Report cleanup.

Do not advance to the next major iteration without its predecessor's checkpoint.
For a cross-repository iteration, create a record/tag in each changed repository
and record the exact AI/testbed revision pair. Keep the AI and testbed histories
separate. Checkpoint creation does not authorise a push, deployment or merge.

## Commands (Git Bash)

From the implementation worktree, after checks and the evidence record are ready:

```sh
git status --short
git add <explicit-iteration-files> docs/checkpoints/<dated-iteration>.md
git diff --cached --check
git diff --cached
git commit -m "<feature-or-refactor>: <result>"
git tag -a checkpoint/YYYY-MM-DD-<iteration> -m "<status>; evidence: docs/checkpoints/<dated-iteration>.md"
git show --no-patch checkpoint/YYYY-MM-DD-<iteration>
git status --short
```

A checkpoint record cannot contain the hash of its own future commit. Record its
base hash and intended tag; the annotated tag resolves to the resulting commit.
Record that resolved hash in the chat handoff or the following iteration.

To inspect or continue an older checkpoint without discarding current work:

```sh
git tag --list 'checkpoint/*'
git worktree add -b feature/<next-iteration> <new-worktree-path> checkpoint/YYYY-MM-DD-<iteration>
```

Use a new corrective commit or branch to recover. Do not rewrite checkpointed
history with a hard reset, forced tag update, or force push.

## Validation lanes

- Routine: formatting, relevant unit/integration tests, lint, diff check.
- AI behaviour: independent scenarios, fairness guards, fixed reproducible deals,
  owner review, and properly labelled evaluation evidence.
- Release: complete required suite, explicit private-data lane if available,
  paired testbed build/typecheck/component tests, privacy/history review.

Bound builds (`CARGO_BUILD_JOBS=2`, own `CARGO_TARGET_DIR`) and tests
(`--test-threads=2`). A local annotated tag is a durable Git reference, **not an
off-machine backup**. Remote backup/publication is a separate authorised step.

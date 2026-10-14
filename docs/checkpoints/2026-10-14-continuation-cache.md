# Checkpoint: decision-local continuation result cache

- Date: 2026-10-14
- Status: **incomplete - validation pending**
- Intended tag: `checkpoint/2026-10-14-continuation-cache`
- Branch: `work/2026-10-14-v1-auction-reading`
- Base: `a136b4a24d4dc65fb1b5f53e787b82edd9e57b90`
- Integration target: released control `845551dafdb1ed253106632e4823f5179a9bfc2f`, integrated by root as `9903444`.
- No builds, tests, benchmarks, deployment, pushes or subagents started here.

## Scope and handoff

This commit owns only `crates/heuristic/src/concepts.rs` and this record. The
separate private patch adds the outlook integration and focused tests after root
applies this commit. It does not include or replace control's released changes.
The prototype and semantic identity were described in the thread before edits.

Patch: `C:/Users/Johannes/Documents/MarjaAI/tmp/v1main-work/continuation-cache/outlook-integration.patch`

SHA-256: `24EE2D8CABB61A81B0B0882D24581B579E112D770828F7F6E00A540177304A55`

Patch files: `crates/heuristic/src/outlook.rs` and
`crates/heuristic/tests/internal/continuation_cache.rs`.

## Exact dependencies and identity

- `concepts::Reads` owns a typed map from `ContinuationQuery` to
  `TerminalScoreDifference`. Existing joint holding queries and their traversal
  allowance remain separate and unchanged. No global or persistent cache exists.
- `ContinuationQuery` includes all four exact hands, phase, concrete engine move,
  selecting actor, both authored activation steps and the plan cursor. Full key
  equality resolves hash collisions. World weight is excluded because it changes
  integration, not the conditional terminal score; weights are still integrated
  by the unchanged `WorldValues` calculation.
- A fresh decision Ctx fixes seat information, own hand, transfers, public state,
  trump, rights, contract and model parameters. Its single lazy Routes pool is
  immutable for these queries. Reads and Routes must not be reused across Ctx or
  epochs. Exact hands provide stable world identity within that scope without
  pointer identity or a world-index mapping.
- The private patch moves the released actor-local Plan data into concepts;
  Plan enumeration, actor ownership, cursor consumption and policy stay in
  outlook. The cache neither shares private plans with partners nor changes
  actor-local knowledge construction.
- `outlook::route_values` asks the named typed continuation query shared by
  HO1/HO2. On a miss it reconstructs the original state and knowledge, applies
  the concrete move, completes the authored-plan playout and inserts its score.
  There is no evaluator callback or generic memo interface.
- `TerminalScoreDifference::from_terminal` rejects nonterminal states and uses
  the existing `engine::game_scores` calculation, including contract and schwarz.
  The cached integer becomes the same f64 cell as before. Aggregation, option
  order, plan selection, uncertainty guard and `expected_score_difference`
  output labels remain unchanged. The expectation is an estimate, not proof.

## Phase equivalence and counterexamples

For a legal Card action only, the integration normalizes Start to Cards after
validating the original state and action. Engine `legal_cards` and `to_move` agree
in these two phases; `FastState::play` consumes the same card, sets Cards and
does not alter rights. `Known::play` does not read phase. Both successors and
their actor knowledge therefore agree before continuation. An invalid Start
state cannot bypass validation through a Cards hit. Announcements, questions
and answers keep their phase in the key.

The same announcement in Cards must not reuse its Start score. Changing world,
move, actor, authored steps or cursor must miss. A fresh Ctx must start empty.
Incomplete or invalid calculations cannot insert a typed terminal result.

Deadline checks remain in `WorldValues::over`: before starting a batch and at
the existing world boundaries after MIN_WORLDS. No per-cell cutoff, tightened
deadline or changed world count is introduced. Expired batches are still skipped,
even when warm. Faster completion may admit a longer prefix under a real deadline;
bit parity is asserted for the same admitted pool without a deadline.

## Focused checks and actual evidence

Written, **not run**:

- Store unit test `continuation_identity_separates_world_phase_action_and_actor_plan`:
  nonterminal rejection, round trip, each key dependency and fresh-store isolation.
- Patch test `cached_and_uncached_continuations_have_identical_cells_weights_plans_and_values`:
  original uncached evaluator versus cold/warm queries, every authored-plan cell,
  weight and mean bits, selected plans, paired guards and best option.
- Patch test `ho1_card_result_is_reused_by_ho2_start_without_weakening_phase_or_plan_identity`:
  lead_values followed by route_choice reuses entries with increased test-only hit
  count; equal Start/Cards successor state and knowledge; invalid announcement
  phase rejected, changed cursor misses, fresh Ctx starts empty and agrees.
- Patch test `expired_batch_does_not_start_uncached_work_or_change_existing_admission`:
  expired initial query returns None with no entries or hits.

Static evidence:

- Inspected engine transitions, scoring, actor-local Plan, Known, Ctx and Routes
  ownership, WorldValues admission/aggregation and HO1/HO2 callers.
- `git -c core.whitespace=cr-at-eol diff --check` passed before staging.
- Plain `git apply --check` passed in clean control worktree at released
  `845551dafdb1ed253106632e4823f5179a9bfc2f`. The patch was not applied there.
  An initial patch serialization lost CRLF; regenerating the patch without shell
  text conversion resolved that check failure.
- Compilation, formatting, tests, benchmark and independent review results are
  pending. Root's tests already in flight do not validate this new patch.

Root applies the concepts commit, then the private patch, and performs serialized
bounded checks through its existing two-CPU, below-normal-priority, 4 GiB wrapper
with CARGO_BUILD_JOBS=1 and retained caches. Suggested focused filters:

```text
cargo test -p marjapussi-heuristic --lib concepts::tests -- --test-threads=1
cargo test -p marjapussi-heuristic --lib continuation_cache_tests -- --test-threads=1
```

Root owns the appropriate broader regression/formatting checks and measurement.
There is no measured speed or playing-strength claim. This incomplete checkpoint
does not replace a verified baseline. No changes to bidding, fitted B-take,
valuation, passing, solver or beliefs cache internals belong to this iteration.

## Cleanup

No owned jobs or build outputs were created. Reclaimed space: **0 B**. Private
patch, source copies and test source remain for the scheduled reviewer and root
integration; shared data, caches and worktrees are retained.

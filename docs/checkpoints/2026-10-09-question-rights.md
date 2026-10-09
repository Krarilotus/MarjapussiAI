# Checkpoint: per-player question rights

- Date: 2026-10-09
- Status: verified for the owner-approved rule correction; no playing-strength claim.
- Tag: `checkpoint/2026-10-09-question-rights`
- Branch: `integration/reviewed-ai`
- Base: `ffc1334326c1d2ca0d91bc3b1196b7ca36fd97a1`
- Testbed checkout: `353c786aad5ee64d683fe489ff6bdcc10d1fcbd2` (unchanged).
- Engine-fork companion: `fix/per-player-question-rights`, based on
  `eb48421c6b7870b6e58d8126789884367ddb60be`; its checkpoint tag resolves its final commit.

## Result

Only asking spends the asker's rights. Asking a pair removes own announcement;
asking any half also removes pair questions. Answers do not spend either player's
rights. In particular, a negative pair answer still permits a later pair question.
One question per won lead remains unchanged.

Remove the extra negative-answer transition from engine and knowledge, delete its
unused helper, and retain the knowledge constraints learned from the answer. The
heuristic skips a known-impossible pair question even with a zero probability
threshold; this belongs in the policy, not in engine legality.

SamuelLess/marjapussi-rs PR #1 already implements the intended asking transitions.
The unwanted negative-answer transition existed in fork commit `70d2a12` on
2026-02-25 and was carried into prepared branch commit `eb48421` on 2026-10-06.
That prepared branch was not an opened upstream PR. The earlier claim that its
extra restriction matched the website's rules was incorrect.

## Evidence

Windows Rust/Cargo 1.91.1; offline locked builds, two jobs, two test threads.
Separate baseline and changed target directories avoid cross-worktree artifact reuse.

- Engine suite: 39 tests pass, including four complete legal question/answer traces,
  exhaustive legal-action equivalence, random traces, scoring and fast/full state checks.
- Knowledge suite: 24 tests pass, three performance tests remain deliberately ignored.
  Includes per-player rights, negative-answer facts, hidden-card swaps and soundness.
- Heuristic suite: 78 tests pass; one pre-existing strategy scenario remains ignored.
  The new zero-threshold regression failed before the policy guard and passes after it.
- All-target clippy for engine, knowledge and heuristic with `-D warnings`: pass.
  Formatting and diff checks: pass.
- Independent reviewer found no material defect in the rule/knowledge/policy diff.
  Fork companion: 13 existing tests plus the same four adapted scenarios pass.
- New engine scenario fixture SHA-256:
  `8de0aa3dfcb9dc2cda72148904f5b193378f0e5710bfe4e9102f25046a10510b`.
  Before the fix, three scenarios pass and the negative-pair scenario fails.

The 2,000-game random trace intentionally changes from `0x3544cf5afa288fc8`
(120,274 actions) to `0xc9a7c1f8a80a3e23` (120,359 actions): the legal action set
now includes repeated pair questions, affecting subsequent random choices.

The private 1,579-record replay changes digest from `0xcb8cf66a3bd29d91` to
`0x8718fcb4f2186c9f`. A record-by-record comparison establishes that the only
changes are 42 newly accepted pair questions and their 42 negative answers in
35 records. Every trick, point total and score is identical. The same 1,577 games
finish; the same two historical illegal-card records stop. Aggregate scores
remain `[34630, 28255]`. Raw records and diagnostic exports stay outside Git.

## Timing

Release builds replay the exact same 1,000 synthetic games, seed 12345:
58,266 actions including 467 negative pair answers. Each recorded action is checked
legal in both engines before timing. Four alternating before/after process pairs,
nine measurements each (36 per version), give these medians:

| Work | Before | After |
|---|---:|---:|
| Apply action plus enumerate legal actions | 271.2 ns/action | 264.9 ns/action |
| Enumerate legal actions on stored states | 212.3 ns/call | 188.8 ns/call |

No slowdown is evident on this workload; small differences are not a speedup claim.
The initial process pair was noisy and retained separately, outside these medians.
This measures the included AI engine; the companion fork has its own benchmark.

Synthetic action fixture SHA-256:
`d43f7db99ea04933e6a75ba2183a6de4ea16b6ffe0e3a87fdde56fd0e3891867`.
Benchmark source SHA-256:
`97d6138482014d4138683759dc669f2cf780c7f11033c3d0d936644fdf19e6c6`.
Source, fixture and raw timings are retained in outer workspace
`tmp/2026-10-09-question-rights/` for the engine submission review.

## Remaining work and cleanup

Question/answer support is still absent from the solver's compact state. Its
deadline, cache reuse and search reductions require their own checkpoint. The
upstream engine submission and additional performance review are separately
authorized by the owner; this checkpoint does not imply that a PR was opened.

Temporary diagnostic tests and benchmark examples were removed from this checkout.
Build caches and shared synthetic fixtures are retained for the next review.
No container or server was started; no cache space reclaimed in this iteration.

# Checkpoint: rollout public activation facts

- Date: 2026-10-14
- Status: incomplete
- Tag: `checkpoint/2026-10-14-rollout-public-activation-facts`
- Branch: `work/2026-10-14-rollout-pair-facts`
- Base: `13d2015192ef04a27b21920ae314bd0853d35278`
- Public deployed AI remains `13d2015`; no paired repository changed here.

## Result

The bounded actor-response rollout now retains public whole-pair exclusions in
`Known.pair_possible`. A single observed engine-move path handles announcements
and both direct and automatic answers. NoPair suppresses a futile repeated Yours
while preserving possible split-half questions. NoHalf excludes both respondent
halves without consulting the asker's hidden allocation. Announce/YesPair reveals
the owner's two halves and overrides its false blank reading. YesHalf clears the
respondent's reading; only a newly called suit proves an asker half. A mere half
question proves no asker ownership.

Historical exchange loading preserves later pair exclusions and clears blank
readings contradicted by `mind.public().count_bounds`, including positive half
constraints with no certain card identity. Hypothetical exchange resets pair
masks only for the transferring team. Engine legality/scoring and knowledge facts
remain authoritative; actor-local continuation semantics and weights are unchanged.

Scenario details: [design](../design/rollout-public-activation-facts.md).

## Evidence

| Command or review | Result | Scope / limitation |
|---|---|---|
| Former-thread `agent_read` | Confirmed stopped before writes | Existing task edits preserved |
| Engine/knowledge/observation source review | Rules support the selected public facts | Static review, no playing-strength claim |
| Owner source review | Core source considered sound; requested fresh-context cache control added | Review does not establish runtime correctness |
| Independent source review | Positive-read path and immutable-context cache ownership reviewed; no hidden-hand inspection found in that path | Final added cache control not runtime-validated |
| Targeted `rustfmt` | Completed for source and public-fact regressions | Existing cache formatting outside owned changes retained |
| `git -c core.whitespace=cr-at-eol diff --check` | Passed | Existing tracked CRLF accepted as line endings |
| Builds, tests, lint, runtime/benchmark checks | Not run by this author, per owner instruction | Root owns serialized two-CPU / 4-GiB validation |

Synthetic regressions cover direct/automatic negative and positive answers,
repeated-Yours suppression with a split-half control, pair ownership, hidden card
swaps, half questions without an asker half, public positive bounds overriding
historical readings, private kept halves not overriding those readings, played
halves, and transferring-team-only mask reset. Existing Start/Cards actor equality
now includes the pair mask. The fresh-context control changes only public NoPair,
warms the exact same query in the old context, checks no stale reuse in the fresh
context, and compares fresh cold/warm cells and values against the existing oracle.

The owner's separate 2,000-game `13d2015` versus `af55` comparison reports -0.30,
approximate interval [-2.48, +1.87]; it establishes neither a drop nor improvement.
This patch is not claimed to fix four selected small-sample losses. The old team
response diagnostic recovered those losses but is outside the intended semantics.
No private datasets or identifiers are added.

## Remaining work

Root must compile and run the affected heuristic public-fact and continuation-cache
regressions, then required broader checks. Any failures need a corrective checkpoint;
this tag is incomplete and must not replace the verified baseline. Full runtime
qualification remains pending. Owner predeclared the next comparison: 2,000 paired
games, seed `2026101404`, against deployed `13d2015`, after correctness checks.

## Cleanup

No builds, owned background processes, Docker resources or disposable build outputs
were created by this author. Reclaimed space: 0 bytes. Existing target caches, shared
fixtures and other worktrees retained. No push, deployment or merge performed.

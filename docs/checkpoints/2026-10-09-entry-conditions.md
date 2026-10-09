# Checkpoint: entries and surviving pair opportunities

- Date: 2026-10-09 (Europe/Berlin)
- Status: incomplete — implementation and automated checks pass; independent
  review and strategic acceptance remain pending.
- Tag: `checkpoint/2026-10-09-entry-conditions`
- Branch: `integration/reviewed-ai`
- Base: `db53997fc45a1768d0f82ba32d29fab2d9b501ec`,
  `checkpoint/2026-10-09-solver-questions`.
- Last verified checkpoint: `846711fe5a811b93040f4847e379ab39d1025aed`,
  `checkpoint/2026-10-09-question-rights`.
- Testbed: `353c786aad5ee64d683fe489ff6bdcc10d1fcbd2`, unchanged.
- Samuel's engine and upstream PRs: unchanged. No publication or push.

## Result

[Design and limits](../design/entry-conditions.md). The solver now offers a small
known-deal trick-control query: independently forceable team wins, seat entries
and surviving pairs activatable at the next opening. All follow, overtake,
ruff, question, answer and trump transitions use the existing engine.

The heuristic lazily reuses the same joint weighted deals for its candidate
cards. Ruff entries must survive opposing play; mere trump possession no longer
suffices. Next-lead value requires the right winner, rights and surviving halves,
and credits at most one activation. Zero-valued activation no longer leaves a
hard half/suit protection mask behind. Concrete immediate entries supplement the
older live-Ace access proxy when a pair requires partner's lead.

Knowledge distinguishes a team winner from a lead retained by the same player.
Announcement timing keeps the existing question-attempt model but uses safe own
runs: no-trump winners, or the current trump run after at most one side winner.
Repeated side rounds are not counted as independently guaranteed entries.
Opportunities end before the last trick. Observations and trump changes trigger
a fresh assessment. Existing endgame cache reuse and the four-card default stay.

Beliefs owns the existing deal-weight formula, now shared with endgame search.
Superseded timing states and unused reach fields are removed; standing-card
selection has one shared implementation. No learned weights or graph framework.

## Evidence

Rust 1.91.1, Windows release builds, two build jobs and two test threads.
Private reproducible tools, traces and logs: outer workspace
`tmp/2026-10-09-entry-planning/`.

| Check | Result / limit |
|---|---|
| Beliefs and heuristic tests | 101 pass, including 30 play scenarios; history and parameter reuse remain covered |
| New short-search scenarios | Four pass: blocked top trump, overruff, purposeful partner choice, own versus team lead, rights, forced half consumption, final-trick cutoff, changed/restored trump |
| Independent full-engine oracle | 120 positions, 384 root actions including 145 questions/answers; 5,376 Boolean goal comparisons agree with full `Game` minimax, seed 913731 |
| Knowledge soundness and hidden swaps | Both pass; retained-own-lead facts checked against actual hands in the existing random-game guard |
| Agent privacy, frozen exchange trace, deterministic self-play | All three pass; 500 valid hidden-card swaps; 50 complete deals played twice with explanation checks |
| Workspace/all-target clippy, formatting, diff hygiene | Pass |
| Independent T3 review | No result received; thread initially had no turn/session, later read timed out. Cancellation requested without acknowledgement. No independent approval claimed |

This is **110 functional tests validated**, plus the independent oracle.
Commands are the ordinary release tests for `beliefs`/`heuristic`; solver test
`trick`; knowledge tests `soundness`/`privacy_swap`; tools tests
`privacy_decisions`/`passing_checkpoint`/`selfplay`; and
`cargo clippy --offline --workspace --all-targets -- -D warnings`.

Human evidence: the earlier randomly selected ten declarer and three partner
examples, then all 256 eligible games involving the requested players in either
role. Eligibility requires successful replay and matching recorded points;
no winner/loser filter. The same 2,790 decisions were replayed before and after
with endgame disabled to expose the heuristic. Fifty actions change. Ruff-rule
selections fall from 86 to 48. Human agreement is 1,864 versus 1,870 decisions:
an exploratory diagnostic, not held-out evaluation or evidence of stronger play.
No player identifiers, raw hands or records are committed.

Dataset SHA-256:
`e41a69d6f81819dc3619655c9a35ca39c906a8ebe647e3aac869bd65b8823865`.
Before/after trace SHA-256:
`2b4c9a697ebc0fb3cce234c67163b442f55b1de6beee12938ebcb7a64e0b667d` /
`b7ea14449af4440a6efc4f5ed7add8320df4d932a12b2da7176199e7bd382c69`.
The private study uses its own frozen dependency lock; workspace lock unchanged.

Timing: `perf --games 100 --repeat 1`, default agent, fixed deals 0–99,
5,258 decisions. Mean non-forced decision **0.922 ms**, maximum **42.564 ms**;
trump/lead p99 **24.287 ms**. This shared-machine observation meets the 50 ms
target on this workload; it is not a hard real-time bound or a speedup claim.
Decision-log SHA-256:
`a940c833c928afa0513ab28a859fd51b2c7773c0ef90b29006582c4e0117df17`.

## Retired checks and remaining work

The historical 0.2% cap on voluntary pair breaks already fails at the unchanged
base: 192 / 72,000 card plays, mostly solver choices. It ignores rights, access
and contract value. Remove that cap; preserve explanation checks in the existing
determinism test and use concrete activation scenarios for correctness. Also
remove the ignored "no entry" fixture that still allowed an Ace entry, the
unsupported human-void calibration test for the absent pass reader, and the
duplicate speed test with thresholds predating the endgame solver. `perf` remains
the reproducible timing tool. Their failures were not fixed by raising thresholds.

During development, a too-broad possible-partner-pair mask failed two existing
play scenarios; limiting the hard mask to held halves or a certain partner pair
fixed it. The new partner-overtake fixture initially omitted the still-live Ten;
it now explicitly accounts for that blocker. Clippy's parity suggestion is fixed.
No unresolved automated failure remains in the stated validation scope.

The longer-horizon access/score valuation is still heuristic. Weighted exposed
deals do not solve strategy fusion, and different query goals may require
different continuations. Next: review concrete multi-trick return plans with card
consumption, legal switches, partner access and conditional contract outcomes;
qualify on separate games before selecting new returns or enabling raising.

## Cleanup

All owned build/test jobs finished; no containers or services were created.
Automatic approval review rejected removal of this iteration's `target-baseline`,
`target/debug` and clean review worktree with "blocked by policy"; no more detail
was supplied. **Zero bytes reclaimed.** About 385 MiB of disposable build output
and the review worktree remain. Release cache and private evidence are retained
for the pending review. No other project's resources were changed.

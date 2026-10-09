# Checkpoint: question-aware endgame search

- Date: 2026-10-09 (Europe/Berlin)
- Status: incomplete — implementation and automated checks pass; independent
  review and acceptance of changed endgame choices remain pending.
- Tag: `checkpoint/2026-10-09-solver-questions`
- Branch: `integration/reviewed-ai`
- Base / last verified checkpoint: `846711fe5a811b93040f4847e379ab39d1025aed`,
  `checkpoint/2026-10-09-question-rights`.
- Testbed: `353c786aad5ee64d683fe489ff6bdcc10d1fcbd2`, unchanged.
- Samuel's engine repository and upstream PRs: unchanged in this iteration.

## Result

The compact state and known-deal search now include questions, every truthful
answer choice, all three rights levels per player and trump changes. Engine
helpers own answer legality and resulting trump; the solver owns traversal,
bounds and pruning. The old own-right mask/open-announcement representation
was replaced. State size remains 56 bytes.

A split pair can now establish or restore trump inside search. Restoring a
called suit scores no second pair bonus. The future-point bound includes whole
and split team pairs, and cache keys separate full rights and pending questions.
A question with no effect on trump or score may be pruned only inside a
known-deal continuation; all legal root questions remain available for comparison.

One budget covers the candidate comparisons. Only whole completed worlds
contribute to averages; an interrupted branch is never cached as exact.
Completed position bounds remain reusable. Results expose `complete` and
`all_deals`, replacing the misleading `exact` flag.

The agent keeps its four-card/64-world defaults, adds a 45 ms deadline, and now
uses search at question opportunities and multiple-answer choices. It still
omits questions with a known negative answer as policy, not engine legality.
If no complete comparison is available, existing heuristic rules remain the
fallback. No new planning policy, fitted weights or generic graph framework.

## Evidence

Rust/Cargo 1.91.1 on Windows, release tests, two build jobs, two test threads.
Evidence and reusable target: outer workspace
`tmp/2026-10-09-solver-questions/`.

| Check | Result and scope |
|---|---|
| Baseline solver suite at base commit | 11 tests pass; two speed tests deliberately ignored |
| Release tests: engine, knowledge, heuristic, solver | 154 passed; six ignored. One additional focused rights test added afterwards also passes: **155 functional tests validated** |
| `fast_vs_game` | Full legal play actions and stepped states agree across 5,000 games, seeds 1,000,000–1,004,999 |
| Exhaustive small search | 2,300 positions with 1–4 tricks, both teams, points/tricks/thresholds; 150 endings compared with full `Game` minimax including questions and answers |
| Rights/trump scenarios | Asking for partner's whole pair yields 170 future points with Yours versus 0 with Ours; respondent remains Ours. Split Red pair yields 149, or 49 when restoring a previously scored suit. Shared-cache query orders agree |
| World evaluation | 2,568 late decisions including pending answers, 482 known deals; 1,399 enumerated spaces of 2–40 deals; hidden-card swaps and weighted samples pass |
| Interrupted search | Node-limited partial means match the independently evaluated completed-world prefix. Resuming the same cache equals fresh unlimited evaluation; expired deadline returns Interrupted |
| Existing private replay | Existing digest/outcome guard passes with the available dataset and its two documented historical illegal-card records |
| All-workspace/all-target clippy | Pass with `-D warnings`; checks all AI workspace callers |
| Formatting / diff hygiene | Pass |
| Independent T3 review | Did not run: thread remained without a turn/session; cancelled and removed its unused clean worktree. No independent approval claimed |
| Testbed release/build qualification | Not repeated; this is not a cross-repository release |

Commands:
```sh
cargo test --offline --release -p marjapussi-engine -p marjapussi-knowledge -p marjapussi-solver -p marjapussi-heuristic --tests -- --test-threads=2
cargo test --offline --release -p marjapussi-solver --test exact partner_pair_requires -- --nocapture --test-threads=2
cargo test --offline --release -p marjapussi-solver --test speed decision_time_by_worlds -- --ignored --nocapture --test-threads=1
cargo run --offline --release -p marjapussi-tools --bin perf -- --games 100 --repeat 1 --out <private-scratch>/agent-decisions.txt
cargo clippy --offline --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

The ignored cases in the broad suite are five deliberate performance checks and
one inherited heuristic scenario `hc1_owner_b_without_an_entry_gives_the_ober`.
The solver decision benchmark was run explicitly afterwards. The older ignored
belief calibration check is outside this four-crate test run.

During implementation, one random fixture's Own-right frequency was accidentally
reduced by introducing three equally likely rights levels. All value comparisons
passed but announcement coverage fell below its existing threshold. Restoring
50% Own / 25% Yours / 25% Ours preserves that coverage; the threshold was not
lowered. A new fixture initially used “10” instead of the engine's “Z” card code;
correcting the fixture made it pass. No unresolved test failure remains.

The private replay data SHA-256 is
`e41a69d6f81819dc3619655c9a35ca39c906a8ebe647e3aac869bd65b8823865`.
No raw records or player identities are committed.

## Timing

Shared Ryzen 7800X3D machine; these are observations, not hard real-time bounds.
The benchmark reuses the same full-engine random-game generator (seeds from 1,
20 positions per hand size). The new model explores additional legal branches,
so before/after time is a feature-cost comparison, not equal work.

Four cards, 100 worlds, unlimited open-contract comparison: baseline mean/max
1.3/6.5 ms; new model 2.3/10.2 ms in the final run (earlier run 2.9/12.6 ms).

With 64 worlds, 45 ms deadline and the agent's 2^16-entry cache:

| Cards | Cold mean / max ms | Complete / partial / no world | Warm mean / max ms |
|---|---|---|---|
| 4 | 2.83 / 11.77 | 20 / 0 / 0 | 0.23 / 1.40 |
| 5 | 13.09 / 45.01 | 18 / 2 / 0 | 2.58 / 25.62 |
| 6 | 33.25 / 45.01 | 13 / 7 / 0 | 11.73 / 45.01 |
| 7 | 45.01 / 45.01 | 0 / 20 / 0 | 45.01 / 45.01 |
| 8 | 45.01 / 45.03 | 0 / 12 / 8 | 45.01 / 45.01 |
| 9 | 45.12 / 46.31 | 0 / 5 / 15 | 45.10 / 46.07 |

Warm repeats the same input and sampled worlds, measuring reusable bounds. It is
not a whole-game continuation benchmark. The four-card workload in the earlier
run measured 2.86 ms mean / 12.10 ms max; both logs are retained.

Existing `perf`, default agents, 100 synthetic self-play games (seeds 0–99):
5,263 actions; maximum **whole-agent** decision 45.125 ms. There were 1,318
solver choices, three marked incomplete, including 265 question choices.
This checks integration/latency, not increased strength or sensible information
value for every new question choice. Synthetic action digest:
`301bc31b65e48432` (the tool's FNV digest).
Benchmark source SHA-256:
`50b13212ffe5457d6ab9acf002cd0aa23244acbbee84690446b45c074ff9418e`.

## Remaining work

- Independent review before promoting beyond the last verified checkpoint.
  Review changed question choices as well as values; tie choices can now select
  questions absent from the former search.
- The exactness claim is restricted to a known deal and the selected objective.
  PIMC can fuse different hidden-information strategies even when every deal is
  enumerated. Estimated reach frequencies are not calibrated win probabilities.
- Deadline polling covers search; deal counting/preparation and sampling are
  not interruptible. Do not promise arbitrary inputs finish within 50 ms.
  Keep the current four-card trigger while broadening whole-agent tail evidence.
- Facts and beliefs still update after every observation. Physical-state cache
  reuse does not authorise blindly following one predicted line.
- The connected concept plan, midgame information-aware search and GPU comparison
  remain separate work. Existing return selection and raising policy are unchanged.

## Cleanup

All owned build/test jobs finished; no server, GPU job or container was started.
Removed clean unused detached worktree `tmp/2026-10-09-concept-audit`:
**1,440,025 file bytes reclaimed**. Retain the approximately 547 MB target for
the pending solver review and next bounded-search checks, plus small logs and
the synthetic trace. Older shared datasets/caches are untouched. Commit/tag
are local; no push, merge or PR update in this checkpoint.
# Checkpoint: B-take and the faster exact solver

- Date: 2026-11-01
- Status: verified (strength on three deal seeds, partnerships, hard limits, tests, review)
- Tag: `checkpoint/2026-11-01-b-take-faster-solver`
- Branch: `hillclimb/frontier`
- Base checkpoint and commit: `checkpoint/2026-10-31-exchange-routes` (`c3`, `bb2f36c`)
- Validated commit: `2b19c6f`, frozen as benchmark policy `c4`

## Result

**B-take on** (`b_take`). Over an opponent's contract at 140 or more, the AI bids on by the
rule fitted to human choices (P(take) and P(make the next step) both at least 0.5). The owner
switched it off on 2026-10-13 as "a restriction to reassess once the declarer play improves".
With the deep endgame and the exchange routes it has: the team declares about 3,200 contracts
instead of 1,800 and still makes 67 % of them. The rule's strongest term is the team's pair
cover, so its bids across 140 keep the convention that such a bid promises a pair. The owner's
examples of B-control and B-offer predate it: their tests read them without B-take and record
where the default player now takes the game instead.

**A faster exact solver** (an Opus delegate; reviewed by Opus, no defect). Exact on every
position: the solver's golden hash is unchanged, and with the node budget removed the agent's
60-game decisions are byte-identical to `c3`.

- A last trick that no question can turn is played out, not searched.
- Leads try a sure winner first, then a partner's winner, then cheap cards; later in a trick a
  partner's winning card counts only if the opponent still to play can't take it.
- Questions are generated only where the leader's team holds an uncalled pair, and the
  transposition key drops called suits, rights and phase that no pair can still use.
  `search::team_pairs` owns that rule for the solver.
- Mean nodes of a 64-world contract decision at 5 / 6 / 7 / 8 cards: 152k / 950k / 3.63M /
  27.6M → 104k / 539k / 2.21M / 14.4M; mean ms 7.3 / 49 / 182 / 1,430 → 5.4 / 26 / 120 / 774.

**1.2M solver nodes per decision** (`endgame_nodes`, was 1M): the search's 65 ms share at
about 55 ns per node. At the old budget the faster solver finishes more searches without a
measurable gain; 1.2M, 1.5M and 2M gain about 1–2 points/game, and only 1.2M fits the move
limit without the deadline (raw maximum 86 ms on an idle machine).

Rejected on the way (see the [log](2026-10-31-hillclimb-log.md), "After checkpoint 3"): HO-3
follows by route, exchange racing at z = 2 and z = 3, more forth-route candidates (the
parameter is removed), HZ-1 by route, HZ-1 over the estimate's spread (coarse and exact), the
auction thresholds' neighbours.

## Evidence

All runs: 2,500 deals in 625 four-game series, each replayed with the partnerships swapped
(5,000 games), policy seed 0, `--rotation series`, `--deadlines off` unless stated, 12
workers. Rerun one line with `./marja-bench c4 c3 c4 c3 5000 --rotation series --policy-seed
0 --workers 12 --compact --seed 1 --deadlines off` (any line: its policies and seed).

| Command or review | Result | Scope / limitation |
|---|---|---|
| `c4` vs `c3`, seeds 1 / 2 / 4 | **+5.04 [+1.35, +8.74] / +4.94 [+1.09, +8.80] / +8.92 [+5.25, +12.58] points/game; +42.65 [+15.55, +69.75] / +58.88 [+29.20, +88.55] / +73.94 [+46.90, +100.98] points/series** | seed 1 measured as `98a16c3` with 1.2M nodes (identical decisions) |
| `c4` vs `base`, seed 2, 16 workers alone on the machine | +23.21 [+18.31, +28.10] points/game, +200.05 [+164.70, +235.40] points/series | `c3`: +25.04 / +191.10 |
| same, `--deadlines wall` | +23.21 / +200.05; 56 of ~105k moves cut by the clock | live behaviour |
| Hard limits, the two runs above (deadlines off / wall) | move max 85.6 / 80.0 ms; bid ≤ 1.6 / 1.6 ms; exchange step ≤ 1.77 / 1.77 s; nothing over a limit | 16 workers pinned one per physical core, machine otherwise idle |
| `c4 c3 c3 c3` vs `c3`, seed 2 | +2.74 [−0.26, +5.74] points/game, +33.59 [+10.58, +56.59] points/series | partnered with checkpoint 3 |
| `c4 base base base` vs `base`, seed 2 | +10.13 [+6.00, +14.26] points/game, +97.06 [+67.24, +126.88] points/series | partnered with the current AI (`c3`: +9.91 / +86.71) |
| `cargo test --workspace --release` | 377 passed, 0 failed (6 ignored) | at `2b19c6f` |
| `python3 -m unittest` (scripts/marja_bench) | 42 passed | |
| Decision hashes, 60 games, `perf` with `deadlines = false` and `b_take = false` | `b4a5b7c0f2a06651` at 1M nodes, `3bc0682a03147330` without a budget: equal at the delegate's commit and after the review refactor | the solver review's changes are exact |
| Opus review of the solver commits | no must-fix; one owner for the question rule, a wider merge test (pending questions, tricks), nits: fixed in `adf9262` | |

## Per-game regressions against the current AI

`changes.py` pairs `c4` vs `base` (seed 2) with `base` alone on the same games: 3,515 of
5,000 games change, +23.21 points/game in total (`c3`: +25.04, 2,979 changed).

| Change against `base` in the same seat | Games | Points/game |
|---|---:|---:|
| our contract made that `base` failed | 210 | +17.9 |
| same result, other points | 1,561 | +5.9 |
| the opponents' contract broken that `base` let through | 73 | +5.4 |
| other raise (final value) | 718 | +4.9 |
| other declarer (the auction; mostly B-take) | 809 | −0.2 |
| **the opponents' contract made that `base` broke** | **58** | **−3.9** |
| **our contract failed that `base` made** | **86** | **−6.8** |

The two regression classes have the causes of checkpoint 3 (route passes misjudged by the
rollouts: 66 of the 86; own-points defence). New is the auction: where B-take takes the game
from `base` (738 games), 515 contracts are made (+9.2 points/game) and 223 fail (−10.4). Against
`base` B-take is about even in game points (`c4` −1.8 points/game below `c3`), while the series
margin rises (+200.05 vs +191.10: the team wins the series bonus in 17.8 % of series, `c3`
15.5 %; `base` 6.2 % and 7.0 %). Against `c3` it gains in both. The gain is therefore opponent-dependent in
points per game, not in points per series.

## Remaining work

- B-take's game-point value depends on the opponent (strong against `c3`, even against
  `base`); human opponents are unmeasured here.
- The routes' agreement with human passes and B-take's with human bids-on still need the
  private human games (`MARJA_GAMES`).
- 1.5M and 2M nodes gain more but exceed the move limit raw; a faster solver or a
  multi-threaded search (an owner decision about the server's cores) would unlock them.

## Cleanup

No jobs of this checkpoint remain. Result folders, frozen policy caches and the scratch
worktrees (`MarjapussiAI-proto*`, `-ref`; prototype branches not merged) are retained.

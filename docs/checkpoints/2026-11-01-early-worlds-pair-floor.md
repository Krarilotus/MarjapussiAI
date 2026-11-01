# Checkpoint: early worlds and B-take's pair floor

- Date: 2026-11-01
- Status: verified (strength on three deal seeds, against the current AI, partnership, hard limits, tests, reviews)
- Tag: `checkpoint/2026-11-01-early-worlds-pair-floor`
- Branch: `hillclimb/frontier`
- Base checkpoint and commit: `checkpoint/2026-11-01-b-take-faster-solver` (`c4`, `2b19c6f`)
- Validated commit: `c52ae96`, frozen as benchmark policy `c5`

## Result

**Early worlds** (`early_worlds`, 256). A move before the exact endgame's scope (more than
`endgame_tricks` tricks left: the first two tricks and the raise-or-lead choice) samples 256
believed worlds for its routes and valuation terms instead of 64; those moves had most of
their 100 ms unused. `DecisionClass::worlds` owns the count. Uncapped, the widest
comparisons (a trick start's activation over every option × 9 plans) took up to 122 ms raw,
so a move's route evaluation now stops after 6,000 rollouts but never below 64 worlds
(`move_route_rollouts`, `DecisionClass::route_worlds`): the widest comparisons keep the cost
and sample they had in `c4`, the narrower ones use the whole pool.

**B-take's pair floor.** B-take (bidding on over an opponent's contract at 140 or more) now
also needs the owner's C43 team-pair cover of its bid (`team_pair_cover_derivable`: my
complete pairs and those derivable from the partner's messages, Green left out) at least
`over_140_cover`, the cover B-over140's tie-break needs. A human partner reads a bid above 140
as a pair (HR-4); without the floor B-take bid 145 with a pairless strong hand (Fable review).
It costs about half a point per game against `c4` and gains against `base`, whose
defence broke many of the pairless takes.

Rejected on the way (see the [log](2026-10-31-hillclimb-log.md), "After checkpoint 4"):
B-take by route, its gates at 0.4 / 0.6, counting the partner's believed pairs in its estimate,
the auction thresholds' neighbours, 48 endgame worlds, 12 minimum worlds, 512 early worlds,
αμ (pending: an Opus prototype).

## Evidence

All runs: 2,500 deals in 625 four-game series, each replayed with the partnerships swapped
(5,000 games), policy seed 0, `--rotation series`, `--deadlines off` unless stated, 12
workers. Rerun one line with `./marja-bench c5 c4 c5 c4 5000 --rotation series --policy-seed
0 --workers 12 --compact --seed 1 --deadlines off` (any line: its policies and seed).

| Command or review | Result | Scope / limitation |
|---|---|---|
| `c5` vs `c4`, seeds 1 / 2 / 4 | +0.34 [−1.97, +2.64] / +0.95 [−1.46, +3.36] / −0.56 [−3.01, +1.90] points/game; +3.33 / +2.43 / +2.65 points/series | **no measurable difference** head to head |
| `c5` vs `base`, seed 2, 16 workers alone on the machine | **+27.67 [+22.96, +32.37] points/game, +231.74 [+197.19, +266.28] points/series** | `c4`: +23.21 / +200.05 |
| same, `--deadlines wall` | +27.67 / +231.74; 84 of ~105k moves cut by the clock | live behaviour |
| Hard limits, the two runs above (deadlines off / wall) | move max 81.9 / 80.2 ms; bid ≤ 1.7 / 1.6 ms; exchange step ≤ 1.80 / 1.76 s; nothing over a limit | uncapped 256 worlds: 121.8 ms, 6 moves over 100 |
| `c5 base base base` vs `base`, seed 2 | +12.15 [+8.12, +16.19] points/game, +109.83 [+80.59, +139.06] points/series | partnered with the current AI (`c4`: +10.13 / +97.06) |
| `cargo test --workspace --release` | 380 passed, 0 failed (6 ignored) | at `c52ae96` |
| `python3 -m unittest` (scripts/marja_bench) | 42 passed | |
| Fable reviews of `f58eee7..7ef9693` and `7ef9693..360634e` | no must-fix in the code; the hard-limit claim (must-fix on evidence) led to the rollout limit and its 64-world floor; tests and docs fixed | |

## Per-game regressions against the current AI

`changes.py` pairs `c5` vs `base` (seed 2) with `base` alone on the same games: 3,457 of
5,000 games change, +27.67 points/game (`c4`: +23.21).

| Change against `base` in the same seat | Games | Points/game |
|---|---:|---:|
| our contract made that `base` failed | 213 | +18.3 |
| the opponents' contract broken that `base` let through | 85 | +6.4 |
| other raise (final value) | 728 | +6.1 |
| same result, other points | 1,654 | +5.6 |
| other declarer (the auction; mostly B-take) | 621 | +2.7 |
| **the opponents' contract made that `base` broke** | **68** | **−4.6** |
| **our contract failed that `base` made** | **88** | **−6.8** |

Where B-take takes the game from `base`, 413 contracts are made (+7.6) and 130 fail (−6.0):
net +1.6 points/game, against −1.2 for `c4` (515 made, 223 failed). The two regression classes
keep the causes of the earlier checkpoints (route passes misjudged by level-0 rollouts;
own-points defence).

## Why adopt a checkpoint that ties its predecessor

Against `c4` the two changes cancel within noise (early worlds about +1, the floor about −0.5
points/game). The checkpoint is kept because it is clearly stronger against the current AI
(+4.5 points/game and +31.7 points/series over `c4` on the same games) and as its partner
(+2.0 / +12.8), and because the floor keeps a bid above 140 a pair message for human partners.
This does not claim a gain over `c4`.

## Remaining work

- The routes' agreement with human passes and B-take's with human bids-on still need the
  private human games (`MARJA_GAMES`).
- αμ for late endgame decisions (strategy fusion): an Opus prototype is in progress.
- The regression classes against the current AI keep their causes: route passes misjudged by
  level-0 rollouts, own-points defence.

## Cleanup

No jobs of this checkpoint remain. Result folders, frozen policy caches and the scratch
worktrees (`MarjapussiAI-proto*`, `-ref`; prototype branches not merged) are retained.

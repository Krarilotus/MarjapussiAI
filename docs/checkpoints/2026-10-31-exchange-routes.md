# Checkpoint: auction thresholds and exchange routes

- Date: 2026-10-31
- Status: verified (strength on three deal seeds, partnerships, hard limits, tests, review)
- Tag: `checkpoint/2026-10-31-exchange-routes`
- Branch: `hillclimb/frontier`
- Base checkpoint and commit: `checkpoint/2026-10-31-deep-endgame` (`c2`, `1b24987`)
- Validated commit: `bb2f36c`, frozen as benchmark policy `c3`; later commits of this
  checkpoint change no decision (decision hash below)

## Result

With the deep endgame the team makes its contracts more often; two changes of the auction
and the exchange turn that into points.

**Auction thresholds.** Two thresholds that were fitted to imitate humans now cost points.
Both decisions send no new message: the conventions' meanings are unchanged, the AI only
sends them more selectively.

- **B-quiet** (`take_140_min`, 0.5 → 0.8): with nothing left to tell, take the game at 140
  only from a fitted P(take) of 0.8. Passing more often lets the holder of a 125–135
  message bid play it; 0.7–0.9 form a plateau, 0.4 is worse.
- **B-over140** (`over_140_cover`, 0.75 → 0.85): step across 140 (a pair message or the
  over-140 tie-break) only with a team-pair cover of 0.85; 0.95 alike, 0.65 worse.
- **Ownership fix**: B-defend's estimate of how high the opponents can play read the same
  parameter; it is now `opp_reach_cover` (unchanged at 0.75), so the crossing threshold and
  the reading of the opponents are separate concepts.

**Exchange routes (P-route).** The store (fitted human pass choices) proposes each pass;
the routes compare it by rollouts over believed worlds and replace it where another
candidate is clearly better (paired 2-SE guard, one owner: `passing::overrule`).

- Back: the store's 64 best returns join the run-keeping candidates
  (`return_store_candidates`, 0 → 64; candidates plateau from 64).
- Forth: the pass forth runs by route (`pass_route`, off → on): the store's best pass per
  blank-suit pattern, whole suit, ace and kept pair.
- An exchange step samples its own pool of 512 believed worlds (`exchange_worlds`;
  `DecisionClass` owns the owner's three limit classes and their world counts). The worlds
  pay up to 512; 768 add nothing and take up to 4.4 s raw.

**Review fixes** (Opus review of `0e441f7`, fixed in `2a03437`): the series end rules and
HZ-1's contract scores come from `engine::scoring`; the endgame's clock cuts are reported;
the endgame table no longer outlives a decision; the series table is built eagerly.

The tests keep the owner's examples at the owner's thresholds and record where the defaults
deviate: the seed-42 140 take, batch 1c's over-140 step, and HP-5's pass forth (the default
passes the Red A, 10 and K by route there).

Rejected and reverted on the way (see the [log](2026-10-31-hillclimb-log.md)): a series-scaled
take threshold, a game-drive threshold, HZ-1 estimate shifts, B-take, racing every four
worlds, 96 endgame worlds, killer moves, larger transposition tables, a reduced rollout
fallback, 768 and 1,024 exchange worlds.

## Evidence

All runs: 2,500 deals in 625 four-game series, each replayed with the partnerships swapped
(5,000 games), policy seed 0, `--rotation series`, `--deadlines off` unless stated, 12
workers. Rerun one line with `./marja-bench c3 c2 c3 c2 5000 --rotation series --policy-seed
0 --workers 12 --compact --seed 1 --deadlines off` (any line: its policies and seed).

| Command or review | Result | Scope / limitation |
|---|---|---|
| `c3` vs `c2`, seeds 1 / 2 / 4 | **+16.37 [+12.56, +20.19] / +19.35 [+15.37, +23.33] / +19.61 [+15.61, +23.61] points/game; +108.49 [+81.74, +135.25] / +139.57 [+109.48, +169.66] / +138.14 [+109.43, +166.84] points/series** | |
| `c3` vs `base`, seed 2 | **+25.04 [+20.81, +29.27] points/game, +191.10 [+160.26, +221.95] points/series** | the whole hill climb so far |
| same, 16 workers alone on the machine | +25.04 / +191.10, identical to the 12-worker run | determinism across worker counts |
| same, `--deadlines wall`, 16 workers alone on the machine | +25.03 / +191.04; 4 of ~105k moves cut by the clock | live behaviour |
| Hard limits, the two runs above (deadlines off / wall) | move max 89.5 / 80.1 ms; bid ≤ 2.3 / 1.7 ms; exchange step ≤ 2.06 / 1.98 s; nothing over a limit | 16 workers pinned one per physical core; the same lanes beside other jobs reached 338 ms moves (both teams over the limit: CPU starvation, not the policy) |
| `c3 c2 c2 c2` vs `c2`, seed 2 | +7.65 [+4.62, +10.68] points/game, +56.76 [+34.20, +79.32] points/series | partnered with checkpoint 2, which passes and reads by the store |
| `c3 base base base` vs `base`, seed 2 | +9.91 [+6.46, +13.36] points/game, +86.71 [+60.05, +113.38] points/series | partnered with the current AI |
| Per-game attribution of the exchange routes (`changes.py`, seeds 1 / 2) | +13.56 / +18.65 points/game: 186 / 204 contracts made that the store's exchange failed (+16.1 / +18.2), 97 / 80 failed that it made (−7.3 / −6.0) | the log's table |
| `cargo test --workspace --release` | 375 passed, 0 failed (6 ignored) | at the checkpoint commit |
| `python3 -m unittest` (scripts/marja_bench) | 42 passed | |
| Decision hash, 60 games, `perf --params` with `deadlines = false` | `fcfc7e72d372853a` (2,961 decisions) at `bb2f36c`, at the review fixes `47712f9` and on the guard prototype's default | the later commits change no decision |
| Opus review of `0e441f7..3100f48` | no correctness defect in the exchange code; findings fixed in `47712f9` or measured | the log, "Review (Opus, `0e441f7..3100f48`)" |

## Per-game regressions against the current AI

`changes.py` pairs `c3` vs `base` (seed 2) with `base` alone on the same games: 2,979 of
5,000 games change, +25.04 points/game in total.

| Change against `base` in the same seat | Games | Points/game |
|---|---:|---:|
| our contract made that `base` failed | 198 | +16.7 |
| the opponents' contract broken that `base` let through | 85 | +6.4 |
| same result, other points | 1,746 | +6.3 |
| other raise (final value) | 706 | +5.4 |
| other declarer (the auction thresholds) | 96 | +1.3 |
| **our contract failed that `base` made** | **81** | **−6.4** |
| **the opponents' contract made that `base` broke** | **67** | **−4.7** |

Causes of the two regression classes:

- 70 of the 81 failed contracts follow a pass the route chose instead of the store's: the
  rollouts' level-0 play misjudges some passes, and failing instead of making swings the
  margin by twice the contract value. A stricter guard withholds more gain than it saves
  (log, "The routes' guard").
- The 67 contracts the opponents now make involve no exchange of ours (only the declaring
  team exchanges). 34 follow a changed earlier game of the series (other totals, so other
  raises); the rest are defences of the deep endgame, whose defenders maximise their own
  points: a defender criterion that also values breaking the contract broke 34 more
  contracts but cost −3.67 points/game overall (log, "Endgame objective").

Both are tradeoffs inside a strongly positive whole, not systematic losses of a class of
deals: no other class nets negative.

## Cooperation with partners that pass by convention

Humans read a pass by convention, and so do the earlier AIs (the store's choices, no
routes). Partnered with checkpoint 2 against checkpoint 2 (`c3 c2 c2 c2`, seed 2,
+7.65 points/game) and paired with `c2` alone on the same games, both routes still gain
where the convention reader is on the receiving end:

| Route of the `c3` seat | Contracts failed that `c2` made | Contracts made that `c2` failed | Net points/game |
|---|---|---|---:|
| forth (the `c2` declarer receives and returns by the store) | 14 games, −1.14 | 33 games, +3.05 | +2.11 |
| back (the `c2` partner receives the return) | 31 games, −2.47 | 71 games, +5.74 | +5.29 |

So the forth route's gain does not depend on a partner that has the routes too. Whether
the routes' passes agree with human passes needs the private human games (`MARJA_GAMES`),
which this machine does not have: open (below).

## Remaining work

- The regressions are the rollouts' level-0 play: better rollouts (or an exact finish of
  the playout) are the lever, not a stricter guard. Measured and rejected so far: guard
  z = 3, a confirmation half of the worlds, 768 and 1,024 exchange worlds.
- The routes' agreement with human passes is unmeasured here (private dataset).
- `forth_store_candidates` (default 0) is under measurement over 512 worlds; it is removed
  unless it pays.
- Raw move work stays near the 100 ms limit; the live deadline cuts at 80 ms, but no
  deadline protects a decision from a starved CPU (the loaded runs above).

## Cleanup

No jobs of this checkpoint remain. Result folders, the frozen policy caches and the
scratch worktrees `MarjapussiAI-proto`, `-proto2` and `-ref` (prototype branches
`proto/exchange-z`, `proto/confirm`; not merged) are retained for the next iteration.

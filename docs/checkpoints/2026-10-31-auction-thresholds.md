# Checkpoint: auction thresholds

- Date: 2026-10-31
- Status: verified (strength on three deal seeds, partnership, hard limits, tests, review)
- Tag: `checkpoint/2026-10-31-auction-thresholds`
- Branch: `hillclimb/frontier`
- Base checkpoint and commit: `checkpoint/2026-10-31-deep-endgame` (`c2`, `1b24987`)
- Frozen as benchmark policy `c3`

## Result

With the deep endgame the team makes its contracts more often, and two auction
thresholds that were fitted to imitate humans now cost points. Both decisions send no new
message: the conventions' meanings are unchanged, the AI only sends them more selectively.

- **B-quiet** (`take_140_min`, 0.5 → 0.8): with nothing left to tell, take the game at 140
  only from a fitted P(take) of 0.8. Passing more often lets the holder of a 125–135
  message bid play it; 0.7–0.9 form a plateau, 0.4 is worse.
- **B-over140** (`over_140_cover`, 0.75 → 0.85): step across 140 (a pair message or the
  over-140 tie-break) only with a team-pair cover of 0.85; 0.95 alike, 0.65 worse.
- **Ownership fix**: B-defend's estimate of how high the opponents can play read the same
  parameter; it is now `opp_reach_cover` (unchanged at 0.75), so the crossing threshold and
  the reading of the opponents are separate concepts.

Both adopted thresholds differ from an owner example at the owner's stated threshold: the
seed-42 140 take and batch 1c's over-140 step. The tests keep those examples at the owner's
thresholds and record that the defaults now pass or step instead.

Rejected and reverted on the way (see the [log](2026-10-31-hillclimb-log.md)): a series-scaled
take threshold, a game-drive threshold, HZ-1 estimate shifts, B-take, racing every four
worlds, 96 endgame worlds, killer moves, larger transposition tables, a reduced rollout
fallback.

## Evidence

All runs: 2,500 deals in 625 four-game series, each replayed with the partnerships swapped
(5,000 games), policy seed 0, `--rotation series`, `--deadlines off` unless stated.

| Command or review | Result | Scope / limitation |
|---|---|---|
| `hp1` (`0e441f7`) vs `c2`, seeds 1 / 2 / 4 | **+3.44 / +1.09 / +3.63 points/game; +22.62 / +3.73 / +20.32 points/series** | 16 workers |
| `hp1` vs `base`, seed 2 | **+8.91 [+5.60, +12.21] points/game, +73.88 [+48.99, +98.78] points/series** | the whole hill climb so far |
| same, `--deadlines wall` | +8.91 / +73.88; 21 of ~105k moves cut by the clock; move max 93.9 ms | live behaviour |
| `hp1 c2 c2 c2`, seed 2 | +0.52 [−0.75, +1.79] points/game, +0.99 [−7.82, +9.81] points/series | partnered with checkpoint 2: no regression |
| `hp1 base base base`, seed 2 | +3.46 [+0.84, +6.08] points/game, +40.01 [+19.23, +60.79] points/series | partnered with the current AI |
| Hard limits vs `c2`, seeds 1 / 2 / 4 | move max 88.5 / 90.2 / 92.4 ms; bid ≤ 1.9 ms; pass ≤ 7.1 ms; no overrun | raw work, 16 workers; other users' jobs on the machine |
| Hard limits in the `base` suite (deadlines off) | move max 101.4–117.3 ms | ran beside a second benchmark, a review build and other users' jobs: contention; the live lane stayed at 93.9 ms |
| `cargo test -p marjapussi-heuristic` | all passed | |

## Remaining work

Raw work leaves little headroom under the 100 ms move limit when the machine is shared;
live play is protected by the deadline. Search capacity remains the largest lever.

## Cleanup

No jobs of this checkpoint remain; result folders and frozen policy caches are retained.

# Hill-climb log (2026-10-31)

Every experiment against the reproducible baseline `base` (`afe2ebb`, v1/main with the
deadline switch), unless another opponent is named. One run = 2,500 deals in 625
four-game series, each series replayed with the partnerships swapped (5,000 games),
deal seed 1 unless stated, policy seed 0, `--deadlines off`, 12 workers on cores 0-11.
Margins are the candidate team's engine score minus the opponent's; intervals are
approximate 95%, clustered by deal (game) and by series. Maximum times are raw work
without deadline cuts on a loaded machine (not a timing qualification).

Rerun one line: `./marja-bench <candidate> base <candidate> base 5000 --rotation series
--policy-seed 0 --workers 12 --cpus 24 --compact --seed 1 --deadlines off`.

## Endgame search depth (parameters of the existing solver; strength probes)

| Variant | Points/game | Points/series | Max move ms | Cause |
|---|---:|---:|---:|---|
| `endgame_tricks = 0` (no solver) | −12.24 [−14.53, −9.96] | −90.17 [−106.92, −73.41] | 34 | exact endgame play is worth 12 points/game |
| `endgame_tricks = 5` | +5.44 [+3.28, +7.60] | +38.61 [+22.08, +55.13] | 367 | earlier exact play; over the 100 ms limit |
| `endgame_tricks = 6` | +10.04 [+7.21, +12.88] | +64.89 [+43.70, +86.08] | 3,211 | ditto; 32x over the limit |
| `endgame_tricks = 5`, 32 worlds | +4.46 [+2.22, +6.69] | +31.67 [+14.94, +48.40] | 210 | half the worlds lose a point |
| `endgame_tricks = 6`, 16 worlds | +4.94 [+2.07, +7.81] | +29.58 [+9.96, +49.20] | 1,127 | too few worlds lose half the gain |
| `endgame_tricks = 7` | +15.99 [+12.84, +19.15] | +110.69 [+87.66, +133.73] | 94,002 | depth keeps paying; 940x over the limit |
| `endgame_tricks = 5`/`6`, 1M solver nodes, ≥ 16 or 32 worlds (old solver, `98fbe05`) | −0.79 to +3.12 | | n/a | **bug**: complete evaluations of fewer deals than the minimum were discarded, so late endgames fell back to the rules; fixed in `ef04487` |

Conclusion: depth is the strongest lever found, and the world count must stay near 64.
The solver must get faster, not shallower, narrower or truncated. (Maximum times of runs
made while other builds ran are omitted: they measure contention, not the policy.)

## Exchange by exact search (prototype `exchange.rs`)

| Variant | Points/game | Points/series | Cause |
|---|---:|---:|---|
| Return: 8 store candidates × 16 worlds, double-dummy contract outcome, store kept unless clearly better | +0.63 [−0.40, +1.65] | +4.95 [−3.14, +13.03] | changed 44 of 1,194 returns (+36 points per changed return), 1.5 s mean, 24.8 s max: too slow and too conservative at 16 worlds |
| Forth: the same search through the declarer's level-0 return | +0.22 [−0.81, +1.25] | +3.63 [−3.79, +11.06] | no measurable gain; a 0/1 open-hand outcome rarely separates passes |

Both stay off (prototype flags); a broader candidate set and a better-separating outcome
are the open questions (research proposal 5).

## Exact speedups (identical decisions)

| Change | Evidence | Effect |
|---|---|---|
| Solver: conserved card points, PVS, zero-point rank packing in the TT key, reach read from the same team's points | golden hash of 300 fixed evaluations unchanged; all solver tests | 64-world contract decisions: 5 cards 152k vs 200k nodes, 6 cards 950k vs 1.28M, 7 cards 3.63M vs 5.93M |
| Continuation store by consumed plan prefix | `perf --games 100` decision hash unchanged | outlook.route 7.8 s → 3.5 s |
| Endgame root filter searched first (`evaluate_moves`) | decision hash unchanged | small: few questions are filtered |
| Root dedup of publicly equivalent cards (rejected) | nodes 152,219 vs 152,406 | the TT already answers equivalent cards; not worth the code |

## Budgeted deep endgame (after the fix; includes the series raise and raise evidence)

| Variant (`ef04487`) | Points/game | Points/series | Bonus rate ours/theirs |
|---|---:|---:|---|
| 5 tricks, 1M nodes, ≥ 32 worlds | +4.89 [+2.65, +7.12] | +47.53 [+30.10, +64.96] | 0.124 / 0.090 |
| 6 tricks, 1M nodes, ≥ 32 worlds | +7.11 [+4.58, +9.65] | +61.44 [+42.15, +80.73] | 0.123 / 0.087 |
| 6 tricks, 2M nodes, ≥ 32 worlds | +8.70 [+6.01, +11.38] | +71.73 [+51.16, +92.30] | 0.126 / 0.088 |

Sweep at a one-million-node budget (`de13b71`, seed 1; identical decisions to `ef04487`):

| Variant | Points/game | Points/series |
|---|---:|---:|
| 6 tricks, ≥ 32 worlds, racing z = 2 | +7.76 [+5.20, +10.32] | +67.95 [+47.86, +88.05] |
| 6 tricks, ≥ 16 worlds | +9.03 [+6.36, +11.69] | +70.22 [+50.01, +90.43] |
| 6 tricks, ≥ 16 worlds, racing z = 2 | +9.00 [+6.32, +11.68] | +71.99 [+51.25, +92.73] |
| 7 tricks, ≥ 16 worlds, racing z = 1.5 | +8.03 [+5.23, +10.83] | +69.46 [+47.97, +90.94] |
| **7 tricks, ≥ 16 worlds, racing z = 2 (adopted)** | **+9.55 [+6.83, +12.27]** | **+77.50 [+56.28, +98.73]** |
| 7 tricks, ≥ 16 worlds, racing z = 3 | +8.85 [+6.14, +11.56] | +75.01 [+54.26, +95.75] |
| 7 tricks, 1.5M nodes, ≥ 16 worlds, racing z = 2 | +9.23 [+6.26, +12.20] | +78.56 [+55.46, +101.66] |
| 7 tricks, 2M nodes, ≥ 24 worlds, racing z = 2 | +10.36 [+7.43, +13.29] | +84.42 [+61.99, +106.84] |

The minimum of 16 worlds beats 32 (fewer fallbacks to the rules); racing adds about a point
at equal nodes; the differences between the 1M variants are within noise. Two million
nodes do not fit 100 ms.

Confirmation and refinements (16 workers pinned, idle machine):

| Variant | Seed | Points/game | Points/series | Max move ms |
|---|---|---:|---:|---:|
| adopted defaults (`1bceec7`) | 2 | +6.97 [+3.94, +10.00] | +63.52 [+40.53, +86.51] | 90.5 |
| same, `--deadlines wall` (`589c69c`) | 2 | +6.99 [+3.95, +10.03] | +64.60 [+41.65, +87.54] | 91.6 |
| + early stop when the budget cannot reach 16 worlds (`1b24987`, kept) | 1 | +8.43 [+5.79, +11.08] | +71.22 [+50.31, +92.14] | 90.7 |
| same | 2 | +7.53 [+4.60, +10.45] | +66.15 [+43.54, +88.75] | 95.6 |
| 800k nodes (rejected) | 1 | +6.91 [+4.31, +9.51] | +61.28 [+40.91, +81.65] | 87.7 |
| 800k nodes (rejected) | 2 | +6.61 [+3.75, +9.48] | +61.02 [+38.92, +83.12] | 91.0 |
| `hp1 base base base` (partner = current AI) | 2 | +3.04 [+0.60, +5.48] | +38.60 [+19.14, +58.06] | 116.3* |

The early stop is strength-neutral (two-seed mean +7.98 vs +8.26 points/game) but cuts
the moves above 80 ms from 126 to 33. Less budget costs strength without lowering the
maximum: the tail is the rules' fallback, not the search. *Raw work in a run without
deadlines; the live lane (deadlines wall) stayed at 91.6 ms.

## After checkpoint 2 (opponent `c2` unless stated)

| Variant | Seed | Points/game | Points/series | Cause |
|---|---|---:|---:|---|
| B-take on (`b_take = true`), owner's "reassess once the declarer play improves" | 1 | −1.63 [−5.11, +1.85] | +0.47 [−24.35, +25.30] | 3,193 vs 1,807 contracts, success 63% vs 65%: still overbids |
| same | 2 | −8.21 [−11.74, −4.69] | −45.02 [−72.73, −17.31] | rejected |
| Rollouts keep to 8 worlds after a spent endgame search (`194da89`, reverted) | 1 | −3.51 [−5.53, −1.48] | −20.59 [−36.21, −4.98] | the full rollout comparison decides many seven-trick positions the budget cannot finish |
| same | 2 | −4.37 [−6.44, −2.29] | −32.63 [−48.52, −16.73] | and the maximum time stayed (93 ms): the tail is the search, not the fallback |
| `target-cpu` x86-64-v2 / v3 (solver benchmark) | | no change within 5% | | the hot path is not bit-count bound |

## Series objective (HS-1) and raising

| Variant | Points/game | Points/series | Cause |
|---|---:|---:|---|
| HZ-1 weighs targets by the series worth (`b7b8d32`) | −1.03 [−2.30, +0.23] | +2.36 [−8.49, +13.22] | fewer made contracts (1,509 vs 1,530); bonus rate 0.113 vs 0.099 |
| + endgame reads the partner's raise (`f6ad71f`) | −0.86 [−2.12, +0.39] | +2.85 [−8.02, +13.73] | the raise evidence alone is about +0.2/game |
| HZ-1 by open-hand team points over 16 worlds (`raise_search`, rejected) | −30.10 [−34.51, −25.69] | −181.04 [−212.21, −149.87] | made 1,039 of 2,500: open hands overstate the declarer's points by far |

## Endgame objective (rejected)

| Variant | Points/game | Points/series | Cause |
|---|---:|---:|---|
| Defenders minimise P(contract made) first (lexicographic) | −3.53 [−4.55, −2.50] | −35.59 [−45.03, −26.14] | see below |
| Criteria weighted by score swing, with the defender criterion | −3.67 [−4.95, −2.40] | −33.05 [−43.86, −22.25] | same cause |
| Criteria weighted by score swing only | −0.69 [−1.58, +0.19] | −4.37 [−11.02, +2.28] | no measurable gain; not adopted |

Per-game regression explanation (598 changed games of the weighted + defender run):
in 514 games the base declarer failed in both runs, yet the new defenders took fewer
points (−4.2 points/game). They gave up real points for contract-breaking chances that
open-hand worlds overstate (strategy fusion: the declarer is credited with perfect
guessing). Breaking 34 more contracts gained +2.2 points/game; 12 contracts made that
were broken before cost −0.8. "Own points" stays the defenders' criterion.

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
| Killer moves per ply (rejected) | nodes +1% / +2% / −4% at 5 / 6 / 7 cards | ordering is already 95% first-move cutoffs mid-trick, 83% at trick starts |
| Transposition table 2^18–2^22 entries (rejected) | 3.63M → 3.52M nodes at 7 cards, slower per node | the table is not the bottleneck |

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
| Racing every 4 worlds instead of 8 | 1 / 2 | −1.05 / −1.23 | −5.88 / −7.87 | early eliminations on too few worlds; rejected |
| 96 endgame worlds | 1 / 2 | +0.19 / +0.47 | +1.07 / +2.16 | no measurable gain |
| 128 outlook worlds (vs `0e441f7`) | 1 / 2 | +0.00 / +2.03 | +3.36 / +17.14 | move max 132–181 ms: over the limit; rejected |
| Endgame from 8 tricks (vs `0e441f7`) | 1 / 2 | −0.03 / +0.15 | −0.14 / +1.34 | almost no eight-trick search finishes in the budget |

## Bidding thresholds (convention-preserving; opponent `c2`, or the frontier `dbc1345` where marked)

| Variant | Seeds | Points/game | Points/series | Note |
|---|---|---|---|---|
| B-quiet `take_140_min` 0.4 | 1 / 2 | −0.55 / −0.64 | −3.06 / −4.06 | takes more often: worse |
| 0.6 | 1 / 2 | +0.59 / +0.30 | +2.84 / +2.17 | |
| 0.7 | 1 / 2 / 4 | +0.96 / +0.45 / +1.50 | +4.43 / +2.99 / +9.36 | |
| **0.8 (adopted, `dbc1345`)** | 1 / 2 / 4 | **+1.28 / +0.29 / +1.90** | **+6.20 / +1.63 / +13.03** | passing lets a 125–135 message bid play |
| 0.9 | 1 / 4 | +1.02 / +2.15 | +3.31 / +14.44 | plateau |
| 0.5 × the series' break-even scale (HS-1, reverted) | 1 / 2 | −0.56 / −0.52 | −2.19 / −2.88 | the scale mostly lowers the threshold: like 0.4 |
| Partnership `dbc1345` + `c2` vs `c2` + `c2` | 1 / 2 | +0.61 / +0.03 | +2.46 / −0.34 | no partnership regression |
| B-over140 `over_140_cover` 0.65 (vs `dbc1345`) | 1 / 2 | −2.51 / −1.83 | −17.42 / −9.27 | crosses 140 more: worse |
| **0.85 (adopted, `c926961`; B-defend's reach kept at 0.75)** | 1 / 2 / 4 | **+2.18 / +0.71 / +1.61** | **+16.04 / +1.55 / +6.64** | identical with the reach also at 0.85 |
| 0.95 | 1 / 2 | +2.43 / +0.45 | +17.92 / +0.11 | plateau |
| Game-drive tie-break 0.6 / 0.4 (vs `dbc1345`, reverted) | 1 / 2 | −0.48 / +0.30; −0.49 / −0.78 | −4.16 / +3.13; −0.81 / −5.79 | neutral |
| HZ-1 estimate shift −5 / +5 / +10 (vs `dbc1345`, reverted) | 1 / 2 | +0.39 / −0.29; −0.97 / −0.55; −0.76 / −0.95 | | the human-fitted estimate needs no shift |

Both adopted thresholds deviate from an owner example at its stated threshold (the seed-42
140 take; batch 1c's over-140 step). The tests keep those examples at the owner's
thresholds and record that the defaults now pass or step instead.

## Exchange routes (opponent: the checkpoint-3 candidate `0e441f7`)

The return route (`return_run_route`, on) compared the store's choice only with returns that
keep a top-controlled run; the forth route (`pass_route`) was off. Both evaluate candidates
with the outlook's rollouts over the believed worlds, and the store's choice stands unless
another is clearly better. The exchange step's limit is 5 s; these used at most 0.26 s.

| Variant | Seeds | Points/game | Points/series | Max pass ms |
|---|---|---|---|---:|
| + the store's 4 best returns (`return_store_candidates`) | 1 / 2 | +2.27 / +2.39 | +17.96 / +17.68 | 17 |
| + 8 | 1 / 2 | +2.41 / +3.57 | +17.34 / +27.21 | 39 |
| + 16 | 1 / 2 | +2.90 / +3.71 | +19.11 / +26.99 | 66 |
| + 32 | 1 / 2 | +4.03 / +4.81 | +28.33 / +32.68 | 152 |
| + 8 and the forth route on | 1 / 2 | +3.65 / +4.87 | +26.23 / +35.90 | 88 |
| + 32 and the forth route on | 1 / 2 | +5.29 / +5.99 | +36.34 / +41.73 | 144 |
| + 32, forth route, + the store's 16 best passes forth | 1 / 2 | +5.36 / +6.11 | +37.72 / +43.83 | 167 |
| + 64 and the forth route on | 1 / 2 | +5.97 / +7.02 | +40.65 / +49.39 | 268 |
| + 128 and the forth route on | 1 / 2 | +6.28 / +7.20 | +42.86 / +49.93 | 489 |
| every legal return (715) and the forth route on | 1 | +6.21 | +42.95 | 3,178 |
| + 64, forth route, 128 exchange worlds (`exchange_worlds`) | 1 / 2 | +9.08 / +10.68 | +59.96 / +75.17 | 547 |
| + 64, forth route, 256 exchange worlds | 1 / 2 | +10.00 / +13.37 | +62.70 / +95.96 | 1,250 |
| **+ 64, forth route, 512 exchange worlds (adopted, `bb2f36c`)** | 1 / 2 | **+13.98 / +19.36** | **+96.99 / +139.62** | 2,109 |
| + 64, forth route, 768 exchange worlds | 1 | +13.59 | +100.11 | 4,386 |
| + 32, forth route, 1,024 exchange worlds | 1 | +14.06 | +100.32 | 10,252 |
| + 64, 512 exchange worlds, forth route off | 1 | +10.99 | +75.94 | n/a |

Candidates plateau near 64-128; the believed worlds behind the comparison pay up to 512, so
an exchange step (5 s) samples its own pool (`DecisionClass::worlds`). From 512 worlds the
rows are measured at `b8d4d8e`, which adds the review fixes (alone +0.42 / +0.71 points/game);
768 worlds or fewer candidates on more worlds gain nothing more and leave no headroom under
the 5 s limit (maxima from a loaded machine; n/a: the run overlapped release builds). The
forth route adds 3 points/game over 512 worlds.

Per-game attribution (`scripts/marja_bench/changes.py`: the adopted routes against the
review fixes alone, same opponent, deals and seats; only the declaring team exchanges):

| Change in the game | Seed 1: games, points/game | Seed 2: games, points/game |
|---|---|---|
| contract made, which the store's exchange failed | 186, +16.12 | 204, +18.24 |
| same result, more or fewer points | 753, +2.35 | 705, +3.42 |
| another final value (raise) | 507, +2.49 | 524, +2.98 |
| **contract failed, which the store's exchange made (regressions)** | **97, −7.33** | **80, −5.99** |
| total | 1,544 changed, +13.56 | 1,515 changed, +18.65 |

A changed game moves the series totals its series' later games start from, which the
series-aware raise reads: 171 / 173 games changed only their value without an exchange
difference (−0.34 / +0.28 points/game), and the 1 / 2 changed contracts of the opponents
have the same cause. Every other change is in a game the team declares.

The regressions are the price of overruling the store: the rollouts' level-0 play misjudges
some routes, and failing a contract instead of making it swings the margin by twice its
value. Of the other changed games 36% score lower too (528 of 1,447; 519 of 1,435), yet
every class but the failures nets positive on both seeds.

## Review (Opus, `0e441f7`)

Three must-fix findings (series end rules re-derived outside `engine::scoring`, HZ-1's
contract scores hand-coded, under-reported clock cuts in the endgame) and ten should-fix
findings (among them the endgame table outliving decisions, a lazily built 60 ms table, a
duplicated paired test, missing generator script, harness variant paths) are fixed in
`2a03437`; the solver speedups were independently confirmed exact (golden hash reproduced on
`afe2ebb`).

## The routes' guard (opponent `c3`; prototypes, not merged)

The review asked whether the 2-SE guard, applied after choosing the best of about 65
candidates on the same worlds, overrules on luck (winner's curse).

| Guard | Seeds | Points/game | Points/series |
|---|---|---|---|
| z = 1 (`exchange_route_z`, `1382947`) | 1 | +0.55 [−2.63, +3.72] | +3.54 [−20.18, +27.25] |
| z = 3 | 1 | −2.61 [−5.09, −0.12] | −20.84 [−40.36, −1.33] |
| choose on 512 worlds, confirm on 512 others (`exchange_confirm`, `dfc4fbf`) | 1 / 2 | −2.19 / −4.65 | −16.58 / −28.45 |

Every stricter test loses: the overrules it withholds gain more than the lucky ones cost.
The 2-SE guard stays; z = 1 is not measurably better.

## Review (Opus, `0e441f7..3100f48`)

No correctness defect in the exchange code. Fixed in `47712f9` (decision hash of 60 games
unchanged): the whole-game determinism test ran with deadlines and 512 exchange worlds, so
the clock could decide it; both routes' guard and decision have one owner
(`passing::overrule`); `changes.py` separates series carry-over and checks that runs are
paired; tests for the exchange class's worlds, candidate de-duplication and the default
player on the owner's pass examples; stale parameter and passing docs. Measured: the 2-SE
guard after choosing the best of about 65 candidates (winner's curse; above). Open: the
dormant `forth_store_candidates` (removed unless it pays).
Answered: P-route stays one rule for both exchange steps (its phase is table metadata;
splitting it would rename recorded rules); `planning_budget_ms` keeps its name (a
configuration key).

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

## After checkpoint 3 (opponent `c3`; prototypes on `proto/*` branches)

| Variant | Seeds | Points/game | Points/series | Cause |
|---|---|---|---|---|
| HO-3: follows by route like HO-1's leads (`outlook_follows`, `0cbdae2`) | 1 / 2 | −0.14 [−1.63, +1.35] / +1.29 [−0.19, +2.77] | +2.58 / +9.13 | not measurably better; raw moves up to 139 ms; not adopted |
| Racing in the exchange routes, z = 2 every 32 worlds (`exchange_race_z`, `65fed24`) | 1 / 2 | −0.43 [−1.34, +0.48] / −0.60 [−1.60, +0.39] | −5.76 / −5.77 | returns 4.3× faster (1.78 → 0.42 s mean), but drops some winners |
| The store's 16 / 64 best passes forth as further forth-route candidates (`forth_store_candidates`) | 1 | +0.03 [−1.31, +1.36] / −0.55 [−2.33, +1.23] | −1.78 / −7.87 | no gain over 512 worlds: the parameter is removed (`c3`'s default 0 kept) |
| HZ-1 by route (`route_raise`) | 1 | −3.73 [−6.87, −0.58] | −24.71 [−48.61, −0.81] | made 1,490 vs 1,637 contracts: still overraises with the deep endgame |
| Racing in the exchange routes, z = 3 | 1 | −0.27 [−0.68, +0.14] | −1.51 [−3.82, +0.81] | returns 2.3× faster (0.77 s mean); 60-game decision hash unchanged |


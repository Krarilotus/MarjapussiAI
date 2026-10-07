# Heuristic Player v1: tuning log

Every change of a default in `crates/heuristic/src/params.rs` is logged here (spec §8). Evidence is labelled per RULES.md R4. All runs: release build, CARGO_BUILD_JOBS=2, PC shared with other projects.

Commands:
- arena: `arena --deals N --opponent hp1|random [--params FILE]`;
- human data: `human_eval --games sources/marjapussi-rs/ml/dataset/games.ndjson [--params FILE]`;
- pass-read calibration: `cargo test --release -p marjapussi-tools --test selfplay c5_ -- --nocapture`.

## 2026-10-06, M1

### `egp_base` stays 60 (measured, not changed)
Coarse sweep; arena = HP1 vs HP1 on 500 duplicate deals; human = agreement on the 1,579 games.

| egp_base | first bid agrees | HB-7 fires (agrees) | self-play contracts made | pass games |
|---|---|---|---|---|
| 60 | 35.2 % | 3,259 (11.2 %) | 54.9 % | 25.0 % |
| 70 | 45.2 % | 2,222 (14.2 %) | 38.7 % | 11.0 % |
| 80 | 54.1 % | 1,233 (20.5 %) | 26.4 % | 2.1 % |
| 90 | 59.7 % | 408 (37.7 %) | 20.3 % | 0.1 % |

Reading: a higher base makes HP1 bid like the humans, but its contracts fail more often, because the same estimate also drives HB-8 (take the game) and raising. The humans send messages below 140 almost regardless of what they can make, so HB-7 gating messages below 140 looks like the misconception, not the base value. That is a rule question for M2 (`findings/heuristic-vs-humans.md`); the default stays at the spec's value.

### `pass_temperature` 4.0 → 1.0 and `forth_read_samples` 64 → 128 (measured)
VH's posterior P(MH void in a suit MH passed), averaged by the shape of the pass, vs. what HP1's MH really holds (1,500 self-play deals):

| setting | 3-1 | 2-2 | 2-1-1 | 1-1-1-1 |
|---|---|---|---|---|
| τ 4, 32 samples | 0.675 | 0.704 | 0.498 | 0.403 |
| τ 1, 32 samples | 0.741 | 0.779 | 0.580 | 0.488 |
| τ 1, 64 samples | 0.773 | 0.849 | 0.616 | 0.499 |
| τ 1, 128 samples | 0.817 | 0.891 | 0.622 | 0.501 |
| real (HP1's MH) | 0.847 | 0.927 | 0.645 | 0.506 |
| humans [A §5.3] | 0.868 | 0.921 | 0.637 | 0.382 |

After the HP-8 change below, with τ 1 and 128 samples: 0.837 / 0.902 / 0.627 / 0.501 vs. real 0.853 / 0.933 / 0.653 / 0.507.

Cost: VH's first decision after the pass forth computes 128 × 126 pass scores once per game: about 0.9 ms (64 samples: 0.5 ms). The mean over all decisions stays at 30 µs (64 samples: 19 µs).

### HP-8 counts Lusche minus tens and halves in a blanked suit (measured)
C4/C5: a blank made of small cards signals "blank in s"; a pass of 10/K only is negative evidence. With the Lusche bonus alone, a 10/K pass did not lower VH's P(MH blank in bells) (prior 0.12 → 0.13). Variants:

| HP-8 term per blanked suit | P(blank) after 10/K pass (prior) | pass forth agrees with humans |
|---|---|---|
| + Lusche | 0.13 (0.12) | 32.7 % |
| + Lusche − every non-Lusche | 0.07 (0.12) | 25.7 % |
| + Lusche − tens and halves (aces excluded: HP-4) | 0.00 (0.07) | 28.0 % |

Kept: the last row. The 4.7-point loss in human agreement against the first row goes to M2 as an owner question.

### Netspec (C12): trust θ mix and likelihood floor ε = 0.02 (measured)
`P_read = θ·P_conv + (1−θ)·P_base`, θ = 0.9. The floor ε is applied where it keeps information:
- **bids:** relative to the most likely hand class per observation (an absolute 0.02 is larger than the likelihood of most single bids and erased all information; HR-7 failed);
- **passes:** ε times the uniform choice probability (a floor relative to the best sample lifted ~100 of 128 inconsistent samples and broke the calibration: 3-1 fell to 0.60).

Calibration after the change (τ 1, 128 samples): 3-1 0.780, 2-2 0.828, 2-1-1 0.569, 1-1-1-1 0.446 vs. real 0.855 / 0.936 / 0.647 / 0.507 (before the θ mix: 0.837 / 0.902 / 0.627 / 0.501). Negative evidence: P(MH blank in bells) 0.07 → 0.01 after a 10/K pass, → 0.95 after a Lusche pass.

### New defaults without a sweep (reported)
`lead_void_lr` 0.5, `trump_lead_cost` 8, `take_margin` 10, `tell_window` 4, `ace_at_140_needs_pair` true, `generic_eps` 0.1, `cross_without_own_pair` 0.5, `generic_cross_no_pair` 0.2, `claim_140_prior` 0.3, `block_prior` 0.3, `block_base` 0.02. Each is set so its scenario test holds; M2/M3 fit them on the human data.

## 2026-10-06, performance (C14)

Guard: `perf --games 2000` plays 2,000 fixed-seed 4×HP1 games and hashes every decision (action, rule and the `why` facts). Every kept optimisation leaves both hashes unchanged: action+rule `ab1bd469bca3dc5d`, with `why` `9ce6ba438faa9e22` (89,234 decisions). The PC was at about 75 % CPU from other projects, so single runs vary up to 2×. The table is the best of 3 alternating runs per side (baseline = `165b8b8`, built in a temporary worktree), measured by Claude.

| phase | decisions | mean µs before → after | p99 µs before → after | max µs before → after |
|---|---|---|---|---|
| first bid | 8000 | 15.9 → 15.0 | 32.8 → 27.7 | 992.1 → 1223.9 |
| later bid | 1835 | 26.4 → 14.0 | 52.4 → 29.4 | 423.7 → 197.3 |
| pass forth | 1483 | 55.0 → 31.8 | 136.7 → 66.2 | 856.7 → 235.8 |
| pass back | 1483 | 838.5 → 479.0 | 1544.3 → 862.2 | 5113.5 → 3037.3 |
| raise | 1483 | 31.2 → 28.2 | 72.3 → 51.0 | 478.3 → 240.5 |
| trump or lead | 16000 | 5.5 → 4.4 | 30.4 → 22.8 | 364.3 → 169.5 |
| lead | 3491 | 10.7 → 5.4 | 38.1 → 19.0 | 606.7 → 128.5 |
| follow | 34672 | 14.9 → 12.2 | 204.2 → 162.9 | 1940.0 → 586.9 |
| all non-forced | | 32.1 → 21.3 | | |

Kept (all exact: the same float operations in the same order):
1. Card points and "higher cards in the suit" as tables computed once from the engine's `points_card` and `bits::takes`.
2. Pass options enumerated without allocating (`for_each_option`, same order), score buffers reused.
3. One scoring body per pass model with two sinks: the per-rule `Score` for the decision log, a bare `Total` for the readers' softmax.
4. Pass models keep per-card byte lanes of the integer counts (aces, halves, points, Lusche per suit, pair membership, cards per suit), so an option sums four lookups instead of about ten software bit counts (the default x86-64 target has no POPCNT; changing target flags is ruled out by R6).
5. Bid posteriors cached per seat as the running product prior × table₁ × … × tableₙ; a new bid multiplies in only its own table.

Measured but not kept (they change decisions; owner decision):

| `forth_read_samples` | pass back mean µs | mean µs, all | games with any different decision (of 2,000) | VH void calibration 3-1 / 2-2 (real 0.85 / 0.94) |
|---|---|---|---|---|
| 128 (default) | 545 | 24.8 | – | 0.78 / 0.83 |
| 64 | 313 | 18.2 | 363 | 0.77 / 0.85 (τ 1, before the θ mix) |
| 32 | 207 | 15.7 | 530 | 0.74 / 0.78 (τ 1, before the θ mix) |

Where the time goes now (micro-benchmark at VH's pass-back decision): 128 × 126 pass scores about 13 ns each, 16k `exp` about 4 ns each, the rest is the cold-cache cost after each engine step (a play decision costs about 3 µs warm and about 12 µs right after the engine applied the previous action). Not done: a factorised (per-suit) partition function for the pass softmax; it is exact in maths but not bit-identical, about 5× faster on the read.

## 2026-10-06, EGP calibration (Judge M1 item 2)

EGP is now a linear model of eight features (`hand::egp_features`): a constant, the team's known aces, tens and halves, my longest suit, my void suits, the expected meld value, and P(partner holds an ace). The weights are fitted by least squares on the realised team points (cards, melds, last trick) of the 1,575 human declarers, with the features taken from HP1's view at the declarer's last bid (`human_study`, measured by Claude).

| feature | old weight | fitted weight |
|---|---|---|
| constant | 60.0 | 45.8 |
| team aces | 7.0 | 20.0 |
| team tens | 3.0 | 6.2 |
| team halves | – | 4.2 |
| my longest suit | – | 1.1 |
| my void suits | – | 1.3 |
| expected melds | 1.0 | 1.0 |
| P(partner ace) | 10.0 | 27.5 |

Mean absolute error: old 55.8, fitted 47.9 (5-fold cross-validated), predicting the mean 53.9. The old EGP was 25–40 points too low in every bucket; the fitted one is calibrated within its intervals (tables in `findings/heuristic-vs-humans.md`).

The fitted EGP describes what *humans* make. HP1's own card play makes less, so the margins on its own ambitions (HB-8, HZ-2) were swept on 2,000 self-play games and the human data:

| take_margin | raise_margin | all-pass | made | value | raised | first bid agrees | raise agrees |
|---|---|---|---|---|---|---|---|
| 10 | 10 | 0.0 % | 19.7 % | 202 | 78.6 % | 60.2 % | 18.2 % |
| 30 | 50 | 0.0 % | 43.5 % | 172 | 64.4 % | 60.8 % | 27.3 % |
| 30 | 70 | 0.0 % | 53.5 % | 161 | 47.9 % | 60.8 % | 33.4 % |
| **30** | **80** | 0.0 % | 56.9 % | 157 | 38.1 % | 60.8 % | 35.8 % |
| 30 | 90 | 0.0 % | 59.0 % | 154 | 28.2 % | 60.8 % | 38.5 % |
| humans | | 0.1 % | 65.8 % | 166 | 38.3 % | | |

Kept: `take_margin` 30, `raise_margin` 80 (the human raise rate). Before the calibration: all-pass 25.3 %, made 53.5 %, first bid agrees 35.1 %, raise agrees 26.9 %.

## 2026-10-06, M3: bidding strength (Judge J17 item 1)

**Finding (Judge):** HP1 with the M2 defaults lost to HP1 with the M1 defaults in duplicate play: −49.2 ± 14.9 (`dev`) and −58.8 ± 14.6 (`check`) points per deal pair, 1,000 deals each (reproduced exactly). M2 declared 71.5 % of games and made 50.1 %; M1 declared 26.4 % and made 60.4 %.

**Cause:** the M2 EGP was fitted on what *humans* make, and the margins were chosen by agreement and symmetric self-play, where both sides overbid equally. HP1's own play makes 20–30 points less than the human fit says (130–164 bucket: predicted 155, realised 125 ± 3).

**Fix, step 1: fit the EGP that HP1 acts on, on HP1's own declarers** (`arena --fit-egp`, 5,000 self-play games, features at the declarer's last auction decision, target its team's realised points):

| feature | M2 (human fit) | fit, M2 play (n = 5,000) | **fit, M1 play (n = 3,735), new default** |
|---|---|---|---|
| constant | 45.8 | −27.9 | **−15.6** |
| team aces | 20.0 | 19.7 | **16.9** |
| team tens | 6.2 | 6.3 | **6.2** |
| team halves | 4.2 | 14.6 | **10.5** |
| my longest suit | 1.1 | 5.7 | **6.3** |
| my void suits | 1.3 | 2.4 | **−0.5** |
| expected melds | 1.0 | 1.1 | **1.2** |
| P(partner ace) | 27.5 | 29.2 | **9.8** |

Cross-validated mean absolute error 40.4 (M1 play) and 43.7 (M2 play); both are calibrated within their intervals in every bucket. The two fits tie head to head (−0.9 ± 3.9); a refit on the new policy's own declarers changes nothing (−0.8 ± 5.0). The M1-play fit is kept, since its declarers cover more kinds of hand.

**Fix, step 2: margins by duplicate strength against M1's defaults** (`dev`, 1,000 deals; points per deal pair, 95 % CI):

| bid_overreach | raise_margin 50 | 80 | 110 | 140 | 200 |
|---|---|---|---|---|---|
| 0 | −7.4 ± 9.7 | +35.5 ± 9.3 | +46.8 ± 9.9 | | |
| −20 | +14.5 ± 10.4 | +47.3 ± 9.9 | **+55.5 ± 10.6** | +54.1 ± 10.7 | +53.5 ± 10.8 |
| −40 | +32.2 ± 11.5 | +52.2 ± 11.2 | +57.4 ± 11.7 | +55.9 ± 11.7 | +55.6 ± 11.8 |
| −60 | | | +56.0 ± 12.2 | +54.6 ± 12.2 | +54.4 ± 12.2 |

(`take_margin` 30; 0 and 15 were worse by 5–12 points at every raise margin.) Against M1 the results plateau from −20 to −60, so the plateau was settled head to head: −20 beats −40 by +5.7 ± 4.1 and −60 by +12.4 ± 5.1; it ties −10 (+0.2 ± 4.1) and beats 0 (+7.1 ± 6.5). At −20, raise margin 110 beats 80 (+9.1 ± 4.6) and ties 140 (+1.6 ± 1.8); take margin 30 ties 10 and 50.

Why such a large raise margin: the realised points spread about ±50 around the estimate, and a raise to T is worth (2 P(points ≥ T) − 1) × T. That is largest well below the mean: for a mean of 200 it peaks near T = 130.

**New defaults:** the M1-play EGP fit, `bid_overreach` −20, `take_margin` 30, `raise_margin` 110. Verified on both suites:

| new default vs | `dev` (seed 1) | `check` (seed 2) |
|---|---|---|
| M1 defaults | +55.5 ± 10.6 | +57.6 ± 10.3 |
| M2 defaults | +88.3 ± 14.1 | +97.5 ± 14.0 |

The new HP1 declares 25 % of games and makes 86–88 % of them (M1 opponents make 53 %).

## 2026-10-06, M3: trump plan (P8, P10, P10a) replaces HQ-0

Duplicate A/B, new default vs the previous default (`meld_stopgap = true`, i.e. HQ-0), 1,000 deals: **+38.7 ± 7.6** (`dev`) and **+38.0 ± 7.4** (`check`) points per deal pair. HP1 made 87.1 % / 89.5 % of its contracts against 81.0 % / 82.7 % with HQ-0 on the same deals; 0.80 questions per game per team against 0.17.

Self-play (5,000 games): 1.6 questions per game (humans 2.1; HQ-0 gave 0.3), 1.2 melds per game (humans 1.2). P10a metric: 0 pairs held at a lead with a spent `Own` right, so 0 pairs lost through spent rights. New params: `partner_pair_min` 0.1, `partner_lead_conf` 0.7 (not swept).

## 2026-10-06, M3: P2 (information first), P3 (seat kinds), P12

Duplicate A/B against the previous default (the trump-plan commit), 1,000 deals, points per deal pair:

| change | `dev` | `check` | kept |
|---|---|---|---|
| P2 HB-9 at `accidental_win_risk` 0.1 + P12 `conf_two_halves` 0.8 + human Red as +15 | −0.7 ± 1.2 | −0.2 ± 1.7 | yes (neutral; changes the reading of humans) |
| P3 reading HP1 seats with HP1's own policy (`read_hp1_as_hp1`) | −1.1 ± 5.4 | −7.7 ± 5.6 | **no**: off |
| the same after refitting EGP under that reading | −5.7 ± 8.2 | −7.5 ± 8.1 | no |

P2 with the convention model predicting every seat's pass (before P3's HP1 pass model) lost heavily: −42.5 ± 11.9 at risk 0.1, −62.2 ± 12.4 at 0.5 (`dev`). HP1 then declared 68.5 % of games (433 at 120) and made 61.4 %: the convention model expects other seats to bid on with any ace, while HP1 seats pass. With HP1's own pass model the risk behind HP1 seats is high, and HB-9 never fires at an all-HP1 table (A/B +0.0 ± 0.0 at risk 0.05–0.3).

Consequence: in HP1 self-play 47 % of games are still all-pass (humans: 0.1 %), since passing weak hands is what wins against HP1 (J17 step 2: the −20 auction gate beats 0 and −10 head to head). Owner question in the M3 report.

## 2026-10-06, C21: belief calibration on the human games (Search Coder S5)

`human_eval --calibration`: at every human decision, the acting seat's HP1 beliefs about each other seat's **current** hand, scored against the true hands wherever the hard facts leave the feature open (has an ace; a pair in suit s; a half of suit s). 1,577 games.

**Where the overconfidence S5 saw comes from:** not the bid model. In the auction and the exchange the beliefs are calibrated (ECE 0.02–0.08). In card play they are not, because the seat features describe the *starting* hand (the bid classes) and were only clipped by hard bounds.

**Tempering the bid likelihoods (S5's proposal) does not help:** auction Brier for aces 0.1434 (power 1) / 0.1473 (0.75) / 0.1566 (0.5); pairs 0.0798 / 0.0812 / 0.0843; halves 0.2311 / 0.2308 / 0.2312. Not adopted.

**Conditioning on the seat's own plays** (`condition_on_plays`, new default): after an ace, whether another ace is left comes from the card marginals; after one half, P(the other half) = P(pair) / P(that half). Card play, Brier (ECE):

| feature | relation | before | after |
|---|---|---|---|
| has an ace | opponents | 0.2705 (0.211) | **0.1966 (0.096)** |
| has an ace | partner | 0.1923 (0.124) | **0.1817 (0.089)** |
| a half of suit s | opponents | 0.2507 (0.075) | 0.2453 (0.078) |
| a half of suit s | partner | 0.2274 (0.079) | 0.2228 (0.080) |
| pair in suit s | opponents | 0.1485 (0.110) | 0.1485 (0.110) |
| pair in suit s | partner | 0.1134 (0.084) | 0.1134 (0.084) |

Duplicate A/B against off: +0.0 ± 0.0 on both suites (no decision changed: play decisions use the card marginals).

**Left:** pairs in card play (forecast 0.012 → observed 0.111 in the lowest bin; 0.96 → 0.50 in the highest). These are pairs a seat received or gave away in the exchange it did not show: the hidden exchange (S5 point 4, proposal P2). Modelling it is agent B's hybrid resampling or a pass-forth model for the hidden exchange; not done in M3.

**Public reading API** for the study (C21): `heuristic::{bid_tables, N_CLASSES, PassReading, back_ctx, forth_ctx, revealed, Revealed}`. `PassReading::of(history, revealed, me, params)` is the pass a seat reads, and `PassReading::likelihood(start, params, scratch)` is P(the observed pass | the giver's start hand), the same function HP1's own pass read samples with (one owner of the trust mix and the floor). The study no longer needs `for_each_option`, `softmax_prob` or its own copy of the likelihood.

## 2026-10-06, M3: planned leads (Judge item 5, P9)

`leads.rs`; each lead A/B'd against the same HP1 with that lead off (1,000 deals, points per deal pair):

| lead | fires (dev) | `dev` | `check` | default |
|---|---|---|---|---|
| HL-6 keep the top trump for the last trick | 175 | +1.6 ± 1.5 | +1.6 ± 1.5 | on |
| HL-3 lead a suit the partner is certainly void in (it trumps) | 500 | +1.7 ± 2.1 | +2.5 ± 2.6 | on |
| HL-5 lead A/10 into an opponent's likely pair | 5 | −0.1 ± 0.1 | −0.4 ± 0.8 | on (rare) |
| HL-4 lead the top trump to draw theirs | 440 | −2.0 ± 3.2 | −0.3 ± 3.3 | **off** |
| HL-1 schnippeln (partner certainly holds the ace) | 0 | +0.0 | +0.0 | on (needs HK-7) |

All four on together vs none: **+2.6 ± 2.3** (`dev`), **+3.6 ± 3.1** (`check`).

P9's pass-back half (HK-7: give my few aces back, keeping a low card of the ace's suit, so HL-1 can schnippeln): `w_schnippeln` 15 / 30 / 60 with at most 1 ace: −1.5 ± 2.5 / −3.2 ± 2.8 / −5.1 ± 3.6 (`dev`); with at most 2 aces down to −20.9 ± 7.1. Why it loses: VH, the declarer, gives away its sure leads, and P8 needs them to announce. Restricted to hands without an own pair it fires 8–10 times per 2,000 games and changes nothing. Off (`w_schnippeln` 0); owner question in the M3 report.

## 2026-10-06, P13 (passing speed): the byte-identical parts do not pay (measured, not adopted)

Where a pass-back decision spends its time (`perf --micro`, 299 decisions): 376 µs in total; the cold belief computation 305 µs, of which VH's read of MH's pass forth is 128 sampled hands × 126 options: 150 µs scoring (10 µs of it building the 128 models), 46 µs `exp`, 20 µs enumeration. HP1's own 715-option pass back is about 70 µs.

Card equivalence (P13 item 2), implemented byte-identically: an option's score depends only on the sums of its cards' lane words, so each distinct sum is scored once and each distinct total gets one `exp`, summed in the original order. Decisions identical on 2,000 games (hash 9276529cf0f7c13c). 46 % of the options share a sum with an earlier one. Yet it is **slower**: interleaved against the base, 608–677 µs vs 469–585 µs per pass back (three runs of 400 games each; the PC is shared). The table probe per option costs more than the ~9 ns score it saves. Reverted.

Per-suit decomposition (item 3) only helps the acting search (the 70 µs), not the reader, whose softmax needs every option's score. The cost is the 128-sample read: the gain needs item 1 (exact marginals instead of samples), which changes the beliefs. On hold with the belief design (orchestrator, 2026-10-06).

## 2026-10-06, evaluation additions (C15, Judge S4, S8, S10)

- **Baseline (S10):** HP1 against HP1's own auction with random card play (`--opponent hp1-random-play`, `dev`, sequential): **+235.6 ± 39.2** per deal pair, stopped after 100 deals (confidence sequence [+114.0, +357.2]); the random-play side made 2 of its 52 contracts. Against pure random play: +1,101.0 ± 17.7.
- **Intention detectors** on 2,000 self-play games (play as of the held C25 WIP): 563 standing A/10 thrown onto the partner's trick; 0 halves of a needed pair led while a question was legal; 0 pairs lost through spent rights; 97.9 pair points per game.
- **Time split (S4)**, `human_eval --until 2024-12` (1,402 games) vs `--since 2025-01` (175 games), HP1 agrees with the human: first bid 54.5 % vs 54.3 %, later bid 67.0 % vs 63.9 %, raise 43.9 % vs 42.3 %, lead 35.6 % vs 37.4 %, follow 63.7 % vs 65.4 %, pass back 9.6 % vs 15.4 %, pass forth **29.1 % [26.8, 31.5] vs 19.4 % [14.2, 25.9]**. Out of sample agreement holds except the pass forth.
- **p99 guard (S8):** the ignored speed test asserts p99 ≤ 2 ms; measured best of 5: mean 14.1 µs, p99 293.7 µs.

## 2026-10-06, C28 (= C25): one valuation on `beliefs`, the score as unit, the endgame solver

A/B scaffolding: the previous heuristic built out of the repo as a second crate (never committed), both agents in one binary, 1,000 duplicate deals per suite, points per deal pair.

| step | vs | `dev` | `check` |
|---|---|---|---|
| 1. HP1 reads `beliefs::Mind` (copies deleted) | 045ac0f | −3.5 ± 6.9 | −4.0 ± 6.5 |
| 2. valuation on `future_win`; six invented rules deleted | 045ac0f | −0.6 ± 7.3 | −3.6 ± 7.0 |
| 3. endgame solver at ≤ 4 tricks (64 worlds) | 045ac0f | +13.6 ± 7.6 | +10.1 ± 7.5 |
| 4. the score as unit (`stakes.rs`, `engine::game_scores`) | 045ac0f | +19.5 ± 7.5 | +11.9 ± 7.6 |

**Leads (Judge point 2), HP1 against itself with leads switched off:**

| opponent | `dev` | `check` | decision |
|---|---|---|---|
| no lead rules (pure `value_of_play` leads) | +27.2 ± 8.3 | +28.5 ± 7.9 | |
| HL-2 off | +27.7 ± 8.2 | +27.9 ± 7.7 | **HL-2 stays** |
| planned leads off (HL-1, 3–6) | +0.6 ± 2.3 | +0.8 ± 1.9 | neutral, kept (owner-sourced); candidates to fold in |

Why HL-2 wins: `future_win` is taken under the current trump. Cashing a standing side card before any trump exists prices what the valuation cannot see, that a later announcement lets a void opponent ruff it (HC-4's reason).

**v1 acceptance against 702b535 on independent seeds:**
- arena: **+19.6 ± 8.2** (`holdout`), **+21.7 ± 7.4** (`seed4242`); contracts made 87.0 % vs 77.0 % (`holdout`);
- mistake meter (`tools/audit`, `holdout`, 2,000 games, ≤ 5 tricks, the same judge for both): **1.37 → 0.62 points lost per decision**, 17,233 → 7,968 per 1,000 games. By family (points lost per 1,000 games): lead 8,410 → 1,644; follow and give 5,251 → 4,182; pairs and announcing 3,574 → 1,700; the endgame solver's own decisions 443. By tricks left: 2: 0.64 → 0.06, 3: 1.63 → 0.34, 4: 1.67 → 0.60, 5: 1.51 → 1.46 (the valuation alone, about equal);
- detectors per 1,000 games: standing A/10 onto the partner's trick 297 → 294 (flat); K/O of an own pair played voluntarily 31.5 → 38.0 (a declarer's meld is worth nothing once the contract is made or out of reach); needed halves led while a question was legal 0 → 0;
- the owner's demo cases (C22, C24) and the HC-1 owner examples A, B, C pass as tests.

**All-pass rate (reported, not fixed; orchestrator):** 45.2 % of 4×HP1 games (`holdout`, 2,000), 46.2 % of the A/B games; humans 0.1 %. In those games 3,001 of the 3,620 first bidding actions are HB-7 (never bid above EGP − 20), 404 HB-0 and 215 HB-6. The best seat's EGP there is a median 126 (90th percentile 140): opening at 120 needs EGP ≥ 140 under the −20 gate, chosen in J17 because passing weak hands beat HP1 head to head.

**Cost:** decisions take 568 µs mean, 9.5 ms p99 (702b535: 16 µs, 0.27 ms): the `Mind` recomputation per epoch (up to ~67 ms in follow decisions) and the solver.

## 2026-10-06, C32: contract restore (no learned values; information only, nothing tuned from it)

Restored as in the auditor's LEARNED list (`findings/audit/rules-and-params.md` §0) and checked against `findings/owner-chat-verbatim.md`: the EGP (M1 hand-set), `bid_overreach` 0, `take_margin` 10, `raise_margin` 10, `egp_sigma` 23.3 (the rules' fair share), card factors 1, uniform exchange kernel, no calibration, `trust_agent` 0.9, `human_red_as_big` 0.

For information (1,000 duplicate deals against 702b535): −17.1 ± 12.9 (`holdout`), −0.9 ± 12.4 (`seed4242`). HP1 now declares 48–49 % of the games and makes 65–69 % (702b535: 24 %, 78–80 %). All passed: 27 % of the A/B games; 12.9 % of 4×HP1 games (C28: 45.2 %), there mostly HB-7 (863 of 1,036 first actions) with the best seat's EGP a median 121. Mistake meter (`holdout`, 2,000 games): 0.56 points lost per decision (C28: 0.62); the solver decided 15,734 of 26,559 measured decisions. Standing A/10 thrown onto the partner's trick: 346.5 per 1,000 games (C28: 294).

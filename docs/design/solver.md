# Design: `solver` (layer 4), v0

Search Coder, 2026-10-06. Brief: `agents/coder-search.md` (phase 2). Sources: Investigator B §2 (`findings/ideas/B-search-solver-learning.md`), RQ-10, `docs/design/fast-core.md`.

## What it owns
- **`Solver`**: exact card play in one full-information world, on `engine::fast`.
- **`evaluate`**: a seat's moves judged over worlds sampled from its knowledge (perfect-information Monte Carlo with exact leaves).

Rules, points and legal moves come only from `engine::fast` / `game::points`. Worlds come only from `knowledge::DealSpace`.

## Objective and invariants
- `value(s, team)`: the exact **future** points of `team` (seat % 2) from `s`. These are card points, the last-trick bonus and `team`'s own announced pairs. `team` maximises them; the other team minimises them.
  - This is the zero-sum reading of "can the declarers reach X": the other team's own pairs count only through the trump they set.
  - Totals are `s.points(team) + value`.
- `reaches(s, team, target)`: `value >= target`, decided by one zero-window probe. For a contract: `target = game value − s.points(team)`.
- `move_values(s, team)`: per legal move, its immediate gain plus the value after it.
- `tricks(s, team)` and `reaches_tricks(s, team, target)`: the same search on tricks instead of points (`team` maximises its tricks). Schwarz: `reaches_tricks(s, team, 1)`. The TT key carries the measure.
- Moves are those of `FastState::moves`: cards, and announcements by the leader at a trick start (`announcements()`). Questions are not solver moves; they carry information and stay at the belief level.

## Search
- **Fail-soft alpha-beta** on future points. `value` uses one full window `(-1, max + 1)`; `reaches` uses `(target − 1, target)`.
- **Transposition table at trick starts:** bounds `lo ≤ future ≤ hi` and the best move.
  - Key: the four hands, leader, trump, called suits, trump rights, `announcements()` and the team.
  - Points so far are path data and stay out. The open trick start matters only through the announcements it allows now, so `announcements()` stands in for it.
  - Fixed size (default 2^18 × 40 B = 10 MB), direct-mapped, always replace; a stored entry tightens existing bounds for the same key.
- **Static bound:** future ≤ the card points still in play + 20 + every pair a team seat may still announce. A node whose bound is ≤ α returns at once.
- **Safe equivalence:** card `c` is skipped if the next lower **live** card of its suit (in a hand or in the current trick) is the mover's own and scores the same.
  - No live card lies between them, so they compare alike with every other card, now and later; their points and pair meaning (K, O, A score uniquely) are equal.
  - Trick cards count as separators. In practice this merges runs of 6–9 and cards whose in-between cards were played.
- **Ordering:**
  - announcements first (high pair first), then the TT move;
  - leading: high cards first;
  - following: winning cards (opponent winning), then points onto a partner's trick, then overtaking the partner, then the lowest dump.

## World evaluation (`evaluate`)
- Inputs:
  - the seat's `Knowledge`;
  - a `Table`, the public part of the card-play state (`FastParts` without hands);
  - an `Objective`;
  - the number of worlds, a `Belief` and an RNG.
- **No hidden hands go in by construction.**
- **Worlds:**
  - if at most `worlds` deals are consistent, every deal is solved once (`DealSpace::for_each_deal`; `Decision::exact`, independent of the RNG);
  - otherwise `worlds` `DealSpace` samples (exactly uniform).
  - Each world is weighted by the belief (importance weights; the effective sample size is reported).
- Each world becomes a `FastState` (`from_parts`), and every legal move is solved exactly in it. The mover's legal moves are the same in every world (the mover knows its hand).
- **Criteria:** a `Decision` lists criteria (a `Measure` and a direction) and, per move, the weighted mean of each. `best()` is lexicographic; means within 1e-9 tie and pass to the next criterion. Measures:
  - `Points { team }`: final points, `team` maximising and the other team minimising;
  - `ReachPoints { team, target }`: 1 if `team` can end with ≥ `target` points;
  - `ReachTricks { team, target }`: 1 if `team` can end with ≥ `target` tricks.
- **Objectives:**
  - `Points`: the deciding team's points;
  - `Reach { team, target }`: maximised by `team`, minimised by the other team;
  - `Contract { playing_team, game_value }` follows HC-9 (queue S4), below.

### `Contract` (HC-9)
| Seat | Situation | Criteria, most important first |
|---|---|---|
| declarer | contract made (points ≥ value) | defenders' points, minimise |
| declarer | open | P(contract made), maximise; then own points, maximise |
| declarer | open, but P = 0 after every move in every world (loss fixed) | defenders' points, minimise (second pass over the same worlds) |
| defender | own team has no trick and the scoring punishes that | P(at least one trick), maximise; then own points, maximise |
| defender | otherwise | own points, maximise |

- **Schwarz is read from the scoring, not hard-coded:** `trickless_costs(team)` asks `game::scoring::game_scores` whether `team` scores less with 0 tricks than with 1, all else equal, in a made or lost contract.
  - Since the site scoring (engine E4, owner decision D2) this holds for both sides: a party played schwarz loses 2A, the declarers included. Trickless declarers therefore get the trick criterion first; no solver change was needed.
  - If the owner changes the declarers' schwarz rule in `game::scoring`, declarers without a trick get the trick criterion first, with no change here.
- No contract (all passed): the caller uses `Points`.

## Guards (`crates/solver/tests`)
| Test | What | Result |
|---|---|---|
| `exact::solver_equals_brute_force` | `value`, `reaches` (5 targets) and `move_values` against plain minimax, both teams | 2,300 positions with 1–4 tricks left, all trick fillings, random trump/called/rights/open start; 217 with an announceable pair; a 2^10 table reused across positions (collisions) |
| `exact::transposition_key_separates_every_public_field` (J15) | pairs of positions with identical hands differing in one key field (trump, called suits, trump rights, open trick start, team), solved through one shared solver in both orders, against brute force; cases drawn per field until 40 pairs differ in value | removing any key field fails it (Judge mutations M5–M9, checked) |
| `exact::shared_solver_is_exact_over_random_public_variants` | 60 hands × 20 random public variants through one solver (2^12 table), points and tricks, against brute force | — |
| `exact::solver_equals_engine_minimax` | final points against minimax over the engine's `Game` (card plays and announcements, scored by `GameFinishedInfo`) | 150 positions from random games (bidding, passing, questions), 1–3 tricks left |
| `worlds::evaluation_runs_…` | `evaluate` (Points and Reach) at every decision with ≤ 4 tricks left in 120 random games; equal to the exact move values when one deal is consistent | 2,232 decisions, 426 with a known deal |
| `worlds::hidden_cards_do_not_change_the_decision` | swap two hidden cards the seat cannot see, replay; if all public events are equal, `Table` and decision (same seed) are equal | 150 swaps |
| `worlds::belief_weights_…` | the effective sample size falls under a non-uniform belief; all-zero weights are an error | — |
| `exact::solver_equals_brute_force` (tricks) | `tricks` and `reaches_tricks` (3 targets) against plain minimax on tricks, both teams | same 2,300 positions |
| `worlds::small_deal_spaces_are_solved_deal_by_deal` | with 2–40 consistent deals: `exact`, worlds = count, the same decision for two RNG seeds, means equal to the plain average over `for_each_deal` | 1,210 decisions |
| `worlds::contract_objective_follows_hc9` | every decision with ≤ 4 tricks left in 60 random games, either team declaring, contracts made / open / out of reach: the criteria match the table above. The fallback is checked by a `Reach` evaluation on the same worlds (all zero). With a known deal, each mean equals the exact measure | made 1,110, open 1,342, lost 1,988, defenders without a trick 104, with a trick 4,336; 1,688 known deals |
| `worlds` unit tests | schwarz read from `game_scores`; lexicographic `best` with ties | — |
| `knowledge` `counts_and_marginals_match_enumeration` | `for_each_deal` equals brute-force enumeration (as a set, no repeats) | 16,537 states with ≤ 1,000 deals |

## Speed (release build, PC shared with other projects)
**T(k)**, one exact `value` per position. The positions are trick starts after random play from a random deal, with announcements taken half the time; k = 9 is the first trick:

| k | positions | mean ms | median ms | max ms | mean nodes | max nodes |
|---|---|---|---|---|---|---|
| 1 | 5,000 | 0.000 | 0.000 | 0.03 | 5 | 5 |
| 2 | 5,000 | 0.002 | 0.002 | 0.36 | 24 | 77 |
| 3 | 3,000 | 0.010 | 0.008 | 0.06 | 124 | 659 |
| 4 | 2,000 | 0.048 | 0.038 | 0.34 | 637 | 4,894 |
| 5 | 1,000 | 0.279 | 0.191 | 2.82 | 3,751 | 37,196 |
| 6 | 300 | 1.66 | 1.06 | 11.1 | 23,292 | 169,165 |
| 7 | 100 | 6.8 | 4.0 | 53 | 92,674 | 708,558 |
| 8 | 60 | 82 | 23 | 1,698 | 861,711 | 16.1 M |
| 9 | 40 | 276 | 86 | 5,343 | 3.37 M | 63.0 M |

For comparison, Investigator B's prototype (no announcements) measured 0.067 ms / 800 nodes at k = 4 and 1.6–2.3 ms at k = 6. From k = 8 on, the mean is driven by a heavy tail (see open points).

**Time per decision** (`evaluate`), 20 decisions from random games per k. The seat to move holds k cards, at a random position in the trick. Small deal spaces are enumerated (since S4).
- Contract: an open contract 30 points ahead, declared by either team in turn.
- The PC was busier than in the phase-2 run: Points at k = 6 / 100 worlds measured 101 ms then, 139 ms now, with identical search code.

| k | mean legal moves | worlds | Points: mean ms | Points: max ms | Reach: mean ms | Contract: mean ms | Contract: max ms |
|---|---|---|---|---|---|---|---|
| 4 | 2.2 | 50 | 1.3 | 7.0 | 0.5 | 0.9 | 4.3 |
| 4 | 2.2 | 100 | 2.5 | 25.3 | 0.7 | 2.9 | 20.4 |
| 4 | 2.2 | 200 | 2.3 | 11.5 | 1.1 | 2.5 | 11.7 |
| 5 | 2.9 | 50 | 7.2 | 29.2 | 3.2 | 8.9 | 34.9 |
| 5 | 2.9 | 100 | 22.9 | 102 | 8.8 | 26.1 | 83.5 |
| 5 | 2.9 | 200 | 42.5 | 162 | 15.4 | 41.6 | 164 |
| 6 | 2.6 | 50 | 58.8 | 262 | 16.0 | 111 | 506 |
| 6 | 2.6 | 100 | 139 | 537 | 37.0 | 202 | 1,003 |
| 6 | 2.6 | 200 | 278 | 1,265 | 45.1 | 231 | 1,115 |

Against the 100 ms budget (S4): k ≤ 5 fits 100–200 worlds on average; k = 6 fits about 50 on average but not in the worst case. The worst cases are single decisions with heavy positions; agent B needs a time check, not only a world count.

## Optimisation log (R3)
| Idea | Measurement | Kept? |
|---|---|---|
| Safe equivalence | k = 7: 169,485 → 92,674 nodes, 12.8 → 7.3 ms; k = 4: 858 → 637 nodes | kept |
| Move ordering | k = 7: 153,422 → 92,674 nodes, 11.1 → 7.3 ms; k = 4: 789 → 637 | kept |
| Static upper bound | k = 7: 109,924 → 92,674 nodes, 8.2 → 7.3 ms; k = 4: 806 → 637 | kept |
| MTD(f) over zero-window probes for `value` (first guess: half the bound) vs one full window, each with its own table | k = 4: 0.078 vs 0.048 ms; k = 6: 2.2 vs 1.75 ms; k = 7–8 level; k = 9 noisy (68 vs 453 ms on 10 positions, 344 vs 292 ms on 40) | rejected: full window |
| Larger table for deep solves (30 positions, k = 9) | 2^18: 149 ms, 2^20: 136 ms, 2^22: 136 ms | default stays 2^18 (10 MB) |

## Open points
1. **Tail at k ≥ 8:** single positions need 16–63 M nodes, and a larger table does not help. Candidates:
   - a TT key normalised by relative rank (B §2.3);
   - a two-entry bucket keeping the deeper entry;
   - killer moves.

   These matter only if search goes beyond k = 6–7.
2. **Declarers and the defenders' schwarz:** HC-9 has declarers minimise the defenders' points once the contract is made, which pushes towards schwarz without aiming for it. A criterion "P(defenders take no trick)" for declarers is not in HC-9; the scoring pays 2A for it.
3. **Defenders' rounding:** the defenders score `round5(points)`. Expected raw points are a close proxy; the exact objective would average `round5` per world (one value per world, no extra solve).
4. **Time budget:** `evaluate` takes a world count, not a time limit. Agent B needs the adaptive count per S4 and a deadline (the max column above).

# Design: observation + knowledge (layers 2–3), v0

Orchestrator, 2026-10-06. Serves I10, I11, I13, I15. Sources: owner's `refactor_step2` (`gamestate.py`), Investigator A (`findings/ideas/A-reasoning-and-deduction.md`), engine `crates/engine`.

## Intent of the owner's design (refactor_step2), and what we keep
Each seat keeps, for every other seat, the cards it **may** hold and the cards it **certainly** holds, and updates them after every observed action by three passes:
1. invert the trick rules (`_possible_player_cards`);
2. resolve facts about pairs and halves from questions and answers, retiring them when a half is played (`_pair_concepts_check`);
3. counting by hand size until nothing changes (`_set_logic_check`).

Probabilistic beliefs from bidding live apart, as "concepts".

All of that is kept. What changes:
- `possible`/`certain` become 36-bit sets.
- The concepts about pairs and halves become one typed cardinality constraint, `lo ≤ |hand ∩ mask| ≤ hi`, which is updated when a card is played.
- Conventions stay out of the hard layer: they will become likelihood weights (soft layer, v1).

The three bugs Investigator A found:
- (S10) a self-announced trump is not recorded;
- (S14) a half answer ignores halves already known;
- (S16) a joint half answer bypasses the uniqueness update.

All three are impossible by construction here.

## Layers
| Crate | Owns | Must not |
|---|---|---|
| `engine` (`bits` module, new) | `CardSet` (36-bit) and `play_levels`: the play rule as an ordered list of card sets, mirroring `cards::allowed_cards` exactly (differential test) | — |
| `observation` (new) | The **privacy boundary**. Turns engine `GameEvent`s into `SeatEvent`s for one seat; hidden passes lose their cards. Detects completed undos | read hidden cards; use `GameInfoPlayer.meta_info` (it carries all start hands) |
| `knowledge` (new) | `Knowledge` (`Copy`, ~0.3 KB): possible/certain per seat, played, hand sizes, trick, trump, cardinality constraints; `Tracker` (undo) | depend on `engine::game::Game` or `GameState` |

## Play-rule inversion (one owner)
For a play, `allowed_cards` returns `hand ∩ L_k`, where `L_k` is the first level of an ordered list that meets the hand:
- **lead:** aces, then green, then all (trick 1); otherwise all;
- **follow:** led ace (trick 1), then higher of the led suit, then the led suit, then higher, then trump, then all.

If card `c` is played and `k_min` is the first level that contains `c`, then the hand met no level before `k_min`. So `possible -= L_1 ∪ … ∪ L_{k_min-1}`. That is the strongest sound single-event deduction, and it reproduces S4–S9 including S5 and S6, which the old code missed. The levels are computed by the engine's own `high_card`, and a test checks `first_nonempty(levels ∩ H) == allowed_cards(H)` on random tricks and hands. The rules keep one owner.

## Q&A → constraints
| Event | Effect |
|---|---|
| AnnounceTrump(s) by x / YesPair(s) by x | `certain[x] ∪= KO[s]`, trump = s |
| NoPair by y | for each uncalled s with both halves live: `|H_y ∩ KO[s]| ≤ 1` |
| NoHalf(s) by y | `possible[y] −= KO[s]` |
| YesHalf(s) by y, callback NewTrump/StillTrump | `|H_y ∩ KO'| = 1`, `|H_asker ∩ KO'| = 1`, other seats `−= KO'`, trump = s (KO' = live halves) |
| YesHalf(s) by y, callback OnlyHalf | `1 ≤ |H_y ∩ KO'| ≤ 2`, `possible[asker] −= KO'` |
| card c played by p | removed from every mask; if `p` owns the constraint: `lo, hi −= 1` |

## Propagation (fixpoint)
- **Constraint:** `k = |C∩M|`, `p = |P∩M|`. If `k = hi`, clear `P∩M`. If `k+p = lo`, move `P∩M` to C. If `k > hi` or `k+p < lo`, it is a contradiction.
- **Hand size:** `free = n − |C|`. If `|P| = free`, P becomes certain. If `free = 0`, clear P.
- **Uniqueness:** a live unknown card possible for exactly one seat becomes certain there; for none, it is a contradiction.

A contradiction can only mean an engine or observation bug: tests panic, production reports it.

## Undo
The engine can undo only the last bid, stop or card. `Tracker` keeps one snapshot from before the last such event and restores it when `observation` reports a completed undo (2 accepts).

## Guard tests (permanent)
1. **Soundness:** random games including undo, all 4 seats; after every event `certain ⊆ truth ⊆ certain ∪ possible` and hand sizes match.
2. **Engine mirror:** `play_levels` vs `allowed_cards` on random tricks and hands.

## Eager vs. lazy (owner I18)
Rule: **cheap, certain facts are eager and incremental; expensive or probabilistic facts are lazy, cached, and invalidated by explicit rules.**

| Tier | What | When computed | Cache key / invalidation |
|---|---|---|---|
| 0 eager | possible/certain sets, hand sizes, trick, trump, constraints (rules S1–S19) | on every event, O(1) bit ops + a fixpoint that usually ends after one pass | none: it *is* the state (`Copy`, ~0.3 KB, snapshot = undo) |
| 1 lazy, exact | voids, trump-count bounds, "can still meld", sure winners (S21–S22), exact enumeration when ≤ 12 unknown, exact marginals (signature DP) | on first query after a change | `epoch` (bumped by every event that changes tier 0); a query compares epochs, recomputes only if stale |
| 2 lazy, soft | convention likelihoods (bids, passing shape), per-seat trust, world weights for sampling | only when a consumer asks (bidding policy, sampler, explanations) | keyed by the **bidding/pass history length** and the seat's trust version: play events do not invalidate them; a hard fact that contradicts a convention only re-weights, never mutates tier 0 |

Further rules:
- Tier 2 never writes into tier 0 (a bluff cannot make the knowledge inconsistent).
- Caches live beside the `Copy` state, not inside it, so search can copy states cheaply and discard caches.
- Every cache gets a debug check that recomputes from scratch and compares (in tests).

Measured v0 baseline (tier 0 only, release build, PC under load from other projects): **51 ns per event, 2.8 µs per seat per game** (`tests/speed.rs`). Before tier 0 used bit operations for the play rule, it was 1,632 ns.

## Not in v0 (next)
- Soft layer (convention likelihoods from the 1,579 games).
- Exact enumeration when ≤ 12 cards are unknown.
- Signature-class DP marginals and world sampling.
- Sure winners (S22).
- "Can still meld" (S21).
- A benchmark.

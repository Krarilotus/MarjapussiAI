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
| `observation` (new) | The **privacy boundary**. Turns engine `GameEvent`s into `SeatEvent`s for one seat; hidden passes lose their cards. Detects completed undos | read hidden cards; read `Game.info.player_start_cards` (all start hands) |
| `knowledge` (new) | `Knowledge` (`Copy`, ~0.3 KB): possible/certain per seat, played, hand sizes, trick, trump, cardinality constraints; `Tracker` (undo) | depend on `engine::Game` or `GameState` |

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

## Deal sampler (tier 1, `sample.rs`)
`DealSpace::new(&knowledge)` (or `knowledge.deals()`) covers the deals consistent with a seat's knowledge: certain ⊆ hand ⊆ may_hold, hand sizes, every live constraint. It gives:
- `count()`: the exact number of such deals;
- `holder_counts()` / `marginals()`: the exact number (probability) of deals in which seat y holds card c; a zero count means the card is impossible there, so this also gives the exact possible sets;
- `sample(rng)`: one deal, exactly uniform, without rejection;
- `exact_sets()`: `certain` and `possible` made exact (cards held in every deal, and in some but not all). `Tracker::exact_sets()` caches it by event count, which only grows (undos included), so the cache cannot go stale. The cache is a `Cell`, so `Tracker` is `Send` but not `Sync`. Guards after every event: cache = fresh computation; true hand ⊆ exact may_hold; every live card has a holder; tier-0 certain ⊆ exact certain, exact possible ⊆ tier-0 possible; equal to enumeration on small states;
- `for_each_deal(f)`: every consistent deal once, for exact evaluation when the count is small. Guard: equal to brute-force enumeration on 16,537 states with ≤ 1,000 deals;
- `sample_worlds(rng, n, belief)`: uniform samples weighted by a `Belief` (a likelihood hook for the soft layer; the weights are importance weights).

Method (Investigator A §6.4–6.5):
- Unknown cards are grouped by signature: the hidden seats that may hold them, at most 7 classes. Cards inside a class are exchangeable, so a DP over the hidden seats' remaining capacities counts the deals class by class (`ways`, at most 14 × 14 states per class).
- Cards in a live constraint are grouped more finely (same signature, same constraints; usually the K and O of one suit). An outer enumeration splits each such group over the seats and checks the constraints. The DP then counts the rest of each split.
- Sampling draws a split, then a split per class in proportion to `C(n, split) · ways(rest)`, then which cards go where, uniformly. Marginals: forward counts × `ways`, shared equally inside a class.
- The class with the most splits goes last; its split is forced, which keeps sampling cheap.
- Counts are `u64`: three hidden seats and at most 36 cards give at most 3³⁶ < 2⁵⁸ deals. Random draws use an exactly uniform integer method (Lemire).

Guards (`tests/sample.rs`):
- every sample satisfies all facts (300 random games, every seat after every event; 72,224 states, 4,276 of them with constrained cards);
- counts and holder counts equal brute-force enumeration exactly on all states with ≤ 12 unknown cards (20,703 states, 1,140 constrained, 32.4 M deals);
- sample frequencies are uniform by chi-square (50 states, 25 constrained, 200 draws per deal);
- the belief weights reproduce an exactly enumerated weighted marginal.

Speed (`tests/sample_speed.rs`, release, 500 random games, PC shared with other projects), in µs:

| unknown cards | states | constrained | median deals | build | sample | marginals | worst build |
|---|---|---|---|---|---|---|---|
| 0 | 9,061 | 0 | 1 | 0.28 | 0.01 | 0.29 | 0.7 |
| 1–6 | 19,121 | 480 | 6 | 0.57 | 0.06 | 0.78 | 2.1 |
| 7–12 | 21,129 | 2,126 | 700 | 0.82 | 0.11 | 1.12 | 3.3 |
| 13–18 | 19,804 | 2,814 | 342,342 | 0.97 | 0.13 | 1.30 | 8.2 |
| 19–24 | 19,205 | 1,464 | 2.7·10⁸ | 0.89 | 0.11 | 1.09 | 24.3 |
| 25–27 | 20,160 | 0 | 2.3·10¹¹ | 0.59 | 0.10 | 0.87 | 2.3 |

The worst case is a NoPair over four suits: 8 tied cards. Splitting each suit's pair as one group cut it from 182 µs (cards placed one by one) to 26 µs. Tier-0 speed is unchanged (A/B against `main`, interleaved: 35–44 ns per event on both).

## Known forms K1–K6 (C26, C26b, C27; reasoning-system design v2 §3.1)
Each concept below has a **known** form here: sound (it never claims what the true hands
contradict; checked after every event in `tests/soundness.rs`), from one seat's view, with the
cards it depends on (its **mask**, a dependency note: an event that changes no fact about these
cards cannot change the answer). The belief layer invalidates by **epochs**, not by these masks
(design v2 §8 F3: no card masks unless a benchmark shows a need). The **believed** form (a probability, 1 or 0
where the known form decides) belongs to the `beliefs` crate (I2), not here.

Notation: `C(x)` certain, `M(x) = C(x) ∪ P(x)` may hold, `Pl` played, `n(x)` hand size,
`KO(s)` the K and O of suit s, `S(s)` the suit, `H(c)` the cards of c's suit that take c
without trump (`engine::takes`), `t` the trump (if any), `opp(x)` x's two opponents.

**Incremental or lazy.** State that changes per event is updated in `apply` (a few bit
operations: the per-seat played cards, the exchange ledger, the public facts of K6). The queries
K1–K5 are computed on read from that state (tens of ns, no allocation); `Knowledge` is `Copy`, so
a caller keeps a result for the rest of a decision. Cost per event and per query: see the
measurements at the end of this section.

### K1 `standing(x)`: the cards of x that certainly win if x leads them now
`c ∈ C(x)` stands iff
1. every live card of `H(c)` is certainly with x's team: `H(c) − Pl ⊆ C(x) ∪ C(partner(x))`;
2. if a trump `t ≠ suit(c)` is set: each opponent `o` certainly must follow (`count_lo(o, suit(c)) ≥ 1`) or certainly holds no trump (`M(o) ∩ S(t) = ∅`).

- **Top trumps count:** for `suit(c) = t`, rule 2 is void (nothing beats a trump but a higher trump).
- **The team, not the card:** if the partner holds a higher card, it may have to overtake; the trick is still ours. "Stands" means "our team wins this trick when I lead c".
- **Trump changes:** "if led now" uses the trump as it is now; an announcement before the lead is a new event (and a new answer). Without trump nobody can ruff, so rule 2 is empty.
- **Trick 1:** the lead rules (ace, then green) restrict *which* cards may be led, not whether they win; `standing` answers for every held card, the caller intersects with the legal cards.
- **Last trick:** no special case (one card each; the rules above still decide).
- **Undo:** a query; it follows the restored state.
- **Mask:** `S(suit(c)) ∪ S(t)` (the suit for rule 1 and the opponents' following; the trump suit for the ruff) plus the opponents' hand sizes (count bounds, K4).

### K2 `pairs(x)`: x's pairs, certain and still meldable
For each suit `s` not yet called with both halves live (`KO(s) ∩ Pl = ∅`):
- **certain:** `KO(s) ⊆ C(x)`;
- **possible (meldable):** `KO(s) ⊆ M(x)` and no constraint of x caps `|H_x ∩ KO(s)|` below 2 (e.g. a "no pair" answer).

Cards only: whether x still has the **right** to announce is a public fact (K6 `rights`), combined by the caller. A called pair is not meldable again (a half answer can only re-call it as trump, "StillTrump", without points). **Mask:** `KO(s)`.

### K3 `team_pairs(x)`: the pairs of x's team, by holder
MY (x holds both), YOUR (the partner holds both), OUR (one half each, in either assignment), each **certain** (from `C`) and **possible** (from `M`, with K2's constraint check for MY and YOUR). Same suit conditions as K2. **Mask:** `KO(s)`.

### K4 `count_bounds(x, s)`: sound bounds on `|H_x ∩ S(s)|`
- `lo = max(|C(x) ∩ S(s)|, n(x) − |M(x) − S(s)|, lo of x's constraints whose mask lies in S(s))`;
- `hi = min(|M(x) ∩ S(s)|, n(x) − |C(x) − S(s)|)`.

Voids are `hi = 0` (`is_void` is the same fact, kept as the cheap form). **Mask:** all cards (the second terms read x's whole hand through `n(x)`).

### K5 the dealt hand (provenance; C26 K5, design D-e)
Bids describe the **dealt** hand, before the exchange. The exchange is two passes in order: the first passer F gives 4 cards to its partner R (the declarer), then R gives 4 back (they may include cards F gave). From x's view, `gave(x)` / `got(x)` are the cards when x saw the pass (`Passed::Seen`), else `Hidden`.
- **The first passer F:** `dealt(F) = ((now ∪ played) − got) ∪ gave`. Cards it gave and got back are in `gave`, so they stay.
- **The second passer R:** `dealt(R) = (now ∪ played ∪ gave) − got`. Cards it passed on from `got` are removed by `− got`.
- **Without exchange** (all passed): `dealt = now ∪ played`.

Known forms from x's view (`now` known as `C`, unknown as `M`):
- `dealt_certain(y)`: the formula with `now = C(y)`, where a **hidden** `got(y)` means "any card not certainly dealt elsewhere". So only y's cards that cannot have reached it in the exchange count (cards it played or holds outside what the giver could have given).
- `dealt_possible(y)`: the formula with `now = M(y)`, a hidden `gave(y)` read as "anything its partner may hold or played" (the cards went there, or came back), minus what was certainly dealt to another seat.
- **Mine** (x = me): exact, since I see both of my passes.
- **Undo:** passes cannot be undone, plays can (the snapshot restores `played_by`).
- **Mask:** the passed cards and the cards played by y.

### K6 public facts (the engine's derivations, carried per seat)
Rights per seat (Own / Yours / Ours), the declarer, the game value, the trick winners, and the team points (cards with the last-trick bonus, plus announced pairs). These are public, change only per event and must be **undone with it**, so they live in `Knowledge`'s per-event state. Every transition calls the engine's pure rule function (E8), the one owner of the rule: `winning_seat` for the trick winner (also of the trick in progress), `trick_points` for a finished trick (the last-trick bonus included), `trump_points` for a pair made trump, and `PlayerTrumpPossibilities::after_question` / `after_no_pair` for the rights. `knowledge` restates none of them; with a rule mutated in the engine (`trick_points` bonus + 7, "Yours" always to Ours), the soundness harness still passes, because both sides follow the engine. Card points and pair points are kept per team apart (their sum is `team_points`), and `Knowledge::public_state(to_move, announce_open)` gives the engine's `PublicState` for the solver's worlds (C28); the harness checks it equal to `FastState::from_game` at every event. The soundness harness checks after every event that every seat's public facts **equal the engine's** (`GameState::declarer`, the players' `trump` rights, `team_points`), so the engine stays the owner of their values.
- Voids: `is_void` (K4), never re-derived from the plays in an agent.
- **Mask:** none (event-level facts).

### Measured (C27, release, PC shared with other projects)
- **Per event** (`tests/speed.rs`, 435,060 seat events, best of 5): **+3 ns (+11 %)**. Measured by the Judge (J20-I1) in 5 interleaved rounds with two worktrees: `main` 0d4c56c 27–28 ns, I1 30–31 ns, in every round (my own 4 rounds, 31–33 vs 31–38 ns, were too noisy to see it). It is the copy of K5's ledger and K6's public facts in `Tracker`'s per-event snapshot (~40 bytes more), about 0.14 µs per seat per game.
- **Per query** (`known_form_queries`, read after every event, ~20 ns timer overhead included): `standing(me)` 80 ns, `team_pairs` 55 ns, `count_bounds` 32 ns, `dealt_certain` 30 ns, `dealt_possible` 37 ns. All are lazy: a query nobody reads costs nothing.
- **Soundness** (`tests/soundness.rs`, 3,000 random games with undos, 180,472 events × 4 seats): every K1–K5 answer against the true hands and the true dealt hands, every K6 fact equal to the engine's after every event. 1.2 M standing cards checked (53,849 of them trumps); 8.6 M dealt-hand facts beyond the current hands.

## Not in v0 (next)
- Soft layer (convention likelihoods from the 1,579 games); it plugs into `Belief`.
- A benchmark.

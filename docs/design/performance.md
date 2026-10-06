# Engine performance

Two branches, newest last: feat/engine-fast (2026-10-06, merged as 955d19c) and
feat/engine-v2 (E4, 2026-10-06).

# feat/engine-fast

Owner: Engine Coder. API of the fast core: [fast-core.md](fast-core.md). Evidence level: **measured
by us** on the owner's PC unless marked otherwise.

## Method
- Release build (`lto = "thin"`, `codegen-units = 1`), one thread, best of N runs.
- **A/B against `main` (68c98f0) in one process.** `main`'s `crates/engine` is copied as a
  package renamed `marjapussi-engine-ref`. A scratch harness depends on both engines and
  compiles the same benchmark code (the macro in `crates/engine/examples/bench/game.rs`) for each.
  Runs alternate old/new, and each metric keeps the best of all rounds. The same harness ran the
  lockstep diff (below). It lived in `tmp/` and is deleted; it can be rebuilt from this
  description in a few minutes.
- `fast` numbers: `cargo run --release -p marjapussi-engine --example bench -- 300 7`.
- **Load:** the PC was shared with other projects and showed 93–100 % CPU during every run.
  Two runs of the same build differ by up to 1.5×. Only ratios from interleaved A/B runs are
  reliable; absolute numbers are upper bounds.
- **Pitfall:** two checkouts of this workspace must not share one `CARGO_TARGET_DIR`. Cargo's
  artifact hash for path packages does not include the workspace location, so the builds
  overwrite each other.

## Before / after: `Game` (A/B against `main`, 300 random games, 11 interleaved rounds)
| Metric | main 68c98f0 | this branch | Speed-up |
|---|---:|---:|---:|
| ns per action, `apply_action_mut` | 7,201 | 610 | ×11.8 |
| ns per action, `apply_action` (immutable) | 9,448 | 4,482 | ×2.1 |
| ns per full random game (mut, incl. `Game::new`, ~58 actions) | 421,690 | 35,693 | ×11.8 |
| ns per `legal_actions()`, all phases | 3,729 | 913 | ×4.1 |
| ns per `legal_actions()`, card play | 1,571 | 198 | ×7.9 |
| ns per random card playout, 8 tricks left (`Game` clone included) | 182,926 | 16,184 | ×11.3 |
| Heap allocations, 36 card plays of one game (`tests/alloc_guard.rs`) | 2,696 | 148 | ×18 |
| Heap allocations, the two passes of one game | 2,224 | 5 | ×445 |

## `fast` (this branch; three runs, range)
| Metric | Value |
|---|---:|
| ns per random card playout, 8 tricks left (the same positions as the `Game` row) | 622–727 |
| ns per full 9-trick random playout (announcements included) | 733–1,151 |
| Full playouts per second, one thread | 0.87–1.36 M |
| ns per move (generation + apply + random choice) | 20–32 |
| Moves per second, one thread | 31–49 M |
| ns per `legal_cards()` + `play` | 11.8–14.2 |
| ns per play in a full tree walk, 3 or 4 tricks left (copy-make, like a solver) | 16.9–17.7 |
| Heap allocations in a whole game of card play | 0 |

- `fast` vs `Game` on the same 8-trick playouts: about ×25 against this branch's `Game` and about
  ×270 against `main`'s.
- Investigator B's prototype (reported, same PC, also loaded) took 0.57–0.78 µs for this playout.
  The engine-owned core matches it while using the engine's rule functions.

## Where `Game` time goes now (per action kind, 3,000 games)
| Action @ phase | per game | ns per apply | share |
|---|---:|---:|---:|
| `CardPlayed` @ `Trick` | 31.4 | 538 | 33 % |
| `Pass` @ `PassingForth` (builds the 715 passes of the partner's 13 cards) | 1.0 | ~17,000 | 33 % |
| everything else | | | 34 % |

What remains is bound by the public types:
- `ActionType::Pass(Vec<Card>)`: one `Vec` per pass, 841 per game (now pooled);
- `GameEvent.time: String`: one allocation per action;
- the eager `legal_actions: Vec<GameAction>` field.

Changing them would break the API (see open questions).

## Optimisation log (R3: where, before/after, kept or rejected)
Ratios are `apply_action_mut` per action, A/B against `main` after each step.

| # | Change | Where | Result | Status |
|---|---|---|---|---|
| 1 | Apply in place instead of cloning `GameMetaInfo` + `GameState` per action; immutable `apply_action` clones once and skips the undo snapshot it would replace | `game.rs`, `apply_action.rs` | with 2–3: 7,061 → 1,392 ns (×5.1) | kept |
| 2 | Card legality and trick winner from `bits` (no `Vec` temporaries, no `concat` per card) | `legal_actions.rs`, `apply_action.rs`, `cards.rs` | `legal_actions()` in card play ×13 at this step | kept |
| 3 | Time string formatted once per second per thread (same text and resolution) | `game.rs` | not measured separately | kept |
| 4 | Undo snapshot reuses its buffers (`clone_from` for `GameState`, `Player`); `legal_actions` buffer reused; pass legality checked directly instead of a scan of 715 `Vec`s | `gamestate.rs`, `player.rs`, `game.rs` | 7,890 → 1,271 ns (×6.2) | kept |
| 5 | Per-thread pool for the `Vec<Card>` buffers of pass actions (≤ 1,024 buffers of 8 bytes) | `legal_actions.rs` | 7,188 → 737 ns (×9.8) | kept |
| 6 | One pool borrow per pass list; 4-card sort by bit index (a sorting network) | `legal_actions.rs` | 7,788 → 580 ns (×13.4) | kept |
| 7 | `Card`, `PlaceAtTable`, `FinishedTrick` and 4 small enums derive `Copy` (additive) | several | enables 2–6 | kept |
| 8 | `PlayLevels` with a fixed length (unused levels `ALL`) | `bits.rs` | `play_levels().allowed()` 19.4 → 17.3 ns (×1.1, micro A/B) | kept |
| 9 | A straight-line `allowed` beside `play_levels` | — | 17.9 → 15.0 ns on random inputs; a second formulation of the rule | **rejected** (one owner beats 16 %) |
| 10 | Make/unmake for `fast` (6-byte undo record) | — | perft 3–4 tricks left, 3 runs: copy 95–176, make/unmake 83–338 ns per leaf; no consistent winner | **rejected**, copy kept |
| 11 | Shared or `Rc` event log so `apply_action` (immutable) stops copying `all_events` | — | would change the type of `Game.all_events` (public); hot loops use `apply_action_mut` | **not done** (open question 2) |
| 12 | A faster global allocator (e.g. mimalloc, MIT) | — | a library must not choose it; candidate for simulation binaries | idea |

## Guards (correctness; all green on this branch)
| Guard | What | Result |
|---|---|---|
| `tests/golden_trace.rs` | 2,000 seeded random games (questions, answers, announcements, undo requests/accepts/declines), half `apply_action`, half `apply_action_mut`; the full state, the legal actions and every event digested at every step, plus `GameFinishedInfo` | digest recorded on `main` 68c98f0, unchanged (120,274 actions) |
| Lockstep diff (scratch harness) | `main`'s engine and this branch side by side, all observable state compared at every step, plus an illegal action rejected by both every 7 steps | 10,000 games, 600,691 actions, 0 differences |
| `tests/human_replay.rs` | the 1,579 human games | 1,577 replay to the end, 2 stop on an illegal card (as before; E2: data from 2022-09, rejected by marjapussi-py too); outcome digest unchanged; score totals pinned |
| `tests/fast_vs_game.rs` | `fast` vs `Game` at every card-play state of 5,000 random games | 212,215 states, 181,852 cards, 2,002 announcements, 45,446 tricks, 0 differences |
| `bits` test `play_levels_mirror_allowed_cards` | the former `Vec` rule (kept as the oracle) vs `cards::allowed_cards` (including card order), `play_levels` and `excluded_by`, plus the trick winner vs the old `high_card` | 200,000 random tricks |
| `tests/alloc_guard.rs` | allocation counts of 1, 4–6 (deterministic guard of the speed wins) | `fast` 0; `Game` ≤ 5 per card, ≤ 20 per two passes |
| `tests/scoring.rs` | E1 scoring | see below |
| Knowledge crate | all tests; speed A/B (`speed` test, separate target dirs) | main 42–45 ns per event, branch 43–44 ns: unchanged |

## Scoring (E1)
`game::scoring` owns:
- **Game scores:** `game_scores` (rules page "Spielbewertung"), `GameFinishedInfo::scores()`.
- **Series:** `SeriesSettings`, `SeriesLength`, `SeriesTally` (the site's series table).
- **Schwarz factor:** `SCHWARZ_FACTOR_FIFTHS`, which is also the series default.

`scores()` equals the reviewed 264fa3d version game by game:
- 3,000 random games;
- 1,577 human games.

Against the site's own table (`SeriesTable.vue`, run on each record's fields), 1,487 of 1,577
human games score the same. All 90 differences have a known cause; the test asserts there is no
unexplained difference:
- 71: N took no trick, but the record has no schwarz flag (2022 site version). We score N −2A as
  the rules text says.
- 11: the record's team points differ (2022-10 site versions). These include pairs announced
  after a question, which the old site did not count, and one all-pass game in which the site
  counts an announced pair and the engine does not.
- 8: **the playing party took no trick.** The site gives it −2A; the rules text gives −A (open
  question 1).

## Open questions for the owner
1. **Schwarz of the playing party:** the rules text gives −A (only N's schwarz is special), but
   marjapussi.de's series table gives −2A. This affects 8 of 1,577 human games. Default kept:
   rules text (as reviewed on 264fa3d).
2. **All-pass games with an announced pair:** the engine's `team_points` leaves out pairs when
   nobody played, but the site counts them. This affects 1 of 1,577 games; the rules page is
   silent. Default kept: the engine (unchanged behaviour).
3. **Breaking API changes for more speed** (each needs the owner's yes; all consumers would
   change):
   - (a) `ActionType::Pass([Card; 4])`;
   - (b) a lazily computed `legal_actions`;
   - (c) a shared event log or a time stamp that does not need its own `String`.

   Estimate: about 35 µs → about 15 µs per random game and ×3–4 for immutable `apply_action`
   [estimate from the profile above].

## Decided (owner decision D2, 2026-10-06)
The three questions above were answered:
- marjapussi.de is the source of truth for scoring (questions 1 and 2);
- breaking API changes are allowed for speed (question 3).

They are implemented on `feat/engine-v2` (E4) below.

---

# feat/engine-v2 (E4)

## Scoring follows the site (E4.1)
- **Schwarz:** a party that took no trick scores −2A while the other party scored positive. N is
  covered when S made its contract; S is covered always (new: declarers played schwarz lose −2A
  instead of −A). Declarers who simply fail still lose −A.
- **All-pass games:** announced pairs count in each team's `team_points` (new).
- **Guards:**
  - `tests/scoring.rs` checks one case per rule, and `scores()` against a line-by-line port of
    the site's `SeriesTable.vue` on 3,000 random games;
  - the human replay checks every one of the 1,577 replayed games against that port.
- **Against the site's table on each record's own fields:** 1,492 of 1,577 human games agree
  (was 1,487). The 8 flagged declarer-schwarz games and the all-pass game now agree. The 85
  remaining differences are old records, and none is unexplained:
  - 75: a team took no trick, but the record has no schwarz flag (2022 site versions, which
    scored without it). This includes 4 declarer-schwarz games from 2022.
  - 10: the record's team points differ (2022-10 site versions; pairs after a question were
    not counted).
- **Human replay digest:** recomputed once, `0x9a387d77f3bc8e2d` → `0xcb8cf66a3bd29d91`. Only
  `team_points` of one all-pass game changed (+40); reverting that one line restores the old
  digest. Score totals re-pinned: `[34,630, 28,255]`. The golden trace is unchanged: none of
  its random all-pass games had an announcement.

## Before / after: `Game` against `main` 80eaf23
Interleaved A/B in one process (scratch harness, method as above), 200–300 random games,
15–21 rounds, CPU at 97–100 % from other projects. The range covers the final runs of steps d
and e.

| Metric | main 80eaf23 | feat/engine-v2 | Speed-up |
|---|---:|---:|---:|
| ns per action, `apply_action_mut` + reading the legal list (random play) | 513–633 | 236–346 | ×1.5–2.1 |
| ns per action, replaying known actions (`apply_action_mut` only) | 506–597 | 106–159 | ×3.4–4.8 |
| ns per action, immutable `apply_action` | 3,257–4,906 | 497–654 | ×6.4–8.2 |
| ns per full random game | 25,627–36,788 | 13,133–20,100 | ×1.5–2.1 |
| ns per `legal_actions()`, all phases | 734–822 | 221–308 | ×2.7–3.7 |
| ns per random card playout, 8 tricks left | 10,692–15,654 | 5,430–8,802 | ×1.7–2.3 |
| Heap allocations, 36 card plays (`tests/alloc_guard.rs`) | 148 | 10 | ×15 |
| Heap allocations, the two passes | 5 | 0 | |

Against the original `main` 68c98f0 (before feat/engine-fast), per action is about
7.2 µs → 0.24–0.35 µs, ×20–30.

`fast` is unchanged by E4 (see the `fast` table above).

## Optimisation log (E4)
Ratios are A/B against `main` 80eaf23 after each step, interleaved.

| # | Change (breaking unless noted) | Result | Status |
|---|---|---|---|
| a | `ActionType::Pass([Card; 4])`; `ActionType` and `GameAction` are `Copy`; pass pool removed | per action ×1.30, `legal_actions()` all phases ×2.38 | kept |
| b | `GameEvent.time: u64` ms (`game::now_millis`, `game::format_time`); `GameEvent` is `Copy` | immutable `apply_action` ×1.27 → ×2.11 | kept |
| c | Legal actions on demand (`legal_actions()`, `legal_actions_into`, `is_legal`); the field is gone | per action incl. list ×1.39; replaying known actions ×2.2–2.5 | kept |
| d | Inline state (`engine::inline::InlineVec`); `Player.name: Arc<str>`; `Game.info: Arc<GameMetaInfo>`; `Player.tricks` removed | immutable ×6.4–7.7; replay ×3.9–4.8; per action ×1.5–1.75; card-play allocations 148 → 10 | kept |
| e | `Game.clock` / `Clock::Off` (additive; default unchanged) | the wall clock costs 33 ns per event (measured); new engine 236 → 226 ns per action, 121 → 106 ns replaying, 6.7 → 5.4 µs per playout | kept |
| f | mimalloc as global allocator (simulation binaries only; MIT) | new engine: per action 0–16 %, immutable ~25 %, card playout 15–30 %, replay ~20 % faster (2 runs each); `main` gains more (it allocates more) | recommended for arena/eval binaries; not added on `main` (no simulation binary there) |
| g | Make `GameState` fully `Copy` (bidding history inline too) | at most ~70 bids inline would add ~800 bytes to every snapshot copy; the `Vec` costs one allocation per fresh snapshot only | not done |

## Guards (all green on feat/engine-v2)
| Guard | Result |
|---|---|
| Golden trace (2,000 random games, both apply paths) | digest `0x3544cf5afa288fc8`, unchanged since 68c98f0 |
| Human replay | 1,577 of 1,579 to the end, 2 illegal cards (E2); digest recomputed once for E4.1 (see above) |
| Lockstep diff against `main` 80eaf23 (scratch harness) | 10,000 games, 601,132 actions identical; scores and all-pass `team_points` excluded (E4.1) |
| `tests/legality.rs` (new) | `is_legal == legal_actions().contains` at every state. 500 games in the suite; one release run of 1,500 games checked 98.4 M candidate actions over 118,395 states, 0 differences |
| `tests/fast_vs_game.rs` | unchanged, green |
| `tests/alloc_guard.rs` | tightened: ≤ 16 allocations per 36 card plays (measured 10), ≤ 2 for the passes (measured 0); `fast` 0 |
| `tests/scoring.rs` | rule cases for D2; `scores()` equals the site port on 3,000 random games |
| knowledge, observation | all green (tests updated for the API) |

## API changes in feat/engine-v2 (for the heuristic and devtool branches)
| Before | After | How to adapt |
|---|---|---|
| `game.legal_actions` (field) | `game.legal_actions()` → `Vec`; `game.legal_actions_into(&mut buf)`; `game.is_legal(&a)` | call the method; in hot loops, reuse a buffer |
| `ActionType::Pass(Vec<Card>)` | `ActionType::Pass([Card; 4])` (high to low) | `cards.try_into().unwrap()` after sorting; reading as a slice works as before |
| `ActionType`, `GameAction`, `GameEvent` `Clone` | now `Copy` | drop `.clone()` (clippy `clone_on_copy`) |
| `GameEvent.time: String` | `u64` ms since the Unix epoch | `game::format_time(t)` for text |
| `Game.info: GameMetaInfo` | `Arc<GameMetaInfo>` | reads unchanged; `(*game.info).clone()` for an owned copy |
| `Player.name: String` | `Arc<str>` | `&*p.name` or `p.name.to_string()` |
| `Player.cards`, `GameState.current_trick`, `all_tricks`, `trump_called`, `players_started`, `players_accept_undo`: `Vec` | `InlineVec<_, N>` (derefs to a slice) | slice methods work; `.to_vec()` for an owned `Vec`; `CardSet::from_cards(&p.cards)` works |
| `Player.tricks` | removed (it was always empty) | — |
| `ActionType::apply_action` (deprecated) | removed | `Game::apply_action(_mut)` |
| — | `Game.clock`, `Game::with_clock(Clock::Off)` | optional, for simulations |
| scoring | declarers played schwarz lose −2A; pairs count in all-pass `team_points` | — |
| `GameInfoPlayer`, `GameFinishedInfo` | unchanged types (`Vec` fields) | — |

## E5: ownership and abstraction debt (same branch)
| Debt | Change | Evidence |
|---|---|---|
| D4 privacy | First redacted (E5.1: no start hands, only the seat's own legal actions, with a hand-swap test that failed on the leak), then **deleted** with `series.rs` in E6 (J18/J19): the engine offers no per-seat view | the type no longer exists |
| D10, D9 | `cards` is the leaf that owns the card codes (`Suit::code/from_code`, `Value::code/from_code`, `FromStr`), `CARDS`, `SUITS`, `VALUES`, `Card::index/from_index`; the `Vec` rule wrappers and the `Vec` oracle test are removed; knowledge uses `CARDS` instead of a bare 36; `itertools`, `strum`, `strum_macros` removed | digests unchanged |
| D2 | `apply_action.rs`: one function per action kind over `{state, undo snapshot, meta info}`; legal actions work on the state and snapshot; answers follow from the phase instead of an event-log scan | `kg caps`: no engine function over the cap (was 182 lines); legality guard green |
| D1 | `game.rs` only declares modules; `Game` in `game/table.rs`, time in `game/clock.rs`, `GameMetaInfo` in `game/meta.rs`; `inline.rs` is a leaf | `kg cycles`: the 12-file engine cycle is gone (one remains inside knowledge, not engine code) |
| D3 | Façade: `lib.rs` re-exports the public API; `bits`, `fast`, `game`, `inline` are private; consumers import `engine::X` | `kg facade`: 0 deep-import targets (was 8 on main, 24 measured on the heuristic branch) |
| Derived facts | `GameState::declarer()`, `playing_team()`, `trick_leader()`; `FinishedTrick.leader`, `seat_of_card(i)`, `seats()`; `GameFinishedInfo.declarer`; card codes from `cards` | digests unchanged (declarer from the bidding history equals the old event scan on all guard games); `fast_vs_game` checks leaders and seats at every card-play state |

Speed after E5: unchanged within noise (A/B vs main: ×1.86 per action incl. the legal list, ×5.9 immutable, ×3.4 replay).

### More API changes for the heuristic and devtool branches (E5)
| Before | After |
|---|---|
| `engine::game::…::X`, `engine::bits::X`, `engine::fast::X` | `engine::X` (all public names at the root) |
| `bits::index(&c)`, `bits::card(i)` | `c.index()`, `Card::from_index(i)` |
| `fast::to_action(m, seat)`, `fast::from_action(&a)` | `m.to_action(seat)`, `Move::from_action(&a)` |
| `cards::allowed_cards`, `allowed_first`, `high_card`, `is_higher_card`, `higher_cards`, `halves`, `pairs`, `print_cards`; `parse::parse_card(s)` | `play_levels(..).allowed(hand)`, `trick_winner`, `takes`, `CardSet::halves`; `s.parse::<Card>()` (strict `r-A` format) |
| `GamePhase::legal_actions`, `legal_actions::{legal_bidding, legal_passing, legal_cards, legal_question, legal_answer}` | `Game::legal_actions()` / `legal_actions_into` / `is_legal` |
| declarer derived from the event log | `game.state.declarer()`, `GameFinishedInfo.declarer` |
| seats per trick rebuilt from the action log | `FinishedTrick.leader`, `seat_of_card(i)`, `seats()` |
| `points_players` | removed (unused) |

### E6 (Judge J18/J19, same branch)
| Before | After |
|---|---|
| `GameInfoPlayer`, `PublicInfo`, `GameEventPlayer`, `game/series.rs` (`Series`) | deleted (series scoring is `SeriesTally`; a per-seat view belongs to the server/observation layer) |
| `c / 9`, `c % 9`, `deck[27..36]`, bare `36` in consumers | `suit_of(i)`, `value_of(i)`, `Card::index/from_index`, `CARDS`, `hands_from_deck(&deck)` |
| `FastParts { hands, trick, …, pair_points }` | `FastParts { hands, public: PublicState }`; `FastState::public()`, `FastState::from_public(public, hands)`, `PublicState::to_move()` |
| solver-side move codes (`ANNOUNCE = 36`) and `Table` | `Move::code()`, `Move::from_code()`, `Move::ANNOUNCE`, `Move::CODES`; `PublicState` replaces `solver::Table` |

`fast` after E6 (2 runs): 20–27 ns per move, 14–17 ns per play in a full tree walk; unchanged.

## E8: one owner for the public rule transitions (fix/engine-e8)
Pure functions at the crate root (`game/rules.rs`, `bits`, `game/bidding.rs`), used by `Game` and
`FastState`; knowledge and the agents delete their copies against them:
`PlayerTrumpPossibilities::{may_announce, may_ask_pair, after_question, after_no_pair}`,
`trick_points(trick, tricks_done)`, `trump_points(suit, already_called)`,
`winning_seat(leader, trick, trump)`, `is_bid(value, bid)`; `TRICKS` now lives next to `HAND_SIZE`.

A/B against `main` 5da451f in one process (CPU ~28 % busy; 9 interleaved rounds, two runs):

| Metric | main | E8 | main / E8 |
|---|---:|---:|---:|
| fast: ns per card, random playouts | 22.5–23.0 | 22.3–22.4 | ×1.01–1.03 |
| fast: ns per play, 4-trick tree walk | 14.5–14.6 | 14.0–14.2 | ×1.02–1.04 |
| Game: ns per bid legality check | 46.3–46.4 | 4.3 | ×10.7 |
| Game: ns per action, replaying random games | 259–268 | 256–260 | ×1.00–1.05 |

Log: a first `trick_points(CardSet, …)` (masked popcounts) made the tree walk 10 % slower
(14.0 → 15.6 ns per play, two runs); taking the trick's four indices with table lookups
removed it (kept). `is_bid` is pinned to `bid_range` by an exhaustive test (tests/rules.rs).

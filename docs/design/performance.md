# Engine performance (feat/engine-fast, 2026-10-06)

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

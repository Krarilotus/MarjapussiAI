# The fast card-play core (`FastState`, API for search)

Owner: Engine Coder. Branch `feat/engine-fast`. Speed numbers: [performance.md](performance.md).

`FastState` (in the engine's private `fast` module, re-exported at the crate root) is the engine's documented low-level API for search and simulation: a `Copy`
card-play state with cheap make and undo. It covers **card play and the leader's trump
announcement**. Bidding, passing, questions and answers stay in `Game`; questions carry
information and are handled at the belief level.

## Rules have one owner
| Rule | Owner | Used by |
|---|---|---|
| Legal cards | `bits::play_levels(trick, trump, first_trick).allowed(hand)` | `fast`, `Game` (`legal_cards`), `cards::allowed_cards`, knowledge (`excluded_by`) |
| Trick winner | `bits::trick_winner(trick, trump)`, `bits::takes` | `fast`, `Game` (`act_card`), `cards::high_card` |
| Card, pair and last-trick points | points (`VALUE_POINTS`, `CARD_POINTS`, `PAIR_POINTS`, `LAST_TRICK_BONUS`, `points_set`) | `fast`, `Game` |
| Running team points (cards, pairs, tricks) | `team_points` | `fast::from_game`, `GameFinishedInfo`, devtool |
| Game and series scores | scoring (`game_scores`, `SeriesTally`) | `GameFinishedInfo::scores` |

## State
`FastState` takes 56 bytes and is `Copy + Eq + Hash`. Seats are 0–3 clockwise; team = seat % 2.
Cards are bit indices `suit * 9 + value` (`Card::index`, `Card::from_index`). All names below are imported from the crate root (`engine::FastState`, `engine::CardSet`, …).

| Field (via accessor) | Meaning |
|---|---|
| `hands()` / `hand(seat)` | `[CardSet; 4]` |
| `trick()` | cards of the current trick in play order (0–3); unused slots are zero, so equal states hash equally |
| `leader()`, `to_move()` | the leader of the current trick and the seat to move |
| `trump()`, `called()` | trump (`Option<Suit>`); suits already announced (bit `Suit as u8`; each suit once) |
| `may_announce()` | seats whose trump right is still `Own` (bit = seat). Questions change it in `Game`; `fast` only reads it |
| `tricks_done()`, `tricks_won(team)` | tricks played, tricks per team |
| `card_points(team)` | card points including the 20 for the last trick |
| `pair_points(team)`, `points(team)` | announced pairs; card + pair points |
| `in_hands()`, `is_over()` | cards still held; all cards played |
| `trick_winner()` | seat winning the current trick so far |

The first trick (`tricks_done == 0`) uses the first-trick rules (ace / green lead, the led ace
must be played).

## Moves
- `legal_cards() -> CardSet`: cards the seat to move may play.
- `announcements() -> u8`: suits the leader may announce now (bit `Suit as u8`). Allowed at a trick
  start (`Game` phase `StartTrick`, not before the first trick), for an own King + Ober pair not
  announced before, while the leader's right is `Own`.
- `moves() -> Moves`: an allocation-free iterator, announcements first, then cards in index order.
- `play(card)`, `announce(suit)`, `apply(Move)`, `after(Move) -> FastState`. Illegal moves are
  checked only in debug builds.

An announcement makes the suit trump, adds `PAIR_POINTS[suit]` to the leader's team and closes
the trick start (the leader then plays a card). A completed trick credits points and the trick
to the winner's team, and the winner leads next.

## Make and undo: copy
```rust
for c in s.legal_cards().iter() {
    let mut next = s;            // 56-byte copy
    next.play(c);
    value = value.max(search(&next, ...));
}
```
Copy-make measured equal to a make/unmake variant within noise (perft, 3–4 tricks left), so
copy was kept and the unmake was removed (performance.md, log entry 10). A full tree walk costs
about 17 ns per play.

## Building states
- `FastState::new(hands, leader)`: the start of card play (first trick, no trump, all seats
  `Own`).
- `FastState::from_game(&Game) -> Option<FastState>`: phases `Raising`, `Trick`, `StartTrick`
  and `Ended`. Points and tricks per team come from `scoring::team_points`.
- `FastState::from_parts(FastParts) -> Result<FastState, GameError::InvalidState>`, and
  `parts()`: all fields as a plain struct, for sampled worlds (knowledge → sampler → `FastParts`).
  This checks disjoint cards, hand sizes against `tricks_done` and the trick, and that the tricks
  add up.
- `Move::to_action(seat)` and `Move::from_action(&ActionType)` convert to and from `Game`
  actions.

## For the solver (notes, not built here)
- **TT key at trick starts:** `hands()`, `leader()`, `trump()`, `called()`, `may_announce()`.
  Points so far are path data and stay outside the key (Investigator B §2.3).
- **Card equivalence:** `CARD_POINTS` and `Card::index` order (adjacent indices are adjacent
  ranks within a suit).
- **Scores:** for leaf utilities, map `points(team)` and `tricks_won(team)` through
  `game_scores` (one owner of the objective).

## Guards
- `tests/fast_vs_game.rs`: 5,000 random games with bidding, passing, questions, answers,
  announcements (biased up) and undo. At every card-play state (212,215 states, 181,852 cards,
  2,002 announcements, 45,446 tricks) it checks:
  - the legal cards and announcements equal `Game`'s;
  - a state stepped alongside equals one freshly converted from `Game`;
  - `parts()` round-trips;
  - the final points and tricks equal `GameFinishedInfo`.
- `tests/alloc_guard.rs`: a whole game of `fast` card play allocates nothing.

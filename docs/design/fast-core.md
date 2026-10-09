# Fast play state (`FastState`)

`FastState` is the engine's compact rules API for search and simulation.
It covers card play, self-announcements, questions and answers. Bidding,
passing, raising decisions and undo orchestration remain in `Game`.
Search and pruning belong to `solver`, not this engine module.

## Rules have one owner

| Rule | Shared owner |
|---|---|
| Legal cards | `bits::play_levels(...).allowed(hand)` |
| Trick winner | `trick_winner`, `winning_seat`, `takes` |
| Card, pair and last-trick points | Points tables; `trick_points`, `trump_points` |
| Question rights | `PlayerTrumpPossibilities::{may_announce, may_ask_pair, after_question}` |
| Available uncalled pairs and truthful answers | `rules::{uncalled_pairs, question_answers}` |
| Trump resulting from an answer | `rules::answer_trump` |
| Running team points | `scoring::team_points` |
| Game and series scores | `game_scores`, `SeriesTally` |

Both `Game` and the compact state use the shared answer rules. A half answer
changes trump only when the asker holds a counterpart. Previously called suits
can be restored without awarding their pair points again.

## State

The state remains 56 bytes (`Copy + Eq + Hash`). Seats are 0–3 clockwise,
team = seat % 2. Cards use bit index `suit * 9 + value`.

| Accessor | Meaning |
|---|---|
| `hands()`, `hand(seat)` | Four card sets |
| `trick()` | Current cards in order; unused storage is zero |
| `leader()`, `to_move()` | Trick leader and actor; while answering, actor is the leader's partner |
| `trump()`, `called()` | Current trump and suits already scored |
| `rights(seat)`, `rights_code()` | Own / Yours / Ours per seat, packed into eight bits |
| `phase()` | Cards, open Start, or Answer(question) |
| `tricks_done()`, `tricks_won(team)` | Completed tricks |
| `card_points(team)`, `pair_points(team)`, `points(team)` | Card points (including last-trick bonus), pair points and total |
| `in_hands()`, `is_over()`, `trick_winner()` | Remaining cards, terminal state and current winner |

The first trick retains the engine's ace/Green lead restrictions. Questions and
announcements open only after a completed trick, never before the first.

## Moves and transitions

`moves()` is allocation-free: announcements/questions/answers first, then cards.
`legal_cards()` is empty while answering. `announcements()` lists uncalled own
pairs only while the leader may self-announce at an open trick start.

`apply(Move)` handles all four move kinds; `after(Move)` applies to a copy.
`play(card)` and `announce(suit)` remain available. Moves must be legal;
debug builds check them.

A question advances only the asker's rights and transfers action to the partner.
The answer returns action to the asker, closes the question opportunity and may
change trump/score a pair. The answerer's own rights are preserved. A completed
trick scores its cards, transfers the lead and opens the next trick's opportunity.

Copy-make remains the undo mechanism: keep the original state and search
`state.after(mv)`. No allocation or separate undo log is needed.

## Construction and encoding

- `new(hands, leader)`: first trick, no trump, all rights Own.
- `from_game(&Game)`: Raising, Trick, StartTrick, AnsweringPair,
  AnsweringHalf and Ended; other phases return None.
- `PublicState`: all fields except hands, including full rights and question
  phase. Its `to_move()` follows the same rules as the compact state.
- `from_public(public, hands)` checks card disjointness, hand sizes, trick
  counts, phase placement and the asker's post-question rights.
- `FastParts`, `parts()` and `from_parts` provide the equivalent combined form.
- `PlayPhase::from_actions` derives phase from the actor's legal actions.
- `Move::code/from_code` use 58 codes: cards 0–35, announcements 36–39,
  pair question 40, half questions 41–44, NoPair 45, YesPair 46–49,
  NoHalf 50–53, YesHalf 54–57.
- `Move::to_action/from_action` convert to/from full engine actions.

The old own-right bit mask and announcement-open boolean were replaced by full
rights and phase; no parallel legacy representation remains.

## Guards and consumers

`fast_vs_game` compares all supported legal actions and stepped states against
full `Game` across 5,000 seeded random games, including questions, answers and
undo. It checks state round trips and final points/tricks. Curated
`question_rights` traces also compare both representations; `alloc_guard`
keeps compact simulation allocation-free.

The [Game Solver](solver.md) owns cache keys, bounds and safe move reductions.
Its key includes full rights and pending question; these distinguish states
even when no pair is currently self-announceable. Older engine performance
experiments remain in [performance.md](performance.md); they are not new
question-complete search measurements.
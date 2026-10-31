# Game Solver

The Game Solver searches continuations; the engine owns rules and transitions.
This is the question-complete implementation of 2026-10-09. See the
[checkpoint](../checkpoints/2026-10-09-solver-questions.md) for evidence and open gates.

## Scope

- `Solver` solves a **known deal** by team minimax.
- `evaluate` compares the deciding seat's legal moves across deals consistent
  with its knowledge. `evaluate_limited` adds a shared deadline/node budget.
- Inputs are `Knowledge`, `PublicState`, objective, belief weights and an RNG.
  Actual hidden hands are not an input to world evaluation.
- The engine supplies card plays, self-announcements, questions and every
  truthful answer choice. Questions change only the asker's rights. Answers can
  establish trump only in a previously unannounced suit. A called suit cannot
  become trump again.

Exact search inside each exposed deal is not an exact hidden-information
strategy: future choices can differ using cards the real player cannot see.
Enumerating every deal does not remove this strategy-fusion limitation.

## Values and search

`value(state, team)` returns that team's future card points, last-trick bonus
and reachable pair points against best defence. Add its existing points for a
final total. `reaches` checks a future-point threshold with a zero-window search.
`tricks` and `reaches_tricks` use the same search for trick counts.
`move_values` includes every legal root action and its immediate gain.

Search uses fail-soft alpha-beta with principal-variation (zero-window) probes after the
first move, a static upper bound, move ordering and a fixed-size transposition table. The
bound is the card points not yet awarded (conserved through the search, so not summed per
node), the last-trick bonus and the team's reachable uncalled pairs. The default table is 2^18 entries of 40 bytes
(10 MiB); the heuristic agent uses 2^16 entries (2.5 MiB).

At an empty trick, the key contains all hands (the zero-point ranks 6–9 of each suit packed
by their order and owner, scoring ranks literal; the stored best move is translated with
them), leader, trump, called suits,
**all three rights levels per player**, phase/pending question, team and measure.
Existing points/tricks are path data; cached values describe the future.
The hands determine how many tricks remain. Completed bounds survive between
decisions and after interruption; unfinished parents are not stored as exact.

The point upper bound includes remaining card points, the last-trick bonus and
every uncalled pair whose halves remain in the team. It deliberately
overestimates activation rather than incorrectly excluding partner/split pairs.

Two reductions apply inside a known-deal search:

- Adjacent live ranks held by the mover are equivalent if they score equally.
  Trick cards remain separators; pair cards have unique point values.
- A question can be skipped if every truthful answer leaves both trump and
  called suits unchanged: with exposed hands it adds no information and can
  only lose rights. This reduction does **not** remove legal root questions
  from world evaluation or claim equivalence for information-set search.

## World evaluation and interruption

If the deal count fits the requested world count, enumerate it; otherwise
sample uniformly and apply belief importance weights. Every candidate is
compared on the same completed worlds.

A world contributes only after **all** root moves and criteria finish.
An unfinished world contributes nothing. If none finishes, return
`SolverError::Interrupted`; the heuristic continues with its existing rules.
A node cap is exact; search checks the clock every 64 nodes and before each
root solve. Deal-space preparation and sampling are not themselves interruptible,
so this is not a hard real-time guarantee for arbitrary inputs or world counts.

`Decision` reports completed `worlds`, effective sample size `ess`,
`complete` (all requested comparisons finished), and `all_deals` (enumerated
and complete). The former `exact` label was removed. A partial result is a
prefix of the requested worlds, not a calibrated probability or proven bound
on the hidden-information game.

Criteria are compared lexicographically:

| Objective / situation | Criteria, most important first |
|---|---|
| Points | Own final points |
| Reach | Whether the named team reaches its target |
| Declarer, open contract | Contract reached; then own points |
| Declarer, contract already made | Minimise defenders' points |
| Declarer, all compared worlds prove every move loses | Minimise defenders' points in a second pass |
| Defender | Own points |

A team with no trick gets a first-trick criterion when engine scoring penalises
being trickless. The loss-fixed second pass runs only after all primary
comparisons finish; if interrupted, retain the complete primary comparison.

Since 2026-10-31 the agent searches from seven tricks left within one million solver nodes
per decision (`solver::Effort`): a reach criterion is read from the same team's exact
points instead of a second search; racing (`solver::Racing`) drops moves two paired
standard errors behind after every eight worlds; a search whose nodes per world project past
its limit before 16 worlds stops early and leaves the decision to the rules. Under live
deadlines the search's own wall share is 65 ms. The rest of this section records the
earlier four-card design. The agent kept the four-card trigger and 64-world default, with a 45 ms search
deadline. It now also searches at question opportunities and multiple-answer
choices. Known-negative repeat questions are excluded by agent policy, while
remaining legal in the engine. Observations still update knowledge/beliefs
after every action; a cache hit does not freeze the player's information.

## Validation and measured cost

The current guard suite compares compact transitions with `Game` across 5,000
random games, search with plain exhaustive minimax across 2,300 small positions,
and 150 endings with minimax over full `Game`. Other cases check cache-key
separation, split-pair trump changes, the once-only announcement rule, hidden-card
swaps, enumeration, and interruption followed by reuse of the same cache.

Release measurements on the shared Ryzen 7800X3D PC: 20 fixed random-game
decisions per hand size, uniform deal weights, open contracts, 64 worlds,
agent-sized cache. Cold means an empty table; warm repeats the same decision
and RNG, **not** a measured whole-game follow-through.

| Cards held | Cold mean / max ms | Cold complete / partial | Warm mean / max ms |
|---|---|---|---|
| 4 | 2.83 / 11.77 | 20 / 0 | 0.23 / 1.40 |
| 5 | 13.09 / 45.01 | 18 / 2 | 2.58 / 25.62 |
| 6 | 33.25 / 45.01 | 13 / 7 | 11.73 / 45.01 |

On the same four-card inputs with 100 worlds and no deadline, contract mean/max
rose from 1.3/6.5 ms before question support to 2.3/10.2 ms after. This buys
additional legal branches; it is not a like-for-like speed optimisation.
These measurements cover world evaluation, not the agent's belief construction,
server scheduling or a guarantee for every position. A previous run of the same
four-card bounded workload measured 2.86 ms mean / 12.10 ms maximum.

Bounded calls also ran at seven, eight and nine cards. Cold complete/partial/no
completed-world counts were 0/20/0, 0/12/8 and 0/5/15 respectively. The maximum
was 46.31 ms. Supporting larger states does not make their partial estimates
reliable enough to enable full-game search by default.

The existing whole-agent `perf` tool also completed 100 self-play games (seeds
0–99, default parameters): 5,263 actions, 1,318 solver choices, three marked
incomplete. Whole decision latency peaked at 45.125 ms, including belief work.
This is a timing/legality smoke run, not a playing-strength comparison.

Run the existing ignored `speed::decision_time_by_worlds` benchmark to reproduce
both unlimited comparisons and bounded cold/warm timings. Its fixture generator
uses sequential seeds from 1 and 20 decisions at each hand size from four to
nine. Unlimited comparisons run only through six cards.

## Next boundaries

Measured 2026-10-31 (hill-climb log): an unbounded seven-trick search is worth about twice
the budgeted one, so node cost and ordering remain the levers. The earlier advice was to keep
the four-card default until broader whole-agent tail measurements justify
extending it. Information-aware continuation policies, midgame/return planning,
calibrated contract probabilities and GPU evaluation are separate checkpoints.
The existing contract objective also remains an approximation to full series
utility (for example, defenders' score rounding).

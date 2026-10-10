# AI architecture

This describes the current `fix/live-planning-review` branch. The
[testbed](https://github.com/Krarilotus/MarjapussiAI-testbed/tree/feature/takeback-claim)
owns the website, networking and persistence. All nine crates below live in
this AI repository; dependencies point toward the rules and visible information.

| Owner | What belongs here |
|---|---|
| `engine` | Legal actions, transitions, scoring and the compact play state used by search. |
| `observation` | Public events and private exchanges filtered for one seat. |
| `knowledge` | Certain holdings, constraints, voids and exact possible-deal calculations. |
| `beliefs` | Probabilities consistent with those facts, bid evidence and lazy belief queries. |
| `agent` | `Agent`, `SeatView` and the shared `Table` that delivers filtered observations. |
| `heuristic` | Combining concepts into bids, exchanges and plays; bounded return forecasts. |
| `solver` | Search inside known deals and comparisons across plausible hidden deals. |
| `eval` | Duplicate-deal comparisons and evaluation helpers; re-exports the shared table. |
| `tools` | Command-line studies, arena runs, replay adapters and profiling. |

## From a move to a decision

1. The engine applies a legal action. The table produces the events visible to
   each seat; an agent is never given the real hidden hands as evidence.
2. `Knowledge` updates exact constraints. `Mind` in `beliefs` combines them with
   uncertain signals, preserving facts at probability zero or one.
3. The heuristic reads the concepts it needs: for example, possible entries,
   reachable pairs, suit control and the points still needed. It can compare
   return-card candidates through seat-filtered continuations.
4. Near the end, the Game Solver can compare legal moves over deals consistent
   with the seat's knowledge. The agent returns a move and its explanation;
   the caller applies it through the engine.

The [concept catalogue](design/concept-store.md) maps these dependencies.
Its diagram is a historical design snapshot; some concepts remain proposals.
The implementation uses typed Rust helpers, not a generic Bayesian-network
executor or a runtime graph database.

## State and reuse

Each heuristic agent retains its own `Mind`. Facts and signal changes invalidate
only the associated lazy caches; unchanged queries reuse their results. The
agent accepts an incremental history only when the seat, starting hand, seat
kinds, belief settings and recorded prefix still match. Otherwise it rebuilds.

The solver retains completed search bounds in a bounded transposition table.
The heuristic agent continues to update its Mind from visible events; reusing
solver bounds does not freeze its beliefs. Agent lifetime and cache eviction
are the caller's responsibility. The public testbed keeps agents
in its application-owned Rust service, independently of browser connections.

## Correctness boundaries

- Rules and scoring have one owner: `engine`. Search and planning call it.
- Conventions are uncertain evidence; they cannot overturn certain facts.
- Solving a known deal exactly does not solve hidden-information strategy
  exactly. Future hypothetical choices may depend on cards a player cannot see.
- Bounded return forecasts are policy simulations, not proven game plans.
- Passing intent, post-exchange raising and series-aware strategy remain
  separate work. No playing-strength gain follows from architecture alone.

See [solver limits](design/solver.md), [passing](PASSING.md),
[evaluation provenance](EVALUATION.md) and [next steps](NEXT_STEPS.md).

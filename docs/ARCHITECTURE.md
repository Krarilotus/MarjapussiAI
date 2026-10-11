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
   uncertain signals. Joint concept queries return hard proofs separately from
   model probabilities and unresolved bounds.
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

One continuation comparison also caches completed candidate outcomes and
decisions at identical seat views. Reuse requires matching knowledge, visible
history, legal actions, seat kinds and series state under unchanged parameters.
Timed endgame decisions are recalculated. This lets a raising comparison reuse
its unchanged baseline while evaluating the proposed contract separately.

## Facts, beliefs and another player's information

`Mind::concept` evaluates joint card, void, pair and count predicates, optionally
under stated conditions. `knowledge` counts equivalent card allocations;
`beliefs` supplies their joint bidding and exchange weights. A completed query
sums those weights. A truncated query retains bounds for unresolved mass.

`Mind::perspective_concept` asks what another seat can know or believe from its
own observations. Its result distinguishes a proof that the viewer knows a
fact, an expected viewer-model probability, and truth under a hypothetical
condition. Public deductions and shared transfers can settle the proof directly.
Otherwise, possible private hands supply separate viewer models.
Knowledge here means what that seat's information proves; human attention and
playing skill remain part of the actor model.

For example, an off-suit play may prove a void publicly. The giver and receiver
also know a passed trump remains held until subsequent play or transfer removes
it. A ruffing continuation can combine that knowledge with uncertain opponent
holdings. The next trick updates every view through the shared table, so a
consumed trump cannot continue supporting the plan.

Queries use deterministic allocation budgets and cache their inputs. Complete
joint estimates feed pair-protection and trick-winning calculations. Unresolved
queries retain the existing policy estimate. Full-game forecasts still use
weighted deal simulations; exact conditional holding probabilities describe
the belief model, while future choices depend on the continuation policy.

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

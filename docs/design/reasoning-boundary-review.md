# What exists, what is missing, and why

**The foundations are usable. Strategic composition is the gap.** The current
system has exact knowledge, correlated holding queries, actor-specific views,
simulation and caching. It does not have an authored strategic graph that answers
“what should I do next?” through those dependencies.

The strategic audit describes source revision `8ea7a6a` (from `4acefbb`). The
subsequent value-store slice moves score integration, contract-event estimates
and candidate ranking into executable typed dependencies. The continuation leaf
still runs the legacy response approximation, so the strategic gaps below remain.
The implemented value graph is exported from the evaluator on a synthetic fixture;
its numbers illustrate the calculation and are not live-game estimates.

## The mismatch in our work

Your instruction was to generalise the evaluator while carefully modelling the
game’s dependencies. The work instead added procedural strategy helpers, then
described their inputs as a concept catalogue. A sequence executor was proposed
without first making the strategic choices themselves part of the authored model.

The diagrams compounded that error: they presented code modules, work limits and
performance observations alongside game concepts. That made a source inventory
look like a reasoning model. The phrase “deterministic action means 100%” also
answered the wrong question. You wanted estimated consequences and the evidence
for a decision, not how often a deterministic selector repeats its answer.

The correction is an architecture gate: **changing an authored strategic relation
must change the actual evaluation and trace without adding another tactic-selecting
callback.** A diagram reconstructed after a helper has selected its move fails
that gate. The first implementation should pass it on one fully inspectable slice
before wider policy changes.

## Current ownership and the proposed boundary

| Part | Current capability | What the proposed slice changes |
|---|---|---|
| Rules engine | Legal actions, declaration rights, trump transitions and scoring | Remains the single rule authority. |
| Observation and knowledge | Seat-visible evidence, exact exclusions, counts and proof | Anchor every real and hypothetical query. |
| Belief concepts | Card, Void, Pair and Count; joint conditions; proof and probability separated | Remain primitive inference operations beneath strategic dependencies. |
| Actor perspective | Root’s evidence of another seat’s knowledge and expected belief | Supplies response predictions without reading another live agent’s private store. |
| Heuristic policy | Procedural bidding, transfer, announcement and card decisions | Covered choices become authored alternatives evaluated under one score objective. |
| Continuation forecast | Filtered actor simulation and final engine-score comparisons | Executes the same conditional definitions; response approximations remain explicit. |
| Reuse | Lazy deal model and bounded query/decision caches | Adds dependency-aware strategic results and intent-aware decision reuse. |
| Evaluation tools | Replay, counterfactual continuations and comparisons | Consume the actual assessment and trace instead of reconstructing a strategic explanation. |

## Concrete deficiencies and their implications

| Source-backed finding | Why it matters | Proposed correction |
|---|---|---|
| Holding queries do not represent future pair activation. | A held pair can lose its legal opportunity or entry. | Model possession → opportunity → actor response → legal activation → remaining control. |
| Announcement timing uses activation proxies. | The proxy does not compare complete alternative activation routes. | Compare routes under shared joint evidence and actual engine transitions. |
| Auction forecasts compare the baseline pass with the lowest legal raise. | Stronger contract choices can be absent from the comparison. | Record candidate coverage; extend the same value query through bidding after the first slice. |
| Forecast actors run reduced policy settings. | A promising simulated route can diverge when the live policy reconsiders. | Version response levels and test root execution separately from response-model error. |
| The forecast memo stores a decision. | Once intent persists, an action-only hit can omit a required state transition. | Cache the proposed action and intent transition; commit only after accepted history. |
| Hand capacities connect suits. | A card or signal can change beliefs outside its suit. | Invalidate by semantic dependencies; retain broad invalidation when independence is unproved. |

Source owners: `crates/beliefs/src/concepts.rs`, `perspective.rs`, `deals.rs`;
`crates/heuristic/src/lib.rs`, `trump_plan.rs`, `planning.rs`, `planning/cache.rs`;
`crates/engine/src/game/rules.rs` and `scoring.rs`. The accompanying graph’s
**Current** view shows the implemented holding-query boundary. **Proposed** shows
strategic definitions for review, not an execution trace.

## How to inspect the graph

Start at **Best next move**. Inputs point upward to their consumers. The graph
is a DAG: each definition appears once, with all visible direct dependencies
connected to that same node. Select a node to highlight its connections and read
both **Requires** and **Used by**. **Focus on dependencies**, **Show more**, **Back**
and **Full graph** change the projection without duplicating nodes.

Colors identify roles: knowledge in context, rules, belief estimates, hypothetical
continuations and decisions. **Result meaning** describes the proof/estimate
boundary. The expected final score is a belief-based prediction. Exact scoring
under an assumed outcome does not make that outcome known. A current holding
query can return a hard proof and model probability in separate fields.

The main line is joint trick outcomes → changed cards/rights and observations →
next leader's opportunities → subsequent tricks → final game scores → their
belief-weighted difference. The current `routes.rs` computes goal-specific
capabilities in an exposed deal; overlapping capabilities can require different
plays, so they are not mutually exclusive trick-outcome probabilities. Current
`planning.rs` simulates legal continuations and records winners afterwards. It
does not encode that strategic dependency chain in the concept store.

One visible definition is not a promise of one runtime calculation across every
future state. Queries with equivalent relevant bindings reuse a result; changed
hands, observations or models require distinct instances. Their context belongs
in the key. Runtime budgets and cache counters stay assessment metadata.

## Review decisions before implementation

The expected-score-difference objective, knowledge anchor, privacy boundary and
need to revise plans are settled requirements. The remaining review concerns are:

1. Whether the proposed operators expose the important strategic dependencies
   instead of concealing choices inside primitives.
2. The first slice’s action and continuation coverage, and its finite response model.
3. What to do when comparisons remain unresolved, with numerical guarantees and
   fallback behaviour stated explicitly.
4. Whether the worked example and acceptance checks are sufficient to start the
   smallest implementation.

This review supplies no new 4,000-game result or performance claim. The research
assessment is available in the previously delivered
`Marja-Research-2026-10-11/index.html` folder.

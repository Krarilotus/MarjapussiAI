# Cloud research prompts: adaptive sequences and efficient reasoning

These are two independent research tasks for the
[conditional-sequence design](conditional-sequences.md). Each prompt includes
enough context to run without repository access. Findings should refine an
implementable design, not be treated as approval to replace the current stack.

## Task 1: adaptive planning under partial information

```text
Research a practical architecture for adaptive conditional planning in a
four-player partnership trick-taking card AI written in Rust. Use primary
research papers, original algorithm descriptions and authoritative project
documentation. Cite direct sources and distinguish published results from your
recommendations. You do not have our repository; do not invent source findings.

Game and architecture:
- 36 cards, four suits, partners opposite. Players bid a contract and exchange
  cards. Pairs of King/Ober have suit-dependent values; legal declarations and
  questions can establish/change trump. Each suit can establish trump once.
  Following, overtaking, rights and scoring are enforced by an existing engine.
- Observation/knowledge own seat-filtered history and exact deductions. Beliefs
  own a joint weighted distribution over consistent holdings, conditional queries
  and a bounded model of what each other player knows/believes. Proof includes
  hard-consistent possibilities with zero soft-model weight. A solver provides
  legal next-trick capability and small exposed-deal endgames.
- The heuristic evaluates actions with bounded full-game continuations. We want
  composable sequences: preserve resources -> remove ruff threats -> transfer
  lead -> activate a pair -> retain control. Knowing a partner has/knows a card
  is not proof that it will cooperate. Each actor receives only its own view;
  private plans are not shared between seats.
- Observed failures: preserving a pair without retaining its activation route;
  a safe trump draw is evaluated but misranked because its continuation is lost;
  late pair repairs gain points but cannot rescue bad earlier allocation/bidding.
- Proposed baseline: a small typed conditional program, a private persistent
  receipt for selected intent, and the same executor in forecast/live play.
  Forecast selection uses finite response-model levels to avoid recursive search;
  it cannot assume another actor's unknown live intent.
- Preparation may take a second or more. Ordinary play must be fast. New evidence
  must cause both invalid-plan repair and replacement of a STILL VALID plan when
  a better route becomes available. Reuse cannot mean stubborn commitment.

Compare at most four relevant approaches: contingent policies/AND-OR planning,
belief-state online planning, temporal abstraction or options, and plan repair
or hierarchical planning where applicable. Explain which assumptions fit and
which break under imperfect information and independent partner decisions.

Deliver:
1. One recommended minimal hybrid and rejected alternatives, with explicit
   ownership in the existing layers. Avoid a generic planner rewrite.
2. Typed representation and pseudocode for formation, observation update,
   likelihood-only changes, opportunity discovery, suffix repair, goal replacement
   and bounded execution. Separate feasible from valuable and proved from modeled.
3. How to estimate complete sequences without multiplying correlated marginal
   probabilities, selecting actions using hidden simulation state, double-counting
   scores, or treating capability as cooperation. State approximation boundaries.
4. A policy for unfinished reviews and avoiding oscillation WITHOUT inventing
   arbitrary switch penalties or preserving sunk effort as value.
5. Three adversarial examples, including a still-valid plan that should change,
   and concrete tests/ablation measures. Explain worst-case cost and limitations;
   do not claim exact planning over all hidden deals is automatically cheap.
6. A small staged implementation plan compatible with a Rust engine and existing
   belief/solver modules; identify what old logic would be replaced.
```

## Task 2: memoization, invalidation and lazy probability computation

```text
Research a correctness-first incremental computation design for a stateful Rust
card-game planner. Use primary papers, original algorithms and authoritative
documentation. Cite sources. Distinguish exact reuse from approximation and
published performance evidence from unmeasured recommendations. No repository
is available; reason from this supplied architecture only.

Existing infrastructure and constraints:
- Four actors reason separately from their own hand and filtered event history.
  Knowledge stores exact card constraints; beliefs provide joint conditional
  holding queries, lazy deal-model construction and models of another actor's
  knowledge. Solver transpositions cache exposed-deal bounds. The engine owns
  legality and score. All are existing Rust components, not rewrite targets.
- The policy will carry a bounded private receipt for its selected conditional
  sequence, cursor, reserved cards/rights, pending action and evidence/config
  identity. This is semantic state, separate from disposable caches. Receipts
  commit atomically with accepted actions under the existing game's revision
  checks, survive session eviction/restart and restore correctly after undo.
- Current forecasts already reuse actor/solver allocations and memoize decisions
  by complete honest view/history/legal actions/kinds/series. The current memo
  returns only an action: the new design must key on receipt and reproduce the
  resulting receipt transition too. Candidate action plus continuation matters.
- A new card changes hand capacity and potentially posterior probabilities in
  every suit. An unchanged suit mask does NOT justify reusing its probabilities.
  Fact proof, conditional proof, weighted estimate and unresolved mass differ.
- Preparation may take a second or more; ordinary play should be low milliseconds.
  New beliefs/play must reconsider a plan's value, not just its feasibility.
  Existing node limits do not cover upstream DP/model construction. Warm solver
  tables can change completed search under a node or wall-time cap.

Evaluate demand-driven dependency tracking, incremental probabilistic inference,
factored/exact equivalence classes, bounded memoization and reusable search
frontiers. Recommend the smallest useful combination; do not propose a distributed
cache, shared private minds, or a generic graph service.

Deliver:
1. A cache-key/value/owner/lifetime/invalidation table for exact facts, joint
   probabilities, actor perspectives, sequence suffixes, stateful decision
   transitions and solver bounds. Include collision checks and partial results.
2. Precise conditions for local invalidation versus whole-posterior invalidation,
   and sound class merging/splitting for lazy future evaluation. Explain where
   correlations or changed observation likelihoods defeat the shortcut.
3. A bounded work scheduler that spends effort where it can change the decision,
   including rare high-loss cases. Preserve unresolved mass; do not normalize the
   most likely visited cases into certainty. Explain unsupported model cases.
4. Contracts for semantic receipts versus disposable computation, branch isolation,
   undo, cancellation, retry and restart. Separate cache-disabled parity at fixed
   logical work from timing-sensitive search results; handle warm transpositions.
5. Pseudocode for an event update and lazy read, including a still-feasible plan
   whose old value becomes stale. Bound both work and bytes across nested calls.
6. A benchmark/test plan: equal-work uncached/cold/warm runs, injected invalidation
   and collision cases, completed coverage, p50/p95/p99/max by phase, memory and
   fallback frequency. Speedups from smaller search are policy changes, not
   semantics-preserving cache improvements.

End with a prioritized migration and the situations where your design cannot
meet fast play without precomputation, an explicit fallback, or a different
approximation. State those tradeoffs candidly.
```

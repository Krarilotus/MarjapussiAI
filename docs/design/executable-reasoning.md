# Executable strategic reasoning

Status: **contract proposal for review**, based on `8ea7a6a`. The current policy
still selects through procedural helpers and bounded forecasts. This contract
defines the replacement boundary; it introduces no playing-strength claim.

Review with the [worked decision](worked-decision.md),
[current-system audit](reasoning-boundary-review.md) and
[graph definitions](reasoning-graph.json). Build the portable HTML with
`python docs/assets/build_reasoning_review.py --output <review-folder>`
(requires `markdown-it-py` and Graphviz).

The agent asks: **which legal next action has the greatest expected final game score
difference, given my information and the stated continuation model?** The same
computation supplies its explanation, counterfactual comparisons and evaluation
record. Contract success is a named outcome probability, separate from that
objective and from the deterministic selection of a move.

## 1. The knowledge anchor

Each seat starts with its permitted observations and exact deductions. Let
`K(I)` describe all states consistent with that information. A hard fact holds
in every such state; a contradiction is an integrity error, not a reason to
silently relax the facts. The engine remains the authority for legal transitions,
rights, card and pair values, contract scoring and series scoring.

Beliefs assign weights within `K(I)`, using the specified convention, exchange
and response models. They cannot make a logically excluded state possible or
turn a zero-weight alternative into a logically impossible one. A hypothetical
condition narrows a query; it does not mutate the actual knowledge state.

The concept store is the common query surface over these layers. Its strategic
definitions have stable identities, typed results and explicit dependencies.
Existing holding queries and proof operations are leaf evaluators beneath it.
Strategic composition belongs in that authored model. The uncertainty layer
continues to own probability, and the rules engine continues to own legality.

The scoring rule is known, but an expected final score is a **model estimate**.
For a fully specified terminal branch the engine computes the score exactly,
conditional on that branch occurring. Weighting these conditional scores by
estimated branch probabilities retains that uncertainty. Deterministic arithmetic,
a fully evaluated model, or a model probability of 1 does not turn an estimate
into a fact. Proof, conditional exactness, estimate and unresolved status travel
with the result and its scope; the node's computational role is a separate field.

## 2. What is authored and what is evaluated

| Authored domain model | Common evaluator |
|---|---|
| Named concepts, parameters and result types | Instantiates and type-checks queries |
| Prerequisites, alternative routes and counterplay dependencies | Requests dependencies lazily and shares equivalent results |
| Legal action/observation transitions and continuation structure | Applies engine transitions and partitions by permitted observations |
| Which uncertain events and actor responses a route requires | Propagates their conditional joint mass |
| Terminal objective and named diagnostic events | Aggregates outcome values and compares candidates |
| Progress conditions and the scope of a candidate family | Refines unresolved comparisons, advances or repairs retained plans |

The recommended authoring surface is a small, typed Rust definition table with
parameter bindings and indexed node references. It fits the existing stack and
allows static checking. This is a proposal, not a commitment to a graph library,
runtime scripting language or graph database. The same definitions should export
the review graph and trace vocabulary; the visualizer must not maintain a second
strategy catalogue.

An opaque callback such as `choose_announcement()` cannot stand in for the graph
that compares announcement routes. Primitive evaluators may efficiently implement
rules, exact deduction, joint counting or a declared solver boundary. Their
semantics and result guarantees must be inspectable. Existing strategic helpers
remain only outside the migrated scope or as an explicitly labeled baseline;
they cannot silently override the graph's selected action inside that scope.

## 3. Typed query and result contract

These are proposed semantic types, not APIs already implemented:

```text
QueryKey = (definition_version, concept, bindings,
            observer_information, hypothetical_context, model_versions,
            objective, incoming_intent, numerical_guarantee)

TruthEvidence = ProvenTrue(proof_ref)
              | ProvenFalse(proof_ref)
              | Undetermined(reason, possible_true_witness, possible_false_witness)

Estimate<T> = (value_or_bounds, coverage, assumptions, dependency_refs)

DecisionAssessment = (candidate_scope, action_values, selected_action,
                      ranking_status, intent_transition, trace_ref)
```

`observer_information` identifies the seat, its permitted history and own cards.
`hypothetical_context` distinguishes real evidence, assumed conditions and future
action/observation branches. Structural identities may be interned or hashed;
collision resolution and full semantic equality remain correctness requirements.
Execution budgets and cache occupancy are operational metadata, not concepts
about game quality or part of the semantic meaning of a fact.

| Result | Meaning |
|---|---|
| Proven predicate | Holds throughout the applicable hard-consistent support |
| Conditional holding estimate | Probability under a named joint belief model and explicit premise |
| Future-event estimate | Probability under that belief model, candidate intervention and actor-response model |
| Outcome distribution | Engine scores and named events across disjoint modeled continuation outcomes |
| Expected score difference | Weighted `score(us) - score(opponents)` at the declared horizon |
| Unresolved result | Work, model support or numerical guarantees are insufficient for the requested conclusion |

Proof considers hard-consistent states even if the soft model gives them zero
weight. An impossible premise and a premise with zero model mass are distinct
unavailable conditions. Neither yields a usable success estimate. Complete
evaluation of an approximate model is not proof about the real players.
Numerical rounding guarantees are explicit; a floating interval is not certified
merely because it is called a bound.

A counterexample to universal truth establishes “not proved true,” not “proved
false.” `ProvenFalse` requires falsity throughout the scoped hard support.
Negation swaps the two proven states and preserves undetermined status.

## 4. Composition operators

The first implementation should support the smallest set needed by the worked
slice. This table specifies semantics before choosing storage or dispatch:

| Operator | Inputs and result | Required behavior |
|---|---|---|
| `Known` | Constraint/predicate query -> evidence | Delegates to exact knowledge and engine rules |
| `HoldingEvent` | Event expression, premise -> joint estimate | Evaluates the event against one coherent conditional distribution |
| `All`, `Any`, `Not` | Compatible predicates/events -> event expression | Preserves shared variables; does not multiply or add marginal probabilities |
| `Bind` | Definition, suit/seat/card bindings -> instantiated dependency subgraph | Enumerates declared parameter domains; does not pick a preferred tactic |
| `Alternatives` | Applicable action/continuation definitions -> candidate set | Retains candidate scope and reasons for exclusion |
| `Apply` | Legal action, conditional state -> successor and observations | Uses the engine; consumes the actual cards/rights exactly once |
| `Response` | Actor-visible context, response model -> action distribution | Predicts that actor from its information, not root-only knowledge |
| `Observe` | Successors -> information partitions | Groups indistinguishable future situations before subsequent root decisions |
| `GuardObserved` | Actor-visible condition -> selected policy branch | Branches on a visible event, proof or explicitly defined posterior criterion; unknown guards have an explicit successor |
| `Perspective` | Viewer and predicate -> scoped knowledge/belief reading | Separates proof that the viewer knows from expected viewer belief |
| `Then` | Transition/observation branch, continuation -> continuation | Carries changed resources and conditioning evidence forward |
| `Until` | Repeated continuation, goal, progress measure -> finite continuation | Requires monotone game progress; rejects zero-progress recursion |
| `EngineOutcome` | Terminal state -> scores and diagnostic events | Delegates scoring; a nonterminal leaf must remain separately estimated or bounded |
| `EventWitness` | Engine trajectory, named event -> membership | Identifies actual activation, entry or contract success without adding a utility bonus |
| `Integrate` | Disjoint weighted outcomes -> expectation/event mass | Preserves unresolved mass and uses the correct denominator |
| `Best` | Candidate values under one objective -> assessment | Compares consistent evidence and reports the scope/completeness of its ranking |

Evidence edges describe what must be read to evaluate a node. Temporal edges
describe a legal action followed by its observations and later choices. They have
different types and different update semantics. The review renders the definition
dependencies as a DAG: each definition appears once. The inspector describes its
temporal program and successor bindings; evidence arrows alone do not advance play.
Repeated future steps instantiate the same definitions in new contexts. Only
equivalent relevant inputs may share a runtime result.

Each call also binds its successor context and trajectory measure. Retained
control receives the post-activation state, probability integration receives the
specific candidate's observation branches, and scoring receives its terminal
trajectories. These bindings are part of the definition and cache identity;
sharing a node name never licenses sharing results across incompatible contexts.

The existing aggregate `ConceptQuery` result is not a distribution from which
continuations can be recovered. The integration boundary must carry grouped joint
support and multiplicities from `DealSpace` and weights from `DealModel` into
temporal evaluation. Refinement preserves that measure; several scalar query
answers cannot substitute for it.

Legal actions are the ground set. Authored strategic families organize useful
continuations and their relevant refinements; they do not prove that omitted
families are worthless. Exact dominance requires sound current-context bounds.
Heuristic ordering may prioritize work, with exclusions and unresolved comparisons
recorded. A result can be best among completed candidates without being proved
best among all legal continuations.

### The trick is the continuation boundary

The main strategic dependency is **who wins the next trick and what remains**.
Each joint branch contains the winning seat, captured points, spent cards,
remaining rights and the observations each player receives. Those determine the
next leader's legal opportunities and updated beliefs. The continuation compares
cashing or developing winners, drawing trump, transferring the lead, activating
or delaying a pair, and disrupting an opponent's route.

Forced following/overtaking is a rule constraint; choices among legal cards use
an explicit actor-response model. That actor sees its own hand and permitted
observations, updated after every preceding card. A possible partner win is not
proof that partner chooses the necessary card. Two branches with the same winner
may consume different entries, reveal different voids or lose different pairs;
they cannot be merged merely because their winners match.

Sequence definitions bind legal actions, joint trick queries, observations and
successor comparisons. The same trick definition is reused at later contexts,
with finite progress from card and declaration-right consumption. Complete
outcomes remain conditional records; their weights enter the final expectation
once. A score cannot be joined to another branch's probability or resources.

## 5. Probability propagation and future choices

The root candidate is an intervention, not evidence about the cards that would
have caused a rational player to choose it. For a sequence, branch probability
uses the initial joint belief and correctly conditioned response/observation
likelihoods. Shared cards, counts, rights and model assumptions remain connected.

```text
Q(action) = E[game_score(us) - game_score(opponents)
              | current_information, do(action), continuation_model]
```

At a later root decision, worlds with the same permitted observation history
share one choice. The evaluator must not choose a different root action per hidden
deal and average the resulting best scores. Other actors may act differently
when their own hands or observations differ; their predictions still obey their
own information boundary. This requirement applies before any claim of exact
continuation value.

Root models another seat's knowledge and beliefs through visible evidence and
private-hand hypotheses. It never reads that seat's actual backend Mind or plan.
Knowing that partner holds a card, knowing that partner knows a connection, and
predicting partner will use that connection are different dependencies.

The first worked slice must name a finite actor-response model. Production policy
reuse, a bounded evaluator of the same authored graph, and a calibrated human
model are distinct choices with different costs and errors. Recursive “best
response to best response” is not an implicit operator. A cheaper response level
must be versioned, visible in the assessment and compared against actual later
behavior. Live execution and the root's modeled continuation use the same
transition and plan-execution semantics; prediction remains fallible.

For the first slice, use a named finite comparison level whose response calls
bottom out in a declared base model. A frozen existing policy may serve as that
response approximation outside graph coverage. It must not choose the root route
inside the covered graph. Matching executors alone is insufficient: record and
test differences between live and forecast reselection levels as model error.

## 6. Value, risk and context

The objective is expected final game score difference. Contract failure penalties,
opponent scoring, Schwarz and lost opportunities enter through engine outcomes.
Pair possession alone supplies no score; the corresponding legal activation must
occur. A lead opportunity or card cannot pay for incompatible future actions on
the same realized branch. Alternative branches may reserve the same resource.

A plan with a lower make probability can rank higher when its score consequences
justify the risk. Predicted opponent overbidding can be useful for the same reason,
provided the actor-response assumptions support it. No additional safety, loyalty
to the old plan or baiting bonus is implied by this contract.

The first slice uses end-of-current-game score difference and carries existing
series context. Future series evaluation can use the engine's series payouts and
remaining-round state under a declared longer horizon. Immediate game margin and
series value must have different keys and labels. The earlier 500-point strategy
requirement remains part of that extension; this slice does not claim to solve it.

For each action, expose named event probabilities (for example, a pair activates,
the declaring team makes its contract), expected scores for both teams, their
difference, relevant value bounds and missing coverage. Probabilities of success
for alternative actions need not sum to one. “100% selected” is not a confidence
measure. Choosing the maximum expected value is not an estimate of the probability
that the action is objectively optimal.

## 7. Adaptation, reuse and invalidation

The query evaluator records dependencies actually consumed, including membership
dependencies for alternatives which were previously absent. A new opportunity
must invalidate candidate discovery as well as existing values. The event is
accepted into the knowledge layer before affected queries are reevaluated.

| Change | Semantic consequence |
|---|---|
| A returned card refills a supposed void | The ruff-entry prerequisite is false until that suit is exhausted again |
| An opponent cannot follow suit | Hard constraints change; entry and counterplay likelihoods may change |
| A new bid changes convention likelihoods | Posterior-dependent values and response predictions require current evidence |
| A pair activates | Trump, scored pair, remaining rights and dependent continuations change |
| An alternative becomes better while the old plan remains legal | Compare future value and revise the selected continuation |
| Undo, changed history, rules or graph/model versions | Restore compatible semantic state or rebuild it explicitly |

Hand capacities couple suits. Narrow invalidation is justified by recorded
dependencies or a proven conditional independence, not by the suit printed on the
last card. A posterior revision may conservatively dirty many values while
leaving immutable structure reusable. Old-posterior values may guide work order;
they cannot certify current dominance.

Memoization belongs to the evaluator, keyed by semantic query inputs. Partial
work keeps its unresolved bounds/support and may be resumed only under matching
inputs. Eviction loses speed, not facts or intent. Reuse across hidden states
requires query or transition/reward/observation equivalence; matching a few concept
labels is insufficient. Refine a group when a requested continuation distinguishes
its members.

If selected intent persists, store it separately from disposable caches. A cache
hit must reproduce both the decision and its proposed intent transition. Resources
and progress change only when the action is accepted in history. Revision-checked
commit of action and intent is atomic. Forced actions also advance the evidence
and intent state.

## 8. Evaluation built on the architecture

The runtime assessment records definition/model versions, input information,
consumed dependency references, candidate scope, legal transitions, response
assumptions, branch mass, scores, unresolved comparisons and selection. Work and
cache counters attach as metadata. They are not strategic concept nodes.

The explanation view unfolds that record. The replay tool asks the same evaluator
about a different candidate under a declared intervention. A later analysis using
more observations or an upgraded policy receives a different context and is
labeled hindsight/reanalysis. It cannot replace the recorded original reasoning.

Independent enumeration and full-information solving can check reduced fixtures,
but their hidden information does not enter the actor's record. Tests should
challenge dependency semantics, information boundaries and outcomes rather than
mirror the selector's implementation.

## 9. Acceptance and migration

1. Review this contract and a finite worked example with explicit facts, hypotheses,
   actor responses, branches and engine scoring. Confirm the authoring surface,
   primitive boundary and incomplete-ranking behavior before production coding.
2. Implement the smallest evaluator and authored definitions covering that example.
   Changing a strategic relation must change evaluation without adding a procedural
   tactic selector. Export definitions and traces directly for inspection.
3. Verify legality, correlated events, information partitions, zero-weight proof
   counterexamples, resource consumption and agreement with a reduced-state oracle.
4. Verify discovery of new alternatives, cache/fresh parity, undo/restore, forced
   actions and action/intent commit. Record which dependencies recompute and why.
5. Compare known failures and close controls using the same evaluator. Separate
   candidate coverage, inference error, continuation errors and response-model error.
6. Measure cold planning, warm play and repair separately. Compare cache on/off at
   matched logical work, then measure additional useful work under equal elapsed
   limits as a separate experiment. Keep the user's planning/play latency distinction.
7. Replace the covered production selectors and remove their superseded strategy
   logic after acceptance. Extend the shared model through bidding and both exchange
   directions, rather than maintaining separate strategic accounts for each phase.

No broad self-play result or speed guarantee follows from this design document.
The previous conditional-sequence proposal supplies execution/persistence detail;
the authored model here governs strategic construction and comparison.

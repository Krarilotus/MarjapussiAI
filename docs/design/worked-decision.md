# Worked decision: cash first or ask for the pair?

**The cash-first route loses 35 fewer game score points on the retained allocation.** Both routes collect the Green pair and miss the 160 contract. The opponent's score makes the difference.

| Retained continuation | Our game score | Opponents' game score | Difference |
|---|---:|---:|---:|
| Cash Acorn Ten, ask for pair, try entry | -160 | 215 | **-375** |
| Ask for pair, try entry immediately | -160 | 250 | **-410** |

These are recorded outcomes for one allocation. The expected ranking from the player's information also needs the other possible allocations and actor responses. Sections 2 and 4 demonstrate that calculation using explicitly illustrative weights and an unresolved branch. The proposed graph has not yet been executed.

Source revision: `8ea7a6a`. Frozen source-policy identities and later diagnostic factories are kept separate in the provenance section.

## 1. Permitted facts at the decision

The recorded fixture is game 2549, deal 637, rotation 1, step 13. Root is seat 3,
partner seat 1, contract 160. Root leads after winning the first trick; there is
no trump and no called suit. All seats retain their initial declaration rights.
Root knows partner holds the Green pair (40 points) and is void in Acorns.
These facts follow from root's visible exchange and public play. Root holds a
standing Acorn Ten and has a legal whole-pair question opportunity.

The alternatives begin with a legal card or question. A whole-pair question
changes root's own rights; truthful answering leaves the respondent's own rights
intact. Regaining lead does not restore rights. Full hands and recorded action
rows remain in the private evidence package; this repository retains the semantic
example, scoring results and evidence fingerprints.

## 2. Correlated hypotheses

H1 is the retained allocation. H2 exchanges two unobserved enemy cards: Green Ace
and Acorn King. It preserves hand sizes, root's information and the public prefix.
H2's prefix consistency was checked manually against the rule source; H2 was not
executed. The illustrative uncertainty is whether the later opponent is void in
Acorns and holds the Green Ace, so it can overruff the intended partner entry.
Another actor sees only its own hand and filtered history in either hypothesis.

For the worked arithmetic only, choose normalized **illustrative** model weights `w(H1)=0.98`, `w(H2)=0.02`. They are not frozen Mind marginals, empirical calibration, or tuned policy parameters. The deliberately concentrated toy prior permits a clear bound comparison using retained evidence. Hard possibilities remain broader than these two positive-weight model cases; zero-weight worlds still count when proving facts.

Let `V = opponent 2 void in Acorns` and `A = opponent 2 holds Green Ace`. In this toy model:

`P(V)=P(A)=0.98`, but **`P(V AND A)=0.98`, not 0.9604**. That conjunction matters because an Acorn lead after establishing Green can let partner ruff and then let seat 2 overruff with Green Ace. In H2 seat 2 must follow Acorn King instead; the Green Ace is with seat 0, which also must follow Acorns now. Entry and later control remain separate questions. Existing joint grouping/model (`deals.rs:146`); zero-weight proof boundary (`concepts.rs:282`)

## 3. What the authored dependencies must actually evaluate

**Proposed dependency composition, not a depiction of current executable code:**

```text
Expected game score difference for candidate + conditional suffix
 ├─ legal transitions and terminal scoring                       [engine]
 ├─ immediate cash value / root retains lead                     [facts + engine]
 ├─ Green-pair activation
 │   ├─ partner holds both uncalled halves                       [hard proof]
 │   ├─ root has whole-pair question opportunity/right           [engine]
 │   └─ partner's truthful answer choice                         [actor-view model]
 └─ useful partnership control after activation
     ├─ partner can take an entry trick                          [joint hypotheses]
     ├─ later actor cannot overruff                              [joint hypotheses]
     ├─ enemy gains lead with an uncalled pair/right              [joint + response]
     └─ conditional repair / alternate use of remaining cards     [authored alternatives]
```

The temporal programs are distinct from these dependency edges:

- **C — cash, activate, attempt entry:** play `e-Z`; if root retains the lead and the legal opportunity persists, ask whole pair; after a Green answer, consider `e-8` as the entry attempt; continue/repair from accepted observations.
- **E — activate, attempt entry immediately:** ask whole pair; after a Green answer, consider `e-8`; continue/repair.

Both start legally. Each step rechecks phase, rights, current trump, cards and actual legal actions. If partner can truthfully answer several pairs, the modeled actor chooses from its own legal answers; root cannot assume its desired pair solely because it knows that pair exists. On this fixture partner's recorded truthful answer is Green. `pair activated` earns 40; it does not assert `partner wins entry`, `we retain control`, or `contract made`.

**Response model for the retained H1 evidence:** the diagnostic uses factory `previous` for seat 1, `old` for 0/2, and a `scripted` root with specified legal opening overrides, then the recorded `candidate` root factory. All kinds are Heuristic, seed 0, actors fresh at prefix; original private cache/RNG state is unavailable. Policies are called through their own `SeatView`; root does not inspect another live actor's actual Mind/receipt. These saved realizations are witnesses for this declared model, not guaranteed repeated outputs of deadline-limited search or estimates of human behavior. H2's response tail is unresolved, so all legal response outcomes are conservatively bounded. Diagnostic metadata and decisions (`counterfactual-2549-cash-ask-entry.ndjson`); honest rollout boundary (`planning.rs:117`)

The future graph evaluator must own the above composition and instantiate both alternatives. The old diagnostic script is evidence to test against, **not** an existing graph trace or a strategy selector to run beside the graph and explain afterwards.

## 4. Retained terminal engine outcomes and bounded expected score

The completed H1 diagnostics have the same legal original prefix. Full-game outcomes below are retained engine results, including all later policy decisions, not isolated card bonuses. Engine order is `[opponents, us]`:

| H1 continuation | Card points | Pair points | Team points | Tricks | Scores | Us − opponents | Contract made |
|---|---|---|---|---|---|---:|---|
| C: `e-Z, ask_pair, e-8`, then candidate | `[115,25]` | `[100,40]` | `[215,65]` | `[7,2]` | `[215,-160]` | **−375** | No |
| E: `ask_pair, e-8`, then candidate | `[88,52]` | `[160,40]` | `[248,92]` | `[7,2]` | `[250,-160]` | **−410** | No |
| Control: whole-pair ask only, then candidate | `[111,29]` | `[160,40]` | `[271,69]` | `[7,2]` | `[270,-160]` | −430 | No |

Both main alternatives activate Green and fail 160. C improves realized score difference by **35** on H1, despite lower own raw points. Defender scoring and additional enemy pair calls matter; pair activation and contract success cannot replace final-game value. Rounding of 248 to 250 is intentional. Neither side is Schwarz. C result (`counterfactual-2549-cash-ask-entry.ndjson`); E result (`counterfactual-2549-ask-entry.ndjson`); scoring owner (`scoring.rs:76`)

**H2 terminal outcome is unavailable.** Use a conservative bound, not a fabricated score table: all card points total 140, all pair points total 280; root's team already won a trick and cannot become Schwarz. With fixed contract 160, our final score is ±160; defenders score at most 420, or −320 if Schwarz on our made contract. Therefore for either continuation, `D(H2) ∈ [−580,480]`. The bound is loose but valid under the current engine constants. Point ceilings (`points.rs:28`); Schwarz/scoring (`scoring.rs:76`)

**Scope of these values:** the arithmetic compares the retained scripted continuations and their recorded suffixes. An adaptive graph may select a different suffix after an observation; its value must then be recomputed and cannot inherit the recorded -375/-410.

For the retained continuations, without normalizing away H2:

```text
Q(C) = 0.98(−375) + 0.02 D_C(H2) ∈ [−379.1, −357.9]
Q(E) = 0.98(−410) + 0.02 D_E(H2) ∈ [−413.4, −392.2]
Q(C) − Q(E) ∈ [13.1, 55.5]
```

Thus **C outranks E under this illustrative joint model and the stated retained H1 response realizations even with H2 unresolved**. This certifies neither the best move over the entire legal set nor playing strength, and is not a measured posterior preference. With 50/50 illustrative weights these bounds overlap; the honest output would be unresolved ranking, further refinement or a labeled deterministic fallback. A runtime must not acquire a concentrated prior merely because it improves the desired ordering. These illustrative weights are never proposed as the game's actual posterior.

For both alternatives, toy `P(make | us declares) ∈ [0,0.02]`; declaring share is 1, and pair activation is separately represented. **Choosing C deterministically means the selection rule chose C; it does not mean success is certain, or C is optimal under the real unknown response distribution.** The bounds demonstrate why expected score difference can discriminate when observed contract-success indicators are identical. Distinct current outcome fields (`planning.rs:57`)

## 5. Observation-driven reconsideration while the old action remains legal

During C's first cash trick in H1, recorded responses are an Acorn response and two Bells discards. Seat 2's discard proves an **Acorn void**, by the engine's follow rule. In H2 seat 2 instead has Acorn King and must follow it; this distinguishes the two toy branches. Root retains the lead and can still legally ask whole pair or play `e-8`. The old entry action is legal in both cases, but the posterior-dependent overruff/control evaluation has changed. Recorded C continuation (`counterfactual-2549-cash-ask-entry.ndjson`); hard legal-play exclusions (`bits.rs:182`)

The graph must dirty the affected entry/control values, update the joint mass, and compare the entry attempt with available alternatives such as cashing/leading `g-Z` to expose Green Ace first. **No saved comparison establishes that this repair is superior.** Its value is a required query, not a prescribed patch. Even a model probability of one for Green Ace's location remains distinct from hard knowledge because other hard-consistent zero-weight worlds can survive.

The actual subsequent C entry shows the dependency failure: an Acorn lead, an Acorn follow, partner’s Green ruff, and the opponent’s Green-Ace overruff. Partner ruffs, enemy overruffs, enemy 2 gains the lead and announces Red for **100**, changing trump. The Green 40 already earned remains credited; it is not undone. Root must abandon the “partner acquired control” branch, synchronize public trump/rights and re-evaluate from the next legal opportunity. This is an observed failure of this entry attempt, not proof every Green-pair route fails. C action chain (`counterfactual-2549-cash-ask-entry.ndjson`); pair points (`points.rs:32`)

## 6. Acceptance tests on the future actual graph

1. **Executed composition:** load this real prefix through the engine/honest replay helper. The authored alternatives, joint overruff event, pair/right prerequisites and terminal utility nodes actually govern ranking. Changing/removing the entry-threat dependency changes the relevant evaluation/trace without adding a procedural tactic branch. No posthoc second evaluator fills in a reason.
2. **Known versus believed:** private return proves partner pair; bid likelihood does not prove enemy pair; H1/H2 weights never overwrite constraints. Include a hard-consistent zero-weight counterexample and impossible/zero-mass givens. Joint `Void AND Ace` uses correlated mass, not marginal multiplication.
3. **Privacy/response:** H1/H2 have identical root prefix/view. Root receives neither full audit hand table nor live partner receipt. Each simulated actor is synchronized from its own view. Branch-dependent responses must be explainable by that actor's legal observations/private hand; distinguish “viewer knows”, “model predicts choice” and “oracle sees”.
4. **Same executor:** C/E are evaluated and later executed by the same conditional sequence semantics. After the cash observation, update evidence and compare legal alternatives even though `e-8` remains legal. After overruff/Red declaration, consume the actual failure transition; do not continue a success branch. Do not replay the old script and call it graph execution.
5. **Engine oracle:** independently replay both retained H1 paths and verify actors/legal actions, 140 card ceiling, pair credits, final 160, trick counts, round5 scores and −375/−410. Later replay H2's common prefix and bounded selected continuations before claiming completed H2 values. Keep this oracle explicitly separate from actor reasoning.
6. **Trace/work contract:** emitted graph/node versions, actual consumed edges, honest information stamp, receipt before/after, hypotheses/mass identity, candidate **plus suffix**, response-model level, utility horizon/settings, success numerator/denominator, unresolved mass/bounds, represented logical work and actual compute. Visualizers and evaluation read these executions, not reconstructed intent.
7. **Memoization parity:** fixed logical work and tie rules; cache hits charge represented effort and apply the same receipt after-state as misses. Cache-off/capacity-zero, warm/cold, deliberate hash collisions, same first action/different suffix, different receipts, undo/fork and parameter changes preserve semantic results/coverage. Time saved is measured separately; it cannot buy extra candidate work under the same policy.
8. **Performance scope:** collect actual engine/knowledge/query/response/continuation/aggregation work from these graph invocations. Compare equal logical work before timing; separately report incomplete results and byte caps. A low top-level decision-hit count does not measure outcome/model reuse. No timing target or graph speedup is established by this note.

These tests cover the modern authored-graph scope through **best represented next move**, not merely a pair-specific guard. Early bidding/exchange nodes and general composition/recursion remain part of the contract; this vertical slice does not excuse indefinitely keeping those phases procedural.

## 7. Validation provenance and remaining limit

- Existing binaries inspected: `candidate-dryrun.exe` and `counterfactual-dryrun.exe`; the existing atlas replay source is a fixed 4,000-row verifier, unsuitable as a one-fixture CLI. **None was executed for this task.** No build was requested or started. Reads used bounded JSON extraction/hash checks; original records were untouched.
- Engine-only original replay receipt: atlas-replay-run.json (`atlas-replay-run.json`), exit 0, 5.39 s. Its log/source asserts 4,000 games/206,582 actions, legal actor/action matching and final scoring; original source policies are b3 `b3abdaf1f5e47111904db442ce36db7dc4ae9e0b` and old `1632af833199e5253271d493af8531badfac81c0`.
- Later diagnostic suite receipt: conditional-counterfactual-suite.json (`conditional-counterfactual-suite.json`), exit 0. Each of the two selected files reports `completed=expected=2`, `status=complete`, `error=null`. This reviews saved evidence; it does not repeat those executions.
- Counterfactual binary SHA256 from retained manifest (`counterfactual-manifest.json`): `74ab1d164c32ef1144505f7e7cb4ba07ec3905e60fe543fe6d6e88498a3b0bfa`. C file hash checked now: `c230496c7eb0bf58b2a6951743bf4dfe21f0dc7fc947d5d4afb41c905fddab22`; E: `f850cbf8f8bf22a187e7724bea1a5e81863fad6dbec3326fdcfaa7eff4c0f7bf`.
- The original input has externally retained SHA256 `9a1eb1b4e7a5118f6eefb90924c6da60767629f49b5ebfb0a4767777219699b7`; diagnostic header explicitly says its harness did **not** independently verify that hash. It performed identity/selected replay and length/mtime guards. Do not attribute external hashing to that harness.
- **Ready for contract discussion, not a tested graph fixture.** H1 legality/outcomes are retained evidence; H2 prefix consistency is manual source reasoning; probability weights and bounded expected-score composition are an unexecuted design example. H2 final trajectories and the proposed graph trace do not yet exist. No conclusion here says this lost game was a must-win or qualifies a new policy.

# Concept store: from cards to decisions

The implemented store answers holding and perspective queries. Strategic choices
still use procedural composition. The [executable reasoning contract](executable-reasoning.md)
and [worked decision](worked-decision.md) propose the authored strategic layer;
they are under review. The catalogue below contains both current capabilities
and earlier proposed concepts.

This catalogue describes the concepts and their dependencies, including proposed
work. Typed Rust helpers implement [entry conditions](entry-conditions.md), joint
holding queries and [bounded return forecasts](return-rollouts.md). The shared
seat-view table carries legal continuations through exchanges, contract choices,
raising and play. The tables distinguish these implementations from the wider
planning design.

The graphics and machine-readable graph below preserve the **7 October design snapshot**. Their "corrections remain local" labels are historical; the current branch map is in the table below.

![Concept-store overview: visible information becomes facts and beliefs, which feed bidding, passing and proposed game planning. Dashed connections indicate proposed planning work.](../assets/concept-store.png)

[Full-size graphic](../assets/concept-store.svg) · [Machine-readable graph](../assets/concept-store.json)

Read arrows as **“provides an input to”**. L0 contains visible information and exact deductions; L1 describes hands and seats; L2 combines them into team and game concepts; L3 chooses an action. The graphic groups related concepts. The tables below spell out their individual dependencies.

## What is available where

| Version | Available work |
|---|---|
| `main` | Published rules engine, seat observations and exact knowledge. The other layers below describe the wider design, not crates present on this branch. |
| `fix/live-planning-review` | Current heuristic, beliefs, question-aware solver and bounded return forecasts. Paired with testbed `feature/takeback-claim`; see the [architecture](../ARCHITECTURE.md). |
| `v1-preview`, `integration/reviewed-ai` | Earlier agent snapshots. The [publication record](../checkpoints/2026-10-08-publication.md) documents their ancestry. |
| Remaining planning | Bounded entry and return reasoning exists. Coordinated full-game planning, calibrated score forecasts and raising remain incomplete; proposed dependencies below are not shipped guarantees. |

The source is the working catalogue `findings/kb/concepts.md`, with passing clarifications in `findings/kb/03-passing.md`. Both live in the separate local research workspace. This repository carries a portable summary without recorded games, player identities or private workspace links. Later dated corrections in the catalogue supersede earlier drafts. Implementation status was checked against the local Rust source at the revisions above; this documentation update changes no agent behaviour.

## L0 → L1: what a seat can know or believe

An agent starts with its own hand, public events and the cards privately passed to or received by that seat. Other seats' actual hands are never an input. Legality, rank order, card points, pair values and scoring belong to `engine`.

| Concept | Inputs / calculation | Kind and owner |
|---|---|---|
| `possible(y)`, `certain(y)`, `void(y,s)`, `count_bounds(y,s)` | Own visible cards, public plays, truthful answers and observed exchanges constrain possible holdings. | Exact deductions; `knowledge`. |
| `my_pairs`, `my_halves`, `my_aces`, `ace_ten(s)` | Own cards; a pair is King + Ober of a suit, a half is an unmatched King or Ober, `ace_ten` holds both A and Z. | Exact hand features; rules and `knowledge`. |
| `own_standing`, `standing_known(seat)` | Top-down rank runs; own-hand standing and standing supported by known facts are distinct views. | Exact within the relevant view; `knowledge`. |
| `standing_against_us` | Treat all cards outside my hand as opponents' cards; run each suit from the top while my lowest cards fall, and count the remaining opponents' cards when I run out. | Conservative hand count, 0–27; `knowledge`. It is not a probability or the opponents' actual hand. |
| `has_ace(y)`, `has_halves(y,k)`, `has_pair(y,s)` | Facts + auction history + the shared bid encoder read in context; feature-distribution calculations give seat probabilities. | Uncertain seat features; `beliefs::Mind`. |
| `p_holds(y,c)`, `p_void(y,s)` | Fact-constrained card distribution, updated by available bid and exchange evidence. | Beliefs; exact facts force probabilities to 0 or 1. |
| `bluffing(y)` | A signal has zero support under the facts; the catalogue calls for reduced trust in that seat's signals. | Belief-reading concept; it does not weaken exact facts. |
| `seat_style(y)` | How a seat's convention is interpreted. The catalogue's v1 default is our convention for everyone. | Individual style exceptions remain proposed. |

### Joint queries and knowledge of another seat

`ConceptQuery` describes a conjunction of card, void, pair and count predicates,
with optional conditions. `Mind::concept` returns the unconditional hard fact,
truth under those conditions, and joint probability bounds as separate fields.
Proof considers every hard-consistent allocation, including allocations assigned
zero weight by the belief model.

`Mind::perspective_concept(viewer, query, budget)` separates “I know this” from
“I know the viewer knows this” and from an estimate of the viewer's belief.
Public plays and shared transfers provide direct proofs. Private-hand hypotheses
use only the viewer's starting hand and filtered events. Opponents see the public
transfer event; only participants see its cards. Later play, transfers and undo
update or invalidate the supporting facts and caches.

For a ruffing entry, a known void removes one uncertain dependency. Trump
ownership and the opponents' ability to follow or overruff remain joint queries.
Their probabilities are calculated under the same constraints and weights.
Node-limited traversal retains unresolved mass, and hypothetical conditions stay
distinct from observed facts. Full-game continuations still model each actor's
decisions from its own view.

## L1 → L2 → L3: bidding

| Concept | Direct inputs | Used by / current meaning |
|---|---|---|
| `info_to_tell` | Own aces, halves and pairs; previous messages; auction context; what the partner already knows | B-tell sends useful remaining messages. Latest local order: red, bells and acorns pairs before the ace; with a green pair, ace first. |
| `info_channel_open`, `team_plan_formed`, `opponents_exchanging` | Auction values and messages, including a 140 gate | Decide whether to tell or gate; the auction record carries context between steps. |
| `partner_completes(h)` | `p_holds(partner, other_half(h))` | Completion probability for a split pair; also an input to passing. |
| `team_pair(s)` | Own pair/half facts + partner's believed halves/pairs | Team pair possibilities, not a claim about unseen cards. |
| `derived_pair_value` | Own single halves × probability the partner has their counterpart × suit pair value | Expected pair value, excluding green and own complete pairs. It feeds `game_drive`. |
| `team_pair_cover(V)` | Team pair possibilities + excess contract value above 140 | B-over140 checks whether a pair can cover the excess. The latest tie-break uses `team_pair_cover_derivable`: own complete pairs or pairs supported by partner messages, not own halves at base rates. |
| `opp_reach` | Opponents' pair evidence + pair values + 140 card/last-trick ceiling | Caps B-defend; stop protecting against a value the opponents cannot plausibly reach. |
| `game_drive` | Own standing, A+10 suits, own and derived pair values, prospective target, partner's calls and remaining message | Probability of preferring 140 to the information step. A logistic score combines the terms; local v1 acts at 0.5. It is not the final game estimate. |
| `standing_potential` | Intended: own top run + cards the partner may hold and pass | Catalogue target: probability that a suit becomes standing after exchange. **Current C43b implementation is a count proxy:** for A+Z-headed suits of length ≥4, sum `length − top_run_length`. |
| `p_take_140` | Own standing, A+10 suits, own pair value, derivable pair cover at 145, standing-potential proxy, value 135, partner state and aces | Probability of taking 140 rather than passing when nothing remains to tell. C43b distinguishes this from `game_drive`'s tell-vs-140 choice. |
| `hand_quality`, `team_reach`, `schwarz_chance_us`, `pass_quality` | Hand structure, partner signals, pair values and exchange prospects | Wider catalogue concepts. Some calculations are proposed; they are not a complete implemented planning system. |

The C43b local implementation calculates:

```text
p_take_140 = sigmoid(-4.85
  + 0.39 * own_standing + 0.10 * ace_ten_suits
  + 1.07 * own_uncalled_pair_value / 100
  + 3.21 * team_pair_cover_derivable(145)
  + 0.42 * standing_potential_count
  + 0.84 * [value == 135]
  - 0.48 * [partner passed] - 0.21 * [partner not yet acted]
  + 0.76 * own_aces)
```

These coefficients come from the recorded-human decision analysis described in the catalogue. They are empirical choice-model coefficients, not consequences of the game rules or evidence of stronger play. The source reports 1,261 decisions at 125–135; this update did not rerun that analysis.

The current local bidding path checks gate part 2, then B-tell with its cover and tie-break checks, then B-defend, gate part 1, and finally the nothing-left-to-tell take/pass choice. B-defend depends on a first-turn partner pass, opponent bidding, a near-hopeless hand, its limit table and `opp_reach`; it replaces the earlier B-block proposal. The implementation still requires no ace, no pair and fewer than three halves. The later catalogue exception for a lone green pair is an open implementation gap.

For messages landing on 140, +10/+15/**+20** shift to 145 and larger live messages shift with them; +5 neither shifts nor triggers a shift (latest C43c correction). The catalogue bands are ≥0.8 “believed yes”, ≤0.2 “believed no”, and otherwise open. Action thresholds such as 0.5 or the pair-cover check are separate policy decisions.

## Passing: compare the exchange options

Each legal four-card exchange gets a feature vector. Features depend on the seat's view and the hand left by that option, so they are recomputed for each option. `score(x) = Σ weight × feature(x)`; `P(x) = softmax(score(x))`. The local v1 default chooses the maximum; these choice probabilities are not win probabilities.

| Concept family | Direct inputs | What it contributes |
|---|---|---|
| Exact exchange structure | Hand and option `x`, suit/rank rules | Voids, whole side suits, points, green cards, blank/guarded tens, aces, standing after exchange and `pair_broken`. |
| Completing the partner | Passed halves + `partner_completes`; passed aces + P(partner lacks an ace) | Belief-weighted assistance rather than counting every half or ace equally. |
| Pair information | Whole pairs passed + what the partner knows from bids | Separates known and unknown pairs. Latest C42c clarification favours passing an unknown whole-suit pair to consolidate trump; the known case permits either choice. |
| Trump fit | `team_pair` + suit value → `likely_trump` / current pass-model `plan` | Cards of a candidate trump kept or passed, including top trump for the last trick. |
| Ace entry / `schnippeln_feasible(s,x)` | Ace sent back, a lower non-pair card kept, partner's likely suit holding and first-trick green restriction | Give the partner the ace and later lead low to it. For a non-green first lead, the passer must keep no green cards. The catalogue's full feasibility concept is richer than the current option features. |
| `pass_legibility(x)` | Option + bid context + partner's model of the pass + importance of the team pair | Intended probability the partner reads the suit holding or void correctly. Full concept-level legibility remains a design target. |

Current C42 feature names, in their code order:

```text
forth: lusche_from_kept_suit, pair_broken, blank_with_lusche,
       blank_without_lusche, points, tens_guarded, tens_blank, aces,
       aces_x_vh_lacks_ace, pairs_known, pairs_unknown,
       halves_x_completes, green

back:  standing_after, aces_entry_low_kept, pair_broken,
       lusche_from_kept_suit, voids_made, halves, side_suits_complete,
       aces, points, pairs, halves_x_completes, plan_top_trump_kept,
       plan_trump_given, aces_x_mh_holds_suit, green, tens
```

The catalogue's R1–R4 reading proposals infer likely voids from low-card groups, likely remaining suit cards from a ten/king without a low card, a whole suit in the return pass, and a mild low-card indicator. **Pass-choice interpretation is not implemented.** The older reader was unused and has been removed. Active beliefs account for exchange constraints without inferring why the giver chose those cards; see [passing models](../PASSING.md). A future reader must use the giver's possible view, never its hidden remaining hand.

## Proposed game plan: connect the exchange to later play

These edges are the catalogue's batch-3 proposal, represented by dashed boxes and arrows in the overview. They must not be read as a shipped planner. Some lower-level valuation and pass-model helpers already exist, but do not implement the complete chain.

| Concept | Direct dependencies | Intended result |
|---|---|---|
| `trump_candidates(s)` | `team_pair`, `partner_completes`, entries | Ways to activate trump: own announcement, partner announcement, pair question or half question. |
| `sure_tricks(seat)` | Standing cards, opponent `p_void`, current/candidate trump | Top-down tricks adjusted for opponent ruff risk; uncertain cases are estimates, despite the working name. |
| `entries` | `schnippeln_feasible`, standing/winning cards of both partners | Routes for handing the lead to the partner and receiving it back. |
| `schnippeln_gain(s)` | Feasibility, partner's tricks, opponent void/trump risk, trump candidates | Extra tricks and useful lead transfer compared with keeping the ace. |
| `ace_split` | Positive `schnippeln_gain`, ace-entry probability | Split aces only when the feasible low-lead line improves the plan. |
| `trump_plan_legible` | Bids, cards sent back, `partner_completes`, pass legibility and partner reading | Probability the partner understands which cards to keep and when to ask or announce. |
| `trump_split` | Candidate pairs, voids created, `trump_plan_legible` | Give one pair/suit to the partner and keep another to enable ruffing. |
| `ruff_line(s)` | Split/void structure, current trump, opponent card beliefs | One partner leads a suit the other can trump. |
| `activation_order` | Trump candidates, lead access, trick prospects, opponent pair possibilities | Activate pairs in a useful order. Latest proposal: the given pair first, retained stronger/longer trump last. |
| `game_estimate` | Activatable pairs, trick points, ruff lines, entries and last-trick control | Points the plan can secure, rounded down to a multiple of five; not a whole-game exact search. |
| `raise_message`, `raise` | `game_estimate`, own pairs and plan legibility | Raise within the estimate and communicate whether pair questions or a schwarz plan are needed. Red-pair 160 is unconfirmed. |
| `lead_sequence` | Tricks, entries, announcement order, opponent void/trump risk | Safe leads, lead transfers, pair activation and trump/ruff lines. |
| `replan` | New visible event, contradiction or increased failure risk | Recompute the next version of the plan. |
| `win_safety(card)`, `half_potential(card)` | Seats behind, card beliefs, pair completion/value and activation prospects | Prefer a safe ace when it wins without spending a half needed for an unannounced pair. Full connection to the proposed planner remains future work. |

The graph is evaluated for one information state at a time. `entries`, trump activation and trick prospects constrain one another; candidate plans need evaluation under an assumed trump/lead sequence rather than pretending every value can be computed independently. A new observation starts the next evaluation. Similarly, pass reading must use a model for the other seat's possible view, never its actual hidden hand.

### Conditional continuations and contract value

The next planning slice compares legal continuations through this chain:

```text
candidate exchange
  -> both resulting hands and each seat's visible exchange information
  -> suit lengths, voids and surviving halves
  -> legal trick, including forced following and overtaking
  -> winning seat and remaining cards
  -> legal announcement/question/answer and remaining rights
  -> changed trump, later entries and opposing counterplay
  -> captured card points, activated pair points and last trick
  -> contract score
```

This is a state-transition dependency, rather than a sum of independent bonuses.
Returning a card in suit s makes partner non-void in s until it is played. A ruff
plan must start from that resulting hand, not from the inferred pre-return void.
Leading low to partner's Ace and leading a side suit for partner to trump are
different entry mechanisms. Both must check the legal actions of the opponents
and the actual next leader. An entry that requires trump cannot also justify
the first opportunity to establish that trump.

Each continuation uses one joint holding hypothesis throughout its transitions.
For a sequence E1, E2, ..., its probability is conditional:
`P(E1 | I, action) * P(E2 | I, action, E1) * ...`.
In sampled play, retain the same weighted deals and count the deals in which the
whole sequence succeeds. Do not multiply marginal pair, void and entry reads:
they depend on the same cards. Opposing suit exhaustion and higher trumps must
be considered jointly, including cards consumed by earlier tricks.

`beliefs::Mind::deal_weights` supplies the joint auction likelihood used by the
feature model. The hidden-exchange model remains approximate, and pass-choice
intent still needs a model of the giver's available information. A high-level
signal interpretation must retain alternatives: a big-pair signal plus our Red
half excludes partner's complete Red pair, but imperfect signalling leaves some
non-pair mass; a small-pair/halves signal can mean either small pair or several
halves. How much a late bid supports each interpretation requires a model of the
speaker's affordable contract and preceding team signals, rather than a fixed
penalty for Green at a particular bid.

The shared outcome should expose a distribution of final team points and engine
scores, with activated pair values, entry outcomes, contract success and failure
branches available for explanations. Red control includes the reachable
100-point announcement; the same shape in Green includes 40. Red plus Bells plus
one captured Ace contributes 191 only on a continuation that activates both
pairs and captures that Ace. Held pairs alone establish neither that result nor
a guaranteed 190-point bid.

`heuristic::planning` owns continuation comparison; `engine` owns every legal
transition and terminal score; `beliefs` owns uncertainty; `knowledge` owns hard
constraints. Return selection, announcement timing, bidding and raising should
consume that shared result. Candidate comparisons include cashing a standing run,
asking partner first, announcing first, alternative returns, taking the contract
and letting the current bidder play. Bid signalling remains an available purpose
of a bid, including between partners after the opponents pass.

Guaranteed raising keeps its separate certificate. Estimated raising uses the
forecast's success/failure distribution and actual contract scoring, not its mean
points rounded down or the minimum of a small sample labelled guaranteed. After
new observations, recompute affected routes and discard plans whose required
cards, lead or rights have disappeared. Each simulated actor receives its own
view; choosing a different root action after inspecting each hidden deal is not
a legal imperfect-information decision.

Acceptance scenarios cover a returned card cancelling a planned void, forced
partner overtaking, opponent overruffs, a partner Ace entry before trump, two
successive own standing cards, reciprocal pair questions, a negative answer,
consumed halves, spent announcement rights and a later opponent trump change.
The same plan must explain both its useful line and its failure branches. Runtime
qualification must cover joint sampling, inference and continuation work together;
an interrupted comparison keeps the previous choice and publishes no partial
success claim. The bounded implementation shares this evaluation across return
selection, pass-versus-bid decisions, pre-play raises and open leads. Each root
alternative uses the same resampled joint deals; recursive forecasts are disabled
inside continuations. Actual decisions replan from their updated observations.
Raising targets are proposed by repricing terminal engine outcomes, then verified
by replay at the new contract. Guaranteed own-card claims use the knowledge proof.

Eight rollouts are an approximation. Hidden exchange events currently prevent
lawful reconstruction for the defenders, who retain the ordinary policy and
endgame search. Latent exchange histories, inference from pass-choice intent,
affordability-dependent signalling and series-aware objectives remain open.
The [qualification record](../checkpoints/2026-10-10-joint-continuations.md) records
the tested slice and its runtime/strength limits.

## Ownership, updates and maintenance

`engine` owns rules; `observation` exposes the allowed view; `knowledge` owns exact deductions; `beliefs` owns uncertain seat/card features; `heuristic` combines them into decisions. The concept store is currently a Markdown design catalogue implemented through typed Rust helpers, not a generic graph database or a complete dependency scheduler. The separate research `kg` index searches code and documentation; it is not the runtime concept store.

Dependencies describe the intended invalidation boundary: a card play changes holdings and standing/void deductions; a bid changes auction context and beliefs; an exchange changes known cards and candidate hands; an announcement changes trump-related plan values. Reuse unaffected values where supported by the implementation. Lazy calculation and memoisation are design goals, not claims that every table entry already has an incremental cache.

When updating this document, check the latest catalogue corrections against code, retain proposal/implementation labels, and update the version table. Edit `docs/assets/concept-store.json` for the grouped overview and regenerate both images with `python docs/assets/render_concept_graph.py` (Python + Pillow). Keep recorded games and person-specific analysis outside the repository. The embedded PNG works without Mermaid or special Markdown rendering; the SVG remains zoomable.

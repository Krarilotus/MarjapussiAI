# Passing: choices, evidence, and remaining work

The local v1 continuation adds [bounded return forecasts](design/return-rollouts.md):
C42 still selects the first pass and orders return candidates; complete seat-local
rollouts can choose a different return. `return_worlds = 0` uses C42 alone. The
removed HP/HK reader stays removed. Neither path infers intent from observed passes.
The descriptions below explain the retained C42 features and historical cleanup.

Current after `checkpoint/2026-10-08-exchange-cleanup`. C42 and HP/HK are historical
iteration/rule labels, not different neural networks. Both pass policies are small
interpretable scoring functions. Only C42 is retained in executable code.

## What chooses a pass now

`heuristic/src/passing.rs` snapshots the acting seat's estimates, scores the legal
four-card passes supplied by the engine, and selects one. `passing/features.rs`
owns the feature definitions, weights, and existing pass-back trump-plan helper.
Feature calculations take explicit inputs and do not access the engine's hidden hands.

The score is `sum(weight[i] * feature[i])`. There are two feature sets because the
decisions differ:

| Decision | Situation | Features include |
|---|---|---|
| Pass forth, MH to VH | Give four of nine cards to the declarer | Voids made with/without low cards, low cards given from a suit kept, breaking a pair, points, guarded/bare tens, aces, known/unknown pairs, likely pair completion |
| Pass back, VH to MH | Return four from thirteen cards | Standing cards left, ace entries, voids, breaking pairs, likely pair completion, keeping the top planned trump, trumps given, aces where the partner likely has that suit |

For example, the forth score contributes -5.37 for breaking a pair and +2.68 for
making a void while passing low cards of that suit. These are competing terms:
neither is a hard rule. Belief-dependent terms use the giver's estimates, such as
P(partner holds the matching half), not the partner's actual hand.

The back plan first favours the highest-value pair held. Otherwise it favours the
highest-value single half whose suit meets `partner_half_conf` (default 0.5).
The existing estimate is P(partner holds a half of that suit), not an exact joint
team-completion probability. This cleanup preserves that semantics.

By default selection takes the largest f64 score, keeping the first legal option
on a tie. Positive `sample_temperature` samples softmax(score / temperature),
using the existing f32 sampler and seeded RNG. Explanations report each nonzero
weighted contribution. Weights, arithmetic order, selection and explanations are
unchanged in this checkpoint.

The weights came from conditional-logit fitting on 1,575 recorded human passes
in each direction, as described by the source comments and outer-workspace
`tools/kbstudy/passfit.py` (`broken` variant). The inspected report is in-sample:
these are fitted human preferences, not a demonstrated playing-strength result.

## What was removed

The older `ForthModel` (HP rules) and `BackModel` (HK rules) assigned hand-set
bonuses/penalties to voids, information, card quality, pair retention, trump
retention and related conventions. Their optimized scoring machinery, option
enumerator, contexts and 26 unused parameters were retained after C42 replaced
their choices.

`PassReading` used those older scores to compute a likelihood for an observed
pass. It enumerated possible four-card choices from a hypothetical giver hand,
normalized their scores with softmax, mixed in uniform behaviour according to
trust, and applied a small floor. The intended inversion was:

`P(hand | observed pass) ∝ P(observed pass | hand) * P(hand)`.

But **the active belief calculation never called it**. Its existence and stale
comments led the original review to incorrectly describe a runtime mismatch.
Removing it changes no active pass-choice inference. Tests specific to the old
scores were also removed; actual choice scenarios were retained.

Old pass-only TOML keys now fail as unknown keys. `sample_temperature` still
controls current choice sampling. `partner_half_conf` keeps its flat TOML name
and default, but is now owned by `heuristic::Params`, where it is used.
The historical design/tuning notes are not current API documentation.

## What the belief model does now

`knowledge` tracks facts established by rules and visible events. `beliefs::Mind`
adds uncertain bidding evidence and estimates hidden card/hand features subject
to those facts. Exchange bookkeeping adjusts card locations and feature
distributions; the default hidden exchange kernel is uniform.

That is different from learning why a partner chose a particular pass. Seeing
two low cards of a suit might suggest the partner made itself void, but current
beliefs do not evaluate that suggestion using the C42 choice score. It must not
be promoted to a hard fact either.

## A later interpretation iteration

1. Specify whose behaviour is modelled: deterministic current agent, sampled
   current agent, or fallible human convention. Softmax with an arbitrary new
   temperature would not faithfully model the default deterministic chooser.
2. Reconstruct each hypothetical giver's pre-pass information. For a pass back,
   include the first exchange as the giver saw it. Using the recipient's `Mind`
   directly would substitute information unavailable to the giver.
3. Reuse the current feature/score definitions; move them below policy and
   inference only when the second consumer exists. Do not maintain two scorers.
4. Combine the likelihood with the existing posterior without double-counting
   bidding/exchange evidence. Bound its runtime and measure approximation error.
5. Test candidate-hand likelihoods, hidden-card swaps, and same-policy simulated
   void predictions. Evaluate humans separately on held-out games. The old ignored
   human-rate comparison is historical evidence, not an acceptance target.

This is a new inference capability, not part of the behaviour-preserving cleanup.

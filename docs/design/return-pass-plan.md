# V1: plan the return and the following play together

Historical proposal, 2026-10-09. The implemented bounded slice and its limits are
in [return rollouts](return-rollouts.md); broader planning remains future work.

## The decision we want to make

Before returning four cards, ask: **which allocation gives our team a credible
route to the lead, a legal opportunity to activate its pair, and enough subsequent
tricks to fulfil the contract?** Choose the return and that route together.

At the proposal baseline, `passing::decide` chose `back_plan` once from the full 13-card hand: the
highest-value own pair, otherwise a held half with enough partner evidence. It
then scores every return with the C42 features against that same suit. Several
useful concepts already exist, but the candidate's resulting pair ownership,
entry route and later activation are not assessed as one plan. C42 scores are
fitted choice preferences, not probabilities of making the game.

A v1 plan should be a short, conditional route with a fallback, not a fixed
nine-trick script. Every actual action still comes from the engine's legal list.

## Domain constraints first

- Returning `Y` leaves my hand `H13 - Y` with nine cards. The partner's five
  remaining cards plus `Y` also total nine. `Y` becomes certain at the partner;
  the unknown five stay uncertain. Reuse the supplied legal four-card options.
- Treat pair possession, announcement/question rights, and access to a lead as
  separate conditions. Keeping a pair is not the same as scoring its points.
  Its required halves must survive until the relevant legal action.
- The opening cannot announce or ask: the engine's `Raising` phase allows bids
  and cards. The first opportunity is after a completed trick. The opening card
  must be an ace if held, otherwise green if held, otherwise any card. In trick
  one, a held ace of the led suit must be played. Use `play_levels`, including
  its other follow/overtake/trump obligations, rather than restating legality.
- Announcements/questions consume opportunities and can alter future rights.
  Uncalled pairs may change trump; the engine does not impose an increasing
  pair-value order. Opponents can also interrupt the intended trump sequence.
- A hand-only standing rank run, an entry card and an expected trick are different
  claims. `own_standing_of` does not establish ruff safety; `Knowledge::standing`
  already checks current ruff safety, so do not discount that exact result twice.
  An entry may require a later lead or lose to an opponent. Count a trick once
  even when it also creates an entry. Keep lead ownership explicit: team control
  alone does not establish which seat gets the next announcement opportunity.
- Both partners see their exchange, but neither sees the other's remaining hand.
  Nominal bid messages and pass intent remain uncertain. `Mind::view` is a
  fact-constrained view, not an implemented model of the partner's pass reasoning.

## Reuse the existing owners

| Question | Existing owner / intended reuse |
|---|---|
| What may be passed or played, and how do rights/scoring change? | `engine` legal actions, `bits::play_levels`, `game::rules`, score functions. |
| What is certain after this return? | `knowledge` and its visible `SeatEvent::Pass` transition on candidate state. Validate against an actual legal exchange. |
| What remains uncertain? | The acting seat's `Mind`, `Partner::of`, existing card and seat features. Reconcile them with the known transfer; do not reuse pre-return partner features unchanged. |
| What features already describe this allocation? | `passing/features.rs`: kept standing cards, ace with low card retained, pair splits, suit voids and trump allocation. |
| Can the pair become trump? | `trump_plan::Reach`, rights checks and activation timing. Extract only the inputs needed by a concrete candidate consumer; keep one activation owner. |
| How can a seat regain the lead? | Existing `leads.rs`, standing/possible-card facts, `valuation.rs`. HL1 is a specific first-trick rule, not a general entry solver. |
| How does that help win the contract? | `Stakes`, valuation and the engine's scoring semantics. The existing endgame solver remains the late-game consumer. |

Keep cheap allocation facts separate from uncertain estimates. Reuse a shared
pre-return belief snapshot across candidates, with correct transfer-dependent
updates; avoid rebuilding the full belief DP for every four-card option. Measure
this before adding caches. Any cache must include its real history, parameters
and candidate dependencies. The current epoch/history machinery already owns
invalidation; no new graph framework or parallel belief store is proposed.

## Six checkpointed iterations

### 1. Candidate allocation (`plan-allocation`)

Express the resulting own/partner hands and pair allocation using existing facts
and transfer semantics. Cover a retained pair, a whole pair returned, a split
pair and an uncertain completion. Add candidate explanations in the existing
diagnostic/test path while preserving selected actions and fitted feature meaning.

Acceptance: nine cards per seat after return; originally received cards can be
returned too; no duplicate/vanished cards; fresh actual-exchange facts agree;
hidden-card swaps outside the view do not change assessment. The frozen baseline
still matches. Do not add unused generic plan interfaces.

### 2. Reachable activation (`plan-activation`)

Make `Reach` usable with the candidate allocation while retaining its present
live-play consumer. Describe who holds the pair, who must win a lead, which
announcement/question remains available and which cards must be protected.
Label its existing lead/probability approximations; do not call them guarantees.

Acceptance: retained, transferred and split-pair scenarios; all relevant
`Own`/`Yours`/`Ours` rights; a pair without a credible lead route; a half with only
uncertain completion; no announcement in the opening. Still preserve selection.

Correction from the later once-only-trump checkpoint: an already-called suit
cannot become trump again. A repeated half answer keeps the current trump and
awards no further pair points. Candidate continuations must use that engine rule.

### 3. Legal entries (`plan-entries`)

Compose the opening and a short continuation from the existing lead helpers and
engine legality. Distinguish an immediate entry, a delayed entry and a speculative
ruff route. Record the cards and opponent conditions the route depends on.

Acceptance: keeping an ace forces its opening; with no ace, green can prevent a
desired low side-suit lead; a returned ace with a retained legal low card can
support HL1; later ruffs/overruffs can break a side-suit entry. Test those conditions
with synthetic engine scenarios before attaching a score or changing a choice.

### 4. Compare return-and-plan candidates (`plan-choice`)

Review a small explicit set of routes: own pair, partner pair, split pair, and a
card-play fallback when activation is not credible. Consider each only where
its post-return facts and rights permit it. Compare routes in the existing
contract/valuation units, exposing what is certain and what is estimated.

Before enabling selection, review competing returns on the same visible hand:
which pair survives, where the entry lies, which opportunities it costs and what
fails if an opponent wins. Define the comparison and ties from these reviewed
cases. Do not silently apply C42's fitted coefficients to redefined features or
invent another uncalibrated weighted sum. Keep a bounded diagnostic comparison
against the baseline until approval and executable follow-through in stage 5.
Enable the chosen return and its supported continuation together; then replace
the superseded decision path and remove its dead parameters/tests. Pass-intent
inference is a separate task.

### 5. Follow through and replan (`plan-follow-through`)

Connect one approved return route to subsequent lead and activation decisions.
Re-evaluate it from the seat's current facts after each relevant event. If an
entry is gone, an answer is negative or trump changes, choose another feasible
route or use the existing card-play fallback. Do not assume a human partner
will follow a private plan merely because cards were sent.

Acceptance: return → opening → activation → continued play forms a legal trace;
disruption changes the route; needed halves are preserved only while useful;
fresh/reused agents agree after repeats, undo, forks and parameter changes.
The other partner must derive its actions from its own view, never shared memory.

### 6. Qualify a playable v1 (`plan-v1`)

Account for distinct trick prospects, reachable pair points and the last-trick
bonus through the existing scoring owners. A card's points in hand are not secured
game points; a pair contributes only on a reachable activation. An entry trick
cannot also be booked as a second independent winning trick. Review contract
estimates on concrete complete traces before a separate raising checkpoint.

Freeze parameters and compare the old/new policies on a predeclared fixed-size
duplicate suite, splitting private human evidence by whole game/session. Record
exact revisions, seeds, rotations, fixture digest, failures and decision latency.
Report human agreement separately from playing strength; do not tune on holdout.
Keep the existing legality, privacy, history and parameter guards. Disposition
the inherited ignored owner-play case and historical pass-calibration check.

## V1 completion gate

V1 means an owner-reviewed, bounded set of routes that connects return choice to
subsequent play, explains its assumptions, and replans or falls back legally.
Require the scenario suite, fixed-size strength evidence and an agreed measured
latency/resource budget; the testbed's F7 worker/revision work remains necessary
for responsiveness. These gates establish more than matching human choices.

Pre-play raising uses a narrow certified own-card route and one reachable own
pair, with stateless follow-through; sampled estimates do not justify a raise.
This slice has no series utility model. Full-game search, learned policies and
probabilistic pass-intent reading are not prerequisites for the first coordinated
planning slice. Once a replacement is accepted, keep a baseline checkpoint for
comparison instead of retaining deprecated production paths.

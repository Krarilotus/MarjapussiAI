# Entry and activation conditions

Owner-approved continuation, 2026-10-09. Base: `db53997`.
This checkpoint implements immediate trick/entry conditions and activation gates
inside existing owners; it does not claim a complete full-game forecast.

## Evidence before changing policy

The private replay study covers 256 eligible games and 2,790 play decisions by
the three requested players, both declarers and partners, without outcome
filtering. Games replayed successfully and their recorded points agreed.
The heuristic-only baseline selected its partner-ruff rule 86 times; 171
positions offered a lead into a partner's known void. Endgame was disabled
only for this diagnostic so it could expose the underlying heuristic.

Previously reviewed examples show: a returned card must be discarded before
a ruff entry exists; an opponent forced to follow cannot use its top trump;
later rounds can expose an overruff; a trump switch can create or remove an
entry; and an uncalled pair can be worth keeping while an inaccessible high
card can be fed as points. There are also human leads that cannot win: an
observed action is not automatically an entry or proof of optimality.

Private traces remain in `tmp/2026-10-09-entry-planning/`. Public fixtures use
synthetic positions and counterexamples, not copied player records.

## Meaning and ownership

- Engine remains the owner of legal follow/overtake/ruff, trump and rights.
- Solver supplies a small current-trick control query. For each named goal
  (a team's trick, a particular seat's lead, a surviving pair activatable at
  the next opening), its team maximises and opponents minimise that goal.
  Different goals can require different continuations: never sum their results
  as though one plan secures all of them.
- Beliefs owns the existing importance weights over consistent deals. Reuse
  those weights rather than repeat the endgame's conversion of card beliefs.
- Heuristic combines the current player's facts and joint weighted deals into
  an entry estimate. The same worlds serve all candidate leads. Opponents'
  following chances are measured jointly, not multiplied as independent events.
  Previous rounds matter through actual removed cards, revealed voids and
  constraints. The round number itself is not a guessed decay coefficient.
- A changed trump or observed action produces a new decision assessment.
  Future trump choices must be legal engine transitions; no freely assumed trump.

## Behaviour changes

1. The partner-ruff rule requires a winning entry, rather than merely holding
   trump. It keeps the existing confidence setting, now applied to the actual
   entry condition, and reports joint following and remaining-suit information.
   Top trump remains a valid route even when opponents cannot follow.
2. Delaying an announcement requires a specific card that preserves the
   leader's own next lead. A team winner that partner must overtake is not an
   own lead. Keep the existing question-attempt model, but count only a safe
   run: without trump, retained own winners; under trump, the retained trump
   run plus at most one side winner, played first. Later side rounds can become
   ruffable. Cap opportunities before the final trick and reassess after play.
3. Pair protection disappears when the counterpart/rights/remaining opportunity
   calculation gives zero activation value. Partner-held pairs also need a usable
   announcement or question route. Soft probabilities remain estimates.
4. The value of the next lead uses the actual winning seat and surviving halves.
   At most one immediate activation is credited to that opportunity.

Unknown future access is not labelled impossible merely because no immediate
entry was found. Broader multi-trick planning, calibrated score forecasts and
return-and-plan selection remain separate work. The existing longer-horizon
valuation is still an approximation, now constrained by these concrete checks.
Its partner-access estimate retains the live-Ace proxy and also checks immediate
entries when partner access is needed. This does not prove that every retained
pair survives an entire future sequence.

The short query enumerates all consistent deals when there are at most 64,
otherwise samples 64. Every candidate uses the same weighted deals; reads of a
previously queried card reuse its result within that decision. The next observed
state starts a fresh assessment. Marginal-product weights are the existing
approximation, shared with endgame evaluation. Exposed-deal capabilities can
still depend on unseen cards: these estimates are not a solution to strategy
fusion or calibrated game-winning probabilities.

## Acceptance

Check synthetic overruff and blocked-top-trump cases; decreasing suit lengths;
trump switch and restoration; partner overtaking an own winner; missing or
spent halves; all rights levels; last-trick cutoff; and hidden-card swaps.
Compare the short query against complete legal enumeration, then replay the
same human positions and inspect changed choices. Agreement is diagnostic,
not a strength claim. Measure whole-agent latency before the checkpoint.

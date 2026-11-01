# Preserve control before transfer or activation

Implementation note, 2026-10-14, starting at `af55bed`; validation pending.

The reviewed development examples expose three dependencies, not three universal
rules. A lead transfer can spend the team's last convenient entry before enemy
trumps are drawn. Returning low cards from a long top-controlled suit can destroy
its later winners even without a pair. Asking for a pair can replace useful trump
with a suit the opponents control. The available owner notes discuss auction
examples; the play-side scope is authorized by the implementation handoff.

Use the existing calculation chain: actor-visible cards, transfers and public
exclusions -> control/run and entry prerequisites -> candidate continuation in
each joint believed world -> legal engine transitions -> terminal engine score.
Control prerequisites are derived facts about resources, not proofs of the best
decision. Pair/void readings stay fallible. HO1/HO2 retain legal alternative leads
and questions; they may beat the control-preserving prior on expected score.

The small implementation slice is:

- Give drawing a safe top trump priority over a discretionary partner transfer
  when opponents may hold trump and another own trump survives. Keep side cashes
  and the existing last-trick exception. The continuation actor already cashes
  standing trump before transfer; share its activation-control prerequisite with
  the ordinary lead rules.
- With at least three cards, defer only Yours for a concrete cash that retains
  own lead and leaves partner a spare beyond its possible pair. The cash must be
  current trump, or a non-half in a no-trump run whose suit contains an own half
  (excluding partner's full pair in that suit). At two cards there is no spare
  guarantee and no such deferral; AskHalf is never suppressed by this guard.
  Cashing and asking remain competing root routes; when control is gone, ordinary
  activation resumes. A currently useful trump is not automatically the most
  valuable eventual trump.
  A legal own announcement retains priority: a run alone must not delay it until
  the next cash destroys a half. Explicit scored root routes may still compare
  waiting with announcing; an unplanned partner never follows that private wait.
- Add run-preserving return candidates, including runs without a pair, and compare
  them with the store baseline by engine score. Retain the previous known paired
  run prior and share it with the level-0 return model.
  `return_run_route = true` enables only this narrow comparison by default;
  `pass_route` remains false. With no extra candidate or an unresolved comparison,
  the stored return remains. Level-0 returns use the paired-run prior, or keep
  their longest proven top-controlled suit when no trump plan exists; that cheap
  continuation prior is not itself a recursive return forecast.
  Return comparisons use the current contract and do not simulate the subsequent
  Raising decision. Their scores therefore cannot claim optimal raised value.
- Standing checks retain the public union of remaining cards. A private
  activation sequence belongs only to the selecting seat; partner follows its own
  information and default policy. No sampled hidden hand or root plan is given to
  another actor.

The `in_hands()` standing check uses the union of unplayed cards, which is
public in a complete deal; it is not itself an ownership leak and stays unchanged.
The actual information issue was the shared private activation plan.

The existing `lead_draw_trump` switch remains false by default (removed 2026-11-01: switched
on, it changed none of 5,000 games against checkpoint 5). The reordered HL4 fallback is opt-in; the default behavior change comes from the continuation
question prior and actor-local route evaluation. This does not establish that
the recorded early-transfer choices are fixed; their replay remains required.

Counter-cases: no outstanding enemy trump; only one own trump (save an entry);
unsafe top trump; profitable side cash; exhausted run; pair points needed to make
the contract; a partner entry needed before a pair is consumed; a long suit lacking
the uninterrupted top needed to draw its outstanding ranks. No deal IDs, suit
identities or outcomes select behavior. Synthetic tests cover these distinctions
and hidden-card swaps. Root owns all builds, tests, runtime/strength qualification,
and live acceptance; this checkpoint must remain incomplete until that evidence.

Update 2026-10-31 (checkpoint 3): the return route also compares the store's best
returns (`return_store_candidates`), the pass forth runs by route (`pass_route`), and
both sample the exchange's own world pool (`exchange_worlds`); evidence in
[the checkpoint](../checkpoints/2026-10-31-exchange-routes.md).

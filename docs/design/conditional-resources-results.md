# Conditional resources: diagnostic findings

Read-only review, 2026-10-11. No policy edits, builds or additional runs.
These are selected-position diagnostics, not a strength benchmark or proofs of
optimal play. Steps refer to recorded prefixes; after divergence equal step
numbers do not imply equal states.

## Verdict

The bounded correction is not yet strategically sufficient for the three
primary cases. It preserves the examined successful controls and fixes one
rights-wasting opening, but executable multi-trick control dependencies and
earlier contract/allocation feasibility remain material gaps.

## 2403: represented first action, misranked continuation

At step 18 the acting player knows it holds six Acorn trumps, including A, Ten
and King, and partner retains the Red Ace sent in the exchange. Only Acorn
7/8/9 remain outside. Leading A, then further unbeatable trumps as needed, can
exhaust outside trump while retaining the lead. The proposed Red-8 entry is
therefore connected to a concrete resource condition: remove opposing ruff
capacity before attempting entry, while preserving partner's Ace and a way
to regain/control the subsequent lead. The complete entry outcome is not
proved solely by possession of the Ace.

Both policies instead lead Red-8. Candidate already compares Acorn-A: seven
root card candidates, eight completed worlds; reported made frequency 0.75
and mean score 396.25 for Acorn-A versus 0.875 and 483.125 for Red-8. This is
not a missing first-card candidate. These are finite-model rollout estimates,
not calibrated probabilities or certificates.

In counterfactual-2403-one, forcing ONLY Acorn-A at step 18 then resuming the
normal live candidate makes 200 with 200 raw points, versus normal candidate
87/200. All three outside trumps appear on that first trick (7/9/8); normal
candidate next leads Acorn-Ten at step 22. The three-forced-trump diagnostic
also makes 200. This demonstrates an executable successful continuation on
this actual fixture without claiming a win over every allowed hidden deal.

Forecast compares one opening followed by reduced-policy continuations:
planning.rs::Forecast::new disables return and recursive planning and caps
endgame at two tricks/eight worlds. Live candidate uses return/planning eight
and endgame four tricks/64 worlds. The actual forced-opening success thus
does not validate the forecast's fallback continuation. Missing explicit
draw-until-threat-removed/entry resource evaluation is a concrete architecture
gap; available records do not isolate its contribution from world coverage
or reduced continuation policy. Increasing first-card coverage alone cannot
fix this example. A useful next increment should evaluate and follow an
actor-visible conditional sequence, updating the threat condition after each
observed trick, using existing exact rights/entry and certificate ownership.

## 125 and 2549: late activation repairs do not rescue the contracts

125 at step 13 has a known partner Green pair, own standing Bells-Ten and
own question rights. Candidate still selects Bells-U. Forced Bells-Ten gives
35 raw points; Yours gives 53; Yours then Bells-Ten gives 51. All fail 155;
the question variants collect the known 40-point pair. This exposes a missed
activation opportunity but does not support attributing the contract loss
solely to that play or declaring a tested alternative sufficient.

2549 candidate changes the opening from the Red-half question to Acorn-Ten,
preserving question rights and the known partner Green pair. It later leads
Green-Ten and loses the lead without activating that pair. Retaining rights
is useful but lacks conditional follow-through. Forced Yours gives 69 raw
points; Yours plus Acorn-8 gives 92; Acorn-Ten plus Yours plus Acorn-8 gives
65. Each collects 40 pair points but still fails 160. Normal candidate gives
25 and no pair points in these single-seat replacement continuations.

For both cases, earlier auction/pass allocation and assessment of feasible
control/declaration sequences remain primary investigation targets: the
tested late repairs leave substantial deficits. This does not prove that
the contracts were impossible under every continuation. Root separately
reports full-from-start improvement for 125 (-445 to -210 score margin, new
contract 150) and 2549 (27 to 116 raw points, still failing 160); those are
different starting interventions, not attribution to the focal decision.

## Controls and limits

Teacher-focal has 34 complete results: 17 positions times two policies.
Sixteen of 17 focal actions agree. Partnership continuations agree throughout
15 of 17 trajectories; 2549 and 812 diverge. Examined focal controls 1113,
2649, 3390, 3545 and 162 retain their actions and trajectories. In particular,
1113 retains legitimate standing-card deferral, 3390 retains its certified
raise, and informational questions remain usable in 162/3545. This is local
preservation evidence, not a general safety or strength conclusion.

All eight counterfactual files finish with status complete, 2/2 results and
no reported error. Their replacement mode is Seat: only the focal seat is
replaced, while the other actors retain recorded-side frozen policies.
The partnership suite replaces both teammates. Do not conflate these scopes.
Actors receive filtered views; actual other hands establish retrospective
fixture outcomes, not what the focal actor knew. Private historical caches
and RNG states are unavailable; these are fresh-at-prefix comparisons.
No per-world decision traces/posteriors were captured, so reasons beyond
reported facts and inspected code remain unverified. Scores and raw points
are distinct; paired-game divide-by-two normalization does not apply here.

Evidence: tmp/2026-10-11-conditional-plan/{teacher-focal.ndjson,
continuation-partnership.ndjson,counterfactual-*.ndjson}. Teacher SHA256
0306c69cabe1624de9e65e6ebcb2e1560e3910027fd596e3a17b80b2421b40e7;
partnership SHA256
4ecafd058eae320f6355c91ce760ce96c6ee38b7bdea52b562c1618a95d819be.
Source: heuristic planning.rs::rollout/Forecast::new/compare/choose;
routes.rs entry activation and trump_plan.rs conditional resources.

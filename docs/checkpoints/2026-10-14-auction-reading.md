# Checkpoint: contextual auction reading and own ownership allowance

- Date: 2026-10-14
- Status: **incomplete — validation pending**
- Intended tag: `checkpoint/2026-10-14-auction-reading`
- Branch: `work/2026-10-14-v1-auction-reading`
- Base: `af55bedcebe982644b937573ee739dada254b9fd`, supplied worktree, initially clean.
- No paired testbed change, publication, or deployment.

## Diagnosis and result

The base counted every own bid made from a value of at least 140 as an ownership
challenge, including truthful information. Its keep could be read as TwoHalves
when the telling window remained open. The default four-action window masks that
particular nominal reading in the original long auction, but does not supply a
contextual ownership model. An Ace offer was also penalised for lacking an own
pair, and a pair-holding offerer could be modelled as sending its pair instead.

The diagnosis and plan were sent before edits. The scope follows the supplied
`astra-bidding-dependencies-2026-10-13.md` review, sections 4, 5 and 8: separate
information from ownership, retain the offer through intervention, preserve
external uncertainty, and apply each observation once. No claim of improved
playing strength or optimality follows from this consistency repair.

## Exact changed dependencies

| Concept / function | Inputs and resulting change |
|---|---|
| `beliefs::conventions::Auction::context/apply`, `BidContext` | Public holder, first bid, team Ace already shown, partner Red message; derive an available Ace-offer interpretation and a pending reply. Opponent bids do not erase the reply; the recipient's next action consumes it. No private cards enter public context. |
| `BidContext::live`, `info_to_tell` | A supported Ace offer remains live beyond the ordinary telling window and keeps Ace-first ordering even with an own pair. The existing +5 code is unchanged. |
| `nominal`, `bid_likelihood` | A high +5 outside the supported Ace-offer context has no nominal card message and a class-independent likelihood. Keep/control/information intentions remain unresolved, rather than asserting halves or a further pair. The Ace-offer context removes both own-pair crossing penalties. Existing deviation/trust mass still permits offers without an Ace. |
| `heuristic::bidding::own_challenged` | Only our own bid observations, unchanged auction hand class, existing control-run result and convention parameters. Recognise own control/keep prerequisites before considering truthful live messages; accumulate own told bits to prevent a repeated message being exempted twice. A truthful new information bid does not spend ownership. This reconstruction is not applied to other seats. |
| `own_the_game`, its call in `decide` | Reuse the shared context and borrowed own observations instead of allocating a second history and cloning bid lists. Keep requires the actual Red pair offered to and uses the current legal +5 after intervention. Existing control-run and cheap-partner-pass prerequisites remain. |
| `heuristic::rules::{BControl,BOffer}` | Descriptions now distinguish Ace information from an ownership challenge and a keep from halves. Provenance is owner/derived, not measured human evidence. Rule IDs and milestones remain unchanged. |

The old fitted `take_over_140` implementation and `b_take = false`,
`route_take = false` defaults are unchanged. No heuristic lib/context, valuation,
outlook, play, passing, solver or general belief-cache code was edited. No new
offer increment, hand-length cutoff, confidence weight or strength threshold was
introduced. Allocation removal is a source observation, not a measured speedup.

## Counterexamples and focused tests

All new tests are **written but unrun**. They use synthetic public auctions and
small hand classes/constructed hands; no new private dataset or player IDs are
committed.

| Test | Counterexample / assertion |
|---|---|
| `information_above_140_does_not_spend_the_ownership_challenge` | Recorded truthful Red +20 from 140 to 160, partner Ace offer 165: keep 170 remains available. After that keep, another challenge is unavailable. The current ordinary sender still does not reopen its general information channel at 140. |
| `keep_survives_intervention_but_requires_the_offered_red_pair` | Red 145, offer 150, opponent 155: next legal keep is 160. Substituting an unrelated Bells pair after a Red bluff removes the keep rationale. |
| `an_offering_hand_with_its_own_pair_still_sends_only_plus_five` | Ace plus own Green pair opposite Red 145 still offers 150; no invented +10 offer. |
| `own_information_is_truthful_and_consumed_once_even_when_publicly_ambiguous` | Own untold Ace +5 over an opponent with a team pair shown is information, despite neutral external reading. Repeating it, or removing the Ace, cannot receive the same exemption. |
| `own_control_is_not_reclassified_as_an_untold_ace` | A control step after the own Bells message also numerically fits Ace. Recognising control first spends the allowance; disabling the control prerequisite exposes the distinct information interpretation. |
| `offer_reads_the_ace_without_requiring_an_own_pair_or_tell_window` | Beyond the ordinary telling window, Ace-only and Ace-plus-pair offerers receive identical offer likelihood. A no-Ace hand is less likely but remains possible. |
| `keep_adds_no_halves_or_second_pair_evidence` | With telling open, keep has no nominal halves, flat class likelihood and no change to the earlier pair posterior. The reply is consumed once. |
| `intervention_preserves_offer_but_does_not_create_one` | An intervention after an offer preserves it; an opponent holding the bid before a would-be offer prevents it. Ordinary low TwoHalves reading remains. |

Existing bidding tests retain the control-run missing-Ace/gap cases, the
once-only control response, the unsafe first-pass control, and the offer without
an Ace. They were not rerun or presented as new evidence.

## Validation evidence and handoff

| Check | Actual result / limit |
|---|---|
| Source and caller inspection | Read repository instructions, workflow, overview, next steps, supplied review, bidding/convention code and relevant fixtures. All in-repository explicit `BidContext` initialisers are within the edited convention module; its auction constructor sets the new fields and tests use defaults. |
| Diff review | Only the four scoped Rust files and this checkpoint are intended for staging; no fitted-take/default changes. |
| Whitespace | `git -c core.whitespace=cr-at-eol diff --check` passes. Plain `git diff --check` reports CRLF as trailing whitespace in the two existing CRLF source files; their line endings are intentionally retained. |
| Build, formatting, tests, benchmarks, privacy/scenario execution | **Not run**, as expressly required by the task. Compilation and runtime behaviour remain unverified. |
| Strength/performance qualification | None. No benchmark dataset was generated or consumed. |

Root validation should run, one job at a time, through its existing two-CPU,
below-normal-priority, 4 GiB process-tree wrapper, with `CARGO_BUILD_JOBS=1`, the
checkout's retained cache and the optimized standard test profile:

```text
cargo test -p marjapussi-beliefs --lib conventions::bids::tests -- --test-threads=1
cargo test -p marjapussi-heuristic --lib bidding::ownership_tests -- --test-threads=1
cargo test -p marjapussi-heuristic --test bidding -- --test-threads=1
```

Root also owns formatting/lint and affected posterior/privacy/regression review.
Share its actual results before promotion. No verified bidding checkpoint was
established in this task; the supplied `af55bed` baseline and all earlier tags
remain intact. This incomplete tag does not replace a verified baseline.

## Remaining limitations

- A class-independent high +5 deliberately discards possible information and
  strength evidence when intent is unresolved. It is not a calibrated mixture
  or a claim that external actors always follow the current policy. Its effects
  on later pair-cover decisions need root validation.
- Own intent is reconstructed from current semantic prerequisites, not recovered
  from a persisted decision-ID log. Arbitrary imported own histories or a policy
  change during an auction can remain ambiguous. This bounded implementation
  does not extend the agent's state/cache API.
- The inherited keep remains a policy prior, not a comparison of receiver,
  return and defensive routes. Preserving a reply after intervention is grounded
  in the negotiation history, not proof that paying the new price is profitable.
  The reply path and neutral reading need owner review after validation.
- No full control-versus-defence evaluator, scoring/auction continuation model,
  pass-likelihood redesign or inferred external ownership counter was added.

## Cleanup

No builds, tests, benchmarks, services, subagents, containers or disposable build
outputs were started or created. No cleanup was needed; reclaimed space: **0 B**.
Existing caches, data and worktree resources are retained for root validation.

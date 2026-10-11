# Common continuation ownership — source checkpoint

Status: incomplete validation. This is a source handoff, not a verified runtime or playing-strength checkpoint.

## Change

Auction/return and play formation/repair now use `Forecast::evaluate_programs`, one completed-response adapter, and one assessment/receipt installation and deferral path. Typed mode and allowance inputs preserve the different explanation labels and formation/repair scheduling. Cohort construction, candidate ordering, auction conventions and minimum-bid shortlist, return nomination, response parameters, deadlines, finite phase horizon, retained intent and recorded contexts are unchanged. Return-only repair still uses the original requested cohort size when play planning is disabled.

Caller review found no production users of the old `Forecast::compare`, scalar `Outcome`, rollout or planning decision cache after the auction/return migration. Those implementations and their obsolete direct tests were removed. Reuse scenarios now exercise the common evaluator path: repeated and expanded domains, reused versus fresh hypothetical actors across the existing seeded forks, receipt/context installation and deferred invalidation, and changed visible metadata/private hands.

The post-exchange joint-cohort fixture previously retained only one Ace, making the opening forced and producing no strategic choice assessment. It now retains two Aces, varies only opponents' cards, and explicitly checks identical actor-visible information and two legal opening leads after the actual modeled exchanges. The assertion requiring one assessed choice with both original weighted hypotheses remains intact.

## Validation and integration

- No build, test, replay or benchmark was run for this iteration, as instructed by the root owner. Formatting and whitespace checks and focused-patch applicability checks are source checks only.
- The root's preceding phase-runtime run reported 72 passing tests and the one original joint-cohort fixture failure. The revised fixture and new common-path tests have not run; the failure is not claimed resolved by runtime evidence.
- The common adapter uses the existing strict completed-response compatibility helper. A's typed leaf/effort contract remains a separate coordinated integration; canonical solver-effort propagation is not newly claimed here.
- The focused handoff patch is against the root owner's captured, frozen files, including its dirty auction/live-response and return-only repair integration. Apply that focused patch rather than assuming this worktree's parent commit contains every shared dependency. Evaluator and concept definitions were not edited.
- Existing finite coverage remains: at most 16 root phase responses, explicit deferred exhaustion, bounded formation/repair work and deadlines, and unsupported historical exchanges whose cards are hidden to the observer. This does not establish complete strategic coverage or playing strength.
- No private fixtures were copied, and no new build outputs, containers or processes were created. Reclaimed space: 0 bytes; shared caches and pending-integration source snapshots are retained.

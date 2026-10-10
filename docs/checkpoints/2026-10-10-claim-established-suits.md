# Remaining-trick claims with established suits

- Status: verified for the claim scope below.
- Branch: `fix/claim-established-suits`; base `a5d7204`.
- Tag: `checkpoint/2026-10-10-claim-established-suits`.
- Paired testbed: `checkpoint/2026-10-10-instant-play-claim`.

The knowledge-layer certificate now admits lower cards after preceding winning
leads exhaust the suit in every other hand. It uses the claimant's current suit
bounds, played cards and legal actions. Trump changes, forced following, possible
ruffs, announcement rights and preservation of own pairs remain part of the proof.
An opening claim can start during raising through an ordinary legal card play.

The owner approved collecting legal own pairs while leaving possible partner or
split pairs unused. The optional claim still certifies every remaining trick;
it does not promise that an arbitrary existing contract will be reached.
Recorded opening claims replay with their automatic-completion provenance.

Validation, using the existing two-CPU/4-GiB sequential runner:

- 14 knowledge claim tests, including exhaustive legal replies for suit exhaustion
  and a counterexample with an outstanding higher trump.
- Knowledge hidden-hand-swap and soundness checks.
- Five legacy claim/import checks.
- Ten paired server claim checks and the native opening/skip/raise scenarios.
- Exact owner-reported nine-card position: nine tricks, 220 points including the
  own Bells pair. A 100-call local seat-view probe took 28.43 ms in total; this is
  a small diagnostic measurement, not a production latency qualification.
- Formatting and diff checks pass.

Evidence is retained under outer-workspace `tmp/2026-10-09-v1/claim-established-*`
and `reproduce-claim-34.*`. The initial paired server check expected the previous
partner-pair refusal; that expectation was updated to the owner's approved rule.
An initial Python run used an older native extension; the rebuilt wheel was
installed and all affected tests rerun successfully. No unresolved test failure
remains in this scope. Independent agent review was unavailable because the
existing T3 agents relay could not be reached; no window was restarted.

Auction, return selection and play policy are unchanged. The current human
auction and live continuation audits are diagnostic work, not a strength claim
or an implementation of their proposed strategic changes. Reusable build caches,
private replay evidence and approved card source archives are retained.

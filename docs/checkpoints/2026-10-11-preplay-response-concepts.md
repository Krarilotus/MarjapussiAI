# Checkpoint: explicit preplay response and exhaustion concepts

- Date: 2026-10-11
- Status: source-only, incomplete; no builds/tests (root owns validation)
- Base: `29db18ce0b25db5dfb2a1dc98657037f347cebed`
- Tag: `checkpoint/2026-10-11-preplay-response-concepts-source`
- Ownership: concept sequence module and dedicated concept tests

## Locked primitive interface

- `Node::Response { next: Stage }`
- `Frontier::Response { next: Stage }`
- `ReadMeaning::Response { next: Stage }`
- `Node::Deferred`, `Frontier::Deferred`, `ReadMeaning::Deferred`
- `Guard::PlayReady`

Response is one modeled actor action followed by the explicit successor. It is
not a root alternative selector or an implicit whole-game rollout. Canonical
suffixes include the successor; validation checks it and rejects cycles. The
dispatcher records the consumed transition separately from Tail's ResponseLeaf.

PlayReady is true exactly when the supplied legal domain contains this observer's
CardPlayed, Question, AnnounceTrump or Answer action. It does not inspect hidden
game phase, opponent hands or another actor's legal actions. Optional raising
with legal cards qualifies; auction-only bids and exchange passes do not.

Deferred is a valid authored leaf and a distinct dispatcher frontier. A's
evaluator must map it to SequenceError::Budget with the phase-gate exhaustion
diagnostic. It must never become Interrupted or a legacy Tail. Root owns the
finite bridge constructor and its 16-response bound, preplay policy routing,
passing-forecast migration and integration tests. A owns grouping by full root
Information, canonical stage and exact legal domain, one strict completed
response for the entire conditional support, receipt proposal/application and
resumption. Unbound prepared Response may conservatively return NeedsReview.

Exact Pass and StopBidding nodes are admitted. Receipts accept only the matching
own StopBidding or own visible Pass, with correct partner recipient and exact
card-set equality. Redacted/wrong-player/wrong-recipient/different-card events
cannot advance the receipt. Preplay actions (Pass, StopBidding, NewBid) and
Response transitions do not spend or replenish the play reconsideration bound;
their explicit successors retain that bound.

## Validation and remaining gates

Rustfmt and whitespace checks passed. Four source-only tests cover actor-owned
legal PlayReady and the 16-response shape ending in explicit Deferred; canonical
Response edges, cycle/index rejection and separation from Tail; accepted-once
StopBidding; and rejected/retried/accepted first-pass and return-pass receipts.
The gate test follows static edges to check the unrolling, not simulated engine
response execution. A/root must validate actual response commitment, common
weighted support, strict leaf completion and parent-value propagation.

Existing catalogue witness traversal explicitly rejects these out-of-scope
preplay operators. No evaluator or root integration file was edited. Suggested
root concept filter after operator integration:
`cargo test -p marjapussi-heuristic --lib concepts::sequence:: -- --test-threads=1`

No smoke, benchmarks, build processes or disposable outputs; reclaimed 0 bytes.

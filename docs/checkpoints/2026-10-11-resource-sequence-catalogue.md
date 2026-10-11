# Checkpoint: finite resource-sequence composition

- Date: 2026-10-11
- Status: incomplete; source-only, no builds or tests (root owns validation)
- Base: `18de60a13b4ca3c215f8db60885c5855c198aa29`
- Tag: `checkpoint/2026-10-11-resource-sequence-catalogue-source`
- Ownership: concept sequence module and independent concept tests

## Authored behavior and scope

Replaces flat per-side activation/draw families with a shared finite DAG:
up to two standing cash leads, an optional activation, up to three standing
trump-control leads, transfer, then next-leader-conditioned recovery and a
second activation/cash opportunity. Root lead, partner lead and opponent lead
are consumed guards. Opponent arrival additionally compares a suit-development
investment; it is not assigned a defensive bonus. Lost lead can retain a goal
through two actual root follows. Exact truthful-answer alternatives permit a
root answer before bounded recovery stages without commanding another actor.

Each held suit also offers a non-standing investment, guarded by a surviving
outside-suit entry, followed by recovery into the shared opening. These cards
are hypotheses, not proven winners. Existing engine legality, actor ownership,
observations and weighted common execution remain the only runtime machinery.

Activation no longer requires a side card or a certainly held partner half.
All announcement/full-question/half-question actions are authored; legal binding
filters their actual availability. Unknown partner ownership remains model mass.
Actual trump is read after responses and each control lead, including a different
truthful suit, negative answer or counter-trump. Surviving entry counts drive
transfer availability; lost entry falls back to a compared cash continuation.
Question attempts are finitely unrolled, not assumed always to consume a new
engine right. Rights and truthful answers are enforced by the engine.

Optional raising shares the same full opening DAG. `with_reconsideration` does
not insert a global boundary after Raises/Exact(NewBid); the ordinary play
opportunity remains available after the bid. Routine stages need zero global
reconsiderations. Global reconsideration still has maximum depth one, independently
of these authored bounds. Bids are not a card-by-opening Cartesian catalogue.

Nodes and successors are interned; identical DAG portions share storage. The
finite stage library replaces the superseded flat families, rather than keeping
them as dormant alternatives. Direct legal-action tails and the named legacy
tail remain explicit outside-scope alternatives. The larger conditional search
still needs completion/performance measurement: static sharing does not remove
distinct resource histories or justify dropping worlds/branches. No claim of
full game-strategy migration or improved playing strength is made. Auction and
allocation consumers remain root's separate integration work.

## Source validation and outstanding gates

Rustfmt and whitespace checking passed. Added independent concept scenarios:
legal engine exchange/raising into a no-side-card activation; uncertain split
inquiry and observed response; two cashes -> activation -> draw -> transfer ->
legal follow preserving a later goal; finite interned structure and equal
raised/no-raise opening horizon. Existing split/raise fixtures now check the
new catalogue; the bid-receipt fixture retains an explicit post-bid boundary
to isolate receipt/binder semantics from where production schedules repair.

All scenarios are source-only and unexecuted. They witness domain/execution,
not an optimal-action or must-win requirement. Root should run concept tests
after A integration, then joint-weight/ancestor-value, privacy, live receipt,
coverage and useful completion gates. No smoke/benchmark is authorized by this
checkpoint. Suggested root filter:
`cargo test -p marjapussi-heuristic --lib concepts::sequence:: -- --test-threads=1`

No build processes or disposable outputs created; reclaimed 0 bytes.

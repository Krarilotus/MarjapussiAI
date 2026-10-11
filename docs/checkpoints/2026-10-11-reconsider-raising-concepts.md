# Checkpoint: optional raising and finite global reconsideration concepts

- Date: 2026-10-11
- Status: incomplete; source-only parallel handoff, no builds/tests
- Tag: `checkpoint/2026-10-11-reconsider-raising-concepts-source`
- Base: local `e6e34e7` (root `19b4efd` cherry-picked after root integration `e0b9c3d`)
- Source ownership: `concepts/sequence.rs`, `tests/internal/sequence_semantics.rs`

## Authored behavior

The first-play catalogue includes one `Actions::Raises` family binding every
actor-owned legal NewBid, alongside its no-raise opening routes. All bids share
the same successor edge. Exact NewBid nodes are valid and receipts consume only
the matching accepted Bid seat/value. No heuristic picks or prunes a raise.

`with_reconsideration(program, 1)` decorates authored action successors with
`Reconsider { remaining: 1, retained }`. At that actual future observation,
`bind_reconsideration` binds the complete current authored catalogue plus the
retained suffix into one semantically deduplicated DAG and an explicit Choice.
The count and retained edge participate in canonical identity. Every successor
uses remaining minus one: zero removes global boundaries, retaining ordinary
authored actions/guards/choices and named tails. Explicitly binding a malformed
zero-count boundary returns Budget; it does not silently pick the retained edge.

The semantic bound is one global reconsideration per path, separate from time
or work budgets. Structural caps are 4096 nodes and 128 alternatives. Admission
failure returns Budget with no truncated catalogue. The latter cap accommodates
all engine bid values plus opening alternatives. The shared graph saves static
bid-by-route construction; distinct contract-dependent responses and outcomes
still require evaluation. This is not a pruning or latency claim.

`Reconsideration` contains exact Information, legal actions, source suffix,
composed Program, actual alternative stages and the retained stage.
`Receipt::install_reconsideration` checks accepted information, no pending action,
the reached source boundary, exact legal list and valid composed data before
installing its program/start. Installation neither chooses nor accepts an action.

## Integration obligations

A owns evaluator and persistent-cache integration. Group full root Information
and source suffix on identical weighted support before binding/ranking. Evaluate
the composed Choice before ancestor values/root selection; do not append a
post-selection ranking. Preserve all mass. First offers must carry their owning
composed Program, because raw stage indices cannot be applied to the original
candidate. Root owns policy routing, initial work allowances and live coverage.
Coverage must refer to the executed composed alternatives at the declared
remaining depth, not catalogue availability or a posthoc graph. Evaluator
exhaustive matches and canonical dependency compilation need the new variants.

The default plain `authored_programs` family uses named tails; root must call
`authored_programs_with_reconsideration(..., 1)` to enable the future comparison.
Further global opportunities after the one boundary require explicit live review;
the exhausted horizon makes no current-dominance claim for those decisions.

## Validation and cleanup

Rustfmt on the two owned files and whitespace diff checking passed. Four added
concept regressions are unexecuted: all legal raises and accepted/mismatched
receipt events; composed-program installation and current-domain inclusion;
boundary identity/depth exhaustion and idempotent decoration; structural
exhaustion and preservation of a still-legal non-catalogue retained route.
Runtime weighted-choice/ancestor propagation tests belong to A/root integration.

After integration root can run:
`cargo test -p marjapussi-heuristic --lib concepts::sequence::tests:: -- --test-threads=1`

No builds, smoke, benchmarks or processes were launched. No disposable outputs
were created; reclaimed space 0 bytes. No playing-strength/performance claim.

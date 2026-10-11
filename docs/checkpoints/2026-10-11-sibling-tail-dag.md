# Sibling tail dependency sharing

Status: **incomplete; source only, no builds/tests executed**. Base: `c49a1c6`.
Owner approved the narrow `sibling-tail-dag-v1` accounting boundary. Intended
immutable tag: `checkpoint/2026-10-11-sibling-tail-dag`.

## Change and equality boundary

Inside one `choose_bound` call, Stage siblings that reach Tail or empty-action
Interrupted through current guard inspection share one completed tail dependency.
The full original joint input is identical by construction: every child is
cloned from the same immutable Waiting group and only its receipt stage changes.
All actor-visible private histories, tables, context, weights, origins and
accepted/pending state are retained. Normalization rejects pending transitions,
non-root actors, ended tables and every non-tail frontier. No engine action is
aliased; no world, horizon, available action or authored alternative is removed.

The first qualifying edge evaluates every member of the joint group. Later
qualifying edges refer to those outcomes within this comparison only. Each edge
gets its own consumed guard/binding reads and interruption bit, followed by the
shared tail's reads. Existing trick and first-offer prefixes remain identical;
Tail supplies the same canonical first-program owner if no first offer existed.
All original alternatives remain in order and receive complete values. The value
store still performs the complete comparison and retains first-on-tie behavior.
There are no child choice certificates inside this dependency. E's parent/local
certificate logic and D's canonical lookup are unchanged.

## Explicit bounded-work policy change

`Work::accounting()` returns `sibling-tail-dag-v1`. A unique sibling tail dependency
is charged once, whether optional caches are disabled, cold or warm. Optional
memo hits still charge their represented dependency cost. Comparisons and actual
engine/solver physics counters retain their existing meanings. This changes the
old repeated-tree bounded-work policy; it is not claimed as an old-policy cache
optimization or as general whole-comparison unique-DAG accounting.

`tail_aliases`, `tail_alias_transitions` and `tail_alias_solver_nodes` describe
aliases actually constructed and their avoided expansion. Their sums saturate
and are diagnostics only. A memoized parent can skip constructing these edges,
so these diagnostics need not match across cache states; represented allowance,
complete values, decisions and receipts must match. Operational reporting is B's
separate `sibling-tail-work-capture.patch`; apply its model label only with this
implementation. No budget setting or fixture was changed.

The local dependency references outcomes already retained by the original
comparison, rather than retaining another cache of traces. Per-edge inspection
storage is bounded by current cohort and finite authored guard depth. This first
slice does not share nonempty actions, general Choice/Response/Reconsider
frontiers, or fresh catalogue values across different retained boundaries.

## Validation and integration

Five source tests cover independent per-candidate outcome references on two
different private completions, all alternative values/order/ties, true/false/
unknown guard evidence, actual alias edge reads/interruption, first receipt
ownership, exclusion of engine-action aliases, current weight/context rebuild,
and exact-limit cache-disabled/cold/warm equality with one-less-budget failure.
Expected values come from ordinary independent candidate evaluation, not the
normalization helper. The direct alias test additionally checks each edge's full
terminals, because ties normally select the first computed edge.

Validation performed: rustfmt parsing/new-file format checks, diff whitespace
review and focused patch applicability checks. Whole `sequence.rs` format check
still reports two pre-existing formatting differences (extra blank line and an
unrelated long test call); those hunks are deliberately untouched. No Rust
compilation or tests run here.
Root owns targeted `cargo test -p marjapussi-heuristic --lib sibling_tail --
--test-threads=1` in its constrained runner, then existing full policy tests.
The unchanged depth-two and six-card multistep fixtures remain completion gates;
their prior represented limits were exhausted. No completion or strength claim.

Owned source: new normalization helper/test module, `choose_bound` child loop,
Work metadata and module registration only. No root edits, build processes or
build outputs created. Reclaimed space: zero bytes; integration evidence retained.

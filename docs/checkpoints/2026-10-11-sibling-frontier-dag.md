# Sibling frontier dependency sharing

Status: **incomplete; source only, no builds or tests run**. Base `0858a49`.
Owner explicitly approved the narrow V2 boundary after E's certificate proof.
Intended tag: `checkpoint/2026-10-11-sibling-frontier-dag`.

## Contract

Within one immutable `choose_bound` joint group, Stage siblings may share a
completed dependency after pure guard inspection reaches an equal typed ordered
domain: Choice's entire canonical successor vector, or Actions with more than
one offer and every ordered (action, canonical successor) pair. Only receipt
entry stages differ. All four views/history, origins, weights, context, accepted
state and pending state retain their original scope. No action is taken during
normalization. No world, action, alternative, horizon or budget is removed.

All original alternatives are still assessed in their original order, preserving
first ties. This is computation equality, not strategic dominance. Singleton
Actions, Response, Reconsider and Deferred use ordinary evaluation. V1's Tail /
empty-binding Interrupted sharing remains unchanged. There is no cross-comparison
cache or root-information-only grouping. Canonical successors include remaining
reconsideration depth and retained edges.

Projection relies on a specific producer boundary: multi-Action/Choice drive
yields before any action or certificate; its completed records are `[Local
entry(E), independent selected descendants]`, and terminals contain `P+E+D`.
The alias validates exact typed domains, complete Local assessment, support,
context, membership, entry/read shape and each positional prefix. It replaces
only its own entry certificate and exact prefix; all deeper records are copied
unchanged. Existing first offer/program ownership wins. Without an incoming first,
Actions reconstructs its original entry owner using `first_step`; Choice keeps
the selected child's owner. Shape mismatch falls back to full ordinary evaluation.
No generic read-span rewriting or public selected-plan type was introduced.

## Work policy and reporting

`Work::accounting()` now returns `sibling-frontier-dag-v2`. One unique eligible
sibling dependency is charged once, with optional caches disabled/cold/warm.
Completed parent memo hits debit the represented unique dependency cost. Actual
engine applications, actual value comparisons and actual solver work retain
their meanings. This explicitly changes bounded represented work, not world
quality or the numeric allowances; it is not an old-policy speedup claim.

New diagnostic fields: `frontier_aliases`, `frontier_alias_transitions`,
`frontier_alias_comparisons` (usize), `frontier_alias_solver_nodes` (u64).
They count actual successful projections and avoided represented expansion,
saturating and never granting allowance. A parent memo can skip alias creation,
so diagnostics may differ across cache states even when represented work is equal.
B's separate `sibling-frontier-work-capture.patch` consumes exactly this API and
must be integrated alongside V2, preserving old capture labels.

The arena references completed outcomes already owned by this comparison. It
retains finite per-edge inspection/domain descriptors, not another parent trace
cache. Benefit and added normalization overhead remain unmeasured.

## Validation and remaining gates

Five new source tests cover independent separate-alternative references on two
legal private completions; order/values/support/evidence; direct alias projection
for every guard edge and actual first-program rebasing; malformed entry/prefix
misses; cache-off/cold/warm and an observed completed parent memo hit under equal
exact allowances; one-less transition allowance rejection; different successor /
ordered domain / frontier-kind exclusions; genuine accepted first-action, read
and completed-trick prefixes; and a later Local descendant assessment. The latter
test also sets an inherited interruption bit solely to check trace transport.
It does not manufacture game history. Existing V1 test updates only its current
accounting label. New tests use raw Execution records and do not depend on the
removed Comparison.prepared field, so D's prepared() borrowing migration fits.

Static checks: rustfmt parses/formats new helper and tests; git diff whitespace
check. Whole sequence.rs formatting check reports existing unrelated helper/test
formatting hunks; they are deliberately untouched. No compilation or test result
is claimed. E reviewed the production projection without finding a blocker.

Root owns `cargo test -p marjapussi-heuristic --lib sibling_frontier --
--test-threads=1` in the constrained single-job runner, then the unchanged policy
gates. Prior V1 result: depth-two fixture passed its unchanged cap; six-card
multistep still exhausted 500000 represented transitions with no candidate
complete. Neither fixture nor limit changes here. No smoke or performance claim.

Owned changes: new frontier helper/tests, choose_bound Stage-child hook, Work
metadata, one V1 label assertion, this checkpoint. D's canonical/suffix helpers
and prepared migration, E's certificates, and B's exporters remain separate.
No root edits/processes/build outputs. Reclaimed scratch space: zero bytes.

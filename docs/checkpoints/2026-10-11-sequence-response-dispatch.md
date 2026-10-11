# Grouped explicit response continuation

Status: **incomplete; source-only handoff**, following owned `a09078cd`.
Depends on C's catalogue `d7dcd149`, fixture correction `29db18ce`, response
operators `75f2b4324a974cf6982bb3d64640e78f61f96517`, and initial-binding horizon
`b5620f169bdc8f413488227f1f1b4e8c16960486`. Imported separately in this worktree.
Root owns its strict deterministic completed response, actual preplay graph,
retained helper, value integration and resource-bounded validation.

## Owned evaluator change

`planning/sequence.rs` handles Frontier::Response { next } using the existing
strict callback and shared completed dependency cache. It waits at the current
entry cursor; canonical entry suffix includes any preceding guards. The common
executor groups full root Information and that suffix before invoking the leaf.
It checks identical actual legal actions and successor suffixes for every group
member, obtains one deterministic completed response for the group, and commits
the same action with each programme's owning next stage. Normal engine application,
filtered observations and receipt synchronization resume the authored successor.
No strategic maximization occurs at Response, and no hidden-world optimum is
selected. Other actors still use only their own hypothetical Table views.

The runtime PreparedChoice records a singleton Offer, selected 0, assessment
None, exact input/context/entry suffix, consumed guard/Response readings and
the entire group's original (origin, weight) support. Its Membership.legal is
canonical (UndoRequest excluded); Membership.programs is empty because no
strategic programme domain was compared. This matches E's dedicated
preplay::prepared_response helper, which checks the legal domain and actual next
suffix. Root must pair that helper with this commit for live carryover.
General prepared_lookup deliberately returns NeedsReview at Response; it neither
duplicates E's policy mechanism nor silently warms legacy Tail.

Frontier::Deferred returns SequenceError::Budget with
DeferredLimit::AuthoredGate. It never enters Interrupted, a legacy tail, a partial
ranked result or a fabricated terminal. Completed response failures stay uncached
and defer the whole comparison. Completion is still enforced by root's strict
wrapper: a bare GameAction return type cannot prove completion or deterministic
canonical solver effort. No ordinary lane, sample count, catalogue ordering or
response strength was changed here. Depth 2 is handled through C's validated
binder; the evaluator adds no special depth policy.

## Evidence and limits

Two new source regressions cover grouped hidden allocations, exact singleton
and whole-support certificates, entry guard consumption, owning pending next,
fresh support/context under nondyadic reweight with completed leaf reuse; and
explicit gate exhaustion with no tail, plus retry proving an incomplete root
response did not populate the cache. All new tests are **unexecuted**.

Rustfmt parsing/formatting and diff whitespace checks are the only local checks.
No builds, tests, benchmarks, smoke or policy runs. Root owns compilation, shared
runtime export and receipt/helper integration. Canonical solver effort on hits,
completed-response eviction, lookup indexing and measured completion/latency
remain distinct follow-ups; this source checkpoint is not exact-acceleration,
strength or readiness qualification. The cache ledger does not bound transient
recursive outputs or response solver memory. No disposable jobs/resources created;
0 bytes reclaimed.

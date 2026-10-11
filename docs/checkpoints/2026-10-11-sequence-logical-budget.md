# Checkpoint: sequence logical cache budget

- Date: 2026-10-11
- Status: incomplete; source-only, compilation/tests reserved for root
- Tag: `checkpoint/2026-10-11-sequence-logical-budget-source`
- Base: `0d2e4c7`, the local cherry-pick of A's frozen `11386b63dcd62a03e80b8f910f6a75df8e1d77bb`
- Owned code: `planning/sequence.rs` and `tests/internal/sequence_execution.rs` only

Direct engine transitions and value comparisons now debit represented logical
work. Completed suffix hits debit both stored costs before publishing results;
tail hits debit their transition cost. Nested cache hits contribute to a newly
stored suffix's represented cost. Actual work counters remain separate, and
response leaves receive the logical transition allowance still available.

`Limits::reuse_suffixes` defaults to true. Disabling it disables completed
suffix/tail reuse while preserving the frozen actor-response realization and
the existing response limits. `Work::represented_comparisons` complements the
existing represented transition count. No warm coverage or strategy changes
are included. Unaffordable cached units reject atomically: aborted work counters
can differ from partial uncached execution, but no partial ranking is returned.

Two unexecuted regressions cover nested comparison reuse and distinct prefixes
sharing a tail. They compare completed values, receipts, execution/prepared
traces and candidate coverage under matched logical allowances; the nested
fixture also compares subsequent leaf allowances. Tight transition/comparison
lanes must defer both with and without suffix reuse.

Validation: rustfmt on the two files and `git diff --check` passed. No builds,
tests, smoke runs or benchmarks were launched. These modules still depend on
root's registration/integration. Root's targeted command after integration:

`cargo test -p marjapussi-heuristic --lib planning::sequence::tests::logical_budget_ -- --test-threads=1`

Root owns the resource-constrained build lane and separate warm global coverage
fix. The first-slice policy is not qualified by this source checkpoint. No owned
processes or build outputs were created; cleanup reclaimed 0 bytes.

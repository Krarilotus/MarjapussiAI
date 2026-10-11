# Checkpoint: borrow selected execution records

- Date: 2026-10-11
- Status: incomplete; source cleanup only, owner validation pending
- Tag: `checkpoint/2026-10-11-comparison-prepared-borrow`
- Branch: `perf/scoped-suffix-reuse`
- Base commit: `4b4e1a22df83195a530f3fe13ab032c7800abc1a`
- Root source baseline: `0858a491152a6899c091c33401f210dbd8db2eb0`

## Result

Remove the redundant `Comparison.prepared` allocation and its completion-time
deep clone. `Comparison::prepared()` borrows exactly
`executions[values.selected].prepared`, preserving raw record order and support.
`SelectedPlan` and its coalesced authorization records are unchanged. Execution
and memo record storage and selected-parent construction are unchanged.

All 18 caller sites across five source/test files now use the accessor, including
the integrated sibling-tail parent-memo and reweighted-context tests. Intentional
test-owned record cloning remains; slice consumers no longer take a needless
reference. Repository-wide caller/constructor search found one production
Comparison initializer and no additional callers. No new fixture was introduced.

This saves one deep copy when a whole comparison completes and when its result
is cloned. It does not remove recursive search work or change any allowance.
No benefit is claimed for the all-Deferred baseline.

## Evidence and remaining work

- Reviewed the focused diff and all remaining `.prepared` field accesses; they
  belong to Execution/memo records. `git diff --check` passes.
- No builds or tests run, per owner instruction. Root must run the existing
  complete selected-execution, authorization, cache-equivalence, and sibling
  parent-memo checks. Existing multistep budget failure remains unresolved.
- This is a separate cleanup patch from scoped suffix reuse. No SelectedPlan API,
  owner pairing, child loop, canonical helper, or model-domain changes here.

## Cleanup

No owned jobs or generated build outputs. Reclaimed 0 bytes; shared fixtures and
caches retained. Root was not edited.

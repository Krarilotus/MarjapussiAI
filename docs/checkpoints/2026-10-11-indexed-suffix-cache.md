# Checkpoint: indexed canonical suffix cache

- Date: 2026-10-11
- Status: incomplete; source review only, validation delegated to the shared runner
- Tag: `checkpoint/2026-10-11-indexed-suffix-cache`
- Branch: `review/sequence-scenario-gates`
- Base: `1e3b29a8457901d810a4da499386c8e7fb3ec724`
- Integration source: root `872bc75fe6cde5f4e025c20b2158090089354c79`
  with current dirty sequence snapshot SHA-256
  `a06c5b391d5f7bff9ea68dacb30d6b152994bf3d31b99bb93ce46d096dee8f55`.

## Result

Replace the canonical-program cache's linear full-program search with a bounded
sorted fingerprint index in the same vector. Every candidate hit still requires
complete Program equality, including version, order and unused authored data.
Store no hashes in receipts or model identities. Reuse one validated lookup for
the complete find_stage scan; preserve the first matching stage and lazy suffix
compilation. Public program validation, cache scope and batch eviction remain.

No catalogue, horizon, worlds, response policy, comparison limit or represented
work changes. The existing size-based accounting includes the new u64 field;
there is no separate hash-table allocation. Suffix copies remain for now.

## Evidence

- Shared structural profile identified 447,330 suffix calls: 4,148.1308 ms
  exclusive, 4,599.0203 ms inclusive; suffix copies used 450.8895 ms. Whole
  sequence evaluation used 12,720.5831 ms inclusive. This is prior-policy
  profiling evidence, not a measurement of this patch.
- Added a forced-collision lookup test, including distinct unused nodes and
  names/versions, and an indexed stage-scan test across disabled, full, warm
  and evicted cache states. Existing order/invalid-input checks remain.
- Strengthened completed_effort_matches_cold_warm_uncached_and_evicting_comparisons
  with warm execution and represented transition/comparison equality. That
  fixture already compares values, receipts, joint prepared support, executions
  and solver work for cache-off and restricted-cache runs.
- Source/diff review only. No builds, tests, benchmark or games run here.

## Remaining work

Root must run the affected cache/effort tests and merged certificate tests, then
measure this patch on the same diagnostic workload. No speed or completion gain
is established. The previous shared combined suite passed 83/85: its real
depth-two preplay fixture exhausted represented transitions without completing
a candidate; the other failure concerned canonical domain assertion ordering.
Neither failure is claimed fixed by this optimization. Comparison-stop removal
and certificate semantics belong to separate owners/iterations.

The sequence.suffix scope now counts one find_stage scan rather than one nested
suffix request per stage. Total scoped duration is comparable; raw call counts
must not be interpreted as the same denominator across revisions.

## Cleanup

No owned running jobs, disposable build output or containers. Reclaimed 0 bytes;
retained shared datasets, caches, frozen source and focused integration patch.

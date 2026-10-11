# Local completion dominance

Base: `ad6e99e`. Status: **incomplete qualification**. The narrow implementation,
cache accounting and capture checks pass; broader planning coverage remains open.
Checkpoint tag: `checkpoint/2026-10-11-local-completion-dominance-integration`.

Knowledge supplies a checked public score envelope for forward play. The same
ValueStore that scores completed continuations propagates its bounds using the
original floating-point coefficients, weights, operation order and denominator.
Signed zero and ordered ties retain their existing meaning.

A Local choice can retain its genuinely completed first continuation when its
value dominates the legal terminal completions enclosed for every original
alternative. This has distinct `CompletionDominance` evidence. Complete values,
unexecuted competitor bounds and selected-parent authorization remain separate.
The original ordered domain, aliases, conditional support IDs and weight bits
are retained. Eligibility currently covers legal card-to-Tail alternatives.
An unsuccessful proof attempt is charged once; ordinary comparison reuses the
already completed first continuation.

Integrated focused sources: B `201ded1` / `ebf4df0`, D `ce321e5`, C `2d63307`.
Independent reviews covered arithmetic, authorization, owned memory, reuse and
capture references. Root fixed a shared serializer lifetime and an explicit
child-module path, and removed newly introduced lint warnings. Existing frontier
tests now account for the failed proof attempt and full comparison separately;
their exact semantic and represented-work assertions remain.

## Validation

One serialized job, two logical CPUs, below-normal priority, 4 GiB process-tree
commit cap, Cargo jobs one and test threads one. Optimized standard test profile.
Changed packages were explicitly rebuilt when switching source snapshots.
Receipts are in the private `tmp/2026-10-09-v1` evidence directory.

| Receipt | Result | Log SHA256 |
|---|---|---|
| `local-completion-dominance-core` | 12 Local/cache tests passed | `968397c635e8309a828b72af14ccbc4fd2bd9a130fe7e82afad03358bf4fc3a8` |
| `local-completion-dominance-policy-counters` | 139 library tests passed; two known expensive cases excluded | `ec4aef2dec0fa98077c2b02d0ae48260ecb2f1a3d457495a23bf538ebc62e1a2` |
| `local-completion-dominance-tools-lifetime` | 18 selfplay tests passed; subsequent graph compilation found the module-path error | `e093cb6928d1a319bdc8ca5df18540bcd3d9d97469c2ccd273a5a251fbe4b88c` |
| `local-completion-dominance-graph-module` | 13 graph tests passed after that fix | `65afeebbe49f55183bf0315c3d0eab7904b061ecffcbb926a71ed22c67862edf` |
| `local-completion-dominance-lint-cleanup` | All-target affected-package Clippy compiled; 13 graph and 12 Local tests passed again after lint cleanup | `ade61e306f8ce64b6af1b284330b03209e0419b3fd69d6c29c9c6cc127d27f86` |

The library suite covers singleton bit parity, exhaustive small score boxes,
signed zeros, subnormals, negative coefficients, zero weights, overflow and
earlier/later ties. Cache tests cover actual completed-parent hits, nonzero
reported leaf effort, rejected charges, eviction and measured byte boundaries.
Capture tests include file roundtripping and wrong-typed or unshared operands.

Earlier failed receipts are retained: `local-completion-dominance-policy`
(137 passed, two stale operation-count assertions failed),
`local-completion-dominance-tools` (lifetime error), and
`local-completion-dominance-clippy` (diagnostic warnings). Final Clippy retains
eight existing heuristic library warnings, three existing test warnings and the
existing graph-node argument-count warning; this is not a strict lint pass.
The unchanged knowledge envelope reuses its six-test validation at `3eb23b2`;
the integration changes its explanatory comment only.

## Remaining work

The excluded cases are
`auction_and_return_reuse_the_real_depth_two_boundary_with_joint_support` and
`selected_parent_authorizes_multiple_local_steps_after_the_final_global_boundary`.
Their planning limits and original joint support remain unchanged. The real
catalogue and six-card root gate still need qualification. The last live-style
diagnostic's 23 deferred comparisons remain unresolved evidence.

Next, check a structurally eligible authored cashing descendant and measure
exact repeated dependencies in the original six-card fixture. Descendant
completion must stay distinct from original parent completion. The 256-game
comparison, new HTML review delivery and deployment remain pending; these unit
tests establish neither playing strength nor a production speed improvement.

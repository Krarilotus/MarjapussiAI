# Checkpoint: scoped suffix reuse and cost attribution

- Date: 2026-10-11
- Status: incomplete; source implementation only, owner validation pending
- Tag: `checkpoint/2026-10-11-scoped-suffix-reuse`
- Branch: `perf/scoped-suffix-reuse`
- Base: `0858a491152a6899c091c33401f210dbd8db2eb0`

## Result

Reuse one exact Program lookup/validation for consumed-stage reads in drive and
bound alternatives in reconsider. A lazy iterator borrows the immutable Program
and Session for its lifetime, keeping the canonical entry index local to a pure
batch. Empty batches retain the previous no-read behavior. No temporary suffix
vector is added to drive. Child-loop and normalization ownership remains separate.

Stage scans compare an existing cached suffix by reference, keeping first-match
order and ordinary construction/admission for missing slots. Full/disabled entry
caches still validate all authored nodes before returning without calculating an
unusable allocation ledger. Exact identity, byte/entry policy, invalidation,
logical effort, model domains and all limits are unchanged.

Profile-only scopes split full lookup, validation, construction, program and slot
admission, and the whole stage scan. There is no per-node or per-equality timing.
Admission includes its ledger traversal and retained clone; lookup includes all
linear Program equality work. Scan exclusive time includes borrowed comparison
and loop overhead; construction/admission children remain separately visible.
Existing suffix-copy timing remains for owned cache-hit outputs.

## Evidence

- Source diff inspected; `git diff --check` passes.
- Two source tests added: batches versus single reads under off/byte/full-entry
  pressure and real response eviction; changed/unreachable-invalid input at full
  capacity, empty batches, and invalid stages. They were not run here.
- Existing first-stage/cache-pressure tests and complete cache off/cold/warm
  action/value/support/represented-work checks remain for the shared runner.
- Exact pre-change baseline is clean base above with sibling-tail-dag-v1.
  Owner-reported two-game profile: 20.95 s, 111.2 MiB, unchanged 105 actions/rules,
  23 Deferred; receipt SHA256
  `d45083ca9554190a521a1ca11e7e5edf45f3e91f27bb8c60d3f5ceb87dfe0d6a`.
  It is baseline evidence, not validation or measurement of this patch.
- No builds, tests, benchmarks or games run here, as instructed by owner.

## Remaining work

Root must compile and run the focused and existing semantic checks, then measure
against the exact sibling baseline. No speedup or useful-completion claim yet.
The known multistep budget failure remains unresolved; no allowance was changed.
Future C frontier sharing is absent from this patch and its baseline.

## Integration correction and independent source review

The owner's first integrated compile failed with E0700: the iterator's hidden
closure type captured the anonymous Session context lifetime. The originally
handed-off source therefore did not compile; the initial diff checks were not a
substitute for compilation. The original tag/patch remain unchanged evidence.

Root corrected exactly two declarations: the implementation names
`impl<'context, F: ...> Session<'context, F>`, and `suffixes` returns
`impl Iterator<Item = Result<Suffix, SequenceError>> + 's + use<'s, 'context, I, F>`.
Independent source comparison against the handed-off implementation confirmed no
other source differences in sequence.rs at review time (root file SHA256
`ee43d9737737d5625241ea6bba5a83e3b336929a1918dffbfa6bd0759c8f775a`).

Source review finds this sufficient for the reported missing lifetime capture:
the closure holds `&'s mut Session<'context, F>`, the `&'s Program`, and
`I::IntoIter`. The precise capture list names both lifetimes and both type
parameters; existing `I`/`I::IntoIter: 's` bounds remain. The return's `+ 's`
and input borrows still bind use to the short batch borrow. Capturing `'context`
does not extend the borrow to that lifetime or detach the iterator from Session.
The mutable Session borrow still prevents eviction during a live batch. No Box,
allocation, ownership relaxation, cache-policy or runtime-algorithm change is
introduced by these two declarations.

This is source-review acceptance of the correction, not a compiler/test pass.
Owner reported a second serial unit run active; its result remains pending here.
No local builds/tests or root edits were performed for this review. Reapplying the
original patch elsewhere requires the two-line root correction above.

## Cleanup

No owned jobs or generated build outputs. Reclaimed 0 bytes. Shared datasets and
caches retained; root and the earlier worktree were not edited.

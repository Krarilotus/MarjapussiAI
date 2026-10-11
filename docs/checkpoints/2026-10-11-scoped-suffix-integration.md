# Scoped suffix reuse: integration evidence

Status: **incomplete**. Base `0858a491152a6899c091c33401f210dbd8db2eb0`.
Tag: `checkpoint/2026-10-11-scoped-suffix-integration`.

Integrated D's `4b4e1a2` lookup/validation batches and `259d493` borrowed
comparison evidence. A batch holds the session borrow so eviction cannot stale
its local index. Completed comparisons expose the selected execution's existing
prepared records rather than storing another deep copy. Domains, ordering,
allowances, optional-cache policy and decision semantics are unchanged.

The first compile reported E0700: the iterator captured the anonymous Session
context lifetime. The root correction names that lifetime and explicitly
captures it in the return type. D independently reviewed the two-line change;
it adds no allocation or runtime branch.

## Validation

Jobs used the serialized two-CPU, below-normal, 4 GiB runner and optimized test
profile. Receipts/logs remain in private `tmp/2026-10-09-v1`.

- `scoped-suffix-policy2`: **107 passed**, 2 expensive fixtures excluded; 68.27 s
  including compilation, 1.23 s tests, peak 619.2 MiB. SHA256
  `29aea4d6383e092ecd7df933ef8a53121d13f5f88f2c27abc77883925cf6cb52`.
  Covers empty/invalid batches, disabled/full/byte-limited caches and response
  eviction, plus existing semantic and parent-memo invariants. The unchanged
  depth-two success and six-card budget failure remain recorded in the preceding
  integration checkpoint; neither was rerun here.
- `scoped-suffix-profile-build`: passed, 38.41 s, peak 360.4 MiB. SHA256
  `7e82165d093c90c993a7640dedc1f9b02cb652283c267dac681fc2b6063756be`.
- `scoped-suffix-profile-two`: 20.94 s, peak 102.1 MiB. SHA256
  `c654d67437adecf26f6c4b630173450bc7697ac5ad904351440811fca05aa468`.
  Same 105 actions/rules as exact `0858a49` baseline. All **23 attempts still
  Deferred**. Actual transitions 167,904 versus 157,099; represented transitions
  240,391 versus 224,922; 14 completed individual candidates in each run.
  These deadline-limited prefixes do not establish a general speedup or useful
  completed-plan coverage. No playing-strength claim follows.

Coarse attribution puts suffix construction at 4.138 s exclusive, copying at
0.263 s, exact lookup at 0.321 s, validation at 0.030 s. Whole suffix time is
5.089 s versus 5.641 s before, with different progress and additional profiling
scopes. Construction, rather than validation alone, remains the measured issue.
Source snapshots and summaries are in private `2026-10-11-scoped-suffix-profile`.

Next: exact sibling frontier sharing, then unchanged completion and production
gates. The 256-game smoke, updated HTML delivery and deployment remain pending.
Shared evidence and build caches are needed for those checks. No owned Docker
resources or disposable build outputs were removed; reclaimed zero bytes.

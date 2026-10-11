# Shared sibling continuations and baseline compatibility

Status: **incomplete**. Base `4bff913bead8f33bcdce27751cfa0cb6e2296e71`.
Tag: `checkpoint/2026-10-11-sibling-sharing-integration`.

Integrated C's focused `0153814` sibling-tail implementation and `58202b5`
parent-memo test, B's matching work capture and outer retry diagnostics, and
A's reviewed shared-core compatibility anchor. Original alternatives, ordering,
values, joint support and per-edge evidence remain represented. The explicit
`sibling-tail-dag-v1` work model charges a shared no-action sibling dependency
once, independently of optional caches. Runtime retry reports zero new work.
No fixture, horizon, action catalogue or resource cap was reduced.

## Evidence

All Rust jobs used the serialized two-CPU, below-normal, 4 GiB runner and
optimized test profile. Private logs/JSON receipts: `tmp/2026-10-09-v1`.

- `sibling-tail-policy`: **104 passed, 1 failed**, 266.10 s including compilation,
  202.25 s tests, peak 597.7 MiB. SHA256
  `0d3e778bba3b0cc723afb7e9c73d40b58a92581039adf6d2c3b6af92c260a263`.
  The unchanged genuine depth-two auction/return fixture now completes and
  passes actual retained-plan reuse and strict route assertions. All five new
  sharing tests pass. The unchanged six-card multistep fixture still exhausts
  500,000 represented transitions before completion: 463,451 actual transitions,
  18,567 sibling aliases avoiding 211,372 expanded transitions. More structural
  work remains; success on the depth-two gate is not live latency qualification.
- `sibling-parent-memo-retry`: **105 passed**, two expensive fixtures excluded
  because their results are recorded immediately above. 55.52 s including
  compilation, 1.10 s tests, peak 501.4 MiB. SHA256
  `574276c17666e0c366014d2092e32b4603677ddd87c23c0ac830e917b6c34fce`.
  Includes an observed completed parent memo hit containing aliases at identical
  logical allowance and the actual outer pending-decision retry/clone path.
- `sibling-tail-capture-tools`: **30 tools + 9 graph tests passed**, 66.76 s,
  peak 399.3 MiB. SHA256
  `867a07cc812bc90ea3268e528ceb0720ce85916d64510ef0d0f547bcead9ef8c`.
- `sibling-tail-contract-gates`: **both tests still fail** because production
  planning defers. Strong-hand overbidding and completed-forecast hidden-swap
  parity therefore remain unqualified. Passing a fallback comparison would not
  establish the intended strategic or privacy evidence.
- Independent source review found no blocker in narrow same-group sharing;
  its missing parent-memo coverage is now included and passes.

Shared-core constructor parity at exact `4bff913`: 134 accepted comparisons,
38 expected hidden-history rejections, 122 matched subsequent actions across
43 prefixes and four completed trajectories. The private frozen-constructor
reference and source audit are recorded in the companion checkpoint. The exact
compatibility anchor was added; shared-core rejection and frozen old-policy
source guards remain unchanged. This establishes no policy-strength result.

Before sharing, the corrected linear suffix lookup diagnostic still had all
23 attempts Deferred over the same 105 decisions. Its 6.393 s suffix time was
lower than the rejected hash-index experiment's 7.956 s, but overall work
progress did not improve. No overall speedup is claimed. The source receipts
and comparison JSON remain in private `2026-10-11-linear-selected-profile`.

Next: pure-guard convergence on identical non-Tail dependencies, with explicit
certificate/edge reconstruction; measure lookup/validation cost before changing
representations. Distinct action futures remain separate. The 256-game smoke,
new runtime HTML delivery and production deployment remain pending.

Shared evidence/caches are retained for these checks. No owned Docker resources
or disposable build outputs were removed; reclaimed zero bytes.

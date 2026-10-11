# Canonical suffix index: measured regression

Status: **incomplete; optimization rejected pending correction**. Base `c49a1c6`;
intended tag `checkpoint/2026-10-11-suffix-index-measurement`.

Integrated D's exact-equality fingerprint index and one-lookup stage scan
(`96970ae`, focused patch). Hashing uses derived model traits; collision hits
still require full Program equality. The only engine hunk adds the Hash trait
to ActionType. A's independent two-pass history oracle is included separately.

`suffix-index-policy-unit`: 97 policy tests passed; the known expensive preplay
fixture remains excluded and outstanding. Cache off/cold/warm values, actions,
support, receipts and represented work agree; forced collisions and eviction
checks pass. `hidden-history-two-pass-oracle`: both independent multiplicity and
fixed-seed sampler checks pass, covering 90,090 paths and 24,310 original splits.

The optimization does not pass its performance gate. Compared with the same
two-deal baseline at `c49a1c6`, `indexed-profile-two` spends 7,956 ms in suffix
work versus 5,858 ms. Under the same wall allowances, actual transitions fall
from 192,959 to 166,751 and represented transitions from 708,042 to 605,239.
Every one of 23 planning attempts still defers. Total times are 20.91/20.92 s,
dominated by the configured deadlines; unchanged totals do not indicate parity.
The stage-scan scope now counts a scan once, so raw scope-call counts differ.

These are instrumented shared-machine diagnostics, not precise release speed
estimates. They are sufficient to reject a claimed improvement. Next isolate
the one-lookup stage scan without repeatedly hashing whole programs; remove
unused index/Hash scaffolding if that smaller change is retained.

Private source receipts and outputs: `tmp/2026-10-11-history-profile` and
`tmp/2026-10-11-indexed-profile`; bounded runner receipts in `tmp/2026-10-09-v1`.
No smoke, deployment or resource cleanup; all retained evidence remains useful.

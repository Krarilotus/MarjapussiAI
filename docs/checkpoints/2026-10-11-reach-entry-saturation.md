# Checkpoint: exact Reach entry saturation

- Date: 2026-10-11
- Status: incomplete overall; equivalence checks passed, planning completion unresolved
- Integration tag: `checkpoint/2026-10-11-reach-saturation-integration`
- Source: `4fb7348`, tag `checkpoint/2026-10-11-reach-entry-saturation`
- Integration base: `7ada1d4` on `design/executable-concept-contract`

## Change and equivalence boundary

Reach's partner-access maximum stops requesting Entry estimates once its current
value is exactly `1.0`, including before the first request. Entry already clamps
all output fields to `[0, 1]`; subsequent estimates cannot increase that maximum.
The small iterator helper checks saturation before demanding its next element.
Near-one estimates do not stop it. Legal-card order, earlier Reach queries and
the existing hard known-entry handling are unchanged.

Only the partner-access scalar is short-circuited. Full Entry construction and
later demand remain in Routes. There is no goal projection, model/strategy change,
policy toggle, world/horizon reduction, new cap, Work API or semantic trace change.
Routes has its own deterministic fresh seeded sampling and does not spend shared
Reads allowance or canonical endgame effort. It may warm the deal-model cache.
Existing pair.reach/routes/routes.entry profile scopes remain; no new timing is
introduced inside the loop. Downstream consumers may still demand skipped entries.

## Source checks and tests for the owner

- Focused diff and the Entry clamp/read ownership inspected; diff check passes.
- New test-only eager oracle copies the original Reach constructor at the base.
- Synthetic initial/late/never saturation checks compare the old eager maximum
  bitwise, including the representable float immediately below one, and assert
  the exact entry-call prefix/order.
- A legal public-history fixture compares every Reach field, shared Reads
  remaining allowance/query order/result bits, later complete Entry field bits,
  and later uncertain opponent-card queries against the eager oracle. Test-only
  allowance capping decreases remaining work to one: the first unresolved query
  must spend it, its cached repeat must preserve state, and a second unresolved
  query must preserve the recorded order. Those queries run while lazy Routes
  remains unconstructed, before later full Entry requests equalize cache warmth.
- Initially saturated access leaves Routes unconstructed; later reverse-order
  full Entry requests compare bitwise with the eager route construction.
- A two-trick completed-leaf fixture primes eager/lazy Reach and compares normal
  play plus strict completion: action/rule/why and canonical solver effort, also
  against the public completed-response API. It requires nonzero solver effort
  and supplies sufficient wall allowance. Both Reach/leaf fixtures assert spent
  rights requiring partner access, no hard entry, known retained partner Ace,
  at least two legal cards and lazy Routes absent versus eager Routes present
  BEFORE subsequent play. It does not establish timed equivalence near a deadline.
- cfg(test)-only Reads helpers snapshot ordered query/result bits and remaining
  allowance, and cap (never refill) allowance; no production Reads API is added.
- New test file formatted with rustfmt. Author and reviewer ran no builds; the
  root's serialized validation results are recorded below.
- Independent E source review found no production semantic blocker but correctly
  rejected the first fixture's unspent-rights path and hard-only downstream query.
  The fixtures were replaced with the explicit branch and depletion assertions
  above. The initial inadequate tests were not run or claimed as validation.
- E re-audited the legal fixture history and new branch/budget assertions and
  found no source blocker. Its suggested ordering improvement was also applied:
  uncertain queries precede later full Entry requests. Source review is not a
  compilation/test result. Initial saturation is exercised through real Reach,
  while late/never saturation remains explicitly synthetic helper coverage.

## Root validation and measurement

All jobs used the serialized two-CPU, below-normal, 4 GiB runner and optimized test
profile. Private receipts/logs: `tmp/2026-10-09-v1`.

- `reach-saturation-focused`: **4 passed**, 67.64 s including compilation,
  0.01 s tests, peak 528.5 MiB. SHA256
  `88c14688112bced33a3e07fb45db47830878e376d9416cf4c00f5026e0606d98`.
- `reach-saturation-policy`: **118 passed**, 1.26 s, peak 51.2 MiB. SHA256
  `6b86d15be57bda45a60321f00b8da1cce9493c362c14e72799a15d20886f5983`.
  Two unchanged expensive tests excluded: genuine depth-two auction/return
  (previously passed), and six-card final-global (previously exceeded its unchanged
  500k represented-transition allowance). The latter failure remains unresolved.
- `reach-saturation-profile-build`: passed, 15.77 s, peak 307.5 MiB. SHA256
  `21c67a57f8a0a474eff5420047d19b473e7eb64622016ac95a4c7bc40984acaf`.
- `reach-saturation-profile-two`: 20.95 s, peak 103.7 MiB. SHA256
  `765b3ad0f9a26a05cac108faf8adfc3291fe358b107d88d3235034919cb9fb13`.
  Same 105 actions and rule IDs as the immediate lazy-frontier baseline;
  **all 23 planning attempts Deferred**. Actual/represented transitions
  161,261/232,929; 12 individual candidates completed, zero frontier aliases.
  Entry requests 66,669 / 2.587 s versus 72,945 / 2.725 s previously; suffix
  construction 4.126 s versus 4.169 s. The visited prefixes differ under deadlines
  on this shared machine, so these totals establish neither a controlled speedup
  nor a regression. The focused oracle checks establish the skipped work and
  equal completed results; no useful whole-plan improvement is established.

Source and actual captures remain in private
`tmp/2026-10-11-reach-saturation-profile`. This small lazy-computation correction
does not resolve whole-plan completion or establish playing strength. No
256-game smoke or deployment has run.

## Cleanup

Validation jobs ended; no owned containers. Shared datasets, source captures and
the build cache remain for planned reuse; reclaimed 0 bytes.

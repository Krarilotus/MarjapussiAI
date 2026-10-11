# Checkpoint: legal activation fixture phase

- Date: 2026-10-11
- Status: source-only fixture correction; root owns builds/tests
- Base: `b5620f169bdc8f413488227f1f1b4e8c16960486`
- Tag: `checkpoint/2026-10-11-activation-fixture-phase-source`

Root's reconsider-runtime-unit2 compiled and reported 56 passed, 3 failed.
The owned no-side-card/uncertain-half scenario expected an announcement directly
after raising. Engine bid application enters Trick, whose legal domain contains
cards only; completing the trick enters StartTrick, which offers eligible
announcements and questions. The fixture expectation was premature.

Both fixture branches now play one legal ace-winning opening through Table and
the existing legal response helper. They assert the completed trick, root winner
and actual StartTrick actor before activation. The no-side branch explicitly
retains only Green cards and its pair; the uncertain split branch still checks
that partner's complementary half is possible but not certain after the trick.
No production binding, belief, constraint, assertion domain or enum was changed.

Root's other catalogue routine-chain and raised/no-raise-horizon tests passed
in that supplied run. The two unrelated reported failures remain root/A-owned.
Rustfmt and whitespace checks passed here; no tests/builds were run. Corrected
fixture execution remains unverified. No outputs/processes created; reclaimed
0 bytes. Supplied evidence: tmp/2026-10-09-v1/reconsider-runtime-unit2.log.

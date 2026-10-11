# Checkpoint: raise fixture independent of legal-action ordering

- Date: 2026-10-11
- Status: source-only correction; root owns builds and test execution
- Base: `d7dcd1493a752a1bee0b619fb550e6dc20bb549d`
- Tag: `checkpoint/2026-10-11-raise-fixture-order-source`

Root's joint test run compiled and reported 52 passed, 3 failed. One owned
failure selected the last generated raise and then asserted MAX_BID, although
the selected value was 125. Legal action ordering is not a numeric-order
contract. The fixture now explicitly selects the maximum NewBid value and
uses a distinct legal offer for the rejected-history branch. The full legal
raise-domain assertion is unchanged; no constraint or expected receipt
behavior is relaxed. Other two failures belong to root's separate diagnosis.

Rustfmt and whitespace checks passed. No builds/tests rerun here; the corrected
fixture still requires root validation. No disposable outputs; reclaimed 0 bytes.

# Control integration validation — 2026-10-14

Integrated af55bed-based control work as 9903444 after cache and auction fixes.
Full heuristic suite: 137 passed under the two-CPU, 4-GiB serial wrapper.
Beliefs suite: 52 passed. Public native adapter compiled against 1e36d7a.

The first full control run exposed two test conflicts. Preserve the established
HC4 explanation when an existing retained-lead certificate already selects a
cash. The new specific-cash fallback also honors lead_cash_standing=false.
The spent-partner-rights fixture now disables cashing to isolate question versus
lead transfer: a safe own cash can defer the question, and subsequent guaranteed
cashes may validly retain the same opportunity. It must not be forced to ask on
the immediately next turn. No hidden information or rights are changed.

Follow-up continuation memoization, real-prefix replays, paired comparisons,
final API tests and deployment remain pending; no strength/release claim here.
Evidence: tmp/2026-10-14-v1-completion/{control-tests-final,auction-integration-tests,native-check}.log.

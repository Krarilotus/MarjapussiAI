# Continuation calculation integration — 2026-10-14

Integrated the typed decision-local continuation store into HO-1/HO-2. Legal
Start/Card and Cards/Card queries share completed results only after checking
original-phase legality. Exact world, action, phase and actor plan identify the
query; the enclosing immutable decision owns public facts, scoring, knowledge,
parameters and world weights. No cross-decision stale results are retained.

Validation: full heuristic suite 141 passed before the additional pending-answer
fixture; all four cache integration tests then passed, including that new test.
Cold, warm and uncached terminal cells, weighted values and selected plans agree
bit for bit. Pending Answer rejects Card even with a warm Cards result. The
fixture obeys the forced opening rule (Green lead with no Ace). Deadline admission
and minimum-world behavior remain unchanged. Existing beliefs suite: 52 passed.

No whole-policy speed or strength claim yet. Release replays, paired smoke games,
final server API validation and public deployment remain pending.
Evidence: tmp/2026-10-14-v1-completion/cache-answer-tests-final.log and the full
heuristic integration log in the same private directory.

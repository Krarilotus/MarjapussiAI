# Checkpoint: first-pass budget classification

- Date: 2026-10-14
- Status: **incomplete - root validation and integration pending**
- Base: `b42c301812426a42825fa72c213039b6d5c053c3`
- Tag: `checkpoint/2026-10-14-first-pass-budget`
- Branch: `work/2026-10-14-v1-performance`

Owner clarified play below 100 ms, return planning/raising below 5 s, and bidding
AND first pass below 1 s. The existing budget function treated every legal Pass
as planning. It now accepts the synced seat Knowledge: a legal Pass with no
`first_passer` uses `bidding_budget_ms`; a return pass uses `planning_budget_ms`.
Existing four-fifths soft-deadline reserve is unchanged: defaults are 800 ms for
forth/bidding, 4000 ms for return/raising, and 80 ms for play/questions/answers.
No new parameter, guessed hand-size threshold, world reduction or policy-score
change is introduced. Nondefault bidding budgets also govern first pass.

Source verification: observation emits a Pass event to every seat, concealing
only the cards; Knowledge::pass records the first passer using get_or_insert.
Engine pass changes PassingForth to PassingBack and the recipient receives four
cards (9 to 13). The test drives that observed transition and asserts all five
budget categories, including question/answer play. This changes time admission,
not an algorithm's values; a previously deadline-limited first pass can therefore
choose differently. It is not proof of playing-strength preservation or a hard
wall-clock bound: unbounded/predeadline work remains.

Owned code: heuristic lib budget/call site, parameter documentation, and
`tests/internal/budget.rs`. No other module or worktree is changed by this commit.
The earlier exchange-cache commit already exists upstream under a different hash;
root should cherry-pick ONLY this new commit, not the whole branch.

Validation: source/staged diff and whitespace review only. No build, test or
benchmark run, per root's serial ownership. Suggested bounded root check:

```text
cargo test -p marjapussi-heuristic --lib budget_tests::phase_budgets_follow_observed_exchange -- --test-threads=1
```

Apply the shared one-job/two-CPU/below-normal/4-GiB limits. No deployment or push.
No build artifacts created or removed; reclaimed **0 bytes**, caches retained.

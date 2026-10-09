# Coherent forecast evidence - 2026-10-09

Base: `3afbeed`; tag: `checkpoint/2026-10-09-coherent-forecasts`.
Status: refactor and diagnostics checked; the wider concept planner is incomplete.

Moved the existing seat-local rollout and score aggregation into
`heuristic::planning`, removing the return module's implementation. Added actual
winning-card and rank-development attribution so a return can explain the wins
retained versus transferred. Development requires the initially external higher
ranks to have fallen in earlier tricks. A void or a long suit receives no new
bonus. Rules, sampled worlds, policy, scoring, candidate order and budget stay
the same. No new dependencies or fitted parameters.

Validation:

- Existing 8 library, 5 history and 9 passing tests pass. Clippy passes with
  warnings denied; formatting and diff checks pass.
- All 22 selected-player development review games replay successfully. Of these,
  18 completed their forecasts in both the earlier and current runs: all chose
  identical returns, with exactly equal reported scores. Nineteen current
  forecasts completed; deadline fallbacks vary between runs on the shared host.
- For completed outcomes, developed wins are bounded by wins, and wins credited
  to kept and returned cards do not exceed the team's total tricks. Eighteen
  cases credit a kept card with a developed win. These are forecasts, not proof
  that the recorded human or AI return was optimal.
- Same private dataset fingerprint and selected cohort as the linked audit;
  receipts `coherent-forecast-check`, `coherent-forecast-replays` and
  `coherent-forecast-clippy`. One job, two logical CPUs, below-normal priority,
  4 GiB process-tree cap; replay/build peak commit 312 MiB.
- Independent T3 review remains unavailable: the assigned thread still has no
  model session. No agents window was restarted.

No strength comparison is warranted for this explanation refactor. This does
not fix the existing blanket pair-suit protection, promotion approximation,
pass-choice inference, bidding continuation, raising or series strategy. Public
multiplayer remains undeployed. See the [concept dependencies](../design/live-planning-audit.md).

# Joint continuation planning

Base: `1ab6f72`; qualified implementation for owner playtesting.
Tag: `checkpoint/2026-10-10-joint-continuations`.

## Behaviour and ownership

`beliefs::Mind::deal_weights` retains the joint auction likelihood introduced in
`6239b6e`. `agent::Table::from_view` reconstructs lawful hypotheses using only the
deciding seat's history. `heuristic::planning::Forecast` samples one common cohort
and finishes every candidate through seat-local policies and engine transitions.
Return selection, open-lead cards/questions/announcements, pass-versus-minimum-bid
decisions and pre-play raising reuse this evaluator. Earlier auction signals keep
their existing owner. Consumed halves, suit exhaustion, entries, trump changes,
pair values and last-trick points affect actual terminal outcomes, not independent
bonuses. Return allocations recompute both hands before continuation.

The default is eight planning worlds; `plan_worlds = 0` disables this comparison.
The two-second deadline retains the previous policy on an incomplete batch.
Recursive forecasts are disabled inside rollouts; each actual decision replans.
Estimated raises are proposed by repricing engine outcomes, then replayed under
the actual proposed contract before selection. The separate guaranteed path reuses
`knowledge::remaining_claim`, including legal own pairs and suit exhaustion, or
the existing partial own-card certificate. A contract already at its claim floor
also follows the proof. Guaranteed and forecasted values have distinct facts.

## Validation

The agent/belief/heuristic suites passed except three old raising fixtures that
assumed all raises must use the narrow certificate. They now explicitly disable
forecasting when testing that certificate. The final six certificate tests pass,
including the complete 240-point route under three hidden allocations and the
already-240 contract. Two separate contract-planning tests cover a forecasted
overbid and identical decisions under hidden-card swaps.

The bounded tools run passed 500 hidden-card-swap checks, return-outcome/opening
checks on three seeds, and 50 complete games comparing fresh versus reused agent
state (101.67 seconds, 281.0 MiB peak committed memory). The first privacy run was
stopped because test-history generation unnecessarily enabled forecasts; only
history generation disables them in the final run. Compared decisions enable
planning. All-target clippy for agent, beliefs and heuristic passed with warnings
denied. One job, two logical CPUs, below-normal priority and a 4 GiB process-tree
cap were used throughout. Independent T3 review could not start: the verified
agents backend was unresponsive. No T3 window was restarted.

## Fixed comparison

Sixteen fixed deals, four team/seat rotations each: 64 completed games against
the deployed `1632af8` heuristic and belief code. No game failures; six forecasts
timed out and retained their baseline action. Mean duplicate-deal score difference
was +39.53, with approximate 95% t interval [-89.96, 169.02]: inconclusive on
playing strength. The new agent completed 357 forecasts and changed 110 decisions
through the shared policy comparison.

New-policy decision p50/p95/max: 0.20/723.98/2028.40 ms; baseline:
0.13/8.73/1564.47 ms. Full comparison: 216.52 seconds including compilation,
249.2 MiB peak committed memory. Planning has a material latency cost. The budget
is cooperative, not a hard real-time guarantee. The final already-at-floor claim
follow-through correction was scenario-tested after this comparison; no strength
claim is made for that correction.

Private evidence remains outside Git in the qualification harness and bounded
runner logs. The benchmark source diff against `1ab6f72` has SHA-256
`8a40c4254fc31ab6c65bbad76236836bd18cfbfd3817e6816e7e99b27cede931`.
Approved human replay input has SHA-256
`e41a69d6f81819dc3619655c9a35ca39c906a8ebe647e3aac869bd65b8823865`.
Earlier joint-belief sampling evidence remains in its immutable checkpoint.

## Limits and next work

The reported live decisions now compare partner-first questions and delayed own
announcements against the existing line. Their root choice is not evidence that
an entire counterfactual game will succeed. Eight worlds and baseline continuation
policies remain approximations. Defenders with hidden exchange histories retain
the ordinary policy/endgame search until a latent-history model exists.
Pass-choice intent, affordability-dependent bidding signals, calibration on held-out
games and series-aware 500-point objectives remain open. The approved human
examples still contain high winning bids the model declines; disagreement alone
does not establish which choice is better. No pair percentages were hard-tuned.

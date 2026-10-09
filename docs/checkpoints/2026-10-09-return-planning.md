# Checkpoint: bounded return planning and lean validation

- Date: 2026-10-09 (Europe/Berlin).
- Status: verified for the bounded policy and engineering checks below; paired live smoke remains a testbed release step.
- Tag: `checkpoint/2026-10-09-return-planning`.
- Branch: `integration/reviewed-ai`.
- Base: `3eda680`, `checkpoint/2026-10-09-simulation-table`.
- Paired testbed: `feature/playable-v1`; its checkpoint records the exact AI revision.
- No engine-upstream change, push or merge in this iteration.

## Result

[Return forecasts](../design/return-rollouts.md) compose existing concepts by
playing candidate allocations through the shared `agent::Table`. Every simulated
player sees its own observations; the engine owns legality, trump changes,
question/answer rights and scoring. Surviving halves need a real entry and legal
activation. Each card and trick is consumed once, including the last-trick bonus.

C42 still selects the first pass and ranks returns. The shortlist includes its
leading four, held-half/pair allocations and an opening toward a returned Ace.
Eight shared weighted deals compare completed continuations; interrupted work
keeps C42. Return simulations use the existing two-card/eight-world continuation;
live search remains four cards/64 worlds with its 45 ms cooperative search budget.
Return forecasts have a separate two-second cooperative budget. Raising is off.
Forecast shares are conditional estimates, not calibrated success probabilities.

The DRY audit removed the second scoring pass over all 715 returns, reused the
C42 explanation builder and retained one filtered Table implementation. Whole-game
fresh/reused-agent validation now belongs in the AI, replacing the server's
obsolete AI-only test. No generic planner graph or new rule owner was introduced.

Routine checks now use Cargo's optimized incremental test profile. Nine empty CLI
harnesses across AI/testbed are disabled; normal integration-test binary builds
remain. Release optimization is unchanged. Validation runs sequentially, one Cargo
job/test thread, on two logical CPUs at below-normal priority with a 4 GiB aggregate
Windows job memory cap. The limiter is private operational glue, not product code.

## Evidence

Rust/Cargo 1.91.1, Windows, shared Ryzen 7800X3D. Private logs and reusable fixtures:
outer `tmp/2026-10-09-v1/`. No new model fitting. [Weight provenance](../EVALUATION.md).

| Check | Result / limit |
|---|---|
| Heuristic unit tests | 8 pass, including expired-return fallback without partial outcome publication |
| Frozen exchange guard | C42 choices/explanations unchanged with return forecasts disabled |
| Hidden-card swaps | 500 comparisons pass with eight-world forecasts and two-card/eight-world search |
| Return scenarios | Three complete eight-world forecasts; legal nine-card hands, unique facts and actual predicted opening |
| Whole-game fresh/reused guard | 50 deals, same seat metadata, complete decisions/actions/scores; nonzero forecast and solver coverage. Uses one world each to keep this correctness check independent of heavy work |
| Independent GPT-6.1-Sol review | Accepted the bounded AI slice with the documented statistical, timing and imperfect-information limits; no blocking code finding |
| AI all-target clippy | Pass, `-D warnings` for heuristic/tools and their dependencies |
| Existing exact solver/entry oracles | Reused unchanged checkpoint evidence: 2,300 small positions, 150 full-engine endings and 5,376 entry-goal comparisons; no repeat exhaustive build |
| Human development sample | Ten declarer and three partner records replayed. All 13 forecasts complete; eight returns change, zero match the recorded return versus two C42 matches. This is exploratory agreement, not strength evidence or a human thought trace |
| Frozen qualification | Fresh seed 202610091, 128 deals, four games each; all 512 finish. Versus C42-only returns: +2.2 points per deal pair, approximate 95% interval +/-23.8. No clear difference; no tuning or early stopping |
| Default-policy timing | 100 games / 5,264 decisions. All 100 return forecasts complete eight worlds; return mean 427 ms, max 817 ms. Ordinary play max 47.8 ms. Of 1,320 reported endgame decisions, three stop before completing all worlds |
| Shared-machine latency diagnostic | Qualification recorded a 191.8 ms endgame outlier. A separate timing-only replay of the same fixtures saw 4/6,724 solver calls above 50 ms (max 164.4 ms), with coarse Windows thread CPU time 0 or 15.6 ms each; all flagged incomplete. Supports scheduling delays for those reproduced outliers, not exact attribution of the original one |
| Default fresh/reused cost | 20 identical visible trajectories / 1,069 decisions: 6.175 vs 5.951 ms mean; zero action differences. Small shared-machine sample, not a general speed guarantee |
| Build footprint | Initial targeted heuristic test build 56.3 s / 309 MiB peak committed; final incremental AI bundle builds in 2.16 s / 232 MiB. Targeted release drivers build in 37.4 s / 249 MiB. Workloads differ, so no invented before/after speedup ratio |

Frozen 133 source/config files: SHA-256
`0911cc2b6c432d108d66dea602f78def90ae09ad7539abdbd6bc3d408b6d1710`.
Qualification log: `77ec10097bfb8e7742bf00921c44d3b10f1ab38b50e94905dc151077afed9ead`.
Default timing log: `95c356991a1b4eb2ed4039c45e57397ca9959887f442b1aad8e7a69774f78086`.
Human source: `e41a69d6f81819dc3619655c9a35ca39c906a8ebe647e3aac869bd65b8823865`;
final derived study: `6ae975c1c61355e581cf8d92ffa75033ba43188fc47c533bae44992d27efb1f2`.

Earlier broad workspace verification was interrupted, not passed. Two eight-world
whole-game determinism attempts failed completion guards under reduced resources;
the guard was retained while workload was separated from default timing checks.
An old duplicate server test had the same issue and is now consolidated in AI.
One combined targeted process ended without a completion report after privacy
passed; unfinished return/self-play targets were subsequently run successfully.
The preliminary smaller-shortlist dev/check runs remain development evidence;
redundant reruns were omitted after the resource trim. The final fresh fixed-size
qualification above is the release comparison.

## Limits and next steps

The shortlist and continuation policy are approximate. PIMC endgame values are
exact within completed exposed deals, not an optimal imperfect-information policy.
No stronger-play or calibrated-probability claim is established. A 50 ms response
is a practical target, not a hard real-time guarantee: preparation/tie-breaking
and operating-system scheduling are outside the search's cooperative checks.
Completed cache bounds remain reusable; actual observations still update beliefs.

Next: paired live Debug review, then concrete mistakes from that review. Keep
raising, broader information-set planning/GPU work and any new fitting separate.
Retain the public solver's pre-existing zero-bit TT edge as a small follow-up;
normal callers use nonzero sizes. Do not add a framework to address it.

## Cleanup

No containers created. Build/test jobs finish sequentially; cached targets and
private evidence are retained for planned reuse. Disposable reviewer worktrees
are removed after final review; exact reclaimed space is recorded in the handoff.

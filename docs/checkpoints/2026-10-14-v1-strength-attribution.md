# V1 strength attribution and public-fact repair qualification

Date: 2026-10-14. Status: **incomplete production qualification; correctness
checks passed, strength improvement and hard play latency not established**.
Base before the repair: `7ac9387` (public policy `13d2015` plus the first-pass
budget classification and recorded replay diagnostics).
Intended immutable tag: `checkpoint/2026-10-14-v1-strength-attribution`.

## Version and release decision

The deployed policy is `13d2015192ef04a27b21920ae314bd0853d35278`.
The Opus handoff is `af55bedcebe982644b937573ee739dada254b9fd`;
it is not the older public policy `b3abdaf1f5e47111904db442ce36db7dc4ae9e0b`.
No deployment or rollback is part of this checkpoint. The public app and Mongo
were healthy when inspected. The old public image and its 73 MiB compressed
archive remain available; no database or frontend was changed.

## Does the handoff-to-candidate comparison show a strength drop?

No established drop. A fixed 2,000-game comparison on 1,000 identical deals with
teams swapped gives **-0.305 engine-score points/game**, approximate
deal-clustered 95% interval **[-2.484, +1.874]**. Wins are 998 versus 999, with
3 draws. Each policy takes 1,000 contracts; 604 versus 609 make. Schwarz is 131
on each side. This does not establish superiority either.

Seed: 2026101403; policy seed: 0. One bounded worker, two logical CPUs on one
physical core, below-normal priority, 4 GiB process-tree commitment limit.
Private evidence: `tmp/2026-10-14-v1-completion/bench-v1base2000` under the outer
workspace. Its worker NDJSON SHA256 is
`e00772403d2b834ced3e1f20245b495521de91a3479827c8b1ef853941e94bec`.

The earlier 256-game result was -5.92 [-13.50, +1.67]. Its four largest paired
losses contributed -6.875 points/game, partly offset elsewhere. They are
development diagnostics, not independent strength evidence.

## Controlled explanation of those four reversals

All ablations reused the original 128 deals, seed 2026101401, policy seed 0,
against the frozen handoff. Frozen source proofs and manifests are retained.

| Variant | Points/game [approximate 95% interval] | Four selected reversals |
|---|---:|---|
| Cache + auction changes, before response-model change (`1e36d7a`) | +1.27 [-4.58, +7.11] | None |
| Candidate with rollout cash guard removed (`d8f035f`) | -6.17 [-13.76, +1.41] | All remain |
| Candidate with team-opportunity clock, actor-local instructions (`f05e1cd`) | -7.15 [-15.09, +0.79] | All remain |
| Candidate with old team response instructions (`342cd81`) | -0.02 [-0.44, +0.40] | All four restore handoff choice |

The selected losses come from how the forecast predicts the partner's response,
not the exchange cache, bidding reader, cash guard, or merely its opportunity
clock. Restoring the old forecast also removes a selected candidate gain.
The old response model treats the root's privately selected plan as instructions
the partner will follow. That is an unsupported coordination assumption, not
real backend information sharing. It remains diagnostic-only and is not merged.
The aggregate 2,000-game result prevents generalising these four losses into an
overall strength regression.

## Concept-model repair and remaining strategic gap

The honest response model needs to preserve public activation facts. The repair
keeps whole-pair exclusions distinct from individual possible cards, applies
direct and automatic answers through the same observation path, and lets public
positive ownership override fallible pass/void readings. The engine owns legal
transitions; existing public Knowledge APIs seed the facts. No strategy weights,
game-specific shortcuts or private partner instructions are added.

An unheld half may legally be asked about. A question alone proves no ownership;
a positive answer proves the respondent has a half, and a newly called pair also
proves the asker has one. The identity of an unobserved half stays unresolved.
Hypothetical transfers invalidate the transferring players' old pair constraints;
loading historical transfers must preserve later answers.

This does not yet model all communicated support duties or retain a multi-turn
plan through later real decisions. Those are open strategic dependencies. It is
not claimed to repair every selected loss or establish stronger play.

The next strategic improvement belongs in the shared actor concepts: observable
signals and actor-local beliefs -> supported next action -> actual next leader
-> surviving activation rights/resources -> reachable pair and trick points ->
engine score. The forecast must use those reasons to predict cooperation, rather
than restoring an uncommunicated root plan or calling the full planner recursively.

## Runtime evidence

Owner limits: play/questions/answers below 100 ms; bidding and first pass below
1 s; return planning and raising below 5 s. First pass now uses the bidding
budget, classified by the observed exchange, with the existing 20% reserve.
These remain soft computation deadlines, not scheduling guarantees.

The 2,000-game run averaged 2.89 ms per candidate decision, p95 16.98 ms.
Bidding, first pass, return and raise remained within their phase limits.
There were 31 play wall-time overruns above 100 ms. They cluster in time across
both policies, but the cause has not been established.

Four 408-485 ms recorded play decisions were replayed eight times each with
recorded heuristic seat kinds, policy seed and warmed observation history.
All 32 selected the recorded action (rule/explanation labels excluded); wall
times were 0.13-6.42 ms. Existing profile scopes were captured. GetThreadTimes
has coarse 15.625 ms accounting here and cannot establish sub-ms CPU precision.
The long pauses did not reproduce; this is not proof that all latency tails are
fixed or that the repair speeds up play.

## Validation and fresh comparison

Integrated public-fact repair: `a6377847f6b4beff584664049f1026db0dfe48c4`
(source checkpoint `874a96baa5bf8310959de5e490d4c12729d277a5`).
An independent read-only source review found no blocker; it independently
recalculated the saved comparison statistics and checked the replay actions.

Passed in the standard optimized test profile, one test thread, bounded runner:

- `cargo test -p marjapussi-heuristic --lib -- --test-threads=1`: 55 passed,
  including the 12 public-fact/cache regressions and first-pass classification.
- Named integration targets `bidding`, `history`, `params`, `passing`, `play`,
  `raising`: 100 passed. No failures or ignored tests in either invocation.
- Source formatting and whitespace checks passed for the owned changes.

Predeclared before results: repair against deployed 13d2015, 2,000 games / 1,000
paired deals, seed 2026101404, policy seed 0, same bounded serial lane. No policy
tuning on that run. The frozen candidate is a637784. All 2,000 games finished:

- **+2.26 engine-score points/game**, approximate deal-clustered 95% interval
  **[-0.10, +4.62]**. Encouraging, but not an established strength improvement.
- Wins 1,000 versus 985, with 15 draws. Contracts made 611/1,000 versus
  603/1,000. Schwarz suffered 147 versus 152.
- Average decision 2.69 versus 2.71 ms; p95 16.20 versus 16.44 ms. This does not
  establish a speedup. Missing forecast-completion flags remain unknown.
- Candidate maxima: bidding 13.28 ms; first pass 2.13 ms; return 7.70 ms;
  raising 30.35 ms. All stay within those phase limits.
- Play had **44/39,763 wall-time overruns above 100 ms**, maximum 382.85 ms;
  the opponent had 35. The hard play-latency gate therefore remains unmet.

Private evidence: `attribution/fixed-candidate2000`, including manifests, raw
games, report and latency audit. Worker NDJSON SHA256:
`4674834344f185d6bd622153d6fd5e56c2a0c3a5edfce0c1f8919d3bcebce1a1`.

Every one of the 44 candidate slow prefixes was then replayed twice, preserving
recorded seat kinds, seed and warmed history. All 88 finished in **0.45-30.95 ms**;
none reproduced the wall-time overrun. 41/44 positions retained the action on
both repeats. Three changed: one recorded endgame decision had evaluated only
one world with complete=0; the other two recorded a rule prior where replay
selected a forecast override. This demonstrates deadline-sensitive decisions,
not proof of the original pause's cause. Replays use the same policy source with
profiling enabled, rather than the exact uninstrumented benchmark executable.

The eight warmed replay prefixes for the four selected earlier loss cases also
completed (8.26-24.03 ms). The repaired model now announces Bells in both rotations
of deal 79, restoring the handoff's choice. The other three selected reversals
remain. This is a decision replay, not a claim the complete game is now won.

The completed profile build and replay logs are retained. The first-pass and
public-fact commits are integrated locally; no production source pin was changed.
As predeclared, keep the existing public version while the stronger-play claim
and hard latency gate remain unresolved. Do not extend this sample opportunistically
until a confidence interval crosses zero; use a new declared test after a justified
next change or qualification decision.

## Cleanup

Removed the three completed, clean diagnostic worktrees after verifying their
immutable checkpoint tags. Reclaimed 8,131,680 source-file bytes (7.76 MiB).
Commits, manifests, datasets, replay evidence and reusable compilation caches
remain. No new containers were created for this iteration. Unrelated work and
the retained public rollback image were left intact.

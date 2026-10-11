# Conditional planning dry run and exact performance reuse

- Date: 2026-10-11
- Status: **incomplete strategic qualification**; focused correctness checks pass,
  exact optimizations pass independent review, selected timings improve.
- Branch: `feature/conditional-plan-review`
- Base: `8ca35c985c6f9695635dd61897d2bfb47c1f0c87`,
  `checkpoint/2026-10-11-concept-pilot` (itself unqualified for release).
- Tag: `checkpoint/2026-10-11-conditional-plan-dryrun`
- Recovery: the pre-change branch/tag remains intact. The earlier verified
  claim scope is `1632af8`, `checkpoint/2026-10-10-claim-established-suits`;
  neither that scope nor this record establishes general playing strength.
- Testbed, published branches and deployment: unchanged.

## Result

The owner requested dry runs on reviewed failures and close successful controls,
including all decisions, followed by performance investigation without weakening
the policy. This iteration supplies the reusable diagnostic runner, a bounded
actor-resource correction, and exact work reuse. **The strategic connections are
not sufficient yet.** None of the reviewed losing full games becomes a win.

`heuristic::trump_plan` separates the actor who needs the opening from the actor
holding the pair. Known pairs reserve rights-exclusive openings; partner entry
uses existing exact knowledge and the root's proof of what the partner knows.
`valuation` no longer pools both actors' lead resources. Return candidates add
bounded combinations of pair ownership and control; the existing forecast ranks
them. These are generalized resource conditions, with no fixture identifiers,
new fitted weights or game-specific action overrides in production.

This remains a bounded increment. Future opening access is approximate; it does
not implement the complete proposed conditional-sequence system. The source
design is [conditional resources](../design/conditional-resources.md), and the
independent scenario review is [diagnostic findings](../design/conditional-resources-results.md).

Performance changes preserve world/depth settings and scoring:

- `solver::trick_control` prunes each Boolean minimax goal once proved/disproved.
- A forecast retains four simulated actors and their bounded solver tables.
  Complete visible-history/parameter checks resynchronize each actor on forks;
  exact transposition keys retain full hypothetical state and objective inputs.
- A dealt-feature cache hit occurs before cloning unused derived parts.
- Opt-in `profile` scopes expose elapsed inclusive/exclusive work. Disabled builds
  contain no clock or thread-local profiling operations.

The explanation now identifies the best rejected action, excluding the chosen
action. Selected and rejected make frequencies both condition on declaring;
the rejected declaring share and joint made mass are also explicit. This last
display-only semantic correction followed the saved diagnostic runs and does
not change their choices or outcomes.

`tools::dryrun` registers existing/frozen factories and compares identical legal
recorded prefixes, one-action/seat/partnership continuations, or full games.
`eval::play_table_with` owns bounded legal continuation. All actors receive only
their seat-filtered view. See [runner contract and commands](../design/targeted-dryrun.md).

## Diagnostic evidence

The recovered original qualification dataset has 4,000 complete games: 1,000
deals, four arrangements, suite seed 20261011, policy seed 0. SHA-256:
`9a1eb1b4e7a5118f6eefb90924c6da60767629f49b5ebfb0a4767777219699b7`.
The orchestrator verified that digest; the worker labels its supplied digest as
externally declared. All row identities and selected complete legal replays were
validated before candidate execution.

Frozen original sides are `b3abdaf1f5e47111904db442ce36db7dc4ae9e0b` and
`1632af833199e5253271d493af8531badfac81c0`. The attribution baseline is later
`8ca35c9`, not either original side. The adapter freezes historical heuristic,
belief and solver sources independently; compatible engine/view contracts are
shared. It records source/archive and binary fingerprints outside Git.

Selection: eight previously reviewed cases (125, 2549, 2403, 1113, 2649, 3390,
3545, 162), plus four preselected close controls (57, 307, 664, 812). Controls
were selected before changes, by made contract and small absolute round-score
margin across declaring-policy/win-loss strata. Their original margins are five;
a made contract is not necessarily a round win.

- 603 identical recorded positions, all phases/all seats: 1,206 policy calls.
- A separate 17-position focal pass: 34 calls.
- 34 one-action and 34 partnership continuations, plus 24 full games.
- Eight manually scripted diagnostic alternatives against normal continuation:
  another 16 full continuations, for **108 completed continuations** in total.
- All selected games completed legally. No new 4,000-game strength run occurred.

Full-game replacement affects the reviewed partnership from the initial deal;
opponents retain their frozen original policy. Points below are raw card/pair
points; margin is final round score difference. Original-prefix continuations
are separate interventions and must not be pooled with these results.

| Game | Raw points, baseline to candidate | Score margin, baseline to candidate | Result |
|---|---:|---:|---|
| 125 | 11 to 81 | -445 to -210 | Still fails; auction 155 to 150 |
| 1113 | 79 to 119 | -440 to -35 | Avoids failing own 160; opponent makes 155 |
| 2403 | 87 to 87 | -455 to -455 | Same failed 200 |
| 2549 | 27 to 116 | -435 to -385 | Still fails 160 |
| 2649 | 300 to 300 | +780 to +780 | Makes 260, all tricks |
| 3390 | 300 to 300 | +900 to +900 | Makes 300, all tricks |
| 3545 | 320 to 320 | +795 to +795 | Makes 265, all tricks |
| 812 | 196 to 187 | +25 to +10 | Both win; contract 150 to 145 |
| 664 | 167 to 167 | -5 to -5 | Makes 150, loses round |
| 57 | 34 to 34 | -315 to -315 | Both raise to 170 and fail |
| 162 | 109 to 127 | -270 to -245 | Both fail; contract 140 to 130 |
| 307 | 149 to 149 | -245 to -245 | Both raise to 155 and fail |

The older policy originally made the close contracts in 57/162/307. Their full
replay failures already occur in the attribution baseline, so they are not new
regressions introduced by this increment. At the recorded focal positions their
made-contract controls remain preserved. In 664 the focal actor is a defender;
full replay instead replaces the original declarer's partnership.

### Remaining causal distinctions

**2403:** Acorn Ace is already in the candidate set. The forecast ranks Red 8 at
87.5% made/483.125 mean score, Acorn Ace at 75%/396.25 across eight worlds.
Forcing only Acorn Ace at step 18 and then resuming the live candidate makes all
200 points, versus 87 for its normal choice. Six own Acorns leave only 7/8/9
outside. Drawing until opposing trumps are exhausted connects a later Red entry
to its missing threat condition. All three outside trumps follow in the first
actual trick. This proves an executable successful continuation on this fixture,
not a universal informed guarantee over every possible deal.

The forecast follows reduced policies (no recursive/return planning, endgame
two tricks/eight worlds), while the live continuation replans with normal
settings. Saved records do not isolate world coverage from continuation-policy
error. More first-card candidates cannot solve this particular misranking.

**125/2549:** manually asking the known Green pair collects its 40 points, but
every tested late repair still fails 155/160. In 2549 the candidate can cash the
standing Ten while retaining its question; it later loses access without using
the question. This is incomplete follow-through, not evidence that the one-move
change fixed the plan. Earlier auction and return feasibility remain material.

The all-prefix pass has 12 changed actions. The separate focal pass changes
2549 step 13, but in the later pass the baseline reaches the two-second deadline
and falls back to the same Ten as the candidate. Completion/work counters explain
that discrepancy. Wall-time differences must not be attributed blindly to strategy.

## Performance evidence

Measurements used one job at a time, two logical CPUs, below-normal priority,
4 GiB process-tree committed-memory limit and the shared reusable target cache.
The machine is shared; these are focused comparisons, not a latency guarantee.

| Comparison | Before | After | Preservation |
|---|---:|---:|---|
| 17 matched unprofiled positions, performance changes only | 7,602.8352 ms total | 5,788.0138 ms total (-23.87%) | 17 actions identical; 16 full explanation/value matches |
| Four-game profile, 159 nonforced decisions | 68.6375 ms mean | 48.5424 ms mean (-29.28%) | All 206 actions/rules/explanation values identical |
| Exact trick-control batch, 2,371 queries, six alternating rounds | 2,493.5 microseconds mean | 1,767.2 microseconds mean | Every returned goal mask identical |

The remaining unprofiled position reaches the existing two-second timeout in
both versions. It completes 3,210 versus 1,889 simulated decisions, retaining the
same fallback action; its work counters differ. Exact reuse can change completed
coverage under a deadline. Universal identical timed choices are not claimed.

The four-game profile performs 1,696 rollouts and 61,089 simulated decision
requests. World construction takes only 12.3 ms total. Before/after elapsed
inclusive time: entry analysis 2,282.5/1,075.5 ms; endgame 1,921.2/322.2 ms;
belief derivation 2,427.3/2,368.1 ms. Rollout-exclusive work falls 422.4/57.9 ms.
Nested inclusive timings must not be added together. The main residual cost is
repeated actor belief/control evaluation, not generating worlds.

Combined strategy/performance means at the 603 matched positions are diagnostic:
return 566.2 to 531.7 ms; open lead 127.6 to 81.0 ms; raising 106.1 to 73.2 ms;
bidding 91.6 to 66.3 ms; engine `Trick` phase 11.65 to 9.22 ms. That phase also
includes leads after declarations/questions; it is not solely following play. Actions can differ,
so these numbers do not isolate optimization from policy work.

The owner subsequently clarified that planning may take a second or more, while
ordinary play should remain responsive. Reclassifying the same saved positions
by whether a trick already contains a card gives the candidate timings below;
no extra policy run or source change was needed. Percentiles use nearest rank.

| Actual context | Decisions | Mean ms | 95th percentile ms | Maximum ms |
|---|---:|---:|---:|---:|
| Following a started trick | 324 | 0.403 | 1.644 | 6.776 |
| Lead, announcement or question before a card | 142 | 77.890 | 438.358 | 1,341.549 |
| Return allocation | 12 | 531.691 | 1,442.158 | 1,442.158 |

The common two-second forecast limit does not enforce that phase distinction.
The remaining in-play performance target is repeated lead/announcement planning.
Preserve completed actor-visible sequence work across those transitions, with
explicit invalidation when its evidence changes. Reducing initial planning depth
or forcing a shallow fallback would not meet the no-functional-compromise goal.
This requirement is recorded for the next sequence increment; it is not claimed
implemented by these timing results.

## Checks and independent review

All commands use the resource wrapper above and single-threaded tests. Receipts
and full logs are retained under `tmp/2026-10-09-v1/conditional-*`.

| Check | Result and scope |
|---|---|
| Solver `trick`, `trick_pruning` tests | Pass; four existing tests and an independent scalar-minimax reference over 64 small positions/all 14 goals |
| Heuristic unit, play, passing, history, raising, contract-planning targets | Pass; 20 unit and 61 integration tests, then 21 final unit tests after diagnostic correction |
| Fresh versus retained rollout actors | Pass; all outcome fields match across 24 completed hypothesis forks |
| Beliefs unit and `consistency` | Pass; 16 unit and three integration tests, including 150 random games and hidden-card invariance |
| Tools library | Pass; 14 tests including 11 runner/fixture/continuation guards |
| Clippy affected libraries/binaries with `marjapussi-tools/profile`, `-D warnings` | Pass after parenthesizing two unchanged set expressions |
| New Rust files `rustfmt --check`; Git whitespace check | Pass; existing compact code was not globally reformatted |
| T3 independent source review | Accepts exact pruning, actor/solver reuse, privacy keys and opt-in profiling; diagnostic make-frequency inconsistency corrected |
| T3 independent strategic review | Confirms insufficient primary-case correction and preserves distinction between actual fixture success and all-world guarantee |
| Standalone HTML review page | Desktop rendered; all 60 game/view selections execute, all 603 matched rows present, no page overflow or console errors. Mobile resize tool timed out; mobile rendering not verified |

Initial implementation checks exposed an invalid test dependency on a private
engine helper, then a malformed test phase. The test was corrected to use public
`FastState` at a legal post-trick position. Production engine rules were unchanged.
No unresolved compilation, lint or focused correctness failure remains.

The final explanation normalization, equivalent parentheses, and formatting
follow the diagnostic binary. Its hash and retained executable identify the
measured implementation; these subsequent edits do not change policy actions.

## Reproduction artifacts

Relative to the outer `Documents/MarjaAI` workspace:

- `tmp/2026-10-11-conditional-plan/`: frozen Cargo adapter, three retained
  executables, source manifests, exact commands, NDJSON, performance/profile
  records, suite driver and report builder.
- `candidate-suite-manifest.json`: commands, result hashes and completion
  records for all eight jobs. Candidate binary SHA-256:
  `5da1495e8b2581559de3f3d44aab5091d60019758a4eb74d0f225caef3fed45d`.
- `counterfactual-manifest.json`: eight scripted diagnostics, kept outside the
  production registry. Their interventions are not deployed policies.
- `reports/2026-10-11-policy-dryrun.html`: self-contained review of outcomes,
  dependencies, all matched decisions and complete continuation traces.
- `reports/2026-10-11-reasoning-atlas.html`: previous architecture/current-versus-
  proposed graph report. Its complete proposal is not claimed implemented here.

Raw games, player identifiers, generated reports and executables stay outside Git.
All reviewers use this same verified dataset and saved runs. Fresh seed 20261012
remains reserved for later strength qualification.

## Remaining work and cleanup

Next is a conditional sequence owner for threat removal, entry, pair activation
and continuing control, with explicit consumed resources and observation-based
invalidation. The same sequence needs evaluation during auction, exchange,
raising and live play. Capture per-world failures and compare reduced versus live
execution before changing probabilities or sample counts. Preserve fact/uncertain
and actor-perspective boundaries; do not turn this one successful scripted Ace
into an unconditional draw-trump shortcut.

No production promotion is authorized by these results. The prior baseline and
its checkpoint remain intact. All owned validation jobs finished. No Docker
resources were created. The temporary report preview server is stopped after
review. No disposable files or empty owned directories remained to remove;
**0 bytes reclaimed**. Reproduction evidence, frozen binaries and shared build
caches remain in planned use for the next sequence iteration and comparison.

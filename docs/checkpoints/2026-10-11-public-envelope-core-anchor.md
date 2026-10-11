# Public outcome envelope: exact shared-core compatibility anchor

Status: **review-only; builder/runtime qualification pending**.
Base and newly reviewed anchor: `043c3ff60887782bd98c1b0b9510dfe1522e5be7`.
Branch `build/public-envelope-core-anchor`; intended immutable tag
`checkpoint/2026-10-11-public-envelope-core-anchor`.

## Source evidence

Rechecked all four shared crate trees against the previously reviewed
`4bff913bead8f33bcdce27751cfa0cb6e2296e71`. Engine, observation and agent are
identical, including manifests/tests. Knowledge differs only by:

- `lib.rs`: one new private module declaration and four new public reexports.
- `outcome_envelope.rs`: the independent public-fact score-envelope API.
- `outcome_envelope_tests.rs`: its six focused tests, compiled only for tests.

No existing knowledge implementation or crate manifest changes. Compared with
validated envelope `3eb23b21ec2378dd1101d4752ebc8f23570dd405`, the entire shared-core
diff is the envelope's three opening documentation lines, updated to acknowledge
the real consumer and distinguish numerical bounds from selection authorization.
The implementation and test source remain identical to that validated owner.

Exact shared crate Git tree IDs at the new anchor:

| Crate | Git tree |
| --- | --- |
| engine | `1b63958196be66b309bce5b9b985b9f4496ad3b2` |
| observation | `90cda3676e40bbbfd3e1b518201dc5d5a8581ca3` |
| knowledge | `75ec758b408c57a85fe4717a6034ae7431f93c1c` |
| agent | `dd6a8ae2cea89fa3e0f2be38687a927c431d9c90` |

The additive API neither changes existing types/methods nor introduces mutable
shared state, initialization hooks or dependency/features changes. Frozen
`b3abdaf1f5e47111904db442ce36db7dc4ae9e0b` heuristic/beliefs/solver do not reference
the new names or import `knowledge::*`. Their existing source behavior therefore
does not acquire new envelope calls or state changes. This extends the existing
4bff913 compatibility review; it does not claim 4bff913 equals b3abdaf, identical
machine code/timing after recompilation, or full historical alias runtime parity.

## Mechanism and boundaries

Only add this full commit ID to `COMPATIBLE_CORES`. Keep all earlier anchors,
`HISTORICAL`, `OLD`, `COMPATIBLE_CORE`, `SHARED`, `FROZEN`, and snapshot/build logic
unchanged. In particular, `previous` remains frozen b3abdaf heuristic, beliefs
and solver; `old` remains the distinct 1632af8 policy. The anchor is not a new
historical policy version.

The existing guard still compares every tracked shared-core path against an exact
reviewed tree and rejects any untracked shared-core path. It does not accept an
arbitrary descendant or skip knowledge checks. A later change in any shared
crate requires review unless its complete source tree matches an existing anchor.
Root's inspected checkout matched the committed shared-core tree with no untracked
shared-core files. No guard execution, snapshot generation or worker build was
performed for this iteration.

This anchor certifies the shared-core delta only. New heuristic consumer
correctness/performance, the actual requested current/frozen worker graph, bounded
planning qualification, deployment and a 256-game campaign are separate gates.
This commit authorizes none of those runs or actions.

## Validation and remaining work

Reused validated envelope evidence: six tests PASS and strict knowledge-library
Clippy PASS on 3eb23b2. Final log SHA256 respectively:
`ad6078d41cd7307f0bad5db2d255cc2dfd3f0bd538acba63ebcbe457e2d6ceb0` and
`9ce230d56096b56e9eff693200dc3102ea48f8f80fb98048d12f3459ea6e202b`.
Root reports 139 policy, 18 selfplay-tool and 13 graph tests passed for 043c3ff;
those are integration evidence, not a full external alias-graph qualification.

This iteration: source/tree comparisons, staged diff review and whitespace check
only. **No builds or tests run.** Root owns later serialized validation. Relevant
builder tests are the existing Python source-freezing/changed-core rejection and
build-provenance lifecycle tests. Before a historical comparison, qualify the
actual requested alias graph through the corrected locked builder, including
unchanged frozen source proofs, selected fresh local artifacts on a binary miss,
and exact copied worker provenance. The small two-package driver does not replace
that qualification. Broader policy/planning gates remain pending independently.

No root edits, processes, containers or build artifacts created. Reclaimed zero
bytes; retain this small isolated source worktree through integration.

Root integration: the 32 existing builder tests passed in the serialized resource
lane. Receipt `public-envelope-core-anchor-python`, 0.94 s, 35.3 MiB peak commit;
log SHA256 `8b6959571eccb3fff5b3718ddb5eb4473cd040f00d77009a1d99e2feb619e646`.
The real current/frozen worker build remains pending.

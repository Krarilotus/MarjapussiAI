# Checkpoint: response entry-only eviction

- Date: 2026-10-11
- Status: incomplete overall; focused correctness checks pass, performance unqualified
- Tag: `checkpoint/2026-10-11-response-entry-eviction-integration`
- Branch: `design/executable-concept-contract`
- Base: `bd647a0000dc6d2967bac14f81a9c44f1e23302f`
- Source: `61f38a52793c795cfc56313d2e6b0e76a8c230cf`, retained at
  `checkpoint/2026-10-11-response-entry-eviction`

## Result and invariant

Response entry-only pressure previously discarded every optional dependency,
then necessarily discarded responses too: deleting canonical suffixes could not
reduce the response count. The new early branch reclaims only responses and their
capacity. It subtracts the existing `response_bytes` from both retained and
reusable totals, resets that category, then uses the existing response admission
path. The caller subsequently adds the admitted response to reusable/response
totals exactly as before.

`response_bytes` accounts only admitted response payloads. `reusable_bytes`
includes responses and other persistent dependencies; `work.retained_bytes`
also includes session-only completed finish results. Thus subtracting only
responses preserves both non-response ledgers, including completed finish data.
No byte-category schema, cap, Program ownership or Work field changes.

Byte-only and combined pressure keep the original clearing/admission order.
Oversized and disabled admission keep the original early return. Pressure-reason
events remain before clearing; `memo_nonempty` now occurs only on the path which
actually clears that memo. No additional timing labels or key witnesses.

## Evidence and tests

Source inspection verifies response payload accounting at its sole admission
site and scope handoff. Rustfmt parsed production source with children skipped;
new test additions were formatted. Diff whitespace checks passed. No compiler,
tests, benchmarks or policy runs were invoked by the source owner before handoff.

Two new tests reuse the existing joint-cohort/response fixture:

- A real completed parent populates canonical slots, membership, hard reads,
  tails and finish memo; real distinct actor inputs fill the response entry cap.
  Entry-only pressure preserves all categories and exact ledger deltas; the same
  suffix has no construction event, and the same parent returns original
  terminals/prepared records while charging its stored represented work.
  A changed input with an invalid unreachable node must still fail validation.
  Byte-only/combined pressure and oversized/zero-entry rejection are controls.
- Full engine outcomes, assessments, receipts, selected-parent authorization and
  represented transitions/comparisons/solver effort must match uncached, cold
  and warm runs with a one-entry dependency cache. Profile assertions require
  actual entry-pressure events rather than assuming that path was exercised.

Existing byte-eviction, zero-capacity, oversized-response, canonical-validation
and logical-effort tests remain unchanged. E independently reviewed the frozen
source and fixtures, verified the ledger invariant and patch hash, and found no
blocking source issue. That source review did not execute tests.

Root validation used the shared serialized runner: one Cargo job, one test
thread, two logical CPUs, below-normal priority and 4 GiB total committed memory.

| Check | Result | Receipt log SHA256 |
|---|---|---|
| `response-entry-eviction-policy` | 120 passed, 2 expensive fixtures excluded; 78.40 s including compilation, 1.22 s tests, 577.5 MiB peak | `47b9ace0fe094a85c587fbf0438f4dc759338b0586e8a4e799178226cea168a0` |
| `response-entry-eviction-profile-build` | passed; 14.11 s, 309.3 MiB peak | `74241b3164342f1b4de2d91a00022250c3a80e9bb338043da0d4fb3fa0bcf51c` |
| `response-entry-eviction-profile-two` | diagnostic completed; 22.05 s, 108.3 MiB peak | `2bdd79557d4afb37a07a269222ee3b9756270341e4b7926adfd941a65de86988` |

Policy command: `cargo test -p marjapussi-heuristic --features profile --lib
-- --test-threads=1`, excluding
`auction_and_return_reuse_the_real_depth_two_boundary_with_joint_support` and
`selected_parent_authorizes_multiple_local_steps_after_the_final_global_boundary`.
The former previously passed; the latter still has the documented 500k logical
work failure. Neither was requalified by this focused run. Both new tests passed,
including positive entry/byte/combined pressure counters and exact represented
work parity after reuse.

The optimized-test-profile `perf` binary ran the same two games, one repetition,
with timings, profile and actual-call decision capture saved privately under
`tmp/2026-10-11-response-entry-eviction-profile`. All 105 action/rule choices
match the prior diagnostic: action hash `f6e72612fb94a667`, action+rule hash
`d1e123e60954fee5`. All 23 planning comparisons still deferred.

The profile observed 14 entry-only response evictions without clearing the
finish memo; 145 byte-pressure events did clear it. It completed 8 individual
candidates, used 124122 actual / 177727 represented transitions and served 32
finish hits. Suffix construction took 4.633 s over 143170 calls. The preceding
diagnostic had 14 individual completions, 166179 / 237338 transitions and 4.421 s
over 162719 suffix constructions. These deadline-truncated prefixes differ;
the shared-machine runs establish neither a speedup nor a causal regression.
Preserving larger linearly searched caches is a remaining performance concern
for a fixed-work comparison. Correct cache preservation is not speed qualification.

The measured profile motivating this change recorded 22 entry-only evictions.
It did not identify evicted repeated keys or quantify reconstruction attributable
to this path. This checkpoint makes no speed, strength, completion or readiness
claim; the all-Deferred and six-card logical-work gates remain unresolved.

## Ownership and cleanup

Owned files: `crates/heuristic/src/planning/sequence.rs`, additions only in
`crates/heuristic/tests/internal/sequence_normalization.rs`, this record.
Root applied the focused patch after hash and applicability checks. Completed
validation processes exited; no containers were created. Shared build cache,
isolated source and evidence remain needed for planned comparison. Reclaimed
space: zero bytes. No release, deployment or playing-strength claim.

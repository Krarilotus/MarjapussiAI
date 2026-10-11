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

## Fixed-work follow-up

Base `7738a3bc287736bc69a174d806534efee4e9463f`; evidence tag
`checkpoint/2026-10-11-response-entry-fixed-work`.
An identical private test shim reused the existing two-world fixture and all six
byte-identical helpers at `bd647a0` and `7738a3b`. Each invocation ran 128
uncached/cold/warm repetitions without a deadline, using the original domain,
response model and limits. Four serial invocations used before/after/after/before
order. All completed, with equal complete values, terminals, receipt, prepared
records and selected-parent authorization. The complete reference text has SHA256
`166f01510f435498b7e7d765c8c28df994238dd5ac3481ca2bf4f135cb614129` in every run.
Each call represents 72 transitions, 2 comparisons and 216 solver nodes.

| Work across 128 calls | Before | After |
|---|---:|---:|
| Cold suffix constructions | 2048 | 1408 |
| Warm suffix constructions | 1536 | 256 |
| Warm actual engine transitions | 9216 | 4608 |
| Warm actual solver nodes | 27648 | 13824 |
| Warm response entry-pressure events | 256 | 128 |

Both repetitions reproduced these counters exactly. Each cold repetition
exercised entry pressure. The cache uses one dependency entry in this fixture;
this does not qualify scan costs at the production capacity. Observed elapsed
means were noisy even for unchanged uncached work: before 0.47/1.09 ms, after
2.61/2.60 ms. There is no elapsed-speed conclusion. Reduced actual work with
identical logical work is established for this fixture only.

Build provenance required a correction: the first after-build reported a fresh
cached executable identical to the before binary. It was rejected before any
measurement. Updating the two changed files' timestamps forced real heuristic
compilation, with source bytes unchanged. The resulting binaries have distinct
hashes and the expected 121/123-test inventories. The four runs used private
copies selected from Cargo's JSON artifact events. Future snapshot comparisons
must invalidate changed local packages and hold the shared lock through build,
artifact verification and copying; requested source paths alone do not establish
artifact provenance.

Private reproducibility bundle: `tmp/2026-10-11-response-entry-fixed-work`,
including source/patch manifest, commands, test lists, binary hashes and
`results.json`. Runner receipts `response-entry-fixed-before-1`,
`response-entry-fixed-after-1`, `response-entry-fixed-after-2`, and
`response-entry-fixed-before-2` record respectively log SHA256
`0fbd27845324867298ac11bc2585469fc73c9c8816d01c0375071bdce61aa6d8`,
`5e262181b2c4c8f760e7c7df84f6d21d59447b59f6d4f19d173d8946a20923cc`,
`e2bef8ac8613f7603c8901220625260efa0cc4c04e0d087041086c48b03d9a3a`,
`67b3db91867165326cc9d2e5da4cc02e72520502b6449ba2703010fdf36764c2`.
The private shim is not part of the policy or its runtime configuration.

## Ownership and cleanup

Owned files: `crates/heuristic/src/planning/sequence.rs`, additions only in
`crates/heuristic/tests/internal/sequence_normalization.rs`, this record.
Root applied the focused patch after hash and applicability checks. Completed
validation processes exited; no containers were created. Shared build cache,
isolated source and evidence remain needed for planned comparison. Reclaimed
space: zero bytes. No release, deployment or playing-strength claim.

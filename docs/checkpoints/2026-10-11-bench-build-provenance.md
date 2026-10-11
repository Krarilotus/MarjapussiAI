# Checkpoint: benchmark build provenance

- Date: 2026-10-11
- Status: **builder lifecycle validated; full planner benchmark pending**
- Base: `a74317963bd879d75b89edbbcd83cdc86c7c8ee5`
- Branch: `fix/bench-build-provenance`
- Tag: `checkpoint/2026-10-11-bench-build-provenance`

## Problem and change

A private before/after build reused a test executable after switching prepared
workspaces in one target directory. Cargo reported the changed candidate fresh
because source mtimes predated the previous compilation. No after measurement
used that executable; the owner rebuilt and verified distinct copied artifacts
before the fixed-work comparison. That incident does not invalidate unrelated
receipts that show real compilation and the intended tests.

The official builder already copied under its CLI resource lock, but a new
content key for the final worker did not force Cargo's local dependencies to
recompile. Different resource locks could also select the same mutable cache.

The existing RunLock now additionally owns the cache's complete prepare lifecycle:
snapshot verification/adaptation, final-cache verification, offline resolution,
package cleaning, build, post-build verification, copy, hashing and publication.
The CLI rejects a resource lock equal to the cache lock before acquiring either.
There is no lock registry, implicit reentrancy or general scheduling framework.

On a final-binary cache miss, the resolved graph selects local path packages by
package ID. Supported Cargo package cleaning uses the same generated manifest,
target and environment as compilation. The current graph includes nine current
crates, the requested historical aliases and worker; registry artifacts stay
cached. Resolution fixes Cargo.lock before locked clean/build. Verified final
cache hits skip these commands.

Cargo JSON must contain non-fresh artifact evidence for every invalidated local
package, successful build completion, and exactly the expected worker executable
target. Source/configuration and generated-harness inputs must remain unchanged;
historical file proofs are rechecked. Only then is that event's executable copied,
hashed and recorded under the lock. Commands, target, selected packages, artifact
events and build-log hash accompany existing provenance. No completed build record
is written for rejected evidence or failed compilation/copy. The record is
atomically published last; incomplete entries rebuild on retry rather than
treating an already copied executable as a completed cache hit.

Current core compatibility anchors, frozen source identities, policy defaults,
world/horizon/budget configuration, scoring and game execution are unchanged.
The 256-game/128-deal smoke remains unrun and requires separate readiness checks.

## Source checks and pending validation

- Python AST parsing passed for the four changed scripts, without importing or
  executing them. `git diff --check` passed.
- Added mocked lifecycle tests for graph-scoped cleaning (registry/inactive-local
  exclusion), shared manifest/target/environment, exact artifact selection rather
  than conventional output path, verified cache hits and tampering rejection.
- Failure cases cover clean/build errors, fresh/missing dependencies, wrong
  manifest, missing/wrong/outside/duplicate worker, absent successful completion,
  source/config/harness changes, and copy failure without a completed record.
- Publication retry test covers a later copy failure leaving an executable,
  record-publication failure, and successful resolution/clean/rebuild on retry.
- CLI test covers explicit same-path lock rejection before lock/prepare calls.
- All tests are **UNRUN**. No Cargo command, compiler, benchmark or game was run
  by this worker. Root owns serialized Python tests and real Cargo qualification.
- Independent A source review found the publication retry gap; it was corrected
  with the atomic completion marker and retry test. Re-review found no further
  source blockers. Source review is not executable validation.

## Limits and cleanup

New final-cache keys rebuild local packages; this correctness cost is intentional,
not a speed improvement. Registry caches and verified final binaries remain
reusable. Cargo event compatibility, Windows locking and a real cold/hit lifecycle
still need the owner's shared runner. No baseline-strength or timing claim follows.
Current local path dependencies are unconditional. If platform-conditional local
dependencies are added later, metadata selection must match the build platform.

No root or other source worktree edited. No owned processes, containers or build
outputs; reclaimed 0 bytes. Private fixed-work snapshots remain for root cleanup;
their saved patches, artifacts and evidence were not modified.

## Root integration and execution

Integrated the focused source patch from `90d1faa7c958873e5ce11765c565947cffa13255`
onto root `a743179`. Source review and the unrun notes above describe the handoff;
the root then ran the following checks under the single validation lock, two
logical CPUs, below-normal priority and a 4 GiB process-tree cap.

- Full Python discovery: **32 tests pass**. Receipt
  `tmp/2026-10-09-v1/bench-build-provenance-python.json`, 1.39 s, 35.3 MiB;
  log SHA256 `56a12af578c3963620343d19e9e3de05c8b642e20bf604da64c235975e033e96`.
- Real Windows/Cargo lifecycle: **passes**, 8.79 s, 251.2 MiB. Receipt
  `tmp/2026-10-09-v1/bench-builder-real-qualification.json`;
  log SHA256 `9a85c51d0db634a238dcfc7e477339c685cb482a97099eee24dd46e746241755`.

The private driver executes the production `prepare`, source fingerprint,
toolchain, lock, graph selection, package clean, artifact verification and
publication code. It substitutes only the generated harness with two tiny local
packages. Three real compilations cover A, B with injected late-copy failure,
and a successful B retry. Dependency source timestamps predate all build outputs.
The copied executables print their intended A/B values and have distinct hashes;
verified A/B hits and restored A require no compilation. Independent child
processes confirm lock exclusion during metadata and copy, and release after
failure/success. Actual Cargo evidence passes; fresh/missing dependency evidence
is rejected. An unrelated target marker survives package-scoped cleaning.

Driver and detailed evidence remain in the private
`tmp/2026-10-11-bench-builder-qualification` folder. This validates full package-ID
clean syntax and the real lifecycle on two local crates, not the full nine-crate
planner/frozen-policy graph or playing strength. The core guard is unchanged.
The 256-game smoke is still unrun. No policy change, push or deployment occurred.

Follow-up integration tag:
`checkpoint/2026-10-11-bench-build-provenance-integration`.
The earlier fixed-work source snapshots were removed after preserving their
patches/binaries/results: 6,469,528 file bytes reclaimed. Shared caches and datasets
remain for planned reuse.

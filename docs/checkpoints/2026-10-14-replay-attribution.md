# Checkpoint: recorded-prefix replay diagnostics

- Date: 2026-10-14
- Status: incomplete — validation pending
- Tag: `checkpoint/2026-10-14-replay-attribution`
- Worktree: `wt/v1-auction-reading`
- Base commit: `82f84cbac861efe4ef78b5807b38f4b3e4f2c30a`
- Scope: `crates/tools/src/bin/replaydiff.rs` only, plus this checkpoint.

## Result

Extend the existing diagnostic tool with parameter/label input, explicit heuristic
seat kinds, recorded-prefix cache warming for the queried seat, and optional
post-decision belief bits. Recorded actions always determine the replayed state;
the policy receives only its seat view. Report actual/recorded phases, lineup,
visible history, legal actions, knowledge, selected action/rule/facts and timing.

Windows GetThreadTimes supplies current-thread kernel+user CPU milliseconds;
unsupported systems or failed calls report unavailable rather than zero. Wall
milliseconds and an absolute start timestamp accompany each target decision.
With the existing profile feature, reset scopes after warming and before the
target, then capture calls and inclusive/exclusive wall time before belief reads.
Profiling is diagnostic and is not final speed qualification.

## Evidence

| Command or review | Result | Scope / limitation |
|---|---|---|
| `git diff --check -- crates/tools/src/bin/replaydiff.rs` | Passed | Static whitespace check only |
| Source inspection | Completed | Existing public APIs and profile feature; no policy changes |
| Owner source review | Reported acceptable for compilation | In-thread review of first 290 lines; not execution evidence |
| Build, tests, replay | Not run, explicitly delegated to root | Windows FFI linking and runtime behavior remain unvalidated |

Private request/provenance artifacts remain outside this commit under the
worktree's `tmp/replay-attribution/`. The loss fixture is bench-v1base256, seed
2026101401, 256 games/128 paired deals. Its worker NDJSON SHA256 is
`f70f84869c250f4cb922a5a4cf941ece60a9f1f90b33065ec0c48117ef84b3f1`.
Requests cover both rotations at the first differing decisions of deals
79/39/110/26 and four owner-specified bench-v1base2000 timing tails. Private source
manifests retain exact file hashes, line numbers, lineups and prefix fingerprints.

## Remaining work

Root must compile the tools binary with `--features profile`, execute the saved
requests under the shared serial resource limits, and check recorded-state
identity, CPU availability and profile isolation. No strength or causal result
is claimed. Actual HO world pools, all candidate/prior scores and complete plans
remain private APIs; exported marginal equality is not proof of identical worlds.
This incomplete diagnostic checkpoint does not promote or replace a verified
playing baseline. Existing policy qualification remains with root.

## Cleanup

No owned build/test/replay jobs or disposable build outputs were created. Reclaimed
space: 0 bytes. Private handoff artifacts and existing caches retained for root.
Unrelated report.py/test_report.py changes remain uncommitted and unchanged.

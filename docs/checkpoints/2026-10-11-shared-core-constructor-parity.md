# Shared-core constructor compatibility: focused checkpoint

Status: focused constructor parity **passed**; compatibility-pin patch source-checked.
Reviewed exact anchor: `4bff913bead8f33bcdce27751cfa0cb6e2296e71`. Frozen reference: `bcf567164d142c969ace2ed63b14e0e6ddedde2e`.

The benchmark builder adds this one committed anchor to `COMPATIBLE_CORES`.
Its complete shared-crate diff and untracked-file rejection guard are unchanged.
Historical heuristic, beliefs and solver revisions, copied-source verification,
adapter generation and build fingerprint logic are unchanged. Future shared-core
changes still need to match an exact reviewed anchor or fail the guard.

Source audit finds engine, observation and knowledge unchanged across whole
crates from the reference. Agent changes are limited to validated hypothetical
hidden-transfer reconstruction, typed failures, cancellation checks, public
exports and tests. Existing `Table::from_view` delegates with no hypotheses;
hidden transfers remain rejected there. Malformed inputs may be rejected more
strictly. Live Table methods from `new` through `hand` are byte-identical.

Root ran the private constructor parity harness on this commit: exit 0,
134 accepted comparisons, 38 hidden-history rejections, 122 matched next
actions, 43 selected prefixes and four terminal trajectories. Fixed shuffled
deals cover auction, visible exchange/return, raising, pair/half questions and
answers, announcement, play and terminal scoring. The reference body is copied
verbatim from `crates/agent/src/table.rs` lines 36–103 of bcf567, adapted only as
a local trait method. Both constructors use the same audited current engine.
The obsolete copy remains private, outside Git and runtime artifacts.

Compared state, rights, ordered reconstructed hands/legal actions, all filtered
seat views/knowledge, snapshots, series, subsequent transitions and final scores
agree. Live shuffled-hand legal order is not compared with sorted reconstruction;
old versus new reconstructed order is compared exactly. Live clocks are Wall;
reconstructed clocks are Off. Initial four Start timestamps and nondeterministic
metadata text are excluded as documented in the private harness.

Completed receipt: `tmp/2026-10-09-v1/shared-core-constructor-parity.json`.
Log SHA256: `6420abfff877f3fb9a9813904ecb8d8d76266ad2ab3abe976e88becc5646fd9b`.
Receipt reports 12.22 s including compile and 246.2 MiB peak committed memory;
test time is 0.01 s. The owner identifies the run revision; the receipt itself
does not embed it. Source bytes were independently checked against this commit.

Private audit: `tmp/2026-10-11-graph-contract/shared-core-parity/provenance.json`.
It records all 58 shared-crate files, per-file SHA256, crate Git tree IDs,
frozen helper/test/lock hashes, completed receipts and original preparation data.
Canonical core-file manifest SHA256: `59c020bbc0e6311d3df6b93bc74741839f56f75fe1f3fdee6b4d295a498e5157`.
Frozen body SHA256: `b3e5a9b6d03d3744ac4bb41fedd1d241895293af077173be9fbe4bf25d64a0ab`.

This qualifies the narrow shared-core adapter pin, not exhaustive constructor
equivalence, hidden-history probability calibration, policy parity, playing
strength, cache fingerprints or hard latency. New heuristic/belief/solver changes
are outside this evidence. Separate policy validation gates remain in force.
Reviewer ran no builds/tests and edited no root files. This patch received only
Python AST/guard-identity and Git apply checks. Root owns integration/checkpoint
commit and tag. No deployment, containers or build outputs changed; reclaimed
0 bytes. Retain private evidence and shared caches for planned validation.

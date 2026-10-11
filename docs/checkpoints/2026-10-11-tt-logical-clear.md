# Checkpoint: solver TT logical clear

- Date: 2026-10-11
- Status: **incomplete**; implementation and tests authored, root validation pending.
- Base: `52c2bba272f5f5e8127fd6cbdddbbb9377316b81`
- Branch: `review/sequence-scenario-gates`
- Tag: `checkpoint/2026-10-11-tt-logical-clear`

## Scope and preservation contract

Only `crates/solver/src/tt.rs`, its new internal tests and this record change.
The TT source matched the root checkout before editing (SHA256
`fc3001e6539bec946b549ac3acad6200cdb1ef1e9bdb8d8691cea9f675b89280`).
Root was read only; previous response implementation files remain unchanged.
This commit can be integrated independently of its parent response snapshot.

The table and entries carry a `u16` epoch. Zero marks invalid entries; active
epochs run from 1 through 65535. Clear normally increments only the table epoch.
Probe requires the current epoch and full key; tightening requires those same
conditions. A stale matching key is replaced with fresh bounds and best move.
Before reusing epoch 1, clear physically fills every entry with EMPTY. This
prevents resurrection of entries untouched throughout the entire epoch cycle.

Hash, slot count, slot selection, always-replace collisions, current-epoch
bound tightening, best-move replacement and solver counters are unchanged.
Thus a logically cleared table supplies the same probes/stores as a physically
empty table for valid solver keys. The canonical effort algorithm name remains
`empty-tt-whole-evaluation-nodes-v1`; reference search semantics do not change.
No policies, objective, world/trick counts, limits or default caps change.
The existing zero-bit shift edge is outside this change and remains untouched.

## Memory and performance scope

The extra two entry bytes fit the old three padding bytes in the expected
64-bit Rust layout: 32-byte key, two i16 bounds, u16 epoch and u8 best move
still round to 40 bytes at 8-byte alignment. A layout test compares size and
alignment with the previous fields and asserts 40 bytes on 64-bit targets;
it has not run yet. Rust layout is compiler-dependent, so this is not an ABI
promise. No side allocation is added. The table header also gains a u16 epoch.

For a 16-bit table the expected entry allocation remains 2,621,440 bytes
(2.5 MiB). Normal clear is O(1); wrap clear remains O(entry count), once per
65535 clears. Each probe/store adds an epoch comparison. **No performance gain
has been measured.** Root must measure before making that claim. Wall-clock
completion near a deadline is timing-dependent; the preservation gate is exact
values/decisions and canonical node effort at matched logical limits, alongside
the existing interruption checks. No hard CPU guarantee is added.

## Validation and remaining gates

Source inspection and standalone rustfmt completed; whitespace diff check
passed. Five tests were authored, not executed:

- unchanged entry size/alignment;
- colliding keys replace bounds and move, with no cross-key tightening;
- current-key tightening and best replacement, then fresh same-key storage
  after logical clear (including bound clamping and NO_MOVE);
- real full epoch cycle clears both untouched first-epoch and last-epoch data;
- deterministic collision-heavy operation stream agrees after each operation
  with an independent physically cleared Option-slot reference.

Root owns the shared runner. Required checks include the new `tt::tests`, the
existing four `completed` integration tests (decisions and canonical effort,
exact node cap, zero work, incomplete worlds and secondary criteria), and
affected existing `exact` solver checks. Existing passes precede this change
and are not claimed as validation of it. No builds, tests, benchmark games or
performance measurements were run here; this checkpoint is not verified.

No owned processes, containers or disposable build output were created.
Reclaimed space: 0 bytes. Existing caches and private datasets retained without
copying or committing them. No push or merge.

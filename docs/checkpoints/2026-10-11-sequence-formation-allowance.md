# Checkpoint: formation work and dependency reuse

- Date: 2026-10-11
- Status: incomplete; observed formation and repair, prepared execution still unqualified
- Base: `19b4efd`
- Tag: `checkpoint/2026-10-11-sequence-formation-allowance`

Opening formation has a separate configurable logical transition allowance (131,072); ordinary repair retains 8,192 transitions and its existing soft 50 ms lane. Retained dependency memory is separately bounded to 64 MiB by default, with configuration validation. The opening wall allowance remains two seconds. These are execution settings, not strategic concepts or confidence values.

The preceding copy/reuse change passed 45 focused heuristic tests. A two-game development replay retained every action and rule: action hash `6cc56a82b0a40341`, action/rule hash `ce1fc33091c9a34b`. Shared-machine timings alone establish no speedup.

With the new allowance, one opening completed 32 candidates in 264.5 ms, with 1,709 actual and 38,969 represented transitions, 675 suffix hits and 28 comparisons. Later fresh repairs completed in 48.3, 23.8, 7.4 and 1.3 ms. Another opening deferred after 323 ms with a full retained-memory ledger; most early ordinary comparisons still deferred. No prepared globally covered continuation was observed. The existing raising path still bypasses authored opening formation and needs integration.

Validation used the shared lock, one optimized-test-profile build at a time, two CPUs, below-normal priority and 4 GiB process-tree commit. Heuristic-test log SHA-256: `2711f9f97c27534deb40ad1220a131ef4dddfcde25e64d5bc469d4d409cce7f1`. Formation-build log: `f0b6e56eb7b58c028dd69b322eea5932e6e484c702cbbbc25e867189bbf8f4d5`. Two-game diagnostic log: `6df8ac149bc794ffe0d75304f9db61ab1886c9a6d374d013358d0830427bf6a1`. Its binary, complete source patch relative to the base, outputs and fingerprints are retained outside Git under `tmp/2026-10-11-executable-sequence/formation-budget-*`.

The 256-game smoke has not started. Next gates are initial raising/continuation composition, globally covered future decisions, exact compatible dependency reuse across repairs, typed deferral causes, and targeted strategic controls. These diagnostic games are not playing-strength evidence. No production change occurred. Shared evidence and build caches remain needed; reclaimed space: 0 bytes.

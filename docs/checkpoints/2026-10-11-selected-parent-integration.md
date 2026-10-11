# Selected-plan authorization integration

Status: **incomplete**. Base `f531d17f409c6a16a8acdffa1b0c68e47bb95018`.
Immutable tag: `checkpoint/2026-10-11-selected-parent-integration`.

Integrated focused patches from E `57e2b356` and B's parent capture, plus D's
linear suffix correction `9f42eaf2`. The import hunk was adjusted for the new
selection module; the deferred forecast test now checks absent selection.
No whole snapshot was cherry-picked. Source-only handoff records remain available.

A completed parent comparison owns immutable records of its selected execution.
Later local choices check their exact information, original joint support,
predicted catalogue and complete bound alternative domain. Explicit global
reconsideration retains its full-domain check. Retry keeps its original legal
domain. Captures link the parent and child scope without serializing the future
tree or claiming that each local continuation is a new global optimum.

The measured slower whole-program hash index was removed. Linear full-equality
lookup keeps the single lookup per stage scan. Unused hash derives and the
fingerprint module were removed. Remeasurement remains pending; this commit
claims no performance improvement.

## Validation

All jobs used the shared serial runner: two logical CPUs, below-normal priority,
4 GiB aggregate memory, one Cargo job/test thread, optimized test profile.
Private receipts are under `tmp/2026-10-09-v1` outside Git.

- `selected-parent-linear-policy`: compile failed on the remaining old
  `deferred.prepared` test reference; migrated to `selection.is_none()`.
- `selected-parent-linear-policy2`: **98 passed, 2 failed**, 85.50 s including
  compilation, 31.67 s tests, peak 557.3 MiB. Log SHA256
  `38037b70a19fc54f5e470cb273b8c19ac4940fb06748f27dc374d886f47e9a8e`.
  The unchanged full-catalogue depth-two auction/return test stopped at
  1,999,991 represented transitions (365,357 actual), before its parent completed.
  The new final-global multistep test stopped at 500,000 represented transitions
  (358,193 actual), also before completion. Neither establishes retained-route
  success yet. The fixtures and limits stay unchanged for the sharing repair.
- `selected-parent-capture-tools`: **30 tools + 9 graph tests passed**,
  68.97 s including compilation, peak 398.2 MiB. Log SHA256
  `32f0306dcee4a1e42c0018c5d1b47e061c019689ead4d591470eaf64e38c3cb1`.
- Diff whitespace check passed. Independent selected-parent review is pending.

No 256-game smoke, playing-strength claim, production update or HTML delivery.
Next: validate same-group equivalent Tail sharing, rerun the two completion
fixtures, inspect parent integrity review, qualify shared-core parity and
performance before starting paired games. Shared caches/evidence are retained;
no owned Docker resources or disposable build outputs removed (0 bytes).

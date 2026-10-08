# Checkpoint: reviewed AI continuation and dependency pairing

- Status: verified for the scoped code and integration checks below
- Tag: `checkpoint/2026-10-08-publication`
- Integration branch: `integration/reviewed-ai`, targeting `v1-preview`
- Publication base: `1c077aedb47218f822597f2b6fe9e37edfc41abb`
- Testbed runtime/UI: `066581872d3f0181ecf377487f2ddcddcfae38e8`
- Testbed setup documentation: `checkpoint/2026-10-08-ai-setup` in the separate
  [testbed repository](https://github.com/Krarilotus/MarjapussiAI-testbed).
  Its record contains this AI checkpoint's resolved hash; both tags identify the
  exact pair without a circular dependency on future commit hashes.

## Scope and provenance

The clean publication history imports the existing continuation from local
`46231e6`, then replays exchange cleanup `8ce5e94` and history fix `6974d82`.
The original private development ancestry is not merged or pushed. Original
local tags remain immutable; corresponding published checkpoints are
`checkpoint/2026-10-08-published-exchange-cleanup` (`61adf38`) and
`checkpoint/2026-10-08-published-history-cache` (`d4be203`).

The preview's ignore safeguards, environment-based player grouping, NOTICE and
concept graphics are retained. Documentation now distinguishes implemented pass
choice from missing pass-choice interpretation. The unchanged graphics are
explicitly labelled as the historical 7 October design snapshot.

Review follow-ups reuse `engine::CARDS`, the existing `Mind::holds_row` accessor
and one private uncalled-pair-value helper for both bidding models. They preserve
the existing calculations and weights; no fit, strategy or dependency is added.
All nine crate manifests now inherit the real repository URL. The AI README
links the engine fork and the testbed's usage instructions.

## Dependency contract

The AI builds its included `crates/engine` source, derived from `marjapussi-rs`
as recorded in `NOTICE`; it does not fetch the upstream engine or fork at build
time. Its internal renamed Cargo dependencies all resolve inside this workspace.
The testbed imports six AI crates by sibling paths, transitively resolving all
nine. Both locked dependency graphs were checked with Cargo metadata for
`x86_64-pc-windows-msvc`. Core-only AI `main` cannot serve this testbed; use
`v1-preview`. External dependencies resolve through the committed lockfiles.

## Validation

Rust/Cargo 1.91.1, two build jobs and test threads; separate owned AI/server
targets under `tmp/2026-10-08-publication`. Rust commands used `--offline --locked`.
Node 24.19.0 / npm 11.17.0; frontend installed with the committed lockfile.

- Heuristic: 71 tests pass, one pre-existing card-play case ignored. The five
  history regressions cover forks, updates, repeats, rewinds, undo and parameters.
- Beliefs: 18 tests pass, including fact consistency and hidden-card invariance.
- Knowledge: 23 tests pass, including exhaustive count/marginal comparison and
  random-game soundness; three opt-in performance tests remain ignored.
- Tools: the 128-pass frozen synthetic trace and 500-hidden-card-swap decision
  guard pass. The fixture digest remains the one in the exchange checkpoint.
- Workspace all-target clippy with `-D warnings`, formatting and diff checks pass.
- Testbed server: 16 tests and all-target clippy pass against this AI tree.
- Frontend: 8 tests, typecheck and production export pass. Next's CPU setting was
  temporarily limited to two for the build and restored byte-for-byte afterwards.
- Publication inspection confirms no raw private development ancestor, new
  dataset path or player-group literal; ignore rules and sanitized adapter match
  the previously published preview.

The cache checkpoint records the unchanged 32-game decision digest and bounded
timing comparison. Backend Judge review requested the three small reuse changes
above and corrected documentation; the final SHA-specific verdict is a merge
gate, recorded in the PR and handoff after this commit is created.

This is scoped regression and integration evidence, not full playing-strength
qualification. The large tools self-play audit and private human-data lane were
not rerun. Existing ignored behavior/calibration cases, configuration validation,
evaluation limitations and coordinated planning remain in `NEXT_STEPS.md`.

## Resource handling

No containers or persistent servers were started. The publication AI/server
caches and frontend dependencies/export are retained for continued work in the
verified sibling setup. Historical private worktrees and shared datasets remain
untouched. Cleanup of superseded owned build output is reported in the handoff.
Automatic approval review blocked removal of the old `2026-10-08-ai-review/target`
with `blocked by policy`; it remains and reclaimed space is zero.

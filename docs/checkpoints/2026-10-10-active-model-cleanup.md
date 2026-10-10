# Active model cleanup

- Status: verified refactor; 2026-10-10.
- Branch: `fix/live-planning-review`; base `ddc07fb`.
- Tag: `checkpoint/2026-10-10-active-model-cleanup`.
- Paired testbed: `feature/takeback-claim`, checkpoint `2026-10-10-shared-site`.

Removed the unused optional HP1 bid reader, its EGP configuration and helpers.
The active reader remains the sole implementation. Studies use that same reader;
obsolete TOML fields and `--agent-reader` are rejected. Passing reuses the existing
standing-card and top-card helpers. Default decisions retain their semantics.

Validation used one job, two logical CPUs and a 4 GiB process-tree limit:

- 63 affected beliefs/heuristic tests pass: library, ownership inference,
  parameters, bidding and passing.
- Four tools checks pass, including the existing exchange checkpoint, return
  forecast and fresh-versus-reused full-game comparison.
- A paired service comparison preserves fixed-deal snapshots, auction choices
  and all 97 decisions in two complete AI games, including passes and questions.
- Clippy for beliefs/heuristic/tools passes with warnings denied; changed Rust
  files pass formatting. Independent T3 review approves the cleanup.

Evidence is retained in `tmp/2026-10-09-v1/cleanup-*` and
`tmp/2026-10-10-site-cleanup/baseline-native-parity.json`. Fixed comparison seeds:
7, 41, 83, 271828. These are refactor checks, not playing-strength evidence.
Reusable build caches remain; no AI datasets or trained weights changed.

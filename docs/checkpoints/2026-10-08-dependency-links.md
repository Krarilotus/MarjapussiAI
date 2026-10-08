# Checkpoint: default-branch dependency links

- Base: `9563626f189103ee543f550ecfebdcf72dff8c74`
- Tag: `checkpoint/2026-10-08-dependency-links`
- Scope: README links/status and repository metadata; no runtime or lockfile change.

The default README links the included engine's fork and the testbed usage guide,
and points to the continuing `v1-preview` agent. The workspace and its three core
crates use the real GitHub repository URL. Cargo metadata and diff hygiene were
checked; source tests were not repeated for these non-runtime changes.

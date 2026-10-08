# Checkpoint: history and belief-parameter cache validity

- Date: 2026-10-08
- Status: verified for the scoped correction; not a playing-strength qualification
- Base: `8ce5e9490f3ca344e4340e689bbbd83c28d8fb70`
- Local tag: `checkpoint/2026-10-08-history-cache`

An agent reuses its `Mind` only when seat, starting hand, seat kinds and belief
parameters match and the incoming raw event stream extends the complete stored
prefix. Only the new suffix is applied. The stored prefix includes undo events;
`Mind::events()` cannot be the key because it removes retracted events.
Redundant cached seat/kinds/count/last fields are removed. No new identity API,
inference model, strategy, dependency or parameter-validation policy was added.
Comparing a prefix costs O(history length) per synchronization and stores one
raw event vector. Identical inputs safely reuse state even across game objects.

## Validation

Rust/Cargo 1.91.1, two build jobs/test threads, offline locked builds, retained
target `tmp/2026-10-08-ai-review/target`.

- New `heuristic/tests/history.rs`: before the fix, three tests failed (fork,
  parameters, branch after undo); afterwards all five pass. Includes unchanged
  and extended parameter updates, repeated views, rewinds and seat/hand/kind changes.
- Entire heuristic suite: 71 passed, one pre-existing ignored card-play case.
- Frozen synthetic exchange fixture: all 128 decisions/explanations unchanged.
- Workspace all-target clippy with `-D warnings`, formatting and diff checks pass.
- Release profiler: 32 games, three repeats, 1,581 decisions. Complete decision
  hash stays `24fe1990eaea2458`; non-forced mean 289.8 us before, 292.8 us after.
  Shared-machine best-of-three measurements are a bounded regression check, not
  proof of a statistically significant timing difference or server throughput.
- Full workspace/private-data qualification was not run for this small fix.
  Independent publication review is recorded separately at the merge checkpoint.

F4 finite/range validation, missing pass-choice inference and server scheduling
remain separate work. The old standalone F3 probe was removed after promotion
into the regression suite; the still-failing F4 probe remains outside Cargo tests.

No containers or background services were created. Build processes completed;
the existing target cache is retained for publication validation. No scratch
deletion or reclaimed space is claimed for this iteration.

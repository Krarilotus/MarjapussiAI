# Checkpoint: condense pass choice and remove the unused reader

- Date: 2026-10-08
- Status: **verified for the scoped refactor**, not a release or playing-strength qualification
- Tag: `checkpoint/2026-10-08-exchange-cleanup`
- Branch: `planning/ai-review-next-steps`
- Base: `checkpoint/2026-10-08-review-plan`, commit `46231e6ea623e1642b9339739077143fa2c12866`
- Testbed: unchanged; no paired application build claimed

## Result

The original F1 review was incorrect about runtime usage: `PassReading` was
exported but never called by the active `Mind` inference. It inverted the older
HP/HK scores, while C42 chose actual passes. This checkpoint corrects that finding
and removes the unused reader, legacy scorers, contexts, helper machinery and
26 unused configuration fields. Tests solely about those scores were removed.

The active policy now separates:

- `heuristic/src/passing/features.rs`: pure C42 features/weights, explicit partner
  estimates, and the existing trump-plan calculation;
- `heuristic/src/passing.rs`: snapshot the giver's visible belief state, interpret
  its own nominal pair messages, score legal options, select and explain.

There is no new inference algorithm, strategy, weight fit or dependency. Scores
keep their arithmetic order and f64 argmax; ties keep the first supplied legal
option. Sampling keeps its f32 conversion, temperature and seed handling.
Only the selected option's features are retained for explanation; per-option
feature-vector allocations are gone. No latency claim is made.

The active `partner_half_conf` setting moved from convention parameters to
heuristic parameters. Its flat TOML key/default are preserved; removed pass-only
keys now produce the existing unknown-key error. This is an intentional API and
configuration cleanup: historical parameter dumps with those keys must remove
them. Repository and inspected testbed callers were checked; external consumers
of the deleted Rust APIs would need updating.

Independent choice scenarios remain. New cases cover legal-order ties, nominal
pair-message coverage and the plan's evidence threshold/held-pair precedence.
Two tests that merely reconstructed the weighted argmax were replaced by a frozen
before/after trace. `AGENTS.md` now records the owner's dead-code removal rule.
The overview, F1 correction, next steps and [model explanation](../PASSING.md)
describe what actually runs.

## Evidence

Rust/Cargo 1.91.1; offline/locked dependencies; `CARGO_BUILD_JOBS=2` and
`CARGO_TARGET_DIR=Documents/MarjaAI/tmp/2026-10-08-ai-review/target` throughout.
The existing isolated cache was reused for this continuation and the planned
state/config iteration. Test commands use `-- --test-threads=2`.

| Command or review | Result | Scope / limitation |
|---|---|---|
| `cargo test --offline --locked -p marjapussi-heuristic` | 66 passed, 1 existing ignored | Includes nine current passing cases; ignored owner card-play disagreement remains. |
| `cargo test --offline --locked -p marjapussi-beliefs` | 18 passed | Includes random-game fact consistency and hidden-card invariance. |
| `cargo test --offline --locked -p marjapussi-tools --test passing_checkpoint --test privacy_decisions` | 2 passed | 128 baseline pass decisions/explanations match; 500 valid hidden-card swaps preserve decisions. |
| `cargo clippy --offline --locked --workspace --all-targets -j 2 -- -D warnings` | Passed | All workspace targets compile without warnings. |
| `cargo fmt --all -- --check`, `git diff --check` | Passed | Formatting and patch hygiene. |
| Production call-site search | No old pass-reader/scorer callers remain | Includes workspace tools and inspected testbed Rust sources. |
| Independent T3 agent review | Unavailable | Spawned reviewer hit its provider's weekly quota before doing work; no independent review claimed. Parent inspected the diff. |

The synthetic fixture `crates/tools/tests/fixtures/passing-46231e6.txt` was captured
**before production edits**, against base `46231e6`. It runs `eval::deal` seeds
0 through 31, no rotations, with default base parameters except RNG seed 42 and
`sample_temperature` 0 then 1. Each run follows the real auction through both
exchanges: 64 runs, 64 forth passes, 64 back passes. Fresh agents are used per run.
The fixture includes cards, decision labels, total scores and weighted explanation
values; comparison ignores only line-ending conventions. It contains no private
human records.

Fixture SHA-256:
`8279f17efbe0f97ddd71433b1a99798681107126c0eb95543ec834d4d63fc9ce`.

This is bounded regression evidence, not proof for every possible hand/configuration.
No full workspace test pass is claimed. The large tools self-play audit and the
two existing ignored behavioural cases were not run as part of this refactor.

## Remaining work

- Pass-choice inference is still missing. The ignored historical human void-rate
  diagnostic now says so accurately; it does not validate the current policy.
- Fix stale history/configuration reuse and configuration validation (F3/F4).
  Then establish held-out and fixed-size evaluation before adding new inference.
- Keep historical optional bid/EGP models outside this exchange iteration; they
  have explicit configuration/study callers and were not unused pass code.
- Preserve this checkpoint and its predecessor; every next major iteration gets
  a new commit, evidence record and immutable annotated tag.

## Cleanup

No containers were created. Owned test/build processes completed. The failed
reviewer's detached scratch worktree was unregistered and all 1,137,753 bytes of
files were removed (about 1.1 MB). Windows denied removal of its empty directory
`tmp/2026-10-08-exchange-review`; it contains no files. The reusable build cache
(about 1.55 GB) and committed synthetic regression evidence are retained for the
next planned iteration. No shared datasets or other worktrees were deleted.

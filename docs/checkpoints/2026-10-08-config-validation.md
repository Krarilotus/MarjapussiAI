# Checkpoint: configuration validation

- Date: 2026-10-08
- Status: verified for configuration validation and preserved default behaviour
- Tag: `checkpoint/2026-10-08-config-validation`
- Branch: `integration/reviewed-ai`
- Base: `117c1682f44e101d5dab402b0e574b01e9dc32f9` (merged `v1-preview`)
- Companion: testbed `docs/verified-ai-setup`, based on
  `26bcb8d0fae24089e692833922a13e953734fab7`; its matching
  `checkpoint/2026-10-08-config-validation` records this AI commit after creation.

## Result

F4 input-domain validation now follows the existing ownership chain:
`heuristic::Params` → `BeliefParams` → `conventions::Params`. The flat loader
validates the completed configuration, so override order does not affect joint
constraints. Existing `HeuristicError::Params` propagates through the testbed's
existing HTTP 400 path before game creation or seat updates are saved.

Reject nonfinite floats, probability values outside [0, 1], nonpositive likelihood
floors/calibration temperatures, negative sampling temperatures/factors, inverted
belief bands, block-plus-claim masses above one, trick counts above
`engine::TRICKS`, and an enabled endgame with zero worlds. Signed coefficients,
probability endpoints and intentional zeros remain supported. In particular,
`endgame_tricks = 0` disables solving and permits a dormant zero world count.

No model, defaults, fitted weights, dependency or decision logic changed. Public
Rust fields remain mutable: direct callers must validate after changes; infallible
constructors document this contract. Validation is not repeated on each decision.
The old standalone F4 probe is removed in favour of maintained regression tests.

## Evidence

Windows, Rust/Cargo 1.91.1, offline/locked builds, two Cargo jobs and two test
threads. Reused separate AI/testbed targets under `tmp/2026-10-08-publication`.

| Check | Result / scope |
|---|---|
| First five loader regressions before the fix | Four failed, one passed; reproduces invalid domains and joint configurations being accepted. |
| `cargo test --offline --locked -p marjapussi-beliefs -p marjapussi-heuristic -- --test-threads=2` | Beliefs: 20 pass. Heuristic: 77 pass, 1 pre-existing owner scenario ignored. Includes eight new parameter tests and existing history/undo guards. |
| Tools `--test passing_checkpoint` | 128 pass choices and explanations match the frozen pre-cleanup trace. |
| Tools `--test privacy_decisions` | Pass; 500 hidden-card swaps, no changed decisions. |
| AI workspace/all-target clippy, `-D warnings`; `cargo fmt --all -- --check` | Pass. |
| Companion server suite | 3 unit + 14 API tests pass; new API test checks rejected creation and unchanged persisted seats/history after an invalid update. |
| Companion all-target clippy, `-D warnings` | Pass. New test matches existing 120-column formatting; unrelated formatter changes were reverted. |
| Independent T3 review | No material defects in the validation diff; confirmed domains, ownership, override order and trusted Rust caller contract. Final commit review recorded in the handoff. |

Shared synthetic fixture: `crates/tools/tests/fixtures/passing-46231e6.txt`,
SHA-256 `8279f17efbe0f97ddd71433b1a99798681107126c0eb95543ec834d4d63fc9ce`.
Seeds 0–31, temperatures 0 and 1, sampling seed 42: 64 games / 128 exchanges.
No private datasets or player identifiers were used or added.

## Remaining work

This is input-domain validation, not a runtime budget. Very large supported solver
settings can still be expensive; extreme finite coefficients can overflow later
arithmetic. Define measured resource limits with the server-responsiveness work
(F7). The trusted direct-Rust configuration contract is explicit, not enforced by
private fields or a new constructor API. No playing-strength claim is made.

The inherited ignored owner card-choice case remains tracked by the review.
Frontend code/dependencies did not change; its earlier publication checks were
not rerun. Next: review the return-pass planning proposal and qualify evaluation
provenance before new strategy or tuning.

## Cleanup

No containers or persistent servers were created. Reusable AI/testbed build
caches are retained for the next iteration; no cache space reclaimed. The shared
synthetic fixture is reused rather than regenerated. Reviewer worktree disposition
is recorded in the final planning handoff.

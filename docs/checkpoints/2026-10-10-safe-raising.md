# Checkpoint: certified pre-play raising

- Date: 2026-10-10
- Status: verified for the certified route; playing-strength effect unmeasured
- Tag: `checkpoint/2026-10-10-safe-raising`
- Branch: `fix/live-planning-review`
- Predecessor: `9ad2a6e`, `checkpoint/2026-10-10-partner-entry`
- Implementation: `4e7cecd`, with the equivalent continuation guard condensed here
- Paired testbed: `46e00b7`

## Result

The raising phase previously discarded every legal raise. It now chooses the
largest legal five-point step covered by a continuous own-card route: opening
Ace, own no-trump winners, one own pair, then its top trump run. The floor covers
every legal opening Ace. Other players' captured cards are credited only if all
nine tricks are guaranteed; that complete run also includes the last-trick bonus.

The agent reconstructs its raised hand from its own visible exchange/history and
follows the certificate before sampled endgame selection. A lost lead, changed
trump, spent right or consumed half invalidates the route. Existing knowledge,
legality, scoring and cash-order helpers remain the owners of those semantics.
There is no extra cached plan state or new probability threshold.

This is post-exchange raising, not auction bidding. It does not estimate arbitrary
hands, uncertain partner routes, optimal score difference or the 500-point series
bonus. The 16-game runtime sample contained no hand meeting a higher certified
target; its actions matched the partner-entry baseline. Synthetic witnesses
exercise both a partial eight-trick floor and a complete nine-trick route.

## Evidence

| Check | Result |
|---|---|
| `cargo test --offline --locked -p marjapussi-heuristic --lib --tests -- --test-threads=1` | All 92 tests passed after integration, including history/undo, bidding, passing, entries and four raising tests. |
| Raising policy witnesses | 24 complete games: four opening Aces, three opponent layouts, partial/full routes. Fresh agents realize the floor with normal endgame enabled. |
| `cargo clippy --offline --locked -p marjapussi-heuristic --all-targets -- -D warnings` | Passed after condensing identical rejection branches. |
| `cargo run --release --offline --locked -p marjapussi-tools --bin perf -- --games 16 --repeat 1` | 810 decisions; action hash `0d8b47366ca28314`. Raising mean 0.266 ms/max 0.812 ms; trump-or-lead max 30.38 ms; pass-back max 967 ms. |
| Independent Sol source review | No blocking findings; certificate, rights, scoring, public-history reconstruction and policy continuation reviewed. |

Initial checks exposed the obsolete disabled-raising test (removed), stale
cross-worktree Cargo artifacts (affected test artifacts cleared), and identical
lint branches (condensed). These failures are retained in local evidence.
The shared-machine timing sample is not a speedup or strength claim.
Evidence is under `tmp/2026-10-09-v1/safe-raising-*`.

## Cleanup and next work

All jobs exited. Clearing stale test artifacts reclaimed 425.1 MiB before the
affected packages were rebuilt; reusable release/dependency caches remain.
Auction contract assessment and conditional partner routes require separate
evidence and design review. Production replacement waits for active play to end.

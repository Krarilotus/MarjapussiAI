# Checkpoint: legal fact-DP reuse fixture

- Date: 2026-10-11
- Status: **incomplete; corrected fixture not executed by this worker**
- Base: `04b97f721f53fd0036bf36c34283acb9141d0d75`
- Tag: `checkpoint/2026-10-11-fact-dp-fixture-repair`

Root's `tmp/2026-10-09-v1/sequence-fact-dp-unit.log` records successful
compilation and 18 passed, 1 ignored, 1 failed beliefs library tests. The failure
is the new history-reuse fixture, not evidence of a fact-DP reuse defect.

The fixture's final opening card was `e-6` after undoing `g-6`. The undo restores
both green cards in seat 0's original hand. `engine::play_levels` requires an ace
on the first lead if held, otherwise green if held. With no ace and two greens,
`e-6` contradicts that rule: knowledge correctly rejects excluding a certain
green card. The earlier `g-6` is consistent with those holding constraints.
Knowledge's Bid handler only updates bidder/value; it does not enforce the
auction, passing phases or acting seat. The old synthetic bid by seat 2 followed
by seat 0's opening therefore was also insufficient legal history evidence.

The replacement drives every action through engine legality and `agent::Table`,
then applies only seat 0's filtered events to the warm Mind. Two complete phase
prefixes cover all-pass and a seat-0 125 contract. The contract path first bids
130, obtains both required undo acceptances, replaces it with 125, completes
the auction, receives four partner cards and legally returns the same cards.
Both paths play `g-6`, obtain an accepted undo, assert that `e-6` is still illegal,
and choose the distinct legal opening `g-7`. After each action, the test compares
the engine-derived view, cold canonical-history replay, exact marginals, seeded
samples and weights. Existing pointer/epoch assertions still check DP reuse and
invalidation. No production code or constraints changed in this repair.

Formatting and `git diff --check` passed. No build or test was run here. Root can
rerun only the failed test first under its existing resource wrapper:

```text
cargo test -p marjapussi-beliefs --lib deals::tests::fact_dp_reuse_tracks_bid_play_undo_and_changed_history_like_fresh_reads -- --exact --test-threads=1
```

The earlier source checkpoint's profile microfixture and memory-retention
measurement remain pending. No broad benchmark, smoke, push or deployment was
started. No owned build outputs/processes were created; reclaimed space: 0 bytes.

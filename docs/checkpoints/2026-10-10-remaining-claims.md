# Proven remaining-trick claims

- Branch: `fix/live-planning-review`
- Base: `ff57dfc`; proof implementation: `a301e90`
- Tag: `checkpoint/2026-10-10-remaining-claims`
- Status: verified for conservative claims and paired native integration
- Paired testbed implementation: `e456510`; final pair is identified by the two
  claim checkpoint tags and the deployment release record.

`knowledge::remaining_claim` returns a sufficient, seat-visible sequence of own
announcements and winning leads. It combines existing standing-card ranks,
publicly played cards, suit-count bounds and engine legality. A small memoized
search tracks remaining own cards, declared suits, current trump and the next
declaration window. Each lead must win for the claimant, including against the
partner; this prevents other players gaining a declaration window.

The certificate preserves every still-legal own pair, announcing it before
consuming a half. Possible reachable partner or split pairs cause refusal.
Trump draws include earlier leads of that suit before it became trump. The
proof accepts at most nine cards and an empty logical trick; failure is
inconclusive. It is a conservative convenience for humans, not a policy change.

Automatic actions remain ordinary engine actions. Legacy archives identify their
terminal suffix with `claim: {by, from}`, using a raw action-line offset. Replay
validates a claim before invoking analysis callbacks. Provenance survives `/1`
conversion as action `claimed_by`; human evaluation, calibration and decision
searches exclude it while retaining complete playback.

## Evidence

- Twelve focused proof tests passed, including rank gaps, future ruffs, drawing
  trump, own pairs across trump changes, spent rights and invalid inputs.
- The existing 3,000-game knowledge soundness suite passed: 180,596 events across
  four seat views, including 129 accepted certificates replayed through engine
  continuations. Ends of at most three cards enumerate every legal reply;
  larger ends use four representative continuations. Hidden-card swaps passed.
- Knowledge and tools Clippy passed with warnings denied. An equivalent Boolean
  expression in archive validation was simplified after the first tools lint.
- Independent GPT-6.1-Sol source review approved the proof and provenance after
  correcting lost archive metadata and validation before analysis callbacks.
- Four legacy claim tests and the existing game-record test passed, including
  valid post-announcement claims and zero analysis callbacks for malformed ones.
- Nine Python/native checks passed against the release wheel, including live
  claim completion, archive metadata, private views, concurrent actions and
  database reopen. The paired ten server claim/import tests also passed.

Evidence is retained under `tmp/2026-10-09-v1/claim-*`. An initial command named
a nonexistent knowledge `privacy` target; the corrected soundness command passed.
No new playing-strength comparison was performed. Pass-choice intent and a
calibrated probability that a heuristic move is best remain separate work.

# Checkpoint: preserve play horizon through initial binding

- Date: 2026-10-11
- Status: source-only; builds and tests not run (root owns validation)
- Base: `75f2b4324a974cf6982bb3d64640e78f61f96517`
- Tag: `checkpoint/2026-10-11-initial-binding-horizon-source`

Adds MAX_BOUNDARY_DEPTH = MAX_RECONSIDERATIONS + 1 = 2. Program validation and
Reconsider inspection accept this explicit initial preplay binding boundary.
Binding decrements it once and constructs the ordinary current catalogue at
remaining 1, matching live strategic formation. MAX_RECONSIDERATIONS and the
with_reconsideration decorator remain capped at 1; ordinary play depth does not
increase. Canonical identity already includes the count and retained edge.

Root must author the phase-ready boundary explicitly at MAX_BOUNDARY_DEPTH,
then let the binder expand it. Do not pass 2 to with_reconsideration, or decorate
the whole bridge with 1: that decorator deliberately caps existing boundaries
and would erase the separate initial-binding allowance. No evaluator API or
callback change is required; A's generic remaining-count handling applies.

The added source-only regression compares the entire bound alternative list
against live formation's depth-1 catalogue, checks that only depth-1 play
boundaries remain, rejects an ordinary decorator request for depth 2, and
rejects node depths above 2. It also checks distinct canonical identities.
Rustfmt and whitespace checks passed. Actual preplay weighted execution and
root integration still require owner validation. No builds, smoke, benchmarks
or disposable outputs; reclaimed 0 bytes.

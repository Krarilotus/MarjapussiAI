# Checkpoint: restore linear suffix lookup and retain stage-scan reuse

- Date: 2026-10-11
- Status: incomplete; corrective source change, root validation/measurement pending
- Tag: `checkpoint/2026-10-11-linear-suffix-scan`
- Branch: `review/sequence-scenario-gates`
- Worker base: `96970ae720ae4955171ef3c06d34aa6b5c7fbce3`
- Integration base: root `f531d17f409c6a16a8acdffa1b0c68e47bb95018`,
  which records the failed index experiment and shared history-oracle evidence.

## Measured failure motivating correction

The root's fixed two-game profiles each contain 105 decisions and 23 Deferred
attempts. The history baseline is c49a1c6 plus its recorded source manifest;
the indexed run adds the derived-hash suffix index. Suffix inclusive time rose
from 5857.9709 ms to 7956.4865 ms. Actual transitions fell from 192959 to 166751;
represented transitions fell from 708042 to 605239. The indexed version is a
measured regression on this deadline-bounded diagnostic, despite its passing
97-test shared unit run. Repeated full-program hashing is the suspected cost;
the profile does not separately isolate hash traversal from other suffix work.
Neither the smaller scope call count nor unit test runtime establishes a gain.

## Correction and compatibility

Restore the original linear `Program` equality lookup and append-only canonical
cache entries. Retain canonical_program/cached_suffix factoring so find_stage
locates and validates its immutable program once for the full stage scan.
Preserve the first matching stage, lazy suffix compilation and existing exact
program validation. No new index, pointer identity or alternate hash path.

Delete sequence/canonical.rs and its now-obsolete collision test. Remove all
nine derives added solely for the index: Stage, Actions, Guard, Node, Program,
Concept, Literal, ConceptQuery and engine ActionType. Existing engine Hash
implementations remain. ActionType's declaration returns to its prior trait
set; no engine fields, Eq, serialization, layout or rule code changes. Root
still owns the refreshed core anchor for the separate agent/history changes.

The stage-scan test now checks insertion order instead of sorted fingerprints,
and still covers disabled/full/warm/evicted caches and first equivalent stage.
Keep the strengthened cold/warm/off action/value/prepared-support/execution and
represented-work assertions. No catalogue, horizons, worlds, response model,
resource allowances, comparison semantics or certificate ownership changes.

## Validation and remaining work

Source/diff and focused patch applicability checks only. No builds, tests,
benchmarks or games run by this worker. Root must rerun the affected equivalence
tests and remeasure the same diagnostic before any performance claim. The prior
97-pass result covers the removed indexed version, not this correction. The
scope continues to count one find_stage scan rather than each internal stage;
raw call counts across the old baseline are not comparable denominators.

## Cleanup

Removed the superseded production fingerprint module and collision-only test.
No owned jobs, containers or disposable build outputs; reclaimed 0 build bytes.
Shared datasets, caches and historical checkpoints retained; tags unchanged.

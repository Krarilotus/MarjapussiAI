# Lazy sibling-frontier projection preparation

Status: **incomplete overall; focused correctness passed, planning gates open**.
Source C `fb5ad80`, tagged `checkpoint/2026-10-11-lazy-frontier-projection`;
root integration base `0d1d59fa12b33a49ae4c842c753fa81eb56f624b`.
Integration tag: `checkpoint/2026-10-11-lazy-frontier-integration`.

## Evidence and scope

Root reported V2's five focused tests passing, then full library 113 pass / one
failure: original depth-two passed, six-card final-global still exhausted its
unchanged 500k represented-transition cap. Root's matched diagnostic had all 23
attempts Deferred and zero frontier aliases, despite 6849 eager normalization
calls taking 0.680s inclusive / 0.013s exclusive, mainly suffix work. This shared
machine diagnostic is not a standalone speed comparison or a strength result.

This patch changes preparation scheduling only. It preserves the V2 equality
boundary, all available alternatives/worlds/horizons, complete values, per-edge
proofs/receipt ownership, and represented work. `Work::accounting()` remains
`sibling-frontier-dag-v2`; counters/exporter API and all numeric budgets are
unchanged. No B metadata migration is required.

## Lazy boundary

The lexical arena first keeps a pure first-branch Choice or multi-Action probe,
original (origin, stage) entries captured before finish consumes/sorts children,
and completed outcome index/cost. This provisional admission authorizes nothing.
The ordinary completed Local entry already owns the canonical domain.

A later sibling scans representatives in original insertion order. Kind, arity
and ordered action mismatches reject cheaply. Only after a shape match are the
candidate's complete ordered canonical successor suffixes constructed, once for
that scan. Different raw stage ids never reject canonical-equal successors. A
last-successor mismatch rejects the representative without hiding later matches.

Only a complete domain match prepares full entry/read/member projection data.
The representative is reconstructed from the SAME lexical per-origin branch,
including its program, table/history, first/trick/read prefixes and receipt,
changing only the stored entry stage. No terminal state supplies this input.
The unchanged `normalized_frontier` gates run on both complete joint groups;
unchanged `alias_frontier` validates the actual completed Local proof, context,
support, exact prefix and ownership. Every miss preserves later representatives
and ultimately the ordinary evaluation path. There is no negative global cache.

Represented cost is captured strictly around ordinary finish. Successful aliases
alone update alias counters. The existing cache-off/cold/warm work policy remains
the same. Optional cache admission/allocation and wall-time prefixes may change;
those are not a new semantic model or a claim that timed attempts are identical.

Structural saving: siblings with no plausible match no longer construct their
canonical entry, canonical consumed-read suffixes or membership just to register
unused projection metadata. Existing completed Local alternatives screen future
requests without copying another canonical domain on admission. Rare matches
still pay the full unchanged proof gates, plus representative reconstruction.
The matched diagnostic below measures this preparation saving; it does not
establish a completed-planning or playing-strength gain.

## Validation and handoff

Original five V2 tests remain unchanged except registration of a new child module.
Two additional source tests compare a frozen eager scheduling reference against
lazy scheduling for every terminal and every entry/descendant record, represented
transitions/comparisons/solver work and alias diagnostics. They cover both typed
frontiers, late successor mismatch followed by a later match, canonical-equal
distinct raw successors, caches off/on and completed-parent memo admissions.
The second checks that pure probing and shape rejection prepare no canonical
suffix/member cache data, while the final proof gate still does.

E independently reviewed code and tests: no eligibility/projection/accounting
blocker. Formatting and diff whitespace checks passed. Root validation used the
serialized two-CPU, below-normal, 4 GiB runner and optimized test profile. Private
receipt/log folder: `tmp/2026-10-09-v1`.

- `lazy-frontier-policy`: **114 passed**, 94.57 s including compilation, 3.06 s
  tests, peak 681.1 MiB. Both new differential tests and the original five frontier
  tests passed. SHA256
  `1fd9956ae18eb848a5f87997a76ca54acb55b844acaa3a395de7e23efd6621de`.
  Excluded unchanged expensive gates: genuine depth-two auction/return (passed
  in the preceding full run) and six-card final-global (failed there at its
  unchanged 500k represented-transition cap). Their runtime qualification remains
  separate; the latter failure is unresolved.
- `lazy-frontier-profile-build`: passed, 41.01 s, peak 369.3 MiB. SHA256
  `5854bdb754df205dedc4587de7a275be214ea1043217873cc87ae35e0be6872f`.
- `lazy-frontier-profile-two`: 20.93 s, peak 110.4 MiB. SHA256
  `207661a9c6d8463d4eb1bb8dff05b6b7326788101e8d8f60f4e5b9a727b3731d`.
  Same 105 actions/rules; **all 23 attempts Deferred**. Actual transitions
  170,773, represented 245,996; comparisons 8,761/9,299; 13 individual candidates
  completed out of 200, with zero frontier aliases and 28,600 tail aliases.
  Cheap frontier probing took 4.883 ms inclusive / 2.018 ms exclusive over 7,568
  calls. The earlier eager run spent 679.569 ms inclusive / 12.708 ms exclusive
  over 6,849 normalization calls. Suffix construction still took 4.169 s over
  171,185 calls. Deadlines and shared-machine conditions change visited prefixes;
  this demonstrates removed preparation overhead, not a controlled total-speed
  ratio or useful policy completion.

Exact source/profile snapshots remain in private
`tmp/2026-10-11-lazy-frontier-profile`. No 256-game smoke, deployment or strength
claim. Production completion gates remain open.

Owned files: frontier.rs, test-module registration, new lazy tests, this record.
No A/D/E implementation overlap. Validation processes ended; shared build cache,
fixtures and evidence remain for planned reuse. Reclaimed space zero bytes.

# Checkpoint: common preplay continuation bridge source

- Date: 2026-10-11
- Status: incomplete
- Tag: `checkpoint/2026-10-11-preplay-bridge-source`
- Branch: `feature/sequence-preplay-bridge`
- Base commit: `c1aff54541905c529419bb5eb7bf764a4d0fb632`
- Integration owner: executable-concept-contract worktree; no paired testbed change.

## Result

Auction and return candidates can enter one authored continuation through an
exact engine intervention, a finite visible phase gate, and post-exchange
catalogue binding. This source adds `planning/preplay.rs`, migrates the return
comparison call, and supplies four synthetic scenarios. Return candidate
nomination/order is unchanged. A separate focused patch lets the integration
owner connect the existing auction Stop/minimum-bid domain and live response
receipt handling without overwriting its dirty files.

All candidates use the same original `Forecast` cohort. The sequence evaluator
groups equal root information before future choices and integrates terminal
engine score differences. Earlier-phase responses use the strict completed
seat-local response primitive. No independent pair, entry or preservation bonus
is added. A completed selection stores the conditional receipt and prepared
choices; an unfinished comparison leaves the existing choice and records a
deferred assessment. The return explanation drops obsolete legacy-rollout
per-card diagnostics; its score is the actual common assessment.

The bridge permits 16 root response transitions before play. Exhaustion reaches
`Deferred`, never a completed legacy tail. Actor-visible legal play actions gate
formation, including optional raising and following. Initial boundary depth 2
preserves the existing one play-reconsideration opportunity. Deadlines and work
allowances remain the root's existing limits, with outer deadlines respected.

## Evidence

| Check | Result | Limitation |
|---|---|---|
| Read repository workflow, shared T3 guide and reasoning contract | Done | Design/source review |
| Compared nomination functions against root source | Byte-identical | No behavioral run |
| Rustfmt on the three owned Rust files | Passed | Formatting only |
| `git diff --check` and staged whitespace review | Passed | No compiler verdict |
| Synthetic scenarios written | Four | Not run |
| Build, tests, private replay, paired benchmark | Not run, explicitly prohibited in this lane | No correctness/strength/latency qualification |

Scenarios cover bid receipt through return before play binding, exact returned
card consumption and divergent allocation rejection, exhausted gate deferral,
and a joint two-world future choice with retained response reuse and changed
context rejection. The joint-cohort scenario uses a small authored lead domain
to isolate bridge semantics; it does not qualify completion of the full
compositional catalogue.

Shared private evidence remains in place under the parent's original dataset,
externally verified SHA256
`9a1eb1b4e7a5118f6eefb90924c6da60767629f49b5ebfb0a4767777219699b7`.
This lane relies on the parent report's fingerprint; it neither copied nor
rehash-verified nor replayed that dataset. No private identifiers enter source.

## Integration and remaining work

This is a source handoff, not a standalone compilable checkpoint. Shared files
belong to other owners and were deliberately not imported or edited here.
Required definitions are C's catalogue `d7dcd149`, fixture fix `29db18ce`,
operators `75f2b432`, and initial-binding horizon `b5620f16`. Required evaluator
base is A's `5750b041`, followed by its policy-action normalization and grouped
Response/Deferred dispatch. Root supplies `Ctx.completed`, strict
`completed_response_before`, and the extended trace fields.

The grouped response must record the actual singleton Offer, no value ranking,
the full information-group support, exact context and legal membership, and
the entry semantic suffix including consumed guards. `prepared_response`
checks that certificate before advancing the live receipt. Root must apply
the auction/module and live-response integration patches provided separately.
It must retain forced-response advancement and repair when observations or
model context do not match. Return-only configurations can reuse a retained
receipt, but fresh play repair when `plan_worlds == 0` still needs explicit
integration treatment; do not claim that path repaired by this source alone.

Auction/exchange responses before the play boundary are the declared base
model, not a full strategic migration of those phases. Completed auction plans
can therefore carry a modeled return response; new return-allocation comparison
occurs when the return selector is entered. Hidden past exchanges that cannot
be reconstructed remain unsupported. Full-catalogue completion, candidate caps,
cache parity, live arrival/recovery and playing strength require root's bounded
checks before the requested 256-pair benchmark. Remove obsolete legacy rollout
selection after root finishes migrating its callers and tests.

The last verified baseline is unchanged; this incomplete source tag does not
supersede it. Existing root test failures are outside this lane and are not
reported fixed here.

## Cleanup

No builds, background processes, containers or disposable build outputs were
created. Reclaimed space: 0 bytes. Shared fixtures and target caches retained.
Private integration patches/status notes are retained for planned root reuse.

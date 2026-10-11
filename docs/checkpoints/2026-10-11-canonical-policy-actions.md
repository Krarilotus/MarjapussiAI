# Canonical eligible policy action domains

Status: **incomplete; source-only**, based on owned
`5ff466b906da08816979e5a436a2b97de376860a`. Intended annotated tag:
`checkpoint/2026-10-11-canonical-policy-actions-static`.
Root reports combined 77/77 checks for the preceding integrated policy. These
new ordering changes/tests have not been run and are not covered by that result.

## Deliberate policy boundary

`heuristic::legal_actions::eligible` is the sole owner of eligibility and action
ordering for the changed policy. It excludes UndoRequest, copies exact actions,
and sorts ascending by (kind, typed payload, player). It does not add/deduplicate
actions, repair Pass arrays, validate game rules or alter authored programme
alternatives/nominations. Engine and agent::Table are unchanged; frozen previous
policy copies retain their original behavior.

Kind order is Start, StopBidding, NewBid, Pass, AnnounceTrump, Question, Answer,
CardPlayed, UndoDecline, UndoAccept. Bids ascend numerically; suits use engine
indices Green/Acorns/Bells/Red. Question order is Yours then YourHalf by suit;
answer order is NoPair, YesPair by suit, NoHalf by suit, YesHalf by suit. Cards
ascend by card index. Pass combinations use lexicographic card-index arrays;
each array stays exactly as supplied by the engine. Player index is the final
key. New/remaining action variants require an explicit match arm.

This deliberately changes tie breaks previously driven by shuffled hand position
or phase enumeration (including descending raises and Red-first half questions).
It also fixes sampled-selection index/interval order to that canonical domain.
Explicit authored alternatives and candidate nominations are not reordered;
ties among those still follow their existing order. It is a policy semantic
change, not a promise that all old actions or playing strength stay unchanged.
The caller's response-model version/frozen revision must identify the new policy;
old private completed caches must not be carried across code revisions.

## Exact keys and modeled actor boundaries

The live heuristic boundary (decide_scoped in root; decide in this older isolated
lib) calls the same helper before forced actions, primitive selection, retry,
retained and strategic planning. Sequence Membership constructors and internal
cache storage normalize legal inputs before authored binding. Structural
inspection, prepared lookup, root cohort validation, Reconsider binding/install,
Response grouping/callbacks, branch keys and tail keys use canonical ordered
vectors. All comparisons remain exact vector equality, not set equality.
Every modeled actor's strict response callback receives that same eligible order.

Remaining raw Table::to_act reads only obtain the actor, classify PlayPhase from
legal action kinds, or verify exact engine action membership in apply. A fixed
Prepared record and choose_bound pass raw lists to the Membership helper, which
canonicalizes them. PlayPhase classification is independent of valid enumeration
order within an engine phase. No actual hidden peer store is accessed.

The engine already emits Pass arrays in strict descending Card order and checks
that order in is_legal_pass. Only the enumeration of combinations changes with
hand storage. We preserve each legal action exactly, so raw legal.contains checks
in Receipt/preplay/live pending handling remain valid. This is why no equivalent-
set application bridge or relaxed action equality was added. E owns pending legal
domain/retry and boundary coverage, calling this helper; no E files are changed.

## Source-only integration and evidence

Before editing, dirty root lib.rs and sequence.rs were copied verbatim with SHA256
and root HEAD into the owned worktree's tmp/canonical-action-order/snapshot.
The evaluator baseline was identical to the owned source. A focused patch against
those snapshots is supplied for root integration; it preserves unrelated dirty
root lib edits. A newer root hard-query optimization landed during this task;
the focused patch was rebased against a second snapshot and preserves its
tracked-Knowledge proof kernel/test module. The isolated branch retains its
older proof implementation. Use the focused patch rather than replacing root
lib/sequence wholesale from this older worktree.

New source tests cover explicit ordering/idempotence/permutations, domain
multiplicity, exact Pass arrays, and a shuffled live Table versus Table::from_view
reconstruction. The latter asserts equal actor Information but different raw
legal enumeration, then equal canonical domains, completed values/actions,
receipts/traces/Prepared records, reuse accounting, positive Prepared lookup from
both raw domains, and primitive live heuristic parity. A combined nondyadic
cohort checks grouping accepts the different raw enumerations. A return-pass
fixture compares complete canonical catalogues and exact engine-valid actions.
The old membership test now expects reordering to reuse a canonical membership,
while changed private information still misses. Tests are outside production src
and reuse the existing public Table/sequence fixtures.

All new/changed tests are **unexecuted**. Only source/caller inspection, rustfmt
parsing/formatting, focused patch applicability and diff whitespace checks ran.
No build/test/benchmark/process was started and no root source was edited.
No disposable resources created; 0 bytes reclaimed. Root owns compilation,
semantic/privacy checks and profiling of the added sorting/copying cost. No
strength, latency or release-readiness claim is made.

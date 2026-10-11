# Checkpoint: derive canonical-program fingerprints from the model

- Date: 2026-10-11
- Status: incomplete; source checks only, shared-runner validation pending
- Tag: `checkpoint/2026-10-11-derived-suffix-hash`
- Branch: `review/sequence-scenario-gates`
- Base: `b0af7ff57adfbf09fe1a06b1500d2038e443c67a`
- Integration base: root `c1a937036c4a5d4d611739e5bba1f4bd081276e1`
  with frozen current files recorded in the private patch manifest.

## Result and compatibility

Replace the manual typed fingerprint traversal with `program.hash(&mut hash)`.
Derive Hash on Program, Stage, Actions, Guard, Node, Concept, Literal and
ConceptQuery so the model declaration owns traversal. The bounded sorted index
and full Program equality check for every collision candidate remain unchanged.
Fingerprints are private lookup hints, never stable semantic or model IDs.

The owner explicitly authorized the only required engine addition: Hash in the
ActionType derive at crates/engine/src/game/gameevent.rs. CardSet, Card, Suit,
QuestionType and AnswerType already implement Hash; their declarations remain
unchanged. This additive trait implementation changes no fields, layout,
PartialEq/Eq, Serialize, discriminants, legal actions, transitions or scoring.
It adds no new baseline call site. The reviewed core source anchor must include
this exact one-line trait addition when refreshed by the root owner. This is a
source compatibility argument, not a claim of completed regression validation.

No certificate, catalogue, proof rule, resource limit, response policy, cache
scope, collision equality, ordering or represented-work changes. The previous
manual hash representation is entirely removed, with no dormant alternative.

## Evidence and remaining work

- Inspected all required field types and the root source snapshot.
- The revised integration patch contains the original bounded suffix index,
  collision/eviction tests, cold/warm/off equivalence assertions and this DRY
  correction. Apply it instead of the earlier manual-hash patch.
- No builds, tests, benchmarks or games run. Root must run the previously
  specified cache/effort/model-scope tests and merged certificate checks, then
  measure runtime and useful completion. No performance gain is claimed.
- Previous checkpoint tags remain immutable. The worker's older full checkout
  is not the integration baseline; use the focused patch against current root.

## Cleanup

No owned processes, containers or disposable build outputs. Reclaimed 0 bytes;
shared data/caches and frozen integration evidence retained.

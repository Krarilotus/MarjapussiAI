# Sequence copies and exact dependency reuse

Status: **incomplete; static handoff, not runtime-qualified**.

Base: `11386b63dcd62a03e80b8f910f6a75df8e1d77bb`, followed by C's
`d06f2bca1146cff45319f59166fd27c9de839df9` logical-budget fix, cherry-picked
locally as `b2d9436`. Intended immutable tag:
`checkpoint/2026-10-11-sequence-copy-reuse-static`.

## Owned changes

- `crates/heuristic/src/planning/sequence.rs`: compare borrowed terminal slices,
  copying only terminal outcomes/weights into the existing ValueStore input.
  No full terminal trace copy solely for the outer score comparison.
- Cache exact immutable program validation and demanded canonical suffixes
  within one comparison. Compile stages lazily. Alternative ordering remains
  semantic; changed/invalid programs cannot reuse another input's validation.
- Cache authored membership by full root Information plus ordered legal actions.
  Cache exhaustion falls back to original computation; it does not remove a
  candidate or turn unknown facts into proof.
- Preflight suffix memo entry/byte admission from borrowed outputs before
  copying terminal/prepared payloads. Charge C's represented logical work before
  copying any suffix/tail hit. Hits do not clone private lookup keys.
- Store one program per suffix memo cohort, rather than a program per world.
  Each cohort derives from one candidate; recursive children change stage,
  accepted history and pending intent, never immutable program data.
- Construct the initial receipt once per candidate, then clone it into worlds,
  rather than validating the same program separately for every world.
- `crates/heuristic/src/concepts/sequence.rs`: six-line crate-private
  `suffix_validated` helper. Public suffix construction still validates inputs.

All new retained cache allocations enter the existing shared byte ledger and
entry bounds. This ledger bounds retained cache data, **not** transient recursive
outputs, caller response state or peak process memory. New optional caches can
change admission/fallback under a tight memory or wall-time limit; completed
comparisons retain the same ordered candidate/value semantics. No response model,
world weights, candidate bindings, public API structs or serialization shape
changed. No adaptive future side-card choice was introduced.

## Validation and remaining work

Rustfmt parsing/formatting and diff whitespace checks passed. Rust compilation,
unit tests, profiling and benchmarks were **not run**, per owner instructions.
The integration branch owns the ValueStore module and wiring; this isolated
worktree is not presented as independently buildable or qualified.

Three inline source regressions cover canonical ordering and changed-invalid
input; exact private-view/ordered-legal membership identity; and zero-byte
dependency cache fallback without skipped validation or response calls. These
tests are unexecuted. Existing C cached/uncached logical-budget tests are retained
unchanged; the root owns its fixture corrections and subsequent validation.

Global conditional comparison coverage/warm gating remains root-owned.
Catalogue fanout and expensive response leaves remain; this iteration does not
claim that a complete repair fits 50ms, or establish any playing-strength result.
No production builds/processes were started, and no disposable build resources
were created or removed (0 bytes reclaimed).

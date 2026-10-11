# Reachable-suffix completion enclosure

- Date: 2026-10-11
- Status: **incomplete**; source review only, builds/tests unrun here.
- Tag: `checkpoint/2026-10-11-completion-dominance-suffix`
- Branch: `feature/completion-dominance-suffix`
- Base: `043c3ff60887782bd98c1b0b9510dfe1522e5be7`,
  `checkpoint/2026-10-11-local-completion-dominance-integration`.

## Result

Only Local dominance eligibility changes. Candidate 0 admits its existing legal
exact-card-to-literal-Tail forms plus literal Tail. Competitors admit validated
reachable DAGs containing Tail, exact card/Cards/Standing successors, Choice,
and Guard. Every Choice entry and yes/no/unknown arm is inspected without querying
facts or binding future actions. Unsupported operators, malformed references and
cycles refuse admission. Full Program validation is reused, then only reachable
operators are enclosed; valid unreachable gates do not alter the compared suffix.

Accepted sinks are actual Tail nodes. Existing empty-binding Interrupted -> Tail
execution is therefore enclosed, without inventing an offer, losing receipt or
successful authored action. This encloses successful legal forward terminal
completions; it proves neither existence nor model totality, future guard/binding
results, or budget passage. Tail's existing legal response policy is unchanged.

No changes to catalogue, horizons, budgets, original ordered domain, support,
SelectedPlan, dispatch, choose_bound, certify_local, numeric arithmetic, cache
keys, proof charging or capture APIs. The selected route must still really
complete; errors propagate, competitors remain bounds only, and parent completion
is never fabricated. Static scans are bounded by existing Program structural caps
and use one temporary node copy plus visited/pending buffers per suffix. No new
cache or performance claim.

## Evidence

- E reviewed the production helper and six source tests: no blocker. Confirmed
  full validation, all-arm traversal, actual owning Tail, incumbent restriction,
  current legal root Offer check, and unchanged dispatch/proof construction.
- Six new tests cover shared/all-arm traversal, unsupported/malformed/cyclic
  refusal, incumbent restrictions, actual literal-Tail Local completion with
  guarded/Choice/Standing competitors, and real empty-binding finish evidence
  (empty Binding, subsequent ResponseLeaf, Interrupted, original support).
- Owned Rust files: rustfmt parser/format check and `git diff --check` only.
- No builds, tests, benchmark, smoke or deployment by this thread.
- Reused base evidence reported by root: 139 policy, 18 selfplay, 13 graph PASS;
  two known-heavy tests excluded; Clippy only baseline warnings. This is base
  evidence, not validation of this extension.

## Remaining validation

Root runs affected tests serially under its shared resource wrapper, beginning
with `cargo test -p marjapussi-heuristic --lib suffix_enclosure -- --test-threads=1`,
then existing `sequence_dominance` tests. A separately owns the authentic unchanged
composed final_cash descendant fixture plus its additive test-only finish probe.
That fixture must establish the actual full receipt, original joint support and
completed selected Tail without minting Parent/Global authorization. Its source
and test results are separate; this basic fixture is no substitute. No claim that
the full six-card catalogue, enclosing root comparison or costly gates finish.

## Cleanup

No owned running jobs, containers or disposable build outputs. Reclaimed 0 bytes;
shared caches and verified fixtures retained. New isolated worktree retained for
root integration and validation.

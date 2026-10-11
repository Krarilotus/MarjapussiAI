# Frontier work capture companion ? incomplete

Base: root `0858a491152a6899c091c33401f210dbd8db2eb0` plus its current tools capture.
Owner: B tools/docs companion; root owns integrated commit/tag and validation.
Apply only after C's frozen V2 Work fields/charging implementation.

Exports `Work::accounting()` unchanged as the runtime label producer and the four
frozen frontier alias diagnostics separately from Tail aliases and actual/represented
solver effort. The graph inspector describes the recorded accounting version; it
preserves V1 labels/semantics and leaves absent/unknown versions explicit. No
operational concept nodes or probabilities are introduced.

Existing serializer test is extended for V2 integer/cost lane preservation. Existing
graph test checks V1, V2, absent and unknown version interpretation without changing
semantic nodes or edges. Synthetic source fixtures are structural tests, not live
policy evidence. The companion does not prove evaluator projection preservation or
playing strength; those remain C's/core validation responsibility.

Validation: static parser, patch applicability and whitespace checks only. No Rust
builds, tests, benchmarks or generated HTML. Tests unrun; checkpoint incomplete.
Root tools/graph 39 PASS and 105 unit PASS were prior evidence before this patch,
not qualification of this companion. No owned jobs/containers or disposable build
outputs; reclaimed 0 bytes. Scratch patch/baseline retained for owner integration.

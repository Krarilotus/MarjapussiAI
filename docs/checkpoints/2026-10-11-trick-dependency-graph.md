# Shared trick-dependency graph

Status: **review-only**. Base: `109a46d83699aa171e2f0c98c7f4b0a70f69c464`.
Tag: `checkpoint/2026-10-11-trick-dependency-graph`.

The portable review uses a dependency DAG with one visible node per definition.
Graphviz computes offline geometry; the page has no external runtime dependency.
Focus, expansion, shared consumers and full-graph inspection operate on the same
identities. A definition may still instantiate distinct runtime queries when its
relevant inputs or hypothetical context differ.

The proposed strategic model follows joint trick outcomes through the winner,
points, spent resources, observations, next-leader opportunities and subsequent
tricks. Pair activation is one possible continuation. Actor-specific responses,
information-set constraints and candidate-bound joint mass remain explicit.
Known scoring rules are separate from conditionally exact terminal scores and
belief-based expected score difference. Each node describes its result's scope;
the diagram does not assert that proposed results have already been evaluated.

## Validation

- Layout validation: 113 proposed projections and 9 current-holding projections;
  unique IDs, exact induced dependency edges, no overlapping node boxes.
- Full browser graph: 31 unique proposed definitions and all 70 direct edges.
- Browser focus, expansion, back and current-view selection passed. The inspected
  trick projection had no text outside its node boxes. JavaScript syntax passed.
- Reviewed the rendered overview and full graph in the T3 preview. Old preview
  initialization timed out; a new hidden preview tab loaded successfully. The
  preview reported Electron preload diagnostics unrelated to this page; page
  rendering and interactions worked.
- Independent semantic review corrected response perspective binding, candidate
  probability identity, single application of branch weights and certainty labels.
- `git diff --check` passed. No game runtime or benchmark was executed for this
  documentation change.

The private worked example remains outside Git. The portable bundle is delivered
to Khaleesi's Downloads folder with a manifest for content verification. Production
selection, beliefs and deployment are unchanged by this checkpoint. The owner has
authorized continuing into an executable estimate-store migration next.

The preview process is owned by this task and will be stopped after review. No
containers or build outputs were created; reusable review inputs are retained.

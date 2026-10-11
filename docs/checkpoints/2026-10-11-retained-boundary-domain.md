# Retained boundary coverage and proposal domains

Status: incomplete source checkpoint. No build, test, replay or benchmark was run by this agent.

Base source checkpoint: `e975f3b03aa4c1cc6f8892b133b91e375262d3bb`, with iteration-owned files copied from the root owner's current dirty snapshot. Apply the focused patches against that snapshot, not a blind cherry-pick of this worktree checkpoint.

## Corrections

- At a reached reconsideration boundary, live coverage checks now require the actual bound alternatives with their remaining horizon, including the retained child. They no longer demand unrelated undecorated alternatives. The existing checks for a completed assessment, actual executed alternatives, exact information/context and conditional support remain.
- Retained proposals store their complete original eligible legal domain. Retry requires domain equality through the shared canonical eligibility helper, in addition to accepted information and action legality. Newly proposed prepared/forced moves replace the domain; accepted moves and deferred invalidation clear it. Common installation records the completed comparison's canonical root legal domain, including when the selected program is a response tail with no choice certificate.
- Legal ordering belongs to the evaluator owner's `legal_actions::eligible` integration. This change neither reorders authored alternatives nor rewrites Pass payloads. Engine-generated Pass arrays already have the required strict descending order, so raw action membership remains valid.

## Scenarios and validation

The new integration scenario evaluates the unchanged auction and return bridge at depth two on two genuinely different opponent allocations with original weights 0.4 and 0.6. A maximum legal bid avoids unrelated raise-domain breadth. Both partnership hands and exchanges match. The finite authored continuation must improve upon the fixture's response tail; live reuse must then publish `Prepared` with the same jointly assessed boundary and no evaluation work. The test does not replace `Reconsider`, resample the cohort or inject a selected certificate. Its larger finite test-only evaluator allowance is not a production policy or budget change.

Additional scenarios require original-domain equality for retry, reject both expansion and contraction while the old action stays legal, accept equivalent eligible ordering, and check clearing/replacement after acceptance and a forced proposal.

Source validation: formatting and whitespace checks, staged-scope inspection, and focused-patch applicability against the frozen root snapshots. Runtime validation of these new scenarios remains pending with the root's serialized job queue. The root reported the preceding common-leaf unit suite at 77/77; that result predates these fixes. Root also reported two separate contract-planning integration failures (completion within the formation allowance, and privacy comparison including runtime work counters); neither is claimed resolved here.

Hidden exchange reconstruction, hard latency control and playing strength remain separate open work. No private data was copied or committed. No disposable build outputs, containers or background jobs were created; reclaimed space: 0 bytes. Keep private source snapshots until integration is complete.

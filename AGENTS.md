# Continuing work

These instructions apply throughout this repository.

- Read `docs/PROJECT_OVERVIEW.md`, `docs/NEXT_STEPS.md`, and `docs/WORKFLOW.md` before a major implementation iteration.
- After **every major feature or code-refactor iteration**, create a focused commit, a record under `docs/checkpoints/`, and an annotated `checkpoint/YYYY-MM-DD-<iteration>` tag. Record actual validation and unresolved failures. Never label a checkpoint verified when required checks failed or did not run.
- Keep existing checkpoint tags immutable. Start a new branch from a checkpoint for recovery; do not reset or overwrite another person's work.
- Commit only files owned by the iteration. Keep datasets, player identifiers, local configuration, and generated build output out of commits. Inspect the staged diff.
- Keep rules in `engine`, visible input in `observation`, exact deductions in `knowledge`, uncertain inference in `beliefs`, and decisions in `heuristic`/`solver`.
- Behaviour changes need scenario explanations and owner review; human agreement alone is not proof of playing strength. Refactors must preserve behaviour with appropriate comparisons.
- Use the `t3-agents` MCP server for subagents. Give each a separate worktree; inspect INPUT questions neutrally before answering.
- Limit builds to two jobs and use a target directory owned by this iteration. Reuse verified benchmark fixtures across reviewers; record their digest and provenance without committing private data.
- Clean owned disposable build outputs and stop owned processes after final use; retain shared data and caches needed for planned reuse. Report reclaimed space.
- Creating checkpoints is local. Publishing, pushing, or merging is a separate action and is not implied by this workflow.

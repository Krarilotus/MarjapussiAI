# Conditional concepts and viewer knowledge: source checkpoint

Status: **incomplete — executable validation pending**.
Base: `b3abdaf1f5e47111904db442ce36db7dc4ae9e0b`.
Tag: `checkpoint/2026-10-11-conditional-concepts-source`.

Knowledge counts equivalent allocations and partitions a viewer's private hand
before aggregating opponent arrangements. Beliefs evaluates joint card, void,
pair and count queries, preserving correlation and distinguishing hard proof,
truth under a condition, model probability and unresolved mass. Perspective
queries use public/shared facts or possible viewer-private information. Proof
includes hard-consistent worlds with zero model weight.

Policy consumers use completed joint estimates for pair-loss exposure,
at-least-one partner pair and void-plus-winning-trump events. Unfinished queries
retain the previous estimate. A shared per-decision allowance and local memo
bound repeated traversals. Return selection can nominate one missing combination
of a partner-known pair and retained standing card; engine-scored continuations
still decide the choice.

Forecast comparisons reuse completed world/action outcomes and decisions with
identical full seat views, histories, legal actions and table metadata under
fixed parameters. Timed endgame decisions are excluded from decision reuse.
This removes the duplicate baseline replay in raising comparisons.

Independent source review found no blocking proof/privacy defect. It prompted
viewer-first grouping, query/cache limits, conservative allowance charging on
errors, a fresh-actor memo regression and explicit historical-core review.
Formatting and whitespace checks passed. Tests are present for allocation
multiplicities, independent small-state enumeration, conditional correlations,
zero-weight proof counterexamples, visible/hidden passes, consumption and undo,
nested private-hand mixtures, budget handling and forecast reuse.

Compilation, test execution and performance/strength qualification are pending
the immutable b3abdaf-versus-1632af8 4,000-game run. No new performance or playing
strength claim is made. Full-game forecasts still use weighted deal simulations;
the exact queries cover holding predicates under the current belief model.
Unobserved viewer-private exchange trajectories and pass-choice intent remain
unmodelled. Node budgets exclude initial/conditional fact-DP construction and
exchange-kernel work, so runtime must be measured under the shared job limits.

Existing engine, observation, agent and solver sources are unchanged. Knowledge
adds helpers and broadens internal visibility; existing algorithms are unchanged.
Historical policies use neither new helper. Runtime adapter compatibility still
requires the focused validation and exact-source guard.

No production files or Docker resources changed. Active benchmark evidence,
review worktrees and reusable caches remain available for validation.

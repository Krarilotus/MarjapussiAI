# Common typed leaf integration — source checkpoint

Status: incomplete validation; no builds or tests run in this worktree.

Follow-up to the common-path source checkpoint. After the evaluator owner froze the single typed callback contract, the common `Forecast::evaluate_programs` adapter now calls `completed_response_with_effort_before` once, passing the remaining solver-node allowance. Both successful and failed responses preserve their reported canonical solver nodes. Deadline expiry after a successful call also preserves spent effort. No default solver-node cap is introduced; ordinary candidate policies and planning allowances are unchanged.

The preplay fixture callback now returns `LeafResponse::unit` and supplies the new budget field. The existing reused-versus-fresh actor scenario also compares represented solver nodes. These changes require the evaluator owner's typed `LeafResponse`, `LeafFailure`, `LeafBudget` and `Work` checkpoint; those shared definitions were not edited here.

Formatting, whitespace and focused-patch applicability are source checks only. Runtime behavior and the revised post-exchange fixture remain unverified here. Root owns serialized validation and all other external callback migrations. Canonical solver nodes measure solver effort, not total belief/valuation work or hard deadline compliance.

No private data or generated build outputs are committed. No owned disposable outputs or containers were created; reclaimed space remains 0 bytes.

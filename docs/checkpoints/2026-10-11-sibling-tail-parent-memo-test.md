# Completed parent memo containing sibling aliases

Status: **incomplete; source-only test follow-up**, base `0153814`.
Tag: `checkpoint/2026-10-11-sibling-tail-parent-memo-test`.

One focused test compares two canonically equal parent candidates (different
cosmetic names), each with four same-tail alternatives, on the existing two-world
fixture. It compares optional cache disabled/enabled under the exact represented
transition/comparison/solver allowance established by the uncached run.

The test requires an observed completed parent memo hit: cached execution has two
actual value comparisons versus three uncached, exactly one suffix hit, and three
actual sibling aliases versus six uncached. A tail/response hit cannot remove a
parent's value comparison or mandatory alias construction. The full values,
executions, receipts, prepared records, selected support/context/order and all
represented counters must agree. Both original parents remain fully assessed.

No production, fixture, budget or tools changes. Validation: rustfmt parsing and
format check, staged whitespace review, focused patch applicability check only.
No builds or tests executed; root's running lane is undisturbed. Root can run
`cargo test -p marjapussi-heuristic --lib
sibling_tail_completed_parent_memo -- --test-threads=1` in its shared constrained
runner after integration. No performance/completion claim. No build outputs or
processes created; reclaimed space zero bytes.

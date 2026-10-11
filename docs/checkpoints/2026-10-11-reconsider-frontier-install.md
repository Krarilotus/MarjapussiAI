# Checkpoint: direct reached-boundary installation

- Date: 2026-10-11
- Status: source-only; builds and tests not run (root owns validation)
- Base: `c6180f13b16060fc75a7d8a72c99fb9122ef6e00`
- Tag: `checkpoint/2026-10-11-reconsider-frontier-install-source`
- Owned files: concept sequence module, independent concept tests, this record

`Receipt::install_reconsideration_at_frontier(binding, view, legal)` is
crate-private. It requires the receipt cursor itself to identify Reconsider,
checks accepted/current information, pending state, legal domain, source suffix,
and valid composed Choice data before installing. It does not inspect guards,
construct Mind, select an alternative, or accept an action. The evaluator must
already have consumed guards and set the actual frontier cursor.

The public Mind-taking method keeps guard inspection and delegates at the
reached stage. Failure restores its previous cursor; no failed installation
changes the receipt. The new regression compares both successful paths and
checks direct guard rejection, stale source/domain/information rejection, and
atomic public/direct failure. Rustfmt and whitespace checks passed. Regression
execution remains for root after evaluator integration; no runtime or performance
claim is made.

Root may run the existing concept unit filter after integrating A's exhaustive
operator handling:
`cargo test -p marjapussi-heuristic --lib concepts::sequence::tests:: -- --test-threads=1`

No builds, benchmarks, smoke runs or disposable outputs; reclaimed 0 bytes.

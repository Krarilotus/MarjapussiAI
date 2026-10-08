# Checkpoint: publication-safe continuation base

- Status: incomplete until final integration validation
- Base: published `v1-preview` at `1c077aedb47218f822597f2b6fe9e37edfc41abb`
- Tag: `checkpoint/2026-10-08-publication-base`

Import the existing continuation at local `46231e6` as a tree delta, including
the newer bidding/pass policy and review workflow. Keep the published ignore
rules, environment-based player grouping, README, architecture/concept documents
and graphics. No private development ancestry is merged or rewritten.

The next commits replay the already checkpointed exchange cleanup and history
fix. Local source hashes in their evidence records identify provenance; published
checkpoint tags use a `published-` suffix and never move the original local tags.
The final publication record documents validation of the resulting AI/testbed pair.

This import adds no new policy beyond the existing continuation. Patch hygiene
was checked; final build, privacy and regression gates remain pending here.

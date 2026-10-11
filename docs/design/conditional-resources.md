# Bounded conditional announcement resources

Strategy owner: the existing Sol thread, in `feature/conditional-resources`.
Base: `8ca35c9`. Root owns validation, performance instrumentation and checkpointing.

This increment extends the current `trump_plan::Reach` owner, rather than adding
a graph runtime or a private remembered plan. Each uncalled suit has explicit
alternative activations: the pair owner leads and announces; the other player
leads and asks Yours; or either holder of a split pair leads and asks its half.
The respondent needs neither the lead nor its own announcing right. Estimates
remain conditional on the pair's ownership, and separate our future control
from partner control. Alternative routes are not added as independent events.

`valuation.rs` uses these alternatives for keeping halves, losing rights and
gaining another lead. `trump_plan.rs` connects a legal next lead with surviving
halves, the next eligible actor's rights and enough remaining cards to act.
Known retained leads and partner-entry winners use existing Knowledge proofs;
uncertain entry value continues to use the solver's joint activation result.
The route is reconstructed from the current actor-filtered view at each choice,
so live play and Forecast's actor-local policies use the same logic. Existing
raising certificates retain priority. Full forecast outcomes still decide
uncertain alternatives; no added strategic score bonus or blanket question ban.

Return shortlisting gains a bounded set of combined pair/control allocations,
ordered by the already-computed C42 scores. These are extra legal candidates,
not a claim of exhaustive pass coverage. First-pass continuation search is a
separate increment; its present feature-based policy is not silently replaced.

Owned paths: `crates/heuristic/src/trump_plan.rs`, `valuation.rs`,
`passing/forecast.rs`, and their focused source/scenario tests. A narrow shared
planning consumer is permitted if required; root's tiny profiling inserts in
overlapping files will be reconciled by root. No solver, engine, knowledge,
belief model, runner or performance-default changes.

Source-only regressions will cover actor-specific rights/control, respondent
rights independence, pair survival across certified deferral, a spent-right
partner entry, consumed entry invalidation, combined return candidates and
deduplication. Existing safe deferrals and certified raises are controls.
Root will run tests and diagnostic dry runs on the selected games; those games
are not asserted to be must-win or to prove strength.

Deferred: durable multi-stage uncertain commitments, pass-intent inference,
full information-set planning, exhaustive declaration-order search and joint
long-horizon control probabilities. Future-control quantities remain model
estimates, not probabilities proved by the hard Knowledge layer.

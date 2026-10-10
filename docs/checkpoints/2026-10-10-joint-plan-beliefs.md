# Joint bidding evidence for planning

- Status: incomplete release qualification; correctness checks below pass.
- Branch: `fix/contract-dependencies`.
- Base: `1632af833199e5253271d493af8531badfac81c0`.
- Tag: `checkpoint/2026-10-10-joint-plan-beliefs`.

## Result

Planning previously multiplied believed/factual single-card marginal ratios.
This discarded the dependence between both halves of a signalled pair. Five
private development positions, each with 100,000 uniformly sampled deals,
showed disagreement between the feature model and the distribution used by
planning. In two representative positions, pair probabilities changed from
72.49% to 66.84% and from 77.25% to 67.69% at that handoff.

`Mind::deal_weights` now reuses the feature model's dealt-hand bid likelihoods.
Known exchanges and plays map cards back to their dealt owner; hidden exchanges
reuse its existing kernel and memoized likelihood calculation. Optional card
factors apply once. No convention confidence, bidding threshold, point value or
record-specific policy was changed. Sampled estimates retain sampling error;
the hidden-exchange kernel remains an approximation.

The concept catalogue specifies the remaining conditional continuation chain:
exchange, resulting holdings, legal entry, next leader, surviving pair and
rights, trump change, later tricks and terminal score. This is a design contract,
not an implemented full-game planner. The owner explicitly prioritizes those
dependencies over tuning isolated probabilities.

## Evidence

- 113 beliefs/heuristic tests pass, including three joint-weight regressions,
  existing privacy/undo checks, bidding, passing, play and certified raising.
- Exact enumeration of bounded late-game deal spaces agrees with the feature
  model for Aces and pairs, for both visible and hidden exchanges. The test
  includes uncertain pairs rather than only settled facts.
- The owner-style big-pair scenario preserves impossible Red at zero and agrees
  with the joint feature model under fixed-seed sampling.
- The same private audit after the correction gives 72.53% and 78.08% in the two
  representative positions. These are sample estimates, not calibrated truths.
- Clippy for beliefs tests with warnings denied passes. Formatting initially
  reported only layout changes; `cargo fmt` applied them.
- T3 independent review could not start: the existing relay was unavailable.
  Direct source inspection and automated checks were used instead.

Private dataset SHA-256:
`e41a69d6f81819dc3619655c9a35ca39c906a8ebe647e3aac869bd65b8823865`.
Only owner-selected players supplied examples. These are development records,
not a held-out strength benchmark. Raw data, identities, sample seeds and audit
outputs remain in the private work area. Validation logs are named
`joint-deal-weights-tests`, `joint-beliefs-and-heuristic-checks`,
`joint-pair-model-audit`, `joint-pair-model-fixed`, and `joint-weights-clippy`.

## Remaining work

Do not promote this checkpoint as a completed bidding/planning upgrade. Qualify
latency and changed decisions before including it in the live AI release. Small
uniform samples can still miss important hypotheses. Full continuation selection
for bidding, announcement order and probabilistic raising remains unimplemented;
pass-choice intent is also absent. Current certified raising remains separate.
The production update branch stays at the previous qualified checkpoint.

## Cleanup

No containers were created for this AI iteration. Validation processes exited;
build caches and private audit evidence are retained for the next qualification.

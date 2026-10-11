# Conditional private histories and cooperative preparation

Status: **incomplete qualification**. Base `c1a9370`; intended tag
`checkpoint/2026-10-11-private-history-integration`.

Forecast preparation retains the existing ordered allocation draws, weighting,
resampling and RNG sequence. It checks interruption between those operations and
publishes only a complete cohort. Typed errors distinguish incomplete work from
invalid support; both production callers retain that distinction.

A defender's redacted exchanges can now be completed with explicit hypothetical
transfers. The shared Table constructor checks each transfer and engine replay,
reconstructs each actor's filtered observations, and verifies the original
observer's information and all final hands. The strict no-hypothesis entry point
still rejects hidden transfers. Event storage stays borrowed until a hypothesis
requires mutation.

For hidden histories, each ordered current-allocation slot keeps its original
weight W(C), and receives a history sampled from the named conditional model
Q(H|C,E). Its uniform reverse proposals are tilted by the existing feature-stay
factors and dealt-hand bidding likelihoods. This is an explicit model extension,
not a proved inverse of the existing marginalized prior or an actual posterior.
Even BaseOnly does not make the resulting histories uniform. Duplicate slots
are retained until histories are assigned; no positive-mass slot is dropped.
Cancellation or exhaustion of 4,096 proposals per slot defers the whole cohort.
Exports identify history sampling separately from current-allocation sampling.

## Validation and review

- `hidden-history-core-unit`: 4 replay/privacy checks passed.
- `history-preparation-policy-unit`: 23 belief and 95 heuristic checks passed.
  The known failing depth-two auction/return fixture was explicitly excluded;
  it remains a required gate, not a waived failure. The canonical-order fixture
  and wrapper-removal tests now pass. Build/run 58.26 s, peak commit 530.1 MiB.
- `history-reconstruction-unit`, after borrowed-event cleanup: all 7 agent
  checks passed, including existing visible reconstruction/legal-order/private
  observation tests. Build/run 5.29 s, peak commit 264.5 MiB.
- Independent source review found no concrete mass/order/privacy blocker.
  It identified the missing independent two-pass multiplicity oracle and the
  unqualified completion rate under the history proposal cap. Follow-up is open.
- The contract privacy regression now requires both forecasts to complete and
  compares actions, rules, semantic facts, full value assessments and sequence
  evidence. Only explicit runtime counters are normalized. This stricter test
  has not yet run; the strong-hand overbid/completion requirement is unchanged.

Inputs are C `0fd5110` plus borrowed-event cleanup `6d4e62e`, B cooperative
preparation/caller and privacy-test patches. Root uses focused integration patches.
All tests used the serialized two-CPU, below-normal, 4-GiB process-tree runner.
Receipts remain in shared private `tmp/2026-10-09-v1`.

Formation completion, selected-parent execution authorization, efficient suffix
lookup, targeted contract tests and paired smoke remain outstanding. No playing
strength, runtime guarantee, deployment or historical-policy compatibility
qualification is claimed. Shared caches and evidence remain needed; reclaimed
space: 0 bytes.

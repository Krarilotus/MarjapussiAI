# Exact finish-key overlap witness

Status: **measured diagnostic; planning gate still deferred**.
Measured source: `ad6e99e`, before Local completion dominance. Intended tag:
`checkpoint/2026-10-11-exact-finish-key-witness`.

One private reporting test observed the original six-card fixture: full authored
catalogue and horizon, two original worlds weighted 0.4/0.6, 500,000 represented
transitions, 32,768 comparisons, 64 MiB evaluator memo, no deadline. It reused the
original response model and caught the resulting Deferred for reporting. The
original fixture was unchanged; test-only hooks did not change evaluation,
charging, cache lookup/admission or error behavior.

The observer resolves the existing full joint BranchKey by equality and records
the nearest enclosing finish dependency. Its separate 64 MiB recorder is bounded
and reports saturation. This records exact reuse opportunities without weakening
the private-state key or introducing result permissions.

## Observations

| Measure | Count |
|---|---:|
| finish calls | 44,953 |
| admitted distinct keys | 1,914 |
| admitted keys completed | 1,910 |
| revisits after an observed completion | 1,298 |
| confirmed calls/keys reused across distinct admitted parents | 12 / 12 |
| those calls finding / missing the optional memo | 10 / 2 |
| key admissions refused at recorder cap | 41,741 |
| calls with unrecorded parent identity | 41,820 |
| recorder retained bytes / cap | 67,097,648 / 67,108,864 |

The 12 confirmed cross-parent repeats are a **lower bound**. Recorder saturation
prevents complete coverage; these counts cannot establish that other reuse is
absent, estimate total savings, or justify an accounting redesign by themselves.
Nested aggregate costs must not be summed as independent saved work.

The original evaluator ended with `Budget`: its next four represented transitions
would exceed the two remaining units. Actual/represented transitions were
463,283 / 499,998; comparisons 29,442 / 29,704; leaf calls 279,416; completed
candidates zero. The reporting test passed; the planning gate did not complete.

## Reproduction and scope

Private bundle: `tmp/2026-10-11-finish-exact-key-witness`, containing the frozen
source manifest and patch, Cargo artifact JSON, copied executable, driver and
run output. Patch SHA256:
`91b5b0ad0f7addbf802a4ea9a37c8174fe2ab2ac372ddacce19310f5f54e4fa7`.
Copied executable SHA256:
`286606cdffa036aacd8c37368338dc67f4426358a819636be1d8c556cc5f6f6f`.
Root receipt `finish-exact-key-witness`, log SHA256:
`563136d7d39626ba0f1055d50c1dd87fb960aeb72915cf9725fb604c2ff5e4c3`.

The resource wrapper enforced one job, two logical CPUs, below-normal priority,
4 GiB process-tree commit, one Cargo job and one test thread. Changed packages
were cleaned through Cargo; artifact verification required the correct manifest
and `fresh:false`. Exactly one copied-executable reporting test ran. Compile plus
run took 132.78 s, peak commit 640.7 MiB; the instrumented test took 12.99 s.
Observer overhead makes that unsuitable for a policy latency comparison.

An exact-dependency accounting change remains a separate design decision. Its
cold-recomputation allowance must have a budget-sufficiency contract; previously
completed effort alone does not prove that property. Mandatory metadata admission,
existing alias ownership and selected-parent authorization also need preservation.
This measurement changes no production accounting, plan domain or allowance.

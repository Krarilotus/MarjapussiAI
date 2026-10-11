# Executable value-store checkpoint

Status: **incomplete strategic migration; value slice validated**. Base
`c3e3f07ad8820479aba11e5bfae93002ac7db8d5`. Intended immutable tag:
`checkpoint/2026-10-11-executable-value-store`.

The policy's completed forecasts now obtain their selection and reported values
from one typed, lazy dependency evaluator. Conditional engine scores remain
separate from expected scores and event estimates. The objective integrates each
branch's score difference; correlated declaration/success events retain their
joint weights and conditional denominator. Repricing uses the same evaluator
before a raised contract receives its own continuation comparison.

An immutable comparison session owns its inputs, model provenance, candidate
bindings and memoized nodes. The exported trace contains actual consumed
references, including shared inputs. `HeuristicAgent::last_assessment` retains the
original visible context and selected assessment, and clears it for subsequent
forced decisions. `reasoning_graph` exports this evaluator on a clearly labeled
synthetic fixture for the portable HTML review.

## Evidence

- 25 heuristic library tests passed, including weighted scoring, event correlation,
  zero/invalid mass, aligned cohorts, lazy reuse, non-dyadic rounding ties,
  observer isolation, forecast reuse and selected-assessment consistency.
  Record: `value-store-final-unit`; 13.34 seconds including compilation,
  281.8 MiB peak committed memory under the shared resource limits.
- Four fixed games before and after the initial migration retained all 220
  actions, rule labels and existing explanation fields. Action/rule hash
  `df46b62954ab7e71`; action hash `c9fcb86f7c2c3bfc`.
- That small timing sample averaged 69.432 ms versus 70.494 ms per nonforced
  decision. It is a parity diagnostic, not a speed or strength qualification.
- Independent review found two tie-order hazards. The final code integrates
  branch differences before averaging and retains the former last-tied raising
  proposal preference. The non-dyadic example has a dedicated regression.
- A plain unoptimized development run was unsuitable for timing and is not used
  as performance evidence. Reusable optimized target artifacts were retained.

## Remaining work

The legacy rollout still supplies conditional terminal outcomes. This checkpoint
is not the authored trick/observation/continuation migration and does not repair
lost strategic follow-through. Conditional programs, information partitions,
plan adaptation, runtime checks and targeted old failure/close-game reviews must
precede the requested 128-deal, team-swapped 256-game comparison against the
actual deployed policy. No deployment or playing-strength claim is made.

Private fixtures and generated output remain outside Git. No Docker resources
were created. Shared compilation caches and benchmark evidence are retained for
the next implementation stage; no disposable resources were removed here.

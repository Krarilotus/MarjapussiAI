# Evaluation boundary for playable v1

No weights are fitted by the return-planning iteration. Existing human-choice
models remain frozen. Their probabilities describe choices, not contract success.

| Model | Existing source | Evidence limit |
|---|---|---|
| C42 forth/back | `passing/features.rs`: named feature order and weights; outer `tools/kbstudy/passfit.py`, `broken` variant | 1,575 passes each way; fitting and reported agreement use the same games |
| B-quiet | `bidding.rs` `P_TAKE_140`; outer `tools/kbstudy/quietfit.py` | 1,261 eligible decisions, in-sample choice fit |
| Game drive | `bidding.rs` `GAME_DRIVE`; historical joint human analysis | Rounded/derived coefficients, including a provisional pair coefficient; no independent calibration claim |
| Bid beliefs | `beliefs/conventions/params.rs` and its source comments | Current defaults mostly owner conventions/hand-set values; old fitted alternatives are not evidence for those defaults |

Historical commands, run from the outer workspace:

```sh
python tools/kbstudy/passfit.py 'pass forth' broken
python tools/kbstudy/passfit.py 'pass back' broken
python tools/kbstudy/quietfit.py
```

Provenance inspected 2026-10-09 (SHA-256):

- Private human source: `e41a69d6f81819dc3619655c9a35ca39c906a8ebe647e3aac869bd65b8823865`.
- Derived `pass_options.ndjson`: `f3f5904762d79e606cdfd161d72aed6f8ce6e1abdb2c3e042cf296c49ad8323a`; counted 1,575 rows per direction.
- `passfit.py`: `e397436191baf70ec8e5d3210ef454ee0399954bbf4cc053ba24ab5048e48333`.
- `quietfit.py`: `05bf9393d3312a043f161726e5282f198bc23ab732dd7ddcbe2e801e5851587b`.
- C42 source including feature order: `0fb933ef650ac0cce6273b556474f6f8172b7e1e22c9ceeba4acab7d5345d404`.
- Bidding source: `78a2f60fb589d1bd7c9d3d9df6a93782e6ad9ad91e5a46654c722db875792445`.

These fingerprints identify the retained artifacts; they do not establish a
reproducible, held-out historical fit. Future fitting must split by whole
game/session before extracting decisions. The inspected human examples are
development data and cannot become a holdout by relabelling them.

Playing-strength comparisons use fixed duplicate deals with seat rotation,
frozen parameters and recorded seeds/revisions. Report the complete result and
approximate deal-level interval, separately from human agreement. Do not use the
existing sequential stopping option for release claims. Rollout contract shares
are conditional policy forecasts from approximate belief weights, not calibrated
win probabilities or guarantees of optimal imperfect-information play.

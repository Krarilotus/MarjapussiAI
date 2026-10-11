# MarjapussiAI

A Rust AI for Marjapussi, the four-player partnership card game played at
[marjapussi.de](https://marjapussi.de). It bids, exchanges cards and plays using
only the information available to its own seat.

**Play or inspect the AI:** use the separate
[MarjapussiAI testbed](https://github.com/Krarilotus/MarjapussiAI-testbed/tree/feature/takeback-claim).
It provides the multiplayer website and the local Debug table. This repository
owns the AI and includes the rules engine; it does not host the website.

## How it works

Each layer has one job:

| Part | Responsibility |
|---|---|
| `engine` | Game rules, legal actions, scoring and fast simulation. Included source derived from [marjapussi-rs](https://github.com/Krarilotus/marjapussi-rs); no separate engine checkout is needed. |
| `observation` | The public events and private cards each seat is allowed to see. |
| `knowledge` | Certain deductions: possible card holders, voids, pairs, standing cards and proven remaining-trick claims. |
| `beliefs` | Estimates where facts leave uncertainty, including evidence from bids. |
| `agent` | The player interface and shared table that supplies each agent's own view. |
| `heuristic` | Bidding, passing and play decisions, with named reasons and shared continuation forecasts. |
| `solver` | The Game Solver: searches card plays, announcements, questions, answers and legal trump changes. |
| `eval`, `tools` | Replays, human-game analysis and comparisons on the same deals. |

The **concept store** is a collection of typed calculations with explicit
inputs, not a separate graph database. Facts constrain beliefs; the agent combines
these with ideas such as entries, pair activation and developing winners. Belief
queries are computed when needed and cached until their inputs change. Continuing
histories reuse state; rewinds, changed histories and belief settings rebuild it.
See the [architecture](docs/ARCHITECTURE.md) and
[concept dependencies](docs/design/concept-store.md).

The Game Solver is separate from the rules engine. It solves an exposed deal
exactly when search completes. With hidden cards it compares possible deals;
those results are estimates, not proof of optimal imperfect-information play.
The current agent normally invokes it when the acting seat holds four or fewer
cards, with up to 64 deals and a 45 ms search budget. That budget is not a hard end-to-end latency guarantee.
See [solver scope and limits](docs/design/solver.md).

## Current version and limits

Continue on **`fix/live-planning-review`**, paired with testbed
**`feature/takeback-claim`**. `main` contains the core layers only;
`v1-preview`, `integration/reviewed-ai` and the portable-v1 checkpoint are older
snapshots. The testbed's [deployment guide](https://github.com/Krarilotus/MarjapussiAI-testbed/blob/feature/takeback-claim/docs/deployment.md)
records the deployed revision pair.

The AI is playable, but there is more to improve:

- Bidding, raising, return selection and open leads compare complete continuations
  on shared, weighted deals. Entries, surviving pairs and trump changes affect the
  engine's final score. Eight sampled deals and a two-second comparison budget
  bound normal planning; incomplete comparisons keep the existing choice.
- Passes update known cards, but inferring a partner's intent from their choice
  of pass is still missing. See [passing models](docs/PASSING.md).
- After the exchange, raising can follow a proven own-card route or an estimated
  profitable continuation. Explanations distinguish those cases. Hidden exchange
  histories, bid affordability and the 500-point series threshold need further
  modelling.
- Playing-strength qualification remains open. The
  [targeted review](docs/checkpoints/2026-10-11-conditional-plan-dryrun.md)
  records remaining control-sequence failures and measured performance improvements.
  The [conditional-sequence proposal](docs/design/conditional-sequences.md) describes
  adaptive plans, persistent intent and dependency-aware reuse for the next iteration.
- Agreement with human moves measures similarity, not playing strength.

The [next steps](docs/NEXT_STEPS.md) and
[checkpoint records](docs/checkpoints/) describe the evidence and remaining work.

## Build and develop

Use stable Rust (tested with 1.91). From this checkout:

```sh
git switch fix/live-planning-review
CARGO_BUILD_JOBS=1 cargo test --locked -p marjapussi-heuristic --lib -- --test-threads=1
```

For a playable table, follow the testbed setup linked above. No recorded human
games are needed to play. Some evaluation tools and tests use a private dataset;
set `MARJA_GAMES` to its `games.ndjson` if you have access. Keep datasets,
credentials and generated output outside Git.

For local policy comparisons, install the [self-play command](docs/SELFPLAY.md)
from Git Bash with `bash marja-bench --install`. Then
`marja-bench new old new old 4000` runs the two partnerships and produces a report
with scores, decision times, contracts, raising and bidding diagnostics.

The [targeted dry-run tool](docs/design/targeted-dryrun.md) compares policies at
the same recorded decisions and follows their alternative plays to completion.

Build and test one job at a time, reusing the target cache. Before contributing,
read the [workflow](docs/WORKFLOW.md): major changes get a focused commit,
validation record and immutable checkpoint. Strategy changes need concrete game
examples and comparisons on fresh deals. Keep rules in the engine, deductions
in knowledge and uncertainty in beliefs; extend the existing concepts.

## Credits and licence

Built on Samuel Leßmann's ([SamuelLess](https://github.com/SamuelLess))
[game site](https://github.com/SamuelLess/marjapussi-online),
[Rust engine](https://github.com/SamuelLess/marjapussi-rs) and
[Python library](https://github.com/SamuelLess/marjapussi), with rule corrections
and player conventions from [Krarilotus](https://github.com/Krarilotus).
Earlier AI work by [schmiddihhh](https://github.com/schmiddihhh/marjapussi-ki)
and [HerrMotz](https://github.com/HerrMotz/marjapussi-ki) informed this project.

GPL-3.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE) for provenance.
Please discuss larger changes with Krarilotus first.

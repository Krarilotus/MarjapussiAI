# MarjapussiAI

**Play Marjapussi yourself at [marjapussi.de](https://marjapussi.de)** (rules: [marjapussi.de/rules](https://marjapussi.de/rules)).

This project builds a fair, fast AI for the card game Marjapussi. It's meant to be fun to play with and against. The goal is to bring it to marjapussi.de in a future update, so people can play there with a working AI partner or opponent. A lot has to be covered and researched before that.

## Sources and people
This work stands on the shoulders of the Marjapussi community's code:

| What | Who | Where |
|---|---|---|
| The game site marjapussi.de, the rules engine in Rust and the Python package that powers the site | Samuel Leßmann ([SamuelLess](https://github.com/SamuelLess)) | [marjapussi-rs](https://github.com/SamuelLess/marjapussi-rs) (Rust), [marjapussi](https://github.com/SamuelLess/marjapussi) (Python, incl. agents with belief tracking) |
| Rule corrections to the Rust engine, the players' conventions this AI follows, and this project | [Krarilotus](https://github.com/Krarilotus) | [marjapussi](https://github.com/Krarilotus/marjapussi) (Python AI experiments), this repository |
| Earlier AI work: perfect-information search and bidding heuristics | [schmiddihhh](https://github.com/schmiddihhh) | [marjapussi-ki](https://github.com/schmiddihhh/marjapussi-ki), [marjapussi-ki-py](https://github.com/schmiddihhh/marjapussi-ki-py), [marjapussi](https://github.com/schmiddihhh/marjapussi) |
| Earlier AI work | [HerrMotz](https://github.com/HerrMotz) | [marjapussi-ki](https://github.com/HerrMotz/marjapussi-ki), [marjapussi](https://github.com/HerrMotz/marjapussi) |

The engine in `crates/engine` is imported from marjapussi-rs with rule corrections; the details are in [`NOTICE`](NOTICE).

## What the project is about
Marjapussi is a 4-player, 2-against-2 trick-taking game with imperfect information:
- an auction from 120 upwards, in which experienced players signal their hands with conventional bids;
- a 4 + 4 card exchange between the declarer and their partner;
- trump made by announcing King + Queen pairs;
- truthful public questions to the partner ("do you have a pair?", "do you have a half?").

The AI is built in layers, each with one job:
- **Rules (`engine`):** legal moves, scoring as on marjapussi.de, and a fast bitset core for search (about 20–30 ns per move).
- **Observation (`observation`):** each seat sees only what that player could see. **The AI never looks at hidden cards;** tests check this.
- **Known facts (`knowledge`):** what a seat can deduce for certain (who can still hold which card, voids, standing cards, pairs). Updated in about 30 ns per event, and checked against the true hands in thousands of games.
- **Beliefs (`beliefs`):** probabilities over the cards that are still uncertain. The base is the combinatorial distribution of the unknown cards under the facts. The conventions of bids, passes and leads adjust it, and probabilities stay exactly 0 or 1 where the facts decide.
- **The players:**
  - **`heuristic`** (agent A): a strategy built from general ideas (what a card is worth to keep or to give, how likely our pair can be announced, what the contract still needs), not a pile of case rules;
  - **`solver`**: an exact endgame search over the deals the player can believe in.
- **Tools (`tools`):**
  - an arena with duplicate deals for fair comparisons;
  - evaluation against recorded human games.

More: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) and [`docs/design/`](docs/design/).

## How to run it

### Build and test
```sh
cargo test --workspace
```
- **Recorded human games:** some tests and the evaluation tools use 1,579 games played on marjapussi.de. **That dataset is private** (it contains player names) and isn't part of this repository. If you have it, set `MARJA_GAMES` to the path of its `games.ndjson` (one game per line, as exported from the site). Without it, those tests skip themselves and everything else runs.
- On a shared machine, limit the build: `CARGO_BUILD_JOBS=2 cargo test -j 2`.

## What's planned
**v1 (now):** a playable heuristic agent that doesn't feel dumb.
- It reads its partner's and opponents' bids, passes and leads as Bayesian evidence.
- It values cards by one consistent idea of the expected score.
- It plays the last tricks with the exact solver.

**After v1:**
- **Stronger play:** search over believable deals in the middle game (determinized and information-set search), a faster solver and learned evaluation. There's plenty of research on this from Skat, Bridge and Doppelkopf.
- **Learning:** policies trained by self-play on top of the facts and beliefs (the neurosymbolic idea: rules and facts exact, learning only for uncertainty and strategy).
- **Feel:** an AI that doesn't feel robotic and can't be exploited: varied, human-like play, partners that read and send the conventions people use, and difficulty levels.
- **Release:** integration into marjapussi.de, so everyone can play with it.

Evaluation is always measured:
- duplicate deals with confidence intervals;
- independent self-play;
- recorded human games as a check.

## How to contribute
- **Ask first.** Open an issue or contact [Krarilotus](https://github.com/Krarilotus) before larger changes.
- **Fairness is non-negotiable.** An agent may only use its own seat's view. New knowledge or belief code needs the soundness and privacy tests to stay green.
- **Facts vs beliefs.** Certain deductions belong in `knowledge` and must be sound. Probabilities belong in `beliefs` and must agree with the facts.
- **One owner per concept.** No duplicated rules, no unexplained numeric literals; reuse the engine's functions.
- **Strategy from ideas, not case rules.** Agent changes come with a duplicate-arena comparison on fresh seeds.
- **Performance matters.** Report the cost per event or per decision for hot-path changes.
- **No personal data.** Never commit recorded games or player names; datasets stay outside the repository.
- **Style:** run `cargo fmt` and `cargo clippy --workspace -- -D warnings`. Use one branch per change.

## Licence
GPL-3.0 (see [`LICENSE`](LICENSE)), as the engine is derived from GPL-3.0 code (see [`NOTICE`](NOTICE)).

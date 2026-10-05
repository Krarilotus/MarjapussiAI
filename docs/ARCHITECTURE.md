# Architecture (draft v0 — orchestrator, 2026-10-06)

Layers, dependencies pointing down. Each crate has one façade (`lib.rs`) and owns its concept alone.

| Layer | Crate (planned) | Owns | Status |
|---|---|---|---|
| 6 serve | `server` | thin edge: game server / UI bridge, difficulty profiles | later |
| 5 policy | `policy` | learned decisions (bid, pass, question/trump, play); trained with PPO-family / GRPO on top of layer 3 features | after research round |
| 4 search | `search`, `solver` | belief-sampled determinized search; exact endgame solver (last ~4 cards per player) | next |
| 3 knowledge | `knowledge` | **what a seat can know**: set-theoretic deduction from its own observation (possible / certain cards per player, voids, pairs and halves, standing cards, bidding conventions) | next |
| 2 observation | `observation` | the seat's view: own hand + public events. The **only** input to layers 3–6 | next |
| 1 rules | `engine` | game rules, legal actions, scoring (imported, corrected core) | **done** |

## Invariants
1. **No cheating (owner, 2026-10-06):** "The AI can NOT know what the actual cards are that everyone has, it can NOT cheat. It needs to be able to derive information itself." Layers 3–6 depend on `observation`, never on the full `GameState`. A test plays games where hidden hands are swapped consistently with the observation and checks that the decisions do not change.
2. **Rules have one owner:** legality and scoring come only from `engine`. Other crates call it; they never re-implement it.
3. **Derived facts are inputs, not things to learn:** the policy gets layer 3's deductions as features. It does not relearn bookkeeping.
4. **Performance is a feature:** the engine and knowledge layers are hot paths (search calls them millions of times). Bitset card sets (36 bits) are the target representation. Every change to them gets an A/B benchmark and must stay correct (differential tests against the current engine and marjapussi-py).

## Open (decided by research round 1 and the repo-idea investigation)
- Search family (PIMC / IS-MCTS / αμ / EPIMC); belief weighting.
- Endgame solver horizon and speed target.
- PPO → GRPO transfer for the policy layer; reward and baseline design.

# Design: `beliefs` (layer 3b): the believed forms, one `Mind` per agent seat

Search Coder, 2026-10-06, queue S9 (phase I2). Binding decisions: `findings/design/reasoning-system-v2.md` §8 F1–F12. Evidence: `findings/reasoning-measurements.md` (S8/S8a/S8c).

## Current implementation

On `fix/live-planning-review`, bids provide uncertain evidence and exchanges
update visible-card constraints and bookkeeping. Pass choice belongs to
`heuristic`; interpreting intent from a chosen pass is not implemented.
The default parameters use neutral card factors, a uniform hidden-exchange
kernel and no fitted calibration. Optional fitted mechanisms below describe
available design/experiments, not the active defaults. See [passing](../PASSING.md)
and [evaluation provenance](../EVALUATION.md).

The remaining sections retain the 6 October design and its staged measurements.
For current ownership and cache lifetime, see [architecture](../ARCHITECTURE.md).

## What it owns
- **The believed forms** of every concept: card probabilities, voids and suit counts, standing and future wins, team pairs and meldability, seat features, nested views.
- **The read-out bands.**
- **The convention models** (`beliefs::conventions`): shared bid messages, targets and likelihoods, with their parameters. Pass scoring lives in `heuristic`; no pass-choice likelihood is active.
- **The signal record** (`beliefs::record`): the auction, passes, melds and questions per seat, for convention reading.

What it does not own:
- **Known forms** stay plain lazy functions on `Knowledge` (F2, I1's K1–K6).
- **Strategy** stays in the agents (`heuristic`, agent B).

```
agent (heuristic / agent B)
   │ reads
   ▼
beliefs::Mind ── one per agent seat; nothing shared between agents (R6, D-d)
   ├── Tracker        own facts (knowledge; K1–K6 known forms on Knowledge)
   ├── Tracker        public facts (Knowledge::public: no hand, every pass hidden), for views
   ├── events         the seat's own SeatEvent stream (the only input: privacy by construction)
   └── caches         keyed by (fact epoch, signal epoch):
         record + bid likelihoods (signal) · feature-DP leaves (fact) · exchange terms (signal)
         · the derived block: card table, counts, seat features now and dealt (both)
```

## Invariants
1. **Privacy:** a `Mind` is built from its seat's start hand and its own `SeatEvent`s only (`observation`). There is no API to give it anything else.
2. **Known ⇒ believed:** a believed read is exactly 1 or 0 wherever the facts decide it. The facts are `Knowledge`, its K1–K6 known forms, and the exact sets of the deal space. Otherwise it lies strictly between 0 and 1: a belief never becomes a fact. Every likelihood and factor is floored at ε (`conventions::Params::convention_floor`), and a 0 or 1 only ever comes from the facts. Tested on random games for every read.
3. **One computation per epoch** (F3):
   - the **fact epoch** (`Tracker::fact_epoch`, F12) moves exactly when the facts change, or on undo;
   - the **signal epoch** moves on every event that is not a card play (bids, passes, questions, answers, announcements, undo);
   - both are drawn from one monotonic counter per `Mind`, so a key never repeats, even after an undo.

   Values that depend on facts only (the exact base) are keyed on the fact epoch. Believed values are keyed on both. No dirty bits and no card masks (F3).
4. **Reads are lookups:** agent A reads about 225 card probabilities per decision (M1). The card table is computed once per epoch; a read is an array lookup.
5. **Undo, one owner:** `Tracker` decides every undo (it restores the snapshot before the last undoable event). The `Mind` drops the last event of its list only when `Tracker` accepted the undo, and that event is always the one `Tracker` retracted; the signal record is rebuilt from that list. The per-play context (who led which card under which trump) is `Knowledge::leads`, undone with the snapshot. Both epochs move on an undo.

## Read API (`Mind`)
| Read | Meaning | Known form it must agree with |
|---|---|---|
| `p_holds(seat, card)`, `holds_row(seat)` | P(card is in that seat's hand now) | 1 if certain (exact sets), 0 if impossible; own hand exact |
| `p_void(seat, suit)` | P(the seat holds no card of the suit) | 1 if `is_void`; 0 if a card of the suit is certain or K4's lower bound > 0 |
| `count(seat, suit)` | distribution of the number of suit cards (`CountDist`, `mean()`) | support inside K4 `count_bounds` |
| `p_stands(card)` | P(my card wins its trick if I lead it now) | 1 if K1 `standing`; 0 if an opponent certainly holds a higher card of the suit |
| `future_win(card)` | P(my card wins a trick before the end) (F10: closed form; the endgame solver stays in the agent) | ≥ `p_stands`; 1 if standing |
| `p_team_pair(team, suit)` | P(the team holds both halves, in one hand or split) | K3: 1 if certain, 0 if not possible |
| `p_meldable(team, suit)` | P(the team can still announce the suit: pair, suit uncalled, a usable right) | 0 if K3 says impossible or no right is left (K6) |
| `features(seat)` | `SeatFeatures` of the hand **now**: P(ace), P(≥ 1/2/3 halves), P(a half) and P(the pair) per suit (F5) | 1/0 exactly where the exact deal space decides (e.g. 0 pairs after "no pair", K2) |
| `dealt_features(seat)` | the same of the **dealt** hand, the hand it bid with (F5, F7) | K5 dealt sets |
| `record()` | the signal record (auction, passes, melds, questions per seat) | the event stream |
| `band(p)` | `KnownNo` (0) / `BelievedNo` (≤ low) / `Open` / `BelievedYes` (≥ high) / `KnownYes` (1) | thresholds are one named parameter (`Bands`, F8) |
| `view(j)` | `View`: what seat j believes (F9), `p_holds(k, card)` | j's known cards from my view |

All reads take `&self`; caches use interior mutability (a `Mind` is `Send`, not `Sync`).

## Parameters (`BeliefParams`)
- `signals: Full | BaseOnly`: the arena switch. `BaseOnly` reads the facts only: the exact n-over-k base for cards, the feature DP without signals (uniform exchange) for seat features.
- `conventions`: the convention models' parameters, with human trust by relation (`trust_partner`, `trust_opponent`) and `trust_agent` for HP1 seats. All seat kinds use the same convention reader, mixed with the class-independent step by θ.
- Fitted parts **per seat kind** (`PerKind { human, agent }`): the per-card factors (F4) and the exchange kernel (F7). Agent parts are fitted on independent self-play; human parts on the human games, shrunk towards "no information" (they are a check, not the target).
- `feature_cards` (F6), `calibration` (F8, one temperature per family), `bands`.

## Computation (by step)
1. **API step (this commit):**
   - `Holds` = the exact n-over-k base under the facts (`DealSpace::holder_counts`; F4 without factors).
   - Voids and counts: the Poisson-binomial of the row, truncated to the K4 bounds.
   - `p_stands` / `future_win`: closed forms from the row.
   - Team pairs: from the rows of both partners.
   - Seat features: from the row, under independence (replaced in step 2).
   - Views: the public tracker plus the cards I know j holds (and, for my partner, the cards it passed me that I still hold), renormalised (IPF).
2. **F4–F8 (step 2):**
   - **Feature DP (F5):** `DealSpace::with_features` splits the unknown aces and the halves of each suit into tied groups; each leaf is one joint placement with its exact deal count. Per leaf and hidden seat, the class of the hand now and of the dealt hand (K5's dealt-certain cards plus the unknown cards it holds). The posterior weights each leaf by every seat's dealt-class likelihood (the product of its bid tables). The leaves are cached per fact epoch, so a bid only reweights them.
   - **Hidden exchange (F7):** for the two opponents after their hidden exchange, the leaf fixes the team's cards after the exchange, not who was dealt which. A kernel (per role ace/half and passer: P(dealt to the seat that holds it after the exchange)) sums the dealt splits; the terms depend on the signals only and are cached per signal epoch, indexed by the held counts.
   - **Planning deals:** `Mind::deal_weights` evaluates those same joint dealt-class likelihoods on complete fact-consistent deals. Visible transfers map cards back to their dealt owner; hidden transfers reuse the exchange kernel. Optional card signal factors multiply the likelihood once. Pair relationships therefore survive the handoff to entry queries, return forecasts and the endgame solver. Marginal calibration remains a reporting transform. The default model has neutral card factors and calibration.
   - **Facts stay exact:** a feature is a fact iff it is true in every or in no leaf (class masses > 0); likelihoods are floored, so the posterior never invents one.
   - **Card table (F4, F6):** the exact base; on the feature cards the DP's exact posterior marginals; × the per-card factors of the partner's pass (value class × cards of the suit in the pass) and of early leads (tricks 1–3, not trump), by the seat's kind; renormalised by IPF over the open cells (facts and pinned cells fixed). `FeatureCards::Pinned` keeps the DP marginals out of the IPF.
   - **Calibration (F8):** one temperature per feature family, kept only where it helps on both data sets.
3. **F9–F11 (step 3):** the lazy views at any depth; `future_win` refined from I3's measurements; agent B's exchange-aware class proposal.

## Costs (measured, step 2; `findings/beliefs-I2.md`)
| Item | Measured |
|---|---|
| fact epoch per event | one `Knowledge` copy and compare (~30 ns) |
| per decision of the seat to act (225 card reads + seat features, computation included) | bidding 126–181 µs mean, p99 0.85–1.2 ms; passing 100–130 µs; play 58–65 µs mean, p99 ~0.67 ms |
| read | array lookup (ns) |

The feature DP dominates: leaves cached per fact epoch, exchange terms per signal epoch.

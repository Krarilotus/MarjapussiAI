# Design: Heuristic Player v1 (HP1), a rule-based benchmark player

> Historical design/tuning record. The pass implementation and APIs have changed;
> see [current passing semantics](../PASSING.md) and the checkpoint records.

Orchestrator, 2026-10-06. **Status: draft for implementation.**

Serves:
- I11: no cheating;
- I13: neurosymbolic inputs;
- I15: performance;
- I18: eager/lazy knowledge;
- the owner's request of 2026-10-06 (quoted below).

Primary sources:
- **[O]** the owner's convention explanation, `MarjaAI/findings/owner-conventions-2026-10-06.md` (verbatim);
- **[RV]** marjapussi.de rules and tips (`sources/marjapussi-online/vue-frontend/src/views/RulesView.vue`);
- **[A]** Investigator A, measured on 1,579 human games (`findings/ideas/A-reasoning-and-deduction.md` §5);
- **[K]** the owner's 2023 branch `refactor_step2`;
- **[J]** schmiddi's rules.

## 0. Purpose

> "Alone those heuristics and the ruls based engine might make a good first player to play against … some player that gets close to the performance of the players in the test games that played often far from perfect. … this player might always be a good first benchmark, even if we go for a different architecture alltogether." [O]

So HP1 must be:
1. **Fair:** it decides only from its own seat's observation and the knowledge layer. The privacy guards apply.
2. **Human-like:** it speaks and reads the human bidding and passing conventions, so humans can partner with it.
3. **Explainable:** every decision names the rule (ID below) that produced it, plus the facts that rule used.
4. **Measurable:** it ships with an arena on duplicate deals and a human-agreement evaluation on the 1,579 games.
5. **Fast:** at most 100 µs per decision without search (target: 10 µs).
6. **Data-driven rules:** a rule table with IDs and sources, ready to export to a logic language later ("a fully fledged logical catalogue … in something like prolog" [O]).

HP1 is **not** the final AI. It is the baseline, an easy-difficulty opponent, a teacher prior for later learning (RQ-9: "supervised prior then RL"), and an engine smoke test.

---

## 1. Architecture

```
crates/
  heuristic/            (new, layer 5 "policy"; depends on knowledge, observation, engine)
    src/lib.rs          façade: HeuristicPlayer, Params, Decision { action, rule, why }
    src/lib.rs          the agent: one `beliefs::Mind` per seat, read on from the view's events (C28)
    src/hand.rs         the card order within a suit (`higher`, `top`)
    src/bidding.rs      HB rules: HP1's gates over the convention encoder (`beliefs::conventions`)
    src/passing.rs      HP (forth) and HK (back): HP1 acts with the pass models of `beliefs::conventions`
    src/raising.rs      HZ rules
    src/trump_plan.rs   HQ rules: when and how to announce or ask
    src/stakes.rs       what points are worth: the expected score difference from `engine::game_scores`
    src/valuation.rs    the one valuation of every card choice (C25, C28)
    src/leads.rs        the planned leads (HL-1, HL-3 … HL-6)
    src/endgame.rs      the last tricks: the solver over the worlds I believe in (F10)
    src/play.rs         card play: trump plan, leads, the valuation, the endgame
    src/rules.rs        the rule table (id, phase, priority, source, description), used for logging and export
  beliefs/              (Search Coder) the believed forms (`Mind`) and the convention models, encoders
                        and readings (D-d: one owner); HP1 reads and acts with them
  heuristic/tests/      unit scenarios per rule; privacy swap test for decisions
  eval/                 evaluation of any agent (C15): the table driver, fixed deal suites, duplicate
                        deals with a sequential test, game statistics, plain-language reports
  tools/arena (bin)     thin CLI over eval: self-play vs humans, duplicate A/B, `--fit-egp`
  tools/human_eval (bin) agreement with the 1,579 human games
```

### 1.1 Agent interface (one owner for "how an AI acts")

The interface lives in its own crate, `crates/agent`, below policy (Judge M1 item 8). Every AI implements it: HP1 (`heuristic::HeuristicAgent`), random play (`eval::RandomAgent`), and later the search agent.

```rust
pub trait Agent {
    /// `view`: what this seat knows (Tracker over SeatEvents). `legal`: the engine's legal
    /// actions for this seat (computed from its own hand + public state only).
    fn act(&mut self, view: &SeatView, legal: &[GameAction]) -> Decision;
}
pub struct SeatView<'a> { pub seat: Seat, pub knowledge: &'a Knowledge, pub events: &'a [SeatEvent], pub series: SeriesScore }
pub struct Decision { pub action: GameAction, pub explanation: Explanation }   // Explanation { label, facts }
```

HP1 also offers `HeuristicAgent::decide()`, which returns its typed decision (`RuleId` plus facts); `act()` turns the rule ID into the explanation's label.

- **No `Game` or `GameState` in this crate.** The Judge's `kg` gate (no-cheat rule) covers `heuristic`.
- `SeriesScore` (both teams' totals, games left) is public information.
- The "decisions do not change under hidden-card swaps" test from `privacy_swap.rs` is extended to `act()`.

### 1.2 Knowledge tiers used (I18)

| Tier | Source | Examples used by HP1 |
|---|---|---|
| 0 eager | `knowledge` crate | possible and certain cards, voids, the trick, trump, constraints |
| 1 lazy exact | `knowledge` known forms K1–K6 (`docs/design/knowledge.md`) | standing cards (top trumps included), meldable pairs, team pairs, count bounds, the dealt hand, public facts (rights, declarer, value, trick winners, points) |
| 2 lazy soft | `beliefs::Mind` (`docs/design/beliefs.md`) | P(holds), P(void), counts, `future_win`, `p_stands`, team pairs, seat features of the hand now and as dealt |

Since C28 the soft layer is the `beliefs` crate: HP1 keeps one `Mind` per seat, built from its start hand and its own events. A believed read is exactly 1 or 0 where the facts decide it. §1.3 and §1.4 describe the conventions and their reading; their code is in `beliefs` now (D-d).

### 1.3 Epistemic layer ("Modallogik light"), all four seats

**Facts vs. beliefs (owner, C8):** facts come only from `crates/knowledge`, from my seat's view; there is no separate epistemic subsystem. "Who knows / believes what" lives only in the soft layer, as a small part of `beliefs.rs`, built from the public `Revealed(y)` profiles and the generative convention models (§1.4).

| Operator | Meaning | Source |
|---|---|---|
| `K_me φ` | I know φ | tier 0 (sound deduction) |
| `B_me(x ⊨ φ)` for **every** other seat x: partner **and both opponents** | what I believe about x's hand | tier 2: x's bid steps (same decoder for all seats, HR-1…6), passes, questions and answers, leads, discards and follow choices (§1.3.1) |
| `B_x(y ⊨ φ)` for every seat x and every y ≠ x (one nested level; not in v1, §1.3.2) | what x believes about y's hand, including what the **opponents believe about me and my partner** | computed from what x can know: the public events decoded by the shared convention, plus private knowledge x has (x's own hand; the passed cards if x is y's partner) |
| `Public φ` | everyone knows that everyone knows φ | public events only |

Cheap construction: for each seat y, keep a `Revealed(y)` profile: the messages y sent by bids, its public answers and announcements, and the voids it showed in play. Then:
- `B_opp(y) = Revealed(y)`;
- `B_partner(y) = Revealed(y)` plus the cards passed between them.

All of this is computed from public events plus my own hand, so it is fair, and it is cached per event like tier 2.

#### 1.3.1 Reading opponents' actions (soft evidence, tier 2)
- **Bids:** the same decoder as for the partner, but with the opponents' own trust `θ` (they may block or bluff). Read opponents' steps in the light of `B_opp(us)`. For example, an opponent bidding to 140, 180 or 200 right after our team showed a big pair is more likely a **block** than a claim (HB-B from their side).
- **Questions:**
  - an opponent asking for a half in suit s ⇒ likelier that they hold the other half (completing a pair), or that they are probing us;
  - "Yours" ⇒ the asker believes their partner may hold a pair.
- **Discards:** an opponent's off-suit discard in suit s ⇒ s is less likely to be their trump (HF-5). The suit their partner keeps is the complement.
- **Leads:** an opponent leading a suit early ⇒ likely long or standing there; leading trump ⇒ trump majority.

#### 1.3.2 Using what the others believe about us: not in v1 (owner D2, 2026-10-06)
No concealment and no blocking from `B_opp(us)` in v1: HB-0 sends every live message the partner needs, without weighing what the opponents learn; HP1 does not predict the opponents' blocks from what they believe about us; questions do not price what an answer reveals to the opponents (P10a: "that only matters later").

`B_x(y)` (one nested level) is not built in v1: no decision needs it (the reading-only code was removed, Judge J19 item 8). Deeper levels ("I think that you think that I think") are research (RQ-16).

### 1.4 Conventions are generative models, read by Bayes inversion (owner, C5)

Owner: "every heuristic not only applies to the actor but also to the observer, and the negative case as well … It needs proper modal logic".

This replaces any per-signal ad-hoc decoder in `conventions.rs` and `beliefs.rs`:
1. Each convention is **one generative model**, `P(action | hand features, public context)`. This is the "speaker" in the rational-speech-acts sense (RQ-16 report). It covers bids (HB, the 140 shift, confidence by context) and passes (HP-1, HP-6, HP-8; HK-2).
2. **Acting:** the player chooses with that model (argmax, or the HP scoring that defines it).
3. **Reading:** for any observed action, the likelihood of each hypothesised hand feature comes from the **same** model (Bayes inversion, the "listener"). Negative evidence is therefore automatic. Example: a pass with no small cards ("Lusche": 6, 7, 8, 9 **and U**; the owner counts the Unter as a Lusche here) in suit s, only a 10 or K, is unlikely under "giver is blank in s", so it reads as "the giver still holds s".
4. **Every observer:** the reading runs from each seat's view with that seat's knowledge: k=1, `B_x(y)` per C1. The partner sees the passed cards; the opponents see only the shape signals that are public (the hidden pass reveals nothing, but **later play** reveals it). k=2 (what the opponents infer that my partner knows) stays out of PR-A.
5. **Structural priors for passing (owner):**
   - the **first passer (MH)** most likely makes **2 suits blank**;
   - the **second passer (VH)** most likely passes **one whole suit** back.

   These are priors in the passing models (`Params`), used on both the acting and the reading side.
6. **Lusche:** define `LUSCHE = {6, 7, 8, 9, U}` once (in `hand.rs`). HP-8's "small cards" means Lusche.

Implementation in PR-A (`conventions.rs`, `passing.rs`, `beliefs.rs`):
- **Bid model:** a hand class is (holds an ace, per suit: no half / one half / the pair), 2 × 3⁴ = 162 classes. `conventions::bid_likelihood(action | class, context)` mixes the convention (HB-1…HB-5 with the 140 shift, confidence by context, HR-2), a literal reading, a generic step distribution and a block component (HR-7). Reading enumerates the 162 classes exactly; the class prior comes from the observer's card probabilities.
- **Pass models:** the HP and HK scores define `P(option | hand) = softmax(score / τ)`. Acting takes the argmax. Reading samples the giver's unseen cards from the observer's knowledge (seeded, deterministic) and weights each sample by the bid likelihood of its class and the pass likelihood.
- **Observers:** `B_me(y)` uses my knowledge; `B_x(y)` for x ≠ me uses the public card prior plus, when x is y's partner and I saw the pass, the passed cards. An opponent's model never contains passed cards.
- The rules HR-1…HR-6 are properties of this model, not separate decoders (HR-3 and HR-5 come out of the inversion).

Tests:
- a round-trip test: for random hands, encode then read gives the true feature the highest posterior in most cases (report the rate);
- negative-evidence scenarios: a 10/K-only pass in s ⇒ P(giver blank in s) drops below the prior;
- MH blanking two suits ⇒ VH's posterior of MH's voids matches the measured rates (A §5.3) within tolerance.

---

## 2. Hand features (`hand.rs`)

All are computed from the seat's own hand (`certain[me]`) as bit operations:
- `pairs`: the suits holding K and O. **Small** pairs: Green 40, Acorns ("Eichel") 60. **Big** pairs: Bells ("Schell") 80, Red 100 [O, RV].
- `halves`: the count of K or O cards; `half_suits`: suits with exactly one half.
- `aces`, `tens`, `suit_len[s]`, `points` (card points).
- `standing(s)`: my cards of suit s that win when led, with no trump, given the outstanding higher cards (tier 1, S22). Under trump t ≠ s they also need the opponents to follow or be void of trump; v1 uses the no-trump version and the "outstanding higher cards" count.
- `blank(s)`: `suit_len[s] == 0`.
- `team_pair_prob`: the probability that our team holds at least one pair, combining my hand with tier-2 beliefs about my partner.
- `expected_game_points` (EGP): the points our team can expect to make if we play the contract, used by bidding and raising. A linear model of eight features (`hand::egp_features`): a constant, the team's known aces, tens and halves, my longest suit, my void suits, the expected meld value (best pair plus half the second, from `team_pair` beliefs), and P(partner holds an ace).
  - The weights HP1 **acts** on are fitted on HP1's own declarers in self-play (`arena --fit-egp`; Judge J17 item 1): what HP1's own card play makes.
  - The fit on the human declarers (M2) describes what humans make; it is 20–30 points higher and is kept in the tuning log for reading human bids (M3 P3).
  - The margins on top (HB-7 `bid_overreach`, HB-8 `take_margin`, HZ-2 `raise_margin`) are chosen by duplicate-deal strength against the previous defaults, never by agreement alone.

  All weights live in `Params`.

---

## 3. Bidding ("Reizen")

### 3.1 The convention: messages and steps

Steps are measured from the current game value `v` (the value starts at 115; the first bid of 120 is a +5 step). One owner, `conventions.rs`, holds this table and serves both `encode` and `read`.

| Message | Step | Meaning | Source |
|---|---|---|---|
| `Ace` | +5 | "I hold an ace", sent only if no ace is announced in my team yet | [O] "The first step being a +5 is almost always an ace"; [A] 98.0 % |
| `TwoHalves` | +5 when my team has already sent `Ace` | "I hold 2 halves" (variant `strict_two_halves`: only if the pair values of those halves' suits add up to ≥ 140) | [O] |
| `SmallOrHalves` | +10 | small pair (Green or Acorns) **or** 3–4 halves | [O], [K]; [A] 91.9 % below 140 |
| `BigPair` | +15 | big pair (Bells or Red) | [O]; [A] 95.7 % |
| `RedPair` | +20, only as a player's **first** step | Red pair | [O] "the +20 at the start is most definitely a red pair" |
| `Nothing` | pass | "nothing to announce" | [O] "Absense of steps at all also means we probably have nothing"; [A] pass ⇒ pair 2.7 % |

**Choosing the message (encoder): HB-1 … HB-6, first match wins.**
- **HB-0 (epistemic filter):** skip a message whose content `B_partner` already holds (a second ace is not announced: "most realistically a second ace is not announced so the second player could pass even if they have an ace" [O]).
- **HB-1** Red pair and this is my first step → `RedPair`.
- **HB-2** a big pair → `BigPair`.
- **HB-3** a small pair, or 3–4 halves → `SmallOrHalves`. This is preferred over `Ace`: "if someone doesn't go for the +5 isntead goes for +10 … they must think it was more important that they have 3 or 4 halves or a small pair, than telling their ace" [O].
- **HB-4** an ace and the team has not announced an ace → `Ace`.
- **HB-5** the team has sent `Ace`, I hold 2 halves (respecting `strict_two_halves`) → `TwoHalves`.
- **HB-6** otherwise `Nothing` (pass), unless a blocking rule in §3.3 applies.
- If HP1 has more to tell after its first step, later turns send the next message by the same order (a later +5/+10/+15 means the next item). For the reading side see §3.2 and Q1.

**The 140 rule (skip 140):**
- Card points total 120, plus 20 for the last trick, so **140 is the most a team can make without a pair**. Going above 140 claims a pair [RV, A: bid > 140 ⇒ team pair 94.5 %].
- **HB-140a (encode, default, owner answer 2026-10-06):** compute the *live* messages for this player in its team context (e.g. `Ace` is not live once the team has sent `Ace`). If a live **+10, +15 or +20** message's raw target `v + step` equals exactly 140 (the red pair's +20 since C43c, owner 2026-10-07: "red pair also applies so goes to 145"), then **that message and every live message with a larger raw target shift by +5**, so messages stay distinct: from 130, +10 → 145 and +15 → 150; from 125, +15 → 145 and +20 → 150. **A +5 (ace, two halves) neither shifts nor makes a shift:** from 135, +5 → 140, +10 → 145, +15 → 150, +20 → 155. If no +10/+15 hits 140, there is no shift. Owner (verbatim file #12): "going over 140 and still being able to tell you have a small pair you probably skip the 140 and make a 15 … a big pair from 125 for example goes to 145 … sometimes from 135 you just do 15 and it means a big pair"; the answer in #16: "if there is one step that can land on 140 that's ambiguous it means the next does get too, but some humans are inconsistent and don't care for the +5 step for the two halves" (C39 addendum, 2026-10-07). The encoder and the reading share one function (`beliefs::conventions::follow_target`). The variant `skip_on_any_cross` (add +5 whenever `v < 140 ≤ v+step`) stays as a switch for the data comparison (Q1).
- **HB-140b (don't cross 140 without a pair):** HP1 bids above 140 only if `team_pair_prob ≥ Params.cross_140_pair_conf` (default 0.85). "You seldom go over 140 if you are not certain, or at least confident … you have a pair" [O].
- **HB-140c (bidding exactly 140):** a defensive or claiming bid with no pair message:
  - (i) we expect to play the opponents "schwarz";
  - (ii) we want the game but only if we find a pair ("You don't take the game if you dont have a pair" [O]).

  HP1 bids exactly 140 when EGP ≥ 140 × `Params.claim_140_margin` and no pair is known, or as a block (§3.3).

**Not exceeding what we can make:**
- **HB-7:** never bid above `EGP + Params.bid_overreach` (default 0) unless a blocking rule applies.
- The bid that carries a message is still subject to HB-7. If the message would overbid, pass instead (the information is lost; the partner reads "Nothing").

### 3.2 Reading bids (decoder → tier 2)

`conventions::read(history, me)` returns, for each other seat, likelihood factors `P(observed step | hand feature)`, applied as world weights. They are **never** hard facts.

- **HR-1:** decode each step in context: `(step, v_before, is the player's first step?, has the team sent Ace?, has the team sent anything?, did the step land on or cross 140?)` → a distribution over messages.
- **HR-2 (decode with context-dependent confidence, owner answer 2026-10-06):** a step is read as a mixture of "follows the shift rule" and "ignores the step rules". The confidence depends on the context (fitted on the 1,579 games; starting priors in brackets):
  - the general case (0.8);
  - a `TwoHalves` step (0.5): "some humans are inconsistent and don'T care for the +5 step for the two halves";
  - the team's first step (0.6): "if its the first step in the team it might also just be ignored";
  - the partner passed directly at the start (0.4): "players might ignore the step rules altogether if they just don't care and the partner has passed directly at the start".

  The confidence scales how strongly a step updates beliefs: the likelihood is mixed with the base rate. A step that crosses 140 is read under both the shifted and the literal reading. [A]: +15 crossing 140 ⇒ big pair only 49 %; +20 crossing ⇒ 54 %.
- **HR-3 (exactly 140 is uninformative about pairs):** [A] 19 % pair vs a 21 % base rate. It updates only a "strong hand" feature (aces, tens), with a small weight.
- **HR-4 (bids above 140 ⇒ the team has a pair):** the factor P(team pair | bid > 140) starts at the measured 0.945. It applies to the **pair of seats** (bidder plus partner), not to the bidder alone.
- **HR-5 (absence is information):**
  - first action a pass ⇒ P(pair) ≈ 0.027;
  - first step +10 instead of +5 ⇒ lowers P(ace), but only mildly: "or simply don't have an ace which is mostly the case" [O];
  - no `TwoHalves` after the partner's `Ace` ⇒ fewer than 2 halves (weak).
- **HR-6 (order and seat):** steps are read relative to what the **team** has already sent ("its only counted per team" [O]). The second `+5` in a team means `TwoHalves`, rarely `Ace` (a mix weight from the data).
- **Calibration:** the 1,579 games give counts for each message × hand feature (the [A] tables are the start). Fit Beta posteriors per cell. Keep the owner's structure; let the data set the strengths. The cells and counts are documented in `beliefs.rs`.

### 3.3 Blocking and series-aware bidding (HB-B, HS)

- **HB-B1 (barrier values):** "going for 180 200 220 or so numbers are often considered hard barriers for schwarz when hitting for a specific pair, either side might do that to hinder the other side from getting the game, even if they have bad cards" [O].
  - HP1 computes the expected outcome when the opponents play at `v` (their EGP, from tier-2 beliefs about their bids) against the expected outcome when we play at `v'` ∈ {180, 200, 220} (our EGP).
  - It blocks to the smallest barrier `v'` that makes letting them play worse than our expected loss at `v'`, counting the schwarz risk (they win a schwarz ⇒ we lose double their game value, per the scoring rules [RV]).
  - Off by default at the "easy" difficulty.
- **HS-1 (series objective):** the utility is `Δ(our total − their total) + bonus term`, where the bonus is ±300 for reaching 500 first (engine `SeriesSettings`).
  - Near the end of a series, HP1 shifts risk: it bids more aggressively if it needs points to reach 500 first, and more defensively (avoid schwarz, keep the difference low) when losing is likely [O].
  - Proposed, not implemented: current HP1 ignores `SeriesScore`. The testbed supplies totals and games left, but no risk parameter is computed from them. A series policy also needs the public first-to-500 bonus owner; totals alone cannot reconstruct it.

### 3.4 Trust (anti-bluff)

"there could be bluffs, but its also sometimes unlikely" [O]. HP1 keeps a per-seat trust `θ_seat ∈ [0,1]` (start 0.9).
- After each game the hands are revealed, so `θ` is updated by how often that seat's messages were true.
- Read likelihoods are mixed: `θ · P_conv + (1 − θ) · P_base`.
- Opponents' trust is tracked separately from the partner's. Opponents bluff to block.

### 3.5 M3: information bids, reading by seat kind (P2, P3, P12)
- **Seat kind (P3):** who plays each seat (`agent::SeatKind`: human, HP1, other program) is public table metadata in `SeatView`, like the names on the site; it never says anything about cards. `Revealed(y).kind` carries it to every reader of y's bids.
- **HB-9 (P2, information first up to 140):** a message up to 140 that HB-7 would stop is still sent while the risk of winning the game by accident stays below `accidental_win_risk` (0.1): risk = P(every seat still in the auction passes after my bid) × P(our points stay below the value), with realised points ~ N(EGP, `egp_sigma` = 50). Each seat's pass is predicted over its class posterior: an HP1 seat with HP1's own policy (`bidding::hp1_bid_prob`: its message if its EGP clears HB-7, from the mean and covariance of the EGP features of its hand class over random hands), any other seat with the convention model. *Why conditional:* HP1 seats pass weak hands, so behind an HP1 table my 120 likely stands (the risk is high: no message); human tables bid on (the risk is low: tell the ace).
- **Human reading (P3):** humans show the Red pair mostly as +15 (M2 data: 43.7 % +15, 16.4 % +20), so the convention model sends 73 % (`human_red_as_big`) of a Red pair's message as +15 when it reads a seat that is not HP1. HP1's own encoder keeps +20 = Red.
- **Reading HP1 with HP1's policy (P3, behind `read_hp1_as_hp1`, off):** implemented, but it measured −1.1 ± 5.4 / −7.7 ± 5.6 points per deal pair against reading every seat with the convention model, also after a refit of EGP; so it is off until the reason is understood (owner question in the M3 report).
- **P12:** "second +5 = two halves" is read with confidence 0.8 (`conf_two_halves`; data 2023–2025: 81.5–87.2 %).

---

## 4. Raising (VH after passing back, HZ)

- **HZ-1:** after the exchange, VH knows its own 9 cards and the 4 it gave back (they are certain in MH's hand). It computes EGP from that.
- **HZ-2:** raise to `round_down_5(EGP − Params.raise_margin)` if that is above the current value; otherwise play a card (keep the value).
- **HZ-3:** never raise across 140 without a team pair that is **certain** (tier 0: the pair is in my hand, or in the cards MH holds that I know). "You seldom go over 140 if you are not certain" [O].

---

## 5. Passing

### 5.1 Passing forth (MH → VH), 126 options, scored exhaustively (HP)

Each 4-card option `X ⊂ hand` gets a score; the highest wins; ties go to fewer points given away.

| Rule | Term | Source |
|---|---|---|
| HP-1 | +w_blank per suit I become blank in (`blank` after removing X); strongest when there are 2 such suits. "you want to usually pass two colours blank" | [O] |
| HP-2 | +w_info × (the information VH gains about my hand that `B_partner` does not hold yet): e.g. passing an ace I never announced, or the half that completes VH's announced half | [O] "priority to pass what the other doesn't know from the bidding rules yet" |
| HP-3 | +w_quality × (points + aces + tens in X) | [O] "better cards are preferred" |
| HP-4 | +w_need for cards VH needs for its likely game, read from VH's bids: VH sent no `Ace` ⇒ value an ace highly; VH bid 180 ⇒ suspect green is needed | [O] "if the other one needs an ace … you pass an ace"; "suspect the partner player needs green, because they bid 180" |
| HP-5 | −w_green per green card passed: a **very weak** tie-breaker, first pass (passing forth) only; almost every other passing term overrides it (`w_green` ≈ 5 % of `w_blank`), so VH keeps green options for the first lead and "schnippeln" | [O] addendum: "for the first pass maybe, but only a very weak rule that gets overruled by most other passing rules" |
| HP-6 | Passing a pair forth. **Keep** the pair if the partner knows of it (`B_partner` holds it, e.g. signalled in bidding) or if it is a **big pair** (Bells or Red: I can announce it myself). A **small pair the partner doesn't know about**: Acorns, lean to pass it; Green, neutral. **Pass it** in both small cases when the partner "absolutely needs to know": the partner has signalled no meaningful halves, so it cannot derive that our team has a pair. Splitting a pair (K without O) is penalised | [RV] "Paare … eher nicht hinschieben"; [O] addendum: "eichel you might pass if partner doesnt know, green is not sure, if partner absolutely needs to know it … then yes pass it!" |
| HP-7 | "awkward hand" fallback: if no option makes a suit blank, put weight on HP-3/HP-4 for VH's expected game | [O] |
| HP-8 | **Signal "blank in s" (owner, C4):** when the pass forth makes me blank in suit s, prefer passing s's **small cards (6 up to U, few or no points)**. That signals "I'm blank in s" and shows VH where to start schnippeln: VH leads s low, an opponent must overtake, and I take it (higher card, or trump later). **Reading side (tier 2):** a pass of small cards of a suit ⇒ the giver is likely blank in it (Investigator A measured 87–97 % void rates for 4, 3-1 and 2-2 shapes). VH's lead choice HL-3 uses this to pick the schnippeln suit. | [O] addendum: "You often signal that you don't have a specific suite also by passing small cards up to U that don't have points"; [A] §5.3 |

Cost: 126 options × about 20 bit operations ≈ 3 µs.

### 5.2 Passing back (VH → MH), 715 options, scored by game plan (HK)

VH first picks a **game plan**, then scores the options under it.

**Plans:**
- **P-T(s), trump game in suit s:** I hold the pair s, or the pair can likely be completed with MH (`TwoHalves`, `SmallOrHalves` read, or cards passed).
- **P-N, no-trump game:** win with standing cards (aces and tens) and the last trick.
- **P-B, block or schwarz avoidance:** when we were pushed to a high value with a weak hand.

| Rule | Term (under the chosen plan) | Source |
|---|---|---|
| HK-1 | keep trump: never pass back cards of suit s in P-T(s); keep both halves | [RV] "zurückgeschobene … Trümpfe sind wichtig" (read: what you keep matters) |
| HK-2 | pass back **one complete side suit** that MH can lead, so that VH (now void in it) can take with trump while the opponents must follow and overtake ("snack high points") | [O] |
| HK-3 | keep aces and standing cards; aces passed back count highly for MH only if MH needs entries (MH void elsewhere) | [RV] "zurückgeschobene Asse … sind wichtig" |
| HK-4 | last-trick plan: keep at least one trump (or a sure winner) for trick 9 | [O] "secure the last trick" |
| HK-5 | void-making for VH: being blank in a suit creates trumping chances later | [O], [RV] "sich in einer Farbe blankschieben" |
| HK-6 | P-N: keep long suits with top cards; pass back short suits | — |

Cost: 715 options × plan scoring ≈ 30–60 µs; acceptable once per game.

---

## 6. Trump and questions (HQ)

### 6.0 Common-sense invariants (HC), evaluated before every other play rule

These follow from the rules and the point arithmetic and are not explicit owner heuristics. A pair is worth 40–100 points, more than almost any trick (a trick holds 0–44 card points, +20 for the last one). Announcing therefore dominates card play.

| ID | Rule | Why [D] | PR |
|---|---|---|---|
| HC-1 | **Keep value, not a ban (owner, C4):** when I must choose what to give up, minimise `keep_value(card) + points_given(card)`; a K or O of an announceable pair keeps `pair_value × P_activate` (§6.0.1) | Playing a half can destroy 40–100 points for at most one trick's points | A |
| HC-2 | **Announce in time:** I can announce only while I hold the lead. If I lead now and the next lead is not certain (`sure_leads_after_this == 0`), announce now, or ask now if a question is the way to our trump. More generally, start when `sure_leads ≤ announcements_and_questions_needed`. "Sure leads" count only standing side-suit cards that are not pair halves. | Each announcement needs a won trick; a lost lead may never come back | B |
| HC-3 | **Team activation-order plan (owner, C4):** choose the order in which our pairs become trump, (M) announce mine then ask "Yours", or (P) ask first and let the partner activate mine later (§6.0.2) | Engine question rights; "it really depends on what trumps are held" [O] | B |
| HC-4 | **Why cash standing cards first (and only then):** before any trump exists, a standing side-suit card cannot be trumped; afterwards, void opponents must trump it. So cash safe side-suit winners first, as long as HC-2 still leaves a sure lead for every announcement and question needed. | Rules page tip "Niemals … Trumpf melden solange es noch Stehende gibt", made precise | B |
| HC-5 | **Several pairs:** each announcement needs a lead. Announce first the pair most at risk of being broken (fewest protecting cards, or an opponent likely void there); the last pair announced becomes the final trump, so prefer to finish with my longest suit. | Last-trick and trump control | B |
| HC-6 | **Answering "Yours" with several pairs:** name the pair the team gains most from as trump now (longest combined suit, or most at risk); the other can be announced later by me if I still may. | Truthful, but the choice is free | B |
| HC-7 | **Don't feed the opponents points:** never put an A or 10 into a trick the opponents will win, unless forced or HC-1's keep value says otherwise. When forced, give the fewest points plus keep value. | Card points | A |
| HC-8 | **Forced overtakes:** when the rules force me to beat my partner's winning card, use the cheapest card that does. | Must-overtake rule | A |
| HC-9 | **Contract arithmetic:** track `need = contract − (our card points + our announced pairs)` and how many points are still out. Declarers: once `need ≤ 0` the contract is made (overshoot scores nothing), so minimise the defenders' points; if it can no longer be made, also minimise the defenders' points (the loss is fixed). Defenders: maximise our own points (we score them) and avoid schwarz. | Scoring rules | B |
| HC-10 | **Deny the opponents a lead when they can still announce:** if an opponent likely holds an uncalled pair (tier 1/2), winning tricks denies them the lead they need to announce. Weight trick-taking by `P(opponent pair) × value`. | Mirror of HC-2 | B |
| HC-11 | **Trump counting:** once trump is set, track the outstanding trumps (tier 1). Draw trumps when our team holds the majority, so our side-suit winners become safe again. Never lead into an opponent's known void if that lets them trump a point-heavy trick. | Standard trick-taking | B |
| HC-12 | **The last trick (+20):** with ≤ 3 tricks left, plan so a sure winner or trump is kept for trick 9 when that is worth more than winning an earlier, smaller trick. | +20 | B |
| HC-13 | **Never ask a question whose answer is already known** (tier 0: the partner certainly has, or certainly lacks, the half or pair). It reveals information and, for "Yours" and half questions, may cost later rights. | Engine question rights | B |


#### 6.0.1 HC-1 in detail

Choosing which card to give up is a value comparison, not a ban.
When I must choose what to play into a trick I won't win, or what to keep, minimise `keep_value(card) + points_given(card)` over the legal cards, where:
- `keep_value(K or O of an announceable pair) = pair_value(s) × P_activate`.
  - `P_activate` is the chance our team still gets to announce or ask in time: high (≈ 0.9) if I or my partner have a sure lead, or the partner can easily take over and ask me; lower when nobody on our team has an entry; × P(the other half survives).
- `keep_value(guard)`: a card that keeps us at X+1 cards against the opponents' X outstanding higher cards in that suit becomes standing later. Value it as its card points + `E_entry` if it would give us an entry.
  - Owner's example: the opponents hold the ace, we hold 10 and O. Don't drop both until the ace has fallen; then the 10 stands and can "get us in the game".
- `E_entry` (`Params`): an extra value for the first card that can win us a lead when our team has no sure lead ("if we are in need to at least get in the game").
- Defaults reproduce the owner's rules:
  - between a 10 and the K of an unannounced pair, give the **10** ("here the 10 is probably played");
  - between O and 10, give the **O** only when getting a trick at all matters more (no entry otherwise);
  - if the partner can easily take over and ask, and the O is unannounced, give the **10**.
- **Arena metric (replaces "= 0"):** count voluntary pair-half plays before announcement, each with its rule-log reason. Expected to be rare and always explained by the value model.
- **Tests:** the three owner examples above, each as a scenario.

#### 6.0.2 HC-3 in detail (PR-B)

A team activation-order plan instead of "never ask while holding an own pair".
Choose, for the team, the order in which our pairs become trump. The main options:
- (M) I announce mine, then later I ask my partner "Yours";
- (P) I ask my partner first (my own right to announce is lost); later my partner, while leading, asks me "Yours" and so activates my pair, after cashing standing cards in the first trump suit.

Score each plan by: whose leads and entries are sure, which trump we want first and last (the last becomes the final trump; HC-5), the standing cards in each trump suit, and the risk of the opponents breaking a pair. "it really depends on what trumps are held and if the partner can ask us meaningfully after playing standing cards in the first announced trump suite of his pair, oftentimes it makes sense to plan for a specific activation order." Keep HC-3 in the rule table as `HC-3 activation plan (PR-B)`.

Source tag **[D]** = derived by the orchestrator from the rules and the point arithmetic (owner reviews them).

### 6.1 Trump and question timing (HQ)

The core principle: "you try to stay in the game for as many tricks as possible, so asking trump only when it makes sense not right away, that way you can assure you can play the high cards first, and have enough questions left … to begin asking on the X last card you win the trick safe on" [O].

**Definitions:**
- `sure_leads`: the number of tricks our team can still win in a row from the lead with no trump: our standing cards, tier 1, my own and those certain with my partner.
- `q_needed`: the questions or announcements our team needs to set trump: 0 if I hold an uncalled pair (announce); 1 to ask "Yours" if the partner likely has a pair; 1–2 to ask for halves when we hold matching halves. Computed from tier 0 + tier 2, and from the question restrictions in the engine.
- `need_trump_points`: the trump points we need to make the contract, `value − (current points + expected remaining)`.

**Rules (first match wins, evaluated when I lead after winning a trick):**
- **HQ-1 (cash side-suit standing cards first, only while HC-2 holds):** if `sure_leads > q_needed`, cash a *side-suit* standing card instead, but only while HC-2 still leaves a sure lead for every announcement and question needed. Pair halves are never cashed (HC-1). [RV] "Niemals ohne guten Grund Trumpf melden solange es noch Stehende gibt."
- **HQ-2 (time the questions):** replaced by HC-2's trigger: start announcing or asking when `sure_leads ≤ announcements_and_questions_needed`, or now if the next lead is not certain. [RV] "nach Paaren bzw. Hälften fragen, wenn der nächste Stich nicht sicher ist (aber nicht eher)."
- **HQ-3 (order "mein → dein → unser"):**
  - announce my own pair if I have one;
  - otherwise ask "Yours" if the partner's pair probability ≥ `Params.ask_pair_conf`;
  - otherwise ask for a half in a suit where I hold the other half.

  The question legality (`Own` / `Yours` / `Ours`) comes from the engine. [RV] "Merkregel … mein → dein → unser Paar."
- **HQ-4 (probe for the opponents' pair):** as the defending or declaring side, ask for a half I don't hold when the answer helps locate an opponent pair (it costs a question). [RV] "nach Hälften fragen, die man selbst nicht hat (um Paare beim Gegner zu identifizieren)".
- **HQ-5 (game decided):** if the contract is already made, or already impossible, don't announce further when that gives the opponents points or information. [RV] "falls das Spiel bereits entschieden ist: ggf. nichts mehr melden".
- **HQ-6 (defence, change trump):** as a defender who wins a trick and holds an uncalled pair, announce it when that **destroys the declarers' trump** while they are relying on it, or wins the last trick. [O] "destroy your opponents gameplay and pairs".
- **HQ-7 (question value):** with several legal questions, prefer the one with the largest expected trump value minus the information it reveals to the opponents (public answers: RQ-12).

#### 6.1.1 As implemented in M3 (P8, P10, P10a; `trump_plan.rs`, replaces HQ-0)
At every lead where questions are legal, before the lead rules:
1. **Attempts** (schmiddi's counting, `probabilistic_policy.py` l. 487-575): 0 if I hold an uncalled pair or the partner certainly does; 1 (+1 if the partner may hold a pair) if our team surely holds a pair split between us; otherwise my live standalone halves (+1 "Yours" if the partner may hold a pair, P ≥ `partner_pair_min`). Nothing to reach: lead normally.
2. **Sure leads:** my side-suit standing cards that are no pair half.
3. **HC-4:** attempts < sure leads → cash a standing card (the next lead, and with it the next question, is guaranteed). *Why:* this is the condition behind the M2 pattern "announcement with a standing card in reserve" (+9.5 pp).
4. **HC-2 / HC-5 / HQ-3:** otherwise act now, in the order mine (the most valuable pair; HC-5 with several) → "Yours" → halves (the half the partner most likely completes, weighted by pair value).
5. **HC-13:** a half the partner certainly lacks (its "no", or tier 0) is never asked; "Yours" is not asked after the partner's "no pair".
6. **P10a rights guard:** a question may not spend a right the team still needs: my uncalled pair needs my `Own` (or the partner's "Yours" plus a likely partner lead: a certain standing card or P(live ace) ≥ `partner_lead_conf`); the partner's possible pair needs my "Yours" (or the partner's `Own` plus a lead). Questions that spend nothing are never blocked. `History::rights` tracks the engine's rights per seat.

Information questions beyond reaching a pair (HQ-4, HQ-7) are not asked in M3; what an answer reveals to the opponents is not priced (owner D2).

---

## 7. Card play

### 7.0 As implemented in C28 (the one valuation)
Every card choice is the best `value_of_play` of `valuation.rs`, except the planned leads (HL-1, HL-3 … HL-6), HL-2, the trump plan (§6.1.1) and the endgame. The six rules the C26 audit called invented (HA-1, HL-F, HF-1, HF-3, HF-4, HD-2) are gone; §7.2's HF-1, HF-3 and HF-4 below are history.

- **One unit (`stakes.rs`):** the expected final score difference, ours minus theirs, from the engine's scoring (`engine::game_scores`: made or failed, rounding to 5, schwarz; none restated). The rest of the game is a smooth fair share: of the points still out my team takes X ~ Normal(out / 2, out / (2 √n)) over the n tricks left, summed over whole points; a team without a trick stays without one with probability ½ⁿ. So the worth of a point is not symmetric:
  - declarers: our points count through P(made) × 2 × value, and every point the defenders take costs one, also after the contract is made;
  - defenders: our points count one to one, the declarers' through P(made); our first trick avoids schwarz;
  - a meld: for declarers only near the line, for defenders its points.
- **The trick term:** P(we take it) × the score if we take it + the rest × the score if they do, plus the value of the lead it brings (the pairs it can still activate).
- **Keep values:** the future-trick value is `Mind::future_win(c)` × the worth of a later trick with the card's points (the lead it brings counted only for tricks before the last); the pair term is meld worth × P(the other half is ours) × P(activation) from `Reach`; the plan term (P7) as before.
- **Labels:** the term that separates the chosen card from the runner-up names the rule: HV-0 (the trick now, [D]), HV-1 (future), HC-1 / HC-9 (pair; HC-9 when we declare), HV-2 (plan), HL-7 (a certain loser not led). HC-8 names a forced overtake of the partner, HF-2 a trick term choice when the opponents take the trick, HC-7 a future-term choice whose runner-up fed them an A or 10.
- **Answers (HC-6):** "Yours" is answered with the pair whose meld is most at risk if not named now: its worth × P(I don't activate it later myself).
- **Endgame (HE-1, design D-c / F10):** with at most 4 tricks left, the card or announcement is the solver's (`solver::evaluate`) over the deals consistent with my facts (all, or 64 uniform draws), each weighted by Π P_believed / P_facts of the hidden seats' cards; the objective is the contract (HC-9) or, all passed, our points. Exact ties go to the valuation. The solver plays no questions, so while a half of mine still carries activation value, the valuation decides.

### 7.1 Leading (HL)

- **HL-1 (trick 1, VH):** the engine forces an ace, else green, else anything. Among legal options:
  - if MH likely or certainly holds the ace of a suit I hold low cards in, **"schnippeln"**: lead low there (green when that is the forced suit). An opponent must overtake, and MH must play its ace, because the led ace must be played in trick 1 [O, engine rule S5];
  - otherwise lead my strongest ace (ace–ten combination preferred).
- **HL-2 (cash standing cards first):** "standing" means side-suit standing cards that are **not** halves of an announceable pair (HC-1). Lead a standing card (highest first within a suit) while `sure_leads > 0`, in line with HQ-1. "play the high cards first" [O].
- **HL-3 (plan suit):** under P-T after the pass-back, lead the suit I passed back to MH or MH's long suit, so MH can win or I can trump [O, HK-2].
- **HL-4 (don't lead trump without reason):** lead trump only to draw the opponents' trumps when our team holds the majority (tier 1 bounds) or to win the last trick. [RV] "Niemals ohne guten Grund … Trumpf anspielen".
- **HL-5 (destroy an opponent pair):** if an opponent holds exactly one half of an uncalled suit with no other cards in it, or both halves of a pair not yet called, lead that suit so the must-follow and must-overtake rules force out the K or O [O].
- **HL-6 (last trick):** keep a trump or sure winner for trick 9 when possible; plan the lead order so the last trick is ours [O].
- **HL-7 (don't give a trick away early):** never lead a card that certainly loses when a non-losing alternative exists. [RV] "Niemals … früher als nötig einen Stich abgeben".

### 7.2 Following and discarding (HF), within the legal set

- **HF-1 (partner wins securely):** if my partner is winning and no later player can beat them (tier 0/1), play my **highest-point** legal card ("schmieren").
- **HF-2 (opponent wins, I can't):** play my lowest-point legal card. When discarding off-suit, discard from a suit:
  1. where I can never win a trick: "get rid of colours early … on which you might never get tricks anyways" [O];
  2. different from the suit my partner has been discarding (we keep different suits: "you look what your partner drops in colour and usually keep a different one" [O]);
  3. that is not a half I may need.
- **HF-3 (I can win):** if I am last to play, win with the cheapest winning card when the trick has points or the win gives our team the lead we need. If I am not last, win with a card no later opponent can beat (a standing card or a high trump), or else play low.
- **HF-4 (trump decisions):** when I can trump, do it if the trick is worth `≥ Params.trump_trick_min_points` or the win is needed for the plan. Otherwise discard per HF-2. (The engine forces trumping when I'm void and hold trump; the choice is only which trump: the lowest that wins.)
- **HF-5 (read discards):** an opponent's off-suit discards mark suits they don't value, so those suits are less likely to become their trump. Lower P(their pair in s) in tier 2 [O].

### 7.3 Defence (HD)

- **HD-1:** "as opponents ofc you try to get tricks at all": avoid schwarz first. Take any trick that is cheap to take early.
- **HD-2:** play against the declarers' plan: lead suits that the declarer who passed back is void in only when we can over-trump; otherwise lead their partner's weak suit.
- **HD-3:** combined with HQ-6 and HL-5, destroy their pairs.
- **HD-4:** when the contract is clearly lost for the declarers, maximise our own points and the last trick.

---

## 8. Parameters and difficulty

- All weights and thresholds sit in one `Params` struct, with defaults, a TOML file and documentation.
- Difficulty profiles (later):
  - **easy:** HB-B off, HQ-4 off, and noise on the scoring;
  - **normal:** defaults;
  - **expert:** the strict variants on.
- Tuning: a coarse grid in the arena on fixed duplicate deals, then human agreement as a tie-breaker. Every default change ships with a duplicate A/B against the previous default (≥ 1,000 deals on the `dev` and `check` suites, with the 95 % CI; Judge J17 item 1). Every change is logged in `docs/design/heuristic-player-tuning.md`.

### Rule sources (honest audit, C26/C27)
The owner: "'throw the most points onto the partner's trick' Is no rule that i ever stated". Every rule's source tag was checked against the primary sources: `[O]` only with the owner's verbatim words (`findings/owner-conventions-2026-10-06.md`, "oc"; its Addenda; the owner's quotes in the coder queue), `[RV]` only with the rules page line (`RulesView.vue`, "Tipps"), `[A]` measured, `[D]` with the derivation written here. The six rules the audit found invented (HA-1, HL-F, HF-1, HF-3, HF-4, HD-2) were deleted with C28 (§7.0).

Corrections found: HK-3 was tagged `[RV]`, but the rules page says aces passed *back* are important (the opposite direction); HB-8 was tagged `[RV]` without a matching line; both are `[D]` now.

| Rule | Source | Evidence |
|---|---|---|
| HB-0 | [O] | oc §1: "most realistically a second ace is not announced so the second player could pass even if they have an ace (and ofc its only counted per team)" |
| HB-1 | [O] | oc §1: "the +20 at the start is most definitely a red pair"; Addendum 2 #3: "the +20 is unambiguous gives us a stronger signal" |
| HB-2 | [O] [RV] [A] | oc §1: "For a +15 we derive the big pairs schell or red"; RV: "große Paare deutlich ansagen (Sprung machen)"; [A] 95.7 % |
| HB-3 | [O] [A] | oc §1: "goes for +10 on the first step, they must think it was more important that they have 3 or 4 halves or a small (green or eichel) pair, than telling their ace"; Addendum 2 #1: "Halves announcing first is often better" |
| HB-4 | [O] [A] | oc §1: "The first step being a +5 is almost always an ace"; [A] 98.0 % |
| HB-5 | [O] | oc §1: "if you do a second +5 in the team it means you have 2 halves" |
| HB-6 | [O] [A] | oc §1: "Absense of steps at all also means we probably have nothing to anounce" |
| HB-7 | [D] | Scoring: a contract that is not made scores −value. Above 140 (where P2's information bids end, Addendum 2 #2 "as long as the risk of taking the game accidentally isn't too high") a message is sent only if the team expects to make it |
| HB-8 | [D] | Scoring: with nothing left to tell, taking the game by +5 scores +value when the expected points clear the next value by a margin (the margin covers the spread of the estimate) |
| HB-9 | [O] | Addendum 2 #2: "information takes priority as long as the risk of taking the game accidentally isn't too high" |
| HB-140a | [O] | oc §1: "going beyond 140 means you add +5 for the step you want to announce, so a big pair from 125 for example goes to 145"; Addendum: "if there is one step that can land on 140 that's ambiguous it means the next does get too, but some humans are inconsistent and don't care for the +5 step for the two halves" (so a +5 neither shifts nor makes a shift, C39) |
| HB-140b | [O] | oc §1: "You seldom go over 140 if you are not certain, or at least confident it is very likely you have a pair" |
| HB-140c | [O] | oc §1: "if someone goes to 140, it could mean anything ... it could also just mean: You don't take the game if you dont have a pair" |
| HB-B1 | [O] | oc (owner's first explanation): "going for 180 200 220 or so numbers are often considered hard barriers for schwarz" |
| HS-1 | [O] | oc: "it also is important to get to 500 ... the first team getting to 500 gets the bonus" |
| HR-1 | [O] | oc §1: "derive msot of the information given not only from the steps directly but also the order and who gave them and their own cards" |
| HR-2 | [O] [A] | Addendum: "it is not so clear you see but some confidence can be applied here!" |
| HR-3 | [O] [A] | oc §1: "if someone goes to 140, it could mean anything, its hard to interpret that step" |
| HR-4 | [O] [A] | oc §1: "You seldom go over 140 if you are not certain ... you have a pair" |
| HR-5 | [O] [A] | oc §1: "you can also derive knowledge from the absence of those steps" |
| HR-6 | [O] | oc §1: "(and ofc its only counted per team)" |
| HR-7 | [O] | oc: "either side might do that to hinder the other side from getting the game, even if they have bad cards" |
| HR-8 | [D] | Engine question rules plus the owner's observer principle (Addendum: "every heuristic not only applies to the actor but also to the observer"): a half question pays when the asker holds the other half, so the observer reads it that way |
| HR-9 | [D] | The same observer principle applied to HL-2 (cash standing cards, long suits first): an early side-suit lead suggests a long or standing suit |
| HZ-1 | [O] | Addendum 2 #4: "raising depends on the rough estimated of what the game can be worth, given an averge hand for how the partner bid" (EGP with the cards given back stands in for P4 until the beliefs crate) |
| HZ-2 | [O] [D] | [O] Addendum 2 #4 covers the estimate ("what the game can be worth"). [D] the formula `round_down_5(EGP − raise_margin)`: bids are multiples of 5 (engine `BID_STEP`), so the target rounds down to one; a raise to T is worth (2·P(points ≥ T) − 1)·T, and with realised points spread about ±50 around EGP that is largest well below EGP, which the margin expresses (chosen by duplicate strength, J17) |
| HZ-3 | [O] | oc §1: "You seldom go over 140 if you are not certain ... you have a pair" |
| HP-1 | [O] [RV] | oc: "you want to usually pass two colours blank"; RV: "sich in einer (nicht Trumpf-) Farbe blankschieben" |
| HP-2 | [O] | oc: "priority to pass what the other doesn't know from the bidding rules yet" |
| HP-3 | [O] | oc: "also ofc. better cards are preferred" |
| HP-4 | [O] | oc: "if the other one needs an ace because they dindn't call one out for example, you pass an ace" |
| HP-5 | [O] | Addendum: "for the first pass maybe, but only a very weak rule that gets overruled by most other passing rules" |
| HP-6 | [O] [RV] | Addendum: "unless your partner has absolutely no clue it exists"; RV: "Paare (und Trumpffarbe) eher nicht hinschieben" |
| HP-7 | [O] | oc: "if you can't comfortably pass two colours you might in awkward situations even try to win the bid or pass just better cards" |
| HP-8 | [O] | Addendum: "You often signal that you don't have a specific suite also by passing small cards up to U"; Addendum 2 #11: "just an extra ... just an indicator" |
| HK-1 | [RV] | RV: "zurückgeschobene Asse (und Trümpfe) sind wichtig" (trump stays in the team); the planned pair's halves: HC-1 [D] |
| HK-2 | [O] | oc: "you often want to pass one complete farbe (suite) back that the partner can play" |
| HK-3 | [D] | Aces and standing cards are VH's sure leads, which it needs to announce (P8). Note: RV says aces passed back are important ("zurückgeschobene Asse ... sind wichtig"), the opposite direction; the earlier [RV] tag was wrong |
| HK-4 | [O] | oc: "try to secure the last trick ... keeping trump when you know the opponent has low likely hood to change it" |
| HK-5 | [O] [RV] | Addendum: "probably one whole suite passed if they are the second passer"; RV: "sich in einer (nicht Trumpf-) Farbe blankschieben" |
| HK-6 | [D] | Without trump a trick goes to the highest card of the led suit, so long suits with their top cards make standing cards later (P5's future-trick value) |
| HK-7 | [O] | Addendum 2 #9: "only if we 100% know they hold like we gave them the ace, and it makes only sense if we don't have too many aces" |
| HQ-1 | [RV] | RV: "Niemals ohne guten Grund ... Trumpf melden solange es noch Stehende gibt" |
| HQ-2 | [RV] | RV: "nach Paaren bzw. Hälften fragen, wenn der nächste Stich nicht sicher ist (aber nicht eher)"; replaced by HC-2 |
| HQ-3 | [RV] | RV: "Merkregel für Trumpfmeldung: mein → dein → unser Paar" |
| HQ-4 | [RV] | RV: "nach Hälften fragen, die man selbst nicht hat (um Paare beim Gegner zu identifizieren)" |
| HQ-5 | [RV] | RV: "falls das Spiel bereits entschieden ist: ggf. nichts mehr melden" |
| HQ-6 | [O] | oc: "as opponents ofc you try to get tricks at all and try to destroy your opponents gameplay and pairs" |
| HQ-7 | [O] | P10 (owner): "Asking can be use for information gathering sometimes, just make sure not to disrupt the possibility ..." |

| HC-1 | [D] | §6.0: playing a half destroys 40–100 points for at most one trick's points (owner C2: "playing a king and queen when they were a pair before announcing it, that should be obvious") |
| HC-2 | [D] | §6.0: each announcement needs a won trick; a lost lead may never come back |
| HC-3 | [D] | §6.0 (the owner calls the blanket form "too pauschal": an activation-order plan, planned) |
| HC-4 | [D] | §6.0: a standing side card cannot be trumped before trump exists (RV "Trumpf melden solange es noch Stehende gibt", made precise) |
| HC-5 | [D] | §6.0: each announcement needs a lead; the pair most at risk first |
| HC-6 | [D] | §6.0: answering "Yours" truthfully, the choice of pair is free; §7.0: the pair whose meld is most at risk |
| HC-7 | [D] | §6.0: card points; in the valuation the label of the future-trick term when points alone would have fed an A or 10 |
| HC-8 | [D] | §6.0: the must-overtake rule |
| HC-9 | [D] | §6.0: scoring (overshoot scores nothing; a needed meld decides the contract) |
| HC-10 | [D] | §6.0: mirror of HC-2 |
| HC-11 | [D] | §6.0: trump counting |
| HC-12 | [D] | §6.0: the last trick is worth +20 |
| HC-13 | [D] | §6.0: a known answer only spends a right |
| HV-0 | [D] | §7.0: the trick's points go to whoever takes it, worth what they change in the expected score (`stakes.rs`) |
| HV-1 | [O] | Addendum 2 #5: "Ofc we try to keep cards that can win, we can get more tricks, optimize for staying in the game. But be realistic so if the partner gave us a blanc 10 we might as well throw that" |
| HV-2 | [O] | Addendum 2 #7: "make sure not to play the suite before so it doesn't get destroyed. We need to keep those information what we passed in memory" |
| HL-1 | [O] | oc: "schnippeln, where you play a lower card, the opponent has to go over it, and then your partner takes it wit ha higher card, in this case the ace, as it has to be played on the first trick"; Addendum 2 #9 |
| HL-2 | [O] | oc: "that way you can assure you can play the high cards first" |
| HL-3 | [O] | oc: "pass one complete farbe (suite) back that the partner can play when you have trump and the opponent needs to go higher when you can take the trick with trump" |
| HL-4 | [RV] | RV: "Niemals ohne guten Grund ... Trumpf anspielen" |
| HL-5 | [O] | oc: "maybe you can even secure that by playing the colour they have to then play too and destroy their pair" |
| HL-6 | [O] | oc: "you want to try to secure the last trick ... keeping trump" |
| HL-7 | [RV] | RV: "Niemals ohne guten Grund ... früher als nötig einen Stich abgeben" |
| HE-1 | [D] | The owner's decision D-c as the design records it (`findings/design/reasoning-system-v2.md` §5, §8 F10): the exact solver over sampled worlds the seat could believe in, at the depth that fits the time budget; the solver's values are exact per world, so where it reaches it replaces the closed forms |
| HF-2 | [O] [D] | oc: "Try to get rid of colours early too if there is a choice, on which you might never get tricks anyways" (the hopeless suit); the cheapest card: HC-7 [D] |

| HF-5 | [O] | oc: "you look what your partner drops in colour and usually keep a different one or get clues what the opponents non playing partner drops as colour to understand what might not get trump" |
| HD-1 | [O] | oc: "it might sometimes be worth it to not get played schwarz, as you minimize the loss of points" |

| HD-3 | [O] | oc: "try to destroy your opponents gameplay and pairs" |
| HD-4 | [D] | Scoring: defenders score their own points (HC-9); planned |
| HX-1 | engine | Plumbing: one legal action |
| HX-2 | engine | Plumbing: undo requests are declined |

---

## 9. Evaluation (shipped with the code)

### 9.1 Agreement with human play (`tools/human_eval`)

- Replay each of the 1,579 games with the engine (`parse_legacy_format` logic, or a step-wise replay; see `marjapussi-rs/src/bin/ml_convert_legacy.rs` for decision-point extraction).
- At each decision of a human seat, build that seat's `SeatView` (observation only) and ask HP1 for its action.
- Report top-1 agreement **per phase**: first bid, later bids, pass forth (4-card set and per card), pass back, raise, question/announce, lead, follow.
- Report each with a 95 % Wilson interval, next to two baselines: uniform random legal, and "first legal".
- Report the convention agreement separately: HP1's message vs the human's step, under both 140-rule variants (this answers Q1 with data).
- Plain-language summary in the report, e.g. "HP1 picks the same first bid as the human in N of M cases".

### 9.2 Arena (`crates/eval`, CLI `tools/arena`)

- **Duplicate deals:** each deal is played twice with the teams swapped (`--seats`: also with the hands turned by two seats, so the other partner opens). Fixed deal suites by name: `dev` (seed 1), `check` (seed 2), `holdout` (seed 3, never used for tuning).
- **Sequential test** (`--sequential`): a normal-mixture confidence sequence on the per-deal differences, checked every 50 deals from 100 on; stops when it excludes 0, or when it lies within ±5 points (no meaningful difference).
- **Scores** always from the engine (`game::scoring` via `GameFinishedInfo::scores`).
- **Baselines** (`--opponent`): `random`; `hp1-random-play` (HP1's auction and exchange, random card play; `eval::Composite`, Judge S10), which isolates what the card play is worth; `hp1` with other params.
- **Intention detectors** (`eval::detectors`, on the true hands, for any agent): HC-1 voluntary pair-half plays; P10a pairs blocked by spent rights; C22 standing A/10 thrown onto the partner's trick; C24 halves of a needed pair led while a question was legal; pair points per game.
- **Matchups:** HP1 vs uniform-random; HP1 vs HP1-with-a-rule-ablated (one per rule family: does each family earn its keep?); later HP1 vs search agents.
- **Metrics:**
  - mean score difference per deal pair, with a 95 % CI;
  - contract made rate;
  - schwarz rate;
  - pass-game rate (detects pass collapse);
  - bid-value distribution vs the humans' (histogram distance);
  - decisions per second.
- Report in plain language first, numbers second (owner rule: numbers need context).

### 9.3 Guards (tests)

- One scenario per rule ID (as in `knowledge/tests/rules.rs`).
- Privacy: decisions are identical under the hidden-card swap of `privacy_swap.rs`.
- Determinism: same seed, same game.
- Speed floor: ≤ 100 µs per decision, best of 5, `--ignored`.

---

## 10. Implementation plan (one branch, three milestones, one merge)

Owner (C8, supersedes C6/C7): "create the well known parts in rust and make sure they work, then please not regress from it but isntead create the heuristic engine on top + then actually check the heuristic rules against the real games and check for missconceptions, any more patterns we migth have missed for explainign play and then try the agent on random games against itself". Everything is built on `feat/heuristic-a`; `crates/knowledge` stays as merged and all its tests stay green.

- **M1, the heuristic engine** on top of engine + observation + knowledge (PR-A scope as amended by C1–C5); tests green. Then a Judge checkpoint (no merge):
  - the `heuristic` crate: `Params`, the rule table, `hand.rs`, `conventions.rs` (the bid model, one owner), `passing.rs` (the pass models), `beliefs.rs` (Bayes inversion, `Revealed(y)`, who believes what; tier-2 cache);
  - bidding (HB, without HB-B), raising (HZ), passing (HP incl. HP-8, HK);
  - card play HC-1, HC-7, HC-8, HL-2, HL-7, HF-1–HF-4 (enough to finish games);
  - `tools/arena` (vs random) and `tools/human_eval` (bidding and passing agreement first);
  - tests per rule.
- **M2, check the rules against the 1,579 human games** (`human_eval`):
  - per phase and per rule ID: how often the rule fires and agrees with the human;
  - misconceptions: rules that humans systematically contradict, with examples (seat-view summaries, no names);
  - missed patterns: recurring human choices no rule explains, clustered by phase and situation, with counts.

  Report to `MarjaAI/findings/heuristic-vs-humans.md` with proposed fixes and new rules; then stop for the owner.
- **M3, fixes plus the PR-B scope the data supports** (HQ, HC-2–HC-6, HC-9–HC-13, HL-1, HL-3–HL-6, HF-5, HD, HB-B, HS, trust), after the owner's go. Self-play: HP1 vs HP1 and HP1 vs random on random and duplicate deals (§9.2). Then the Judge's final review and the single `--no-ff` merge into `main`.

Not in v1:
- search;
- learned models;
- the logic-language export (keep `rules.rs` as data so the export is a generator later).

---

## 10.1 M1 implementation notes (coder, 2026-10-06)

What the code adds to, or reads into, the text above. Each item has a rule ID in `rules.rs` and a scenario test.

- **Engine (owner of rules and scoring):** `bits::takes` is now public, so play code compares cards with the engine's own trick order; `GameFinishedInfo::scores()` scores a game by the rules page (assumption: a pass game scores each team's rounded points).
- **Trust and floor (netspec C12):** `P_read = θ·P_conv + (1−θ)·P_base`; the floor ε is relative to the most likely hand class for bids, and ε × the uniform choice for passes (an absolute floor erased the information; tuning log).
- **HR-9 (M1 part):** an early side-suit lead lowers P(leader void there). "Leading trump means a trump majority" needs trump counting (HC-11, M3).
- **Bid model (§1.4):** besides the rule, the literal step and a generic step, `bid_likelihood` has a block component (HR-7), a class-independent mass on exactly 140 and no literal reading of a step that lands on 140 (HR-3), and fewer unexplained jumps above 140 for seats without a pair (HR-4).
- **HB-0 bookkeeping** (C10/C11 lesson): the ace and the halves are worth telling only in the first `tell_window` = 4 bidding actions, and an ace alone not at ≥ 140 unless the team showed a pair.
- **HB-8 (new): take the game.** With nothing left to tell, bid +5 when EGP clears it by `take_margin`. Without it a strong team stops once its messages are used up.
- **HQ-0 (M1 stopgap, replaced by §6.1.1 in M3; kept behind `meld_stopgap` for A/B runs):** when leading, announce my most valuable pair, else ask "Yours" if P(partner pair) ≥ `ask_pair_conf`. Ablation (`meld_stopgap = false`): HP1 loses 172 ± 13 points per deal pair against full HP1 and makes 2.4 % of its contracts instead of 57 %.
- **HA-1:** answering "Yours" with several pairs names the most valuable (until HC-6). **HX-1/HX-2:** a single legal action; undo requests are declined.
- **HL-F (fallback lead):** with no standing card, lead the card with the least keep value plus points; a trump costs `trump_lead_cost` extra.
- **HC-1 when taking a trick:** a pair half does not take a trick while another card can be played; once I surely take the lead, halves are valued with `P_activate` "entry".
- **HP-6 / HP-2 "the partner knows":** a message I sent that covers the pair's class (SmallOrHalves: Green/Acorns; BigPair: Bells/Red; RedPair: Red), and the ace if I sent Ace. The posterior alone is too flat (+10 is also "3 halves").
- **HP-8:** Lusche minus tens and halves in a blanked suit (tuning log).
- **Pass reads:** VH reads MH's pass forth (128 samples), MH reads VH's pass back (8 samples). The opponents' reading of a hidden pass is not in M1.
- **Tier-2 cache:** bid-class features are cached by (auction length, passes, my start hand), pass samples by the pass read; everything that depends on current facts (`CardBeliefs`, clipping, play reads, the consistency filter on samples) is recomputed at every decision.

## 10.2 M2 changes (coder, 2026-10-06; Judge M1 items 1–4, 8)

- **Agent interface** in `crates/agent` (§1.1).
- **Rule labels name the rule that decided:**
  - HC-1 when the keep values changed the choice;
  - HC-7 when an A or 10 was available and not fed (forced feeds are a fact);
  - HL-7 when the unfiltered lead was a certain loser;
  - for passing, the term that separates the chosen option from the runner-up.
- **EGP** is a linear model of eight features (team aces, tens, halves; my longest suit and voids; expected melds; P(partner ace)), fitted on the human declarers' realised points (`findings/heuristic-vs-humans.md` §3.1). HP1's own ambitions keep margins: `take_margin` 30, `raise_margin` 80.
- **Human replay** uses the engine's legacy parser (`engine::game::parse::{parse_action, parse_pass}`, now public) and counts every skipped log event.
- **HK-4** never decides (HK-1 dominates it); it stays as a term.

## 11. Assumptions and open questions

- **A1 = Q1 (answered):** HB-140a shifts every live message from the +10/+15 landing on 140 upwards (a +5 stays 140, C39); `skip_on_any_cross` stays a switch; HR-2 decodes with context-dependent confidence.
- **A2 = Q2 (answered):** HP-6 keeps known or big pairs and passes an unknown small pair (Acorns more readily) when the partner can't know about it.
- **A3 = Q3 (answered):** HP-5 is a very weak tie-breaker on the first pass only.
- **A4:** `TwoHalves` defaults to the non-strict variant; strict is `Params.strict_two_halves`.
- **A5:** later steps by the same player follow the same priority list. A player who already sent `BigPair` and bids +5 again is read as `Ace` if the team has not sent `Ace`, otherwise as `TwoHalves`.

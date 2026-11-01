//! The rule table as data (spec §0.6): every decision names one of these rules. Kept as a
//! plain table so a logic-language export can be generated from it later.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Bidding,
    Reading,
    Raising,
    PassingForth,
    PassingBack,
    Trump,
    Play,
    Any,
}

/// One rule. `milestone` 1 = implemented on this branch now, 3 = planned (PR-B scope).
/// `source` is honest (C27): `[O]` the owner's verbatim words, `[RV]` the marjapussi.de rules
/// page, `[A]` measured on the human games, `[D]` derived (written derivation), `engine`
/// plumbing. The evidence per rule is in
/// `docs/design/heuristic-player.md`, "Rule sources".
#[derive(Clone, Copy, Debug)]
pub struct Rule {
    pub id: RuleId,
    pub code: &'static str,
    pub phase: Phase,
    /// Lower is evaluated first within a phase.
    pub priority: u8,
    pub source: &'static str,
    pub text: &'static str,
    pub milestone: u8,
}

macro_rules! rules {
    ($($id:ident $code:literal $phase:ident $prio:literal $ms:literal $source:literal $text:literal;)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum RuleId { $($id,)* }

        pub static RULES: &[Rule] = &[
            $(Rule {
                id: RuleId::$id,
                code: $code,
                phase: Phase::$phase,
                priority: $prio,
                source: $source,
                text: $text,
                milestone: $ms,
            },)*
        ];
    };
}

rules! {
    HP1 "HP-1" Any 0 1 "[D]" "choose the completed seat-local continuation with the best expected engine score on shared joint-belief deals";
    // Bidding (§3.1)
    // KB batch 1 (`findings/kb/concepts.md`)
    BTell "B-tell" Bidding 0 1 "[O]" "tell the first live message: with a pair, the ace first, then the pair; 3+ halves without a pair before the ace; a +10/+15 landing on 140 goes to 145";
    BOver140 "B-over140" Bidding 1 1 "[O]" "a step across 140 only with team_pair_cover(target) >= over_140_cover (0.85), else the message is skipped; in the tie-break, over 140 when the team pair (derived) covers 145";
    BGate "B-gate" Bidding 4 1 "[O]" "the 140 gate: block instead of telling (part 2), with strong cards and nothing to tell (part 1; thresholds proposed), or in the tie-break when game_drive >= 0.5";
    BDefend "B-defend" Bidding 3 1 "[O]" "nothing to tell, the partner passed first, many cards standing against us: 140, then +5 up to the limit";
    BQuiet "B-quiet" Bidding 5 1 "[O]" "nothing to tell: pass";
    HB8 "HB-8" Bidding 7 3 "[D]" "nothing left to tell: +5 to take the game (off until KB batch 2)";
    BTake "B-take" Bidding 6 1 "[O] [A]" "at 140 or more against an opponent: bid on when P(take) >= 0.5 (human analysis), contract_estimate gives P(make) >= 0.5 and the team likely holds a covering pair (the C43 cover team_pair_cover_derivable, Green left out, >= over_140_cover, as B-over140's tie-break; HR-4)";
    BControl "B-control" Bidding 13 1 "[O] [D]" "at 140 or more, one ownership challenge excluding truthful untold information: +5 with a pair whose suit runs from the top (control_run), against the opponents when my partner passed while telling was cheap; no extra Ace/halves evidence inferred from the control step (the owner's deal 573)";
    BOffer "B-offer" Bidding 14 1 "[O] [D]" "at 140 or more: +5 Ace information over my partner holding the game with the Red pair shown and no team ace shown, when I haven't bid and hold an ace; the Red-pair holder may spend its one ownership challenge on a +5 keep after that offer, including opponent intervention; the keep does not assert halves (the owner's deal 848)";
    HB140c "HB-140c" Bidding 10 3 "[O]" "bid exactly 140 to claim or defend without a known pair (off until KB batch 2)";
    HBB1 "HB-B1" Bidding 11 3 "[O]" "barrier bids 180/200/220 against schwarz";
    HS1 "HS-1" Bidding 12 3 "[O]" "series objective: risk from the series score";
    // Reading (§3.2, §1.3.1, §1.4): properties of the inverted generative models
    HR1 "HR-1" Reading 0 1 "[O]" "decode each step in its public context (team, order, first step, 140)";
    HR2 "HR-2" Reading 1 1 "[O] [A]" "steps follow the rule with a context-dependent confidence";
    HR3 "HR-3" Reading 2 1 "[O] [A]" "exactly 140 carries little pair information";
    HR4 "HR-4" Reading 3 1 "[O] [A]" "bids above 140 mean the team has a pair";
    HR5 "HR-5" Reading 4 1 "[O] [A]" "absence is information (negative evidence)";
    HR6 "HR-6" Reading 5 1 "[O]" "steps are counted per team";
    HR7 "HR-7" Reading 6 1 "[O]" "a barrier bid right after the other team showed a big pair is likely a block";
    HR8 "HR-8" Reading 7 1 "[D]" "a half question suggests the asker holds the other half; Yours suggests the partner has a pair";
    HR9 "HR-9" Reading 8 1 "[D]" "an early side-suit lead suggests a long or standing suit (trump majority: M3)";
    // Raising (§4)
    HZ1 "HZ-1" Raising 0 1 "[O] [A]" "after the exchange, estimate the team's points from my 9 cards and the pairs given back (contract_estimate); raise to the value with the best expected contract score";
    HZ2 "HZ-2" Raising 1 1 "[D]" "raise only within a certified own-card run and one reachable own pair; follow that route before sampled search";
    HZ3 "HZ-3" Raising 2 1 "[O]" "never raise across 140 without a certain team pair (a cap inside HZ-1)";
    HO1 "HO-1" Play 9 1 "[O] [D]" "a lead without a planned rule: the best expected points difference over the believed worlds (the trick by the engine's rules and level-0 replies, then the rest played out with both teams' activations), where it is clearly better than the valuation's lead";
    HO2 "HO-2" Play 8 1 "[O] [D]" "announce, ask or answer by route: when the trump plan activates, or when answering, another option (or playing a card instead) where its expected points difference over the believed worlds is clearly better than the rule's, each option played out with the activations of both teams (owner's games 938 and 1187)";
    // Passing (KB batch 2, C42): the store, score = Σ passing_weights × features
    PForth "P-forth" PassingForth 0 1 "[O] [A]" "pass forth: the best Σ passing_weights × features (human choice analysis, owner-requested)";
    PRoute "P-route" PassingForth 1 1 "[O] [A] [D]" "pass by route over the exchange worlds: forth, the store's best pass per set of suits it makes blank (and per whole suit, ace and kept pair), each played out through the declarer's level-0 pass back (reading the Lusche passed as blank signs) and the game; back, the store's best returns and those keeping a top-controlled run, each played out; another candidate replaces the store's pass where it is clearly better (Astra's review 2026-10-13, the owner's game 3)";
    PBack "P-back" PassingBack 0 1 "[O] [A]" "pass back: the best Σ passing_weights × features (human choice analysis, owner-requested)";
    // Trump and questions (§6.1)
    HQ1 "HQ-1" Trump 1 3 "[RV]" "cash side-suit standing cards first while HC-2 holds";
    HQ2 "HQ-2" Trump 2 3 "[RV]" "replaced by HC-2's trigger";
    HQ3 "HQ-3" Trump 3 1 "[RV]" "order mein, dein, unser";
    HQ4 "HQ-4" Trump 4 3 "[RV]" "probe for the opponents' pair";
    HQ5 "HQ-5" Trump 5 3 "[RV]" "game decided: announce no more";
    HQ6 "HQ-6" Trump 6 3 "[O]" "defence: change trump to destroy the declarers' plan";
    HQ7 "HQ-7" Trump 7 3 "[O]" "information question: expected gain from the answer, never at the cost of a needed right";

    // Common-sense invariants (§6.0)
    HC1 "HC-1" Play 0 1 "[D]" "keep the halves of a reachable pair: meld × P(team holds it) × P(activation)";
    HC2 "HC-2" Play 1 1 "[D]" "announce or ask now unless enough own leads remain for the pending questions";
    HC3 "HC-3" Play 2 3 "[D]" "team activation-order plan";
    HC4 "HC-4" Play 3 1 "[D]" "take a certified own or partner lead while preserving the announcement route";
    HC5 "HC-5" Play 4 1 "[D]" "several pairs: announce the most valuable first (v1 stand-in for the one most at risk)";
    HC6 "HC-6" Play 5 1 "[D]" "answering Yours with several pairs: name the one whose meld is most at risk (worth × P(not activated later))";
    HC7 "HC-7" Play 6 1 "[D]" "don't put an A or 10 into a trick the opponents win, unless forced (the future-trick value)";
    HV1 "HV-1" Play 13 1 "[O]" "future-trick value: keep cards that can still win, give those that cannot";
    HV2 "HV-2" Play 14 1 "[O]" "team plan: don't lead the suit of our unannounced pair while the partner's half can be forced out";
    HV0 "HV-0" Play 15 1 "[D]" "the trick now: (2 P(we take it) − 1) × its points, plus the lead it brings";
    HC8 "HC-8" Play 7 1 "[D]" "forced to beat the partner: the cheapest card that does";
    HC9 "HC-9" Play 8 1 "[D]" "contract arithmetic: a meld the contract needs is worth the contract swing; none once it is made";
    HC10 "HC-10" Play 9 3 "[D]" "deny the opponents a lead while they can announce";
    HC11 "HC-11" Play 10 3 "[D]" "trump counting";
    HC12 "HC-12" Play 11 3 "[D]" "keep a winner for the last trick";
    HC13 "HC-13" Play 12 1 "[D]" "never ask a question whose answer is known";
    // Leading (§7.1)
    HL1 "HL-1" Play 20 1 "[O]" "trick 1: schnippeln when the partner certainly holds the suit's ace";
    HL2 "HL-2" Play 21 1 "[O]" "lead a side-suit standing card that is no pair half, highest first";
    HL3 "HL-3" Play 22 1 "[O]" "lead into partner's known void when a winning ruff entry is likely";
    HL5 "HL-5" Play 24 1 "[O]" "destroy an opponent pair: lead A or 10 of its likely suit";
    HL6 "HL-6" Play 25 1 "[O]" "two cards left: keep the top trump for the last trick";
    HL7 "HL-7" Play 26 1 "[RV]" "never lead a card that certainly loses when another exists";
    // Endgame (F10)
    HE1 "HE-1" Play 28 1 "[D]" "the last tricks: the exact solver over the worlds I believe in";

    // Following (§7.2)
    HF2 "HF-2" Play 31 1 "[O] [D]" "opponent wins: play my cheapest card, discard from a hopeless suit";
    HF5 "HF-5" Reading 9 1 "[O]" "an off-suit discard in s lowers P(pair in s) for the discarder";
    // Defence (§7.3)
    HD1 "HD-1" Play 40 3 "[O]" "avoid schwarz first";

    HD3 "HD-3" Play 42 3 "[O]" "destroy their pairs";
    HD4 "HD-4" Play 43 3 "[D]" "contract lost: maximise our points";
    // Plumbing
    HX1 "HX-1" Any 0 1 "engine" "only one legal action";
    HX2 "HX-2" Any 1 1 "engine" "decline an undo request";
}

impl RuleId {
    pub fn rule(self) -> &'static Rule {
        &RULES[self as usize]
    }

    pub fn code(self) -> &'static str {
        self.rule().code
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_source_is_an_honest_tag() {
        let ok = |s: &str| s == "engine" || s.split(' ').all(|t| ["[O]", "[RV]", "[A]", "[D]"].contains(&t));
        for r in RULES {
            assert!(ok(r.source), "{}: {}", r.code, r.source);
        }
    }

    #[test]
    fn table_is_indexed_by_id_and_codes_are_unique() {
        for (i, rule) in RULES.iter().enumerate() {
            assert_eq!(rule.id as usize, i);
        }
        let mut codes: Vec<_> = RULES.iter().map(|r| r.code).collect();
        codes.sort();
        codes.dedup();
        assert_eq!(codes.len(), RULES.len());
    }
}

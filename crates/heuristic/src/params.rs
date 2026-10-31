//! Every weight and threshold of HP1 in one place (spec §8). Defaults are the "normal"
//! profile. A flat TOML file (`key = value` lines, `#` comments) overrides single keys; the
//! convention keys (bid messages and readings) go to `beliefs`.

use beliefs::BeliefParams;

use crate::HeuristicError;

macro_rules! params {
    ($($(#[doc = $doc:literal])* $name:ident: $ty:ty = $default:expr,)*) => {
        #[derive(Clone, Debug, PartialEq)]
        pub struct Params {
            $($(#[doc = $doc])* pub $name: $ty,)*
            /// The believed forms HP1 reads (`beliefs`), with the convention models it acts
            /// with (D-d: one owner of encoders and readings).
            pub beliefs: BeliefParams,
        }

        impl Default for Params {
            fn default() -> Self {
                Params { $($name: $default,)* beliefs: BeliefParams::default() }
            }
        }

        impl Params {
            fn set(&mut self, key: &str, value: &str) -> Result<(), HeuristicError> {
                match key {
                    $(stringify!($name) => {
                        self.$name = value
                            .parse()
                            .map_err(|_| HeuristicError::Params(format!("bad value for {key}: {value}")))?
                    })*
                    _ => {
                        if !self.beliefs.conventions.set(key, value).map_err(HeuristicError::Params)? {
                            return Err(HeuristicError::Params(format!("unknown key {key}")));
                        }
                    }
                }
                Ok(())
            }

            /// All parameters as a flat TOML document (the convention keys last).
            pub fn to_toml(&self) -> String {
                let mut out = String::new();
                $(out.push_str(&format!("{} = {}\n", stringify!($name), self.$name));)*
                out.push_str(&self.beliefs.conventions.to_toml());
                out
            }
        }
    };
}

params! {
    // Bidding, KB batch 1 (`findings/kb/concepts.md`): the convention's encoder and its
    // weights are `beliefs::conventions::Params`.
    /// B-over140: a step across 140 only if P(our team holds a pair covering the excess) is at
    /// least this ([O] 2026-10-06: "P(team pair) high, ~75 %").
    over_140_cover: f32 = 0.75,

    /// Pass-back plan: minimum P(partner holds a half of the suit) to favour my single half.
    partner_half_conf: f32 = 0.5,

    /// HZ-1: raise after the exchange by `contract_estimate` (the best expected contract score).
    estimated_raise: bool = true,
    /// The owner's hard time limits per decision (ms, 2026-10-31): each exchange step
    /// (Schieben), each bid, and each card play, question or answer (including the raise-or-lead
    /// choice after the exchange), respectively.
    planning_budget_ms: u32 = 5000,
    bidding_budget_ms: u32 = 200,
    play_budget_ms: u32 = 100,
    /// The time limits as wall-clock deadlines (live play). Off for reproducible evaluation:
    /// only the logical work limits (worlds, solver nodes) bound a decision, so its result
    /// does not depend on the machine's load.
    deadlines: bool = true,
    /// B-take: bid on over an opponent's contract at 140 or more by the fitted rule. Off for the
    /// deployed execution (2026-10-13, Astra's review 17.4: a restriction to reassess once the
    /// declarer play improves): at its own decisions bidding was worse than passing (branchdiff,
    /// 439 positions, both branches played out by four HP1: -33 per decision); in 175 of 201
    /// both-fail cases the team missed even the bid price, by a median 51 points; against live,
    /// off vs on +8.8 per game [+2.0, +15.5] on the development deals, and off vs live -1.3
    /// [-10.1, +7.5] on 2,000 untouched games.
    b_take: bool = false,
    /// B-take by route: take_vs_defend over the believed worlds can overrule the fitted rule.
    /// Off: with the level-0 exchange (no signals read) the playouts are pessimistic about
    /// every declarer: from the bidding position the simulated declaring team makes about 40 %
    /// of these contracts (pairs 57, cards about 80 on average) where humans who bid on made
    /// 60 % (358 of 595), even with the exchange reading the auction; from after the exchange
    /// the playout is unbiased, so the gap is in the hands believed at bidding and the
    /// simulated exchange.
    route_take: bool = false,
    /// HZ-1 by route: the raise from the team's final points over the believed worlds. Off: in
    /// 2,000 games vs live it raised 560 times (vs 205) by 37 on average and made 68 % (-6.4 per
    /// game vs without); choosing the highest target picks the hands whose playout estimate is
    /// too high (human games: estimates of 240+ overshoot by 5 to 22 points).
    route_raise: bool = false,
    /// HZ-1 by exact search (`exchange::team_points`) over the exchange's worlds, discounting the
    /// open hands' points by this margin.
    raise_search: bool = false,
    raise_search_margin: i32 = 0,
    /// P-route: the pass forth by route over the believed worlds (`passing::route_forth`). Off
    /// (Astra's review 2026-10-13, followed): the forecast lacks the team's private transfer
    /// facts in play and a receiver-readable route to the giver's kept entries, so its margins
    /// (+178 over the owner's HP-5 pass) are not yet a fair comparison.
    pass_route: bool = false,
    /// The exchange by exact search (`exchange`): the store's best passes, judged over the
    /// believed worlds by the double-dummy contract outcome, forth and back respectively.
    exchange_forth: bool = false,
    exchange_back: bool = false,
    /// The store's best passes the exchange search compares, and the believed worlds it solves.
    exchange_candidates: u32 = 8,
    exchange_worlds: u32 = 16,
    /// Compare the stored return with candidates preserving a top-controlled run.
    /// Independent of the pass-forth route; retains the store on unresolved comparisons.
    return_run_route: bool = true,

    /// HO-1: leads without a planned rule by the plan layer's chain (`outlook`).
    outlook_leads: bool = true,
    /// The valuation's trick term from the trick outcome over the believed worlds.
    outlook_trick: bool = true,
    /// The believed worlds per decision for the outlook and the routes (a compute budget; the
    /// decision deadline stops the evaluations early).
    outlook_worlds: u32 = 64,
    /// The outlook's simulated players see each believed world's hands (classic Monte-Carlo
    /// actors) instead of only the table's knowledge. Off (owner, 2026-10-12, following Astra's
    /// review): actors decide from what they know; they are improved, not given the deal.
    outlook_world_known: bool = false,
    /// HO-2: the trump plan's activation and the answer chosen by route over the believed worlds.
    outlook_activation: bool = true,

    /// HL-2: cash a side-suit standing card before the valuation leads.
    lead_cash_standing: bool = true,
    // Planned leads (M3, Judge item 5; `leads.rs`), each switchable for its A/B against the
    // valuation's lead
    /// HL-1 (P9): schnippeln in trick 1 when the partner certainly holds the suit's ace.
    lead_schnippeln: bool = true,
    /// HL-6: with two cards left, keep the top trump for the last trick.
    lead_last_trick: bool = true,
    /// HL-5: lead A or 10 into an opponent's likely pair; from this P(pair).
    lead_destroy_pair: bool = true,
    destroy_pair_conf: f32 = 0.6,
    /// HL-3: lead into partner's known void when its estimated winning entry meets conf.
    lead_plan_suit: bool = true,
    partner_trump_conf: f32 = 0.8,
    /// HL-4: lead the top trump to draw the opponents' trumps.
    lead_draw_trump: bool = false,

    // Endgame (F10 = D-c): the exact solver over believable worlds
    /// With at most this many tricks left (0: off).
    endgame_tricks: u32 = 4,
    /// Worlds solved per decision (all consistent deals if there are fewer).
    endgame_worlds: u32 = 64,
    /// The endgame search's logical work limit per decision, in solver nodes (0: none). The
    /// worlds completed within it decide; the wall-clock deadline is only the live safety stop.
    endgame_nodes: u64 = 0,
    /// The fewest completed worlds the endgame decides on; with fewer, the play rules decide.
    endgame_min_worlds: u32 = 1,
    /// Racing in the endgame (`solver::Racing`, batches of `RACE_BATCH` worlds): moves trailing
    /// the leader by more than this many paired standard errors are dropped (0: off).
    endgame_race_z: f32 = 0.0,

    // Card play (§6.0, §7): the valuation (C25)
    /// P8/P10a: the partner "may hold a pair" (worth a "Yours", and a right to keep) from this.
    partner_pair_min: f32 = 0.1,
    /// HQ-3: ask for a half only from this P(the partner holds the other half) ([A] the human
    /// games: the ask rate rises clearly only from 0.4).
    half_question_min: f32 = 0.4,
    /// P10a: the partner will plausibly win a lead if P(it holds a live ace) is at least this.
    partner_lead_conf: f32 = 0.7,

    // Action choice (C10/C11): 0 = argmax (the benchmark); > 0 = sample from softmax(score / T)
    // over the scored options (passing and card choices), for human-like play and difficulty.
    sample_temperature: f32 = 0.0,
    /// Seed of the sampling RNG (mixed with the seat and the event count; deterministic).
    seed: u64 = 0,
}

/// How a scored choice becomes an action (C10/C11): the benchmark takes the best option.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Selection {
    Argmax,
    Sample { temperature: f32 },
}

impl Params {
    /// Check the configuration before use. `from_toml` calls this after all overrides;
    /// Rust callers must call it after constructing or changing public fields.
    pub fn validate(&self) -> Result<(), HeuristicError> {
        self.beliefs.validate().map_err(HeuristicError::Params)?;
        for (name, p) in [
            ("over_140_cover", self.over_140_cover),
            ("partner_half_conf", self.partner_half_conf),
            ("destroy_pair_conf", self.destroy_pair_conf),
            ("partner_trump_conf", self.partner_trump_conf),
            ("partner_pair_min", self.partner_pair_min),
            ("partner_lead_conf", self.partner_lead_conf),
            ("half_question_min", self.half_question_min),
        ] {
            if !(0.0..=1.0).contains(&p) {
                return Err(HeuristicError::Params(format!("{name} must be finite and in [0, 1]")));
            }
        }
        if !self.sample_temperature.is_finite() || self.sample_temperature < 0.0 {
            return Err(HeuristicError::Params(
                "sample_temperature must be finite and nonnegative".into(),
            ));
        }
        if self.endgame_tricks > engine::TRICKS as u32 {
            return Err(HeuristicError::Params(format!(
                "endgame_tricks must be in 0..={}",
                engine::TRICKS
            )));
        }
        if self.endgame_tricks != 0 && self.endgame_worlds == 0 {
            return Err(HeuristicError::Params(
                "endgame_worlds must be positive when endgame_tricks is nonzero".into(),
            ));
        }
        Ok(())
    }

    /// The bid conventions and reading parameters.
    pub fn conv(&self) -> &beliefs::conventions::Params {
        &self.beliefs.conventions
    }

    /// `Argmax` when `sample_temperature` is 0, else `Sample` at that temperature.
    pub fn selection(&self) -> Selection {
        if self.sample_temperature > 0.0 {
            Selection::Sample {
                temperature: self.sample_temperature,
            }
        } else {
            Selection::Argmax
        }
    }

    /// Defaults overridden by the `key = value` lines of a flat TOML document,
    /// then validated as one complete configuration.
    pub fn from_toml(text: &str) -> Result<Params, HeuristicError> {
        let mut params = Params::default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| HeuristicError::Params(format!("expected key = value: {line}")))?;
            params.set(key.trim(), value.trim())?;
        }
        params.validate()?;
        Ok(params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_round_trip_and_override() {
        let defaults = Params::default();
        assert_eq!(Params::from_toml(&defaults.to_toml()).unwrap(), defaults);
        let p = Params::from_toml("# override\ntrust_partner = 0.75\nlead_draw_trump = true  # on\n").unwrap();
        assert_eq!(p.conv().trust_partner, 0.75);
        assert!(p.lead_draw_trump);
        assert!(Params::from_toml("nope = 1").is_err());
        assert!(Params::from_toml("w_blank = 12.5").is_err());
        assert!(Params::from_toml("pass_temperature = 1").is_err());
        assert_eq!(
            Params::from_toml("partner_half_conf = 0.7").unwrap().partner_half_conf,
            0.7
        );
        assert!(Params::from_toml("trust_partner = x").is_err());
    }
}

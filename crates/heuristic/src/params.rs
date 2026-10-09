//! Every weight and threshold of HP1 in one place (spec §8). Defaults are the "normal"
//! profile. A flat TOML file (`key = value` lines, `#` comments) overrides single keys; the
//! convention keys (bid messages, EGP weights, readings) go to `beliefs`.

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

    /// Shared hypothetical deals for return forecasts (0: C42 choice only).
    return_worlds: u32 = 8,

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

    // Card play (§6.0, §7): the valuation (C25)
    /// P8/P10a: the partner "may hold a pair" (worth a "Yours", and a right to keep) from this.
    partner_pair_min: f32 = 0.1,
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
        if self.return_worlds > 64 {
            return Err(HeuristicError::Params("return_worlds must be in 0..=64".into()));
        }
        if self.endgame_tricks != 0 && self.endgame_worlds == 0 {
            return Err(HeuristicError::Params(
                "endgame_worlds must be positive when endgame_tricks is nonzero".into(),
            ));
        }
        Ok(())
    }

    /// The bid conventions and EGP parameters.
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

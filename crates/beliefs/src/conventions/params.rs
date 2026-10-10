//! Parameters of bid conventions and their readings. Defaults are HP1's "normal" profile (moved from
//! `heuristic::Params`, D-d). A flat TOML file (`key = value` lines) overrides single keys.

macro_rules! params {
    ($($(#[doc = $doc:literal])* $name:ident: $ty:ty = $default:expr,)*) => {
        #[derive(Clone, Debug, PartialEq)]
        pub struct Params {
            $($(#[doc = $doc])* pub $name: $ty,)*
        }

        impl Default for Params {
            fn default() -> Self {
                Params { $($name: $default,)* }
            }
        }

        impl Params {
            /// The trust θ in a seat of this kind (D-f: one fitted value per kind; a human seat
            /// keeps its relation's field, both at the human value).
            pub fn trust(&self, kind: agent::SeatKind, partner: bool) -> f32 {
                match kind {
                    agent::SeatKind::Heuristic => self.trust_agent,
                    _ if partner => self.trust_partner,
                    _ => self.trust_opponent,
                }
            }

            /// Sets one key; `Ok(false)` if the key is not a convention parameter.
            /// Call `validate` after applying all overrides.
            pub fn set(&mut self, key: &str, value: &str) -> Result<bool, String> {
                match key {
                    $(stringify!($name) => {
                        self.$name = value.parse().map_err(|_| format!("bad value for {key}: {value}"))?
                    })*
                    _ => return Ok(false),
                }
                Ok(true)
            }

            /// All parameters as flat TOML lines.
            pub fn to_toml(&self) -> String {
                let mut out = String::new();
                $(out.push_str(&format!("{} = {}\n", stringify!($name), self.$name));)*
                out
            }
        }
    };
}

params! {
    // The bid encoder (HB-0 … HB-5, HB-140a)
    /// HB-5 variant: `TwoHalves` only if the pair values of the two halves' suits add up to ≥ 140.
    /// KB batch 1b: on ([O] 2026-10-07: "Strict 140+ variant is better"); the encoder and the
    /// reading use it alike (`Message::holds`).
    strict_two_halves: bool = true,
    /// HB-140a variant: shift +5 whenever a step crosses 140 (default: only when one lands on it).
    skip_on_any_cross: bool = false,
    /// HB-0: the ace and the halves are told only in the first this-many bidding actions (0 = always).
    tell_window: u32 = 4,
    /// HB-0: an ace alone is not told at 140 or more unless the team showed a pair.
    ace_at_140_needs_pair: bool = true,

    // The bid reading (HR-2 … HR-7)
    conf_general: f32 = 0.8,
    /// P12: "second +5 = two halves" is trusted like the other messages.
    conf_two_halves: f32 = 0.8,
    conf_team_first: f32 = 0.6,
    conf_partner_passed: f32 = 0.4,
    /// P3, human reading: share of Red-pair holders whose pair message is +15 instead of +20.
    /// C32: 0, humans read by the owner's convention like HP1 seats; the 0.73 measured on the
    /// human games is a human-game adjustment the owner does not want (2026-10-06).
    human_red_as_big: f32 = 0.0,
    /// Share of the non-conforming mass that still reads the step literally (no 140 shift).
    literal_share: f32 = 0.5,
    /// Likelihood floor ε of every bid convention reading: a belief never
    /// becomes a fact.
    convention_floor: f32 = 0.02,
    /// §3.4 trust θ per seat kind (D-f): human seats by relation and HP1 seats. C32: all at the
    /// design's stated start value 0.9 (§3.4, after the owner's "there could be bluffs, but its
    /// also sometimes unlikely"), none fitted.
    trust_partner: f32 = 0.9,
    trust_opponent: f32 = 0.9,
    trust_agent: f32 = 0.9,
    /// Mass of the generic step distribution on jumps other than +5 … +20.
    generic_eps: f32 = 0.1,
    /// HB-140b from the speaker's side: P(cross 140 | message, no pair of its own).
    cross_without_own_pair: f32 = 0.5,
    /// HR-4: factor on unexplained bids above 140 for a seat without a pair of its own.
    generic_cross_no_pair: f32 = 0.2,
    /// HR-3: class-independent mass of a bid of exactly 140 (a claim or a defence).
    claim_140_prior: f32 = 0.3,
    /// HR-7: P(a barrier bid is a block) right after the other team showed a big pair, else.
    block_prior: f32 = 0.3,
    block_base: f32 = 0.02,
}

impl Params {
    /// Validate the completed configuration; individual overrides may temporarily
    /// violate a joint constraint while a file is being loaded.
    pub fn validate(&self) -> Result<(), String> {
        for (name, p) in [
            ("conf_general", self.conf_general),
            ("conf_two_halves", self.conf_two_halves),
            ("conf_team_first", self.conf_team_first),
            ("conf_partner_passed", self.conf_partner_passed),
            ("human_red_as_big", self.human_red_as_big),
            ("literal_share", self.literal_share),
            ("trust_partner", self.trust_partner),
            ("trust_opponent", self.trust_opponent),
            ("trust_agent", self.trust_agent),
            ("generic_eps", self.generic_eps),
            ("cross_without_own_pair", self.cross_without_own_pair),
            ("generic_cross_no_pair", self.generic_cross_no_pair),
            ("claim_140_prior", self.claim_140_prior),
            ("block_prior", self.block_prior),
            ("block_base", self.block_base),
        ] {
            if !(0.0..=1.0).contains(&p) {
                return Err(format!("{name} must be finite and in [0, 1]"));
            }
        }
        if !(self.convention_floor > 0.0 && self.convention_floor <= 1.0) {
            return Err("convention_floor must be finite and in (0, 1]".into());
        }
        for (name, block) in [("block_prior", self.block_prior), ("block_base", self.block_base)] {
            if block + self.claim_140_prior > 1.0 {
                return Err(format!("{name} + claim_140_prior must not exceed 1"));
            }
        }
        Ok(())
    }
}

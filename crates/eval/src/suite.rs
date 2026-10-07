//! Fixed deal suites: a name, a base seed and a number of deals. The same suite gives the same
//! deals on every machine, so A/B runs compare like with like.
//!
//! - `dev` (seed 1): for development and tuning;
//! - `check` (seed 2): the second seed every A/B reports next to `dev` (Judge J17 item 1);
//! - `holdout` (seed 3): never used for tuning; for the final numbers only.
//!
//! Any other base seed works as `seed<N>`. Deal `i` is `deal(seed × 1,000,003 + i)`, the
//! scheme the arena has used since M1, so earlier runs reproduce.

use engine::Card;

use crate::deal;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DealSuite {
    pub name: String,
    pub seed: u64,
    pub deals: u64,
}

impl DealSuite {
    /// A suite by name (`dev`, `check`, `holdout`, `seed<N>`) with `deals` deals.
    pub fn named(name: &str, deals: u64) -> Result<DealSuite, String> {
        let seed = match name {
            "dev" => 1,
            "check" => 2,
            "holdout" => 3,
            other => other
                .strip_prefix("seed")
                .and_then(|n| n.parse().ok())
                .ok_or_else(|| format!("unknown deal suite {other} (dev, check, holdout, seed<N>)"))?,
        };
        Ok(DealSuite {
            name: name.into(),
            seed,
            deals,
        })
    }

    /// The seed of deal `i`.
    pub fn deal_seed(&self, i: u64) -> u64 {
        self.seed.wrapping_mul(1_000_003).wrapping_add(i)
    }

    /// The four hands of deal `i` (seat 0 opens the auction).
    pub fn hands(&self, i: u64) -> [Vec<Card>; 4] {
        deal(self.deal_seed(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suites_are_fixed_and_distinct() {
        let dev = DealSuite::named("dev", 10).unwrap();
        assert_eq!(dev.hands(3), DealSuite::named("seed1", 10).unwrap().hands(3));
        assert_ne!(dev.hands(3), DealSuite::named("check", 10).unwrap().hands(3));
        assert!(DealSuite::named("nope", 1).is_err());
    }
}

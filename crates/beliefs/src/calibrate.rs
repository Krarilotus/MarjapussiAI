//! Calibration per family (F8): a temperature T per read family, `p' = σ(logit(p) / T)`, fitted
//! on held-out human games (`beliefs_study`) so that "believed" is right ≥ 80 % before a
//! decision relies on it. T = 1 leaves a family as computed. Facts are never touched.

/// The read families calibrated separately.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Family {
    /// Cards other than aces, kings and obers.
    OtherCard,
    /// Aces, kings and obers.
    FeatureCard,
    Ace,
    HalvesAtLeast1,
    HalvesAtLeast2,
    HalvesAtLeast3,
    Half,
    Pair,
}

pub const FAMILIES: [Family; 8] = [
    Family::OtherCard,
    Family::FeatureCard,
    Family::Ace,
    Family::HalvesAtLeast1,
    Family::HalvesAtLeast2,
    Family::HalvesAtLeast3,
    Family::Half,
    Family::Pair,
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Calibration {
    /// Per family (in `FAMILIES` order).
    pub temperature: [f32; 8],
}

impl Default for Calibration {
    /// None (C32: no calibration fitted on games).
    fn default() -> Self {
        Calibration::NONE
    }
}

impl Calibration {
    /// No calibration (T = 1 everywhere).
    pub const NONE: Calibration = Calibration { temperature: [1.0; 8] };

    /// The calibrated belief (a fact stays a fact).
    pub fn apply(&self, family: Family, p: f64) -> f64 {
        let t = self.temperature[family as usize] as f64;
        if p <= 0.0 || p >= 1.0 || t == 1.0 {
            return p;
        }
        scale(p, t)
    }
}

/// `σ(logit(p) / t)`.
pub fn scale(p: f64, t: f64) -> f64 {
    let z = (p / (1.0 - p)).ln() / t;
    1.0 / (1.0 + (-z).exp())
}

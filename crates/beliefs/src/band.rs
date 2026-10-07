//! Read-out bands (F8): a yes/no classification of a probability, for decisions that need one.
//! Applied to the concept the decision asks about, never to its input cards first.

/// Thresholds of the bands: one named parameter (F8; S8: "believed" right ≥ 80 % on cards).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bands {
    /// At or below: believed no.
    pub low: f32,
    /// At or above: believed yes.
    pub high: f32,
}

impl Default for Bands {
    fn default() -> Self {
        Bands { low: 0.2, high: 0.8 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Band {
    /// A fact: 0.
    KnownNo,
    BelievedNo,
    Open,
    BelievedYes,
    /// A fact: 1.
    KnownYes,
}

impl Band {
    /// The band of `p`. Only facts are exactly 0 or 1 (`Mind` guarantees it).
    pub fn of(p: f32, bands: Bands) -> Band {
        if p <= 0.0 {
            Band::KnownNo
        } else if p >= 1.0 {
            Band::KnownYes
        } else if p <= bands.low {
            Band::BelievedNo
        } else if p >= bands.high {
            Band::BelievedYes
        } else {
            Band::Open
        }
    }

    pub fn is_known(self) -> bool {
        matches!(self, Band::KnownNo | Band::KnownYes)
    }
}

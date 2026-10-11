//! The believed forms (layer 3b): one `Mind` per agent seat, nothing shared between agents.
//! Design: `docs/design/beliefs.md`; binding decisions `reasoning-system-v2.md` §8 F1–F12.
//!
//! A `Mind` is built from its seat's start hand and its own `SeatEvent`s only. Known forms stay
//! on `Knowledge` (K1–K6); a believed read is exactly 1 or 0 wherever the facts decide it and
//! strictly between otherwise. Values are computed once per (fact epoch, signal epoch); reads
//! are lookups.

mod band;
mod calibrate;
mod concepts;
pub mod conventions;
mod deals;
pub mod factors;
mod features;
mod mind;
mod perspective;
mod reads;
pub mod record;
mod table;
mod view;

pub use band::{Band, Bands};
pub use calibrate::{scale, Calibration, Family, FAMILIES};
pub use concepts::{Concept, ConceptBudget, ConceptError, ConceptEstimate, ConceptQuery, Literal, ProbabilityBounds};
pub use deals::{DealModel, DealWeights};
pub use features::{Kernel, N_FEATURES, UNIFORM_STAY};
pub use mind::{BeliefParams, Epochs, FeatureCards, Mind, PerKind, Signals};
pub use perspective::{PerspectiveBudget, PerspectiveEstimate};
pub use reads::{CountDist, SeatFeatures};
pub use view::View;

/// A card's probability table: `[seat][card]`.
pub type Table = [[f32; engine::CARDS]; 4];

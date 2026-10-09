//! Importance weights over fact-consistent deals, shared by short and endgame search.

use knowledge::{Belief, Hands};

use crate::Mind;

/// Product of believed/factual card marginals. This is an approximate likelihood,
/// not a joint posterior; the deal space still enforces all hard constraints.
pub struct DealWeights([[f64; 36]; 4]);

impl Mind {
    pub fn deal_weights(&self) -> DealWeights {
        let facts = self.knowledge().deals().marginals();
        DealWeights(std::array::from_fn(|y| {
            std::array::from_fn(|c| {
                let base = facts[y][c];
                if y as u8 == self.seat() || base <= 0.0 {
                    1.0
                } else {
                    self.p_holds(y as u8, c as u8) as f64 / base
                }
            })
        }))
    }
}

impl Belief for DealWeights {
    fn weight(&self, hands: &Hands) -> f64 {
        hands
            .iter()
            .enumerate()
            .map(|(y, h)| h.iter().map(|c| self.0[y][c as usize]).product::<f64>())
            .product()
    }
}

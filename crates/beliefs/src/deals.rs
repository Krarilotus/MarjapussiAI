//! Joint signal likelihoods over fact-consistent deals, shared by planning and search.

use std::cell::RefCell;

use engine::CardSet;
use knowledge::{Belief, Hands, Passed};

use crate::conventions::HandClass;
use crate::factors::Factors;
use crate::features::{ExchangeTerms, Kernel, Likelihood};
use crate::{Mind, Signals};

/// The feature model's dealt-hand likelihood, not a product of posterior card
/// marginals. A pair signal constrains a combination of cards, not two independent
/// cards. The deal space supplies the hard constraints and uniform prior.
///
/// Hidden exchanges retain the feature model's exchange-kernel approximation.
/// Optional per-card signal factors multiply that joint likelihood once. Marginal
/// calibration is a reporting transform, not an additional source of evidence.
pub struct DealWeights {
    likelihoods: [Likelihood; 4],
    certain: Hands,
    dealt: Hands,
    played: Hands,
    hidden_exchange: Option<[usize; 2]>,
    kernel: Kernel,
    exchange: RefCell<ExchangeTerms>,
    factors: [[f32; 36]; 4],
}

impl Mind {
    pub fn deal_weights(&self) -> DealWeights {
        self.signals();
        let k = self.knowledge();
        let likelihoods = self.cache.borrow().signals.as_ref().expect("computed above").2.clone();
        let hidden_exchange = k.first_passer().and_then(|first| {
            let second = engine::partner(first);
            (k.given(first) == Passed::Hidden || k.received(first) == Passed::Hidden)
                .then_some([first as usize, second as usize])
        });
        let kernel = match (self.params.signals, k.first_passer()) {
            (Signals::Full, Some(first)) => self.params.kernel.of(self.kinds[first as usize]),
            _ => Kernel::UNIFORM,
        };
        let factors = match self.params.signals {
            Signals::Full => Factors::table(
                &self.params.factors,
                k,
                self.kinds,
                self.params.conventions.convention_floor,
            ),
            Signals::BaseOnly => [[1.0; 36]; 4],
        };
        DealWeights {
            likelihoods,
            certain: std::array::from_fn(|s| k.certain(s as u8)),
            dealt: std::array::from_fn(|s| k.dealt_certain(s as u8)),
            played: std::array::from_fn(|s| k.played_by(s as u8)),
            hidden_exchange,
            kernel,
            exchange: RefCell::new(ExchangeTerms::default()),
            factors,
        }
    }
}

impl Belief for DealWeights {
    fn weight(&self, hands: &Hands) -> f64 {
        let mut weight = 1.0;
        for (seat, hand) in hands.iter().enumerate() {
            if !self.hidden_exchange.is_some_and(|pair| pair.contains(&seat)) {
                if let Some(lik) = &self.likelihoods[seat] {
                    // Known transfers and plays already map certain cards back
                    // to their dealt owner. Only still-unknown cards are added;
                    // the current hand class would read the wrong hand.
                    let dealt = self.dealt[seat] | (*hand - self.certain[seat]);
                    weight *= lik[HandClass::of(dealt).index()] as f64;
                }
            }
            weight *= hand
                .iter()
                .map(|c| self.factors[seat][c as usize] as f64)
                .product::<f64>();
        }
        if let Some(pair) = self.hidden_exchange {
            let lik = pair.map(|s| self.likelihoods[s].as_deref());
            if lik.iter().any(Option::is_some) {
                let held = pair.map(|s| hands[s] | self.played[s]);
                weight *= self.exchange.borrow_mut().likelihood(held, lik, self.kernel);
            }
        }
        weight
    }
}

impl DealWeights {
    /// Refine allocation classes by every role read by the joint model and query.
    /// This is the same weight owner as sampling/search, including factors once.
    pub(crate) fn allocation_labels(&self, masks: &[CardSet]) -> [u64; 36] {
        let features = self.likelihoods.iter().any(Option::is_some);
        let mut keys: Vec<(u8, [u32; 4], Vec<bool>)> = Vec::new();
        std::array::from_fn(|c| {
            let c = c as u8;
            let role = if !features {
                0
            } else if CardSet::ACES.contains(c) {
                1
            } else if CardSet::HALVES.contains(c) {
                2 + engine::suit_of(c) as u8
            } else {
                0
            };
            let key = (
                role,
                self.factors.map(|row| row[c as usize].to_bits()),
                masks.iter().map(|m| m.contains(c)).collect(),
            );
            if let Some(i) = keys.iter().position(|k| *k == key) {
                i as u64
            } else {
                keys.push(key);
                (keys.len() - 1) as u64
            }
        })
    }

    /// Conservative per-deal ceiling. Normalised bid likelihoods and their
    /// exchange-kernel average are at most one; factors may exceed one.
    pub(crate) fn weight_ceiling(&self, live: CardSet) -> f64 {
        // Allow for rounding in finite exchange-kernel summation/products.
        live.iter()
            .map(|c| {
                self.factors
                    .iter()
                    .map(|row| row[c as usize] as f64)
                    .fold(0.0, f64::max)
            })
            .product::<f64>()
            * (1.0 + 1e-10)
    }
}

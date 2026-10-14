//! Joint signal likelihoods over fact-consistent deals, shared by planning and search.

use std::{
    cell::{Ref, RefCell},
    sync::{Arc, OnceLock},
};

use engine::CardSet;
use knowledge::{Belief, DealSpace, Hands, Knowledge, Passed};

use crate::conventions::HandClass;
use crate::factors::Factors;
use crate::features::{ExchangeTerms, Kernel, Likelihood};
use crate::{Mind, Signals};

/// Fact-consistent allocations and their joint evidence weights from one Mind
/// epoch. Inputs are immutable; the fact DP is constructed only on its first
/// read, and weighting memoizes exchange calculations.
/// Its immutable fact DP is shared with exact marginal reads in this Mind only.
pub struct DealModel {
    facts: Arc<FactSpace>,
    weights: DealWeights,
}

/// One lazy, unweighted DP for an own/public fact-epoch pair. Sharing this small
/// owner avoids copying the DP or constructing belief weights for exact reads.
/// OnceLock preserves Mind's Send contract; no process-global cache is involved.
pub(crate) struct FactSpace {
    knowledge: Knowledge,
    space: OnceLock<DealSpace>,
}

impl FactSpace {
    pub(crate) fn space(&self) -> &DealSpace {
        self.space.get_or_init(|| {
            let _timing = crate::profile::scope("belief.deal_space");
            self.knowledge.deals()
        })
    }
}

impl DealModel {
    /// Construct the fact DP once; weights-only consumers do not pay for it.
    pub fn space(&self) -> &DealSpace {
        self.facts.space()
    }

    pub fn weights(&self) -> &DealWeights {
        &self.weights
    }
}

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
    pub(crate) fn fact_space(&self) -> Arc<FactSpace> {
        // Epochs are identities within this Mind, not portable cache keys.
        // Undo advances them; signal-only changes cannot change the fact DP.
        let key = (self.epochs().fact, self.public.fact_epoch());
        let mut slot = self.fact_space.borrow_mut();
        if slot.as_ref().map(|(epoch, _)| *epoch) != Some(key) {
            *slot = Some((
                key,
                Arc::new(FactSpace {
                    knowledge: *self.knowledge(),
                    space: OnceLock::new(),
                }),
            ));
        }
        Arc::clone(&slot.as_ref().expect("computed above").1)
    }

    /// Reuse one weight model per (own epochs, public fact epoch), preserving the
    /// lazy fact DP across signal-only refreshes while both fact epochs match.
    /// Hidden-exchange sums survive refreshes only when their ordered passers,
    /// dealt-class likelihoods and kernel are unchanged.
    /// The model's RefCell is separate from the query/derived caches; callbacks
    /// do not hold a borrow of those caches. Drop this Ref before applying events
    /// or any operation that would replace this epoch's model.
    pub fn deal_model(&self) -> Ref<'_, DealModel> {
        let key = (self.epochs(), self.public.fact_epoch());
        if self.model.borrow().as_ref().map(|(e, p, _)| (*e, *p)) != Some(key) {
            let mut weights = self.deal_weights();
            let mut slot = self.model.borrow_mut();
            if let Some((_, _, previous)) = slot.as_mut() {
                weights.reuse_exchange(&mut previous.weights);
            }
            *slot = Some((
                key.0,
                key.1,
                DealModel {
                    facts: self.fact_space(),
                    weights,
                },
            ));
        }
        Ref::map(self.model.borrow(), |model| &model.as_ref().expect("computed above").2)
    }

    /// Construct independent weights for legacy callers. Use deal_model() when
    /// several reads should retain the same fact DP and exchange memoization.
    pub fn deal_weights(&self) -> DealWeights {
        let _timing = crate::profile::scope("belief.weights");
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
    /// Transfer memoized dealt-split sums, never the previous model's facts or
    /// weights. A term reads only held feature counts, both passer likelihoods
    /// and the ordered kernel. Current/played cards still select its held-count
    /// key in `weight`; rights, factors and other seats' signals do not enter it.
    /// The caller confines this reuse to consecutive models of the same Mind.
    fn reuse_exchange(&mut self, previous: &mut Self) {
        let Some(pair) = self.hidden_exchange else { return };
        if previous.hidden_exchange == Some(pair)
            && previous.kernel == self.kernel
            && pair
                .iter()
                .all(|&seat| previous.likelihoods[seat] == self.likelihoods[seat])
        {
            self.exchange = std::mem::take(&mut previous.exchange);
        }
    }

    /// Acceptance density for the named conditional legal-history completion.
    /// This is NOT the inverse of the marginalized feature kernel: it conditions
    /// concrete dealt owners/transfer paths on legal nine-card starting hands.
    /// Callers sample histories with this density, keeping current W(C) unchanged.
    /// Do not multiply this into current allocation weights a second time.
    ///
    /// Uniform reverse-transfer proposals distinguish ordinary cards uniformly.
    /// Feature stay factors and bid-class likelihoods use this model's owners.
    /// Dividing each stay factor by its maximum is a constant for fixed current
    /// hands, giving a tighter rejection envelope without changing conditional Q.
    /// Returns a finite [0,1] density, or None for invalid/mismatched inputs.
    pub fn history_acceptance(&self, hands: &Hands, dealt: &Hands) -> Option<f64> {
        let pair = self.hidden_exchange?;
        let held = pair.map(|seat| hands[seat] | self.played[seat]);
        let original = pair.map(|seat| dealt[seat]);
        if original.iter().any(|h| h.len() != engine::HAND_SIZE as u32)
            || !(original[0] & original[1]).is_empty()
            || !(held[0] & held[1]).is_empty()
            || original[0] | original[1] != held[0] | held[1]
        {
            return None;
        }
        let mut density = 1.0;
        for (holder, hand) in held.iter().enumerate() {
            for card in (*hand & (CardSet::ACES | CardSet::HALVES)).iter() {
                let role = usize::from(!CardSet::ACES.contains(card));
                let stay = self.kernel.stay[role][holder] as f64;
                if !stay.is_finite() || !(0.0..=1.0).contains(&stay) {
                    return None;
                }
                let factor = if original[holder].contains(card) {
                    stay
                } else {
                    1.0 - stay
                };
                density *= factor / stay.max(1.0 - stay);
            }
            if let Some(likelihood) = &self.likelihoods[pair[holder]] {
                let factor = likelihood[HandClass::of(original[holder]).index()] as f64;
                if !factor.is_finite() || !(0.0..=1.0).contains(&factor) {
                    return None;
                }
                density *= factor;
            }
        }
        (density.is_finite() && (0.0..=1.0).contains(&density)).then_some(density)
    }

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

#[cfg(test)]
#[path = "../tests/internal/deal_model.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/internal/history_density.rs"]
mod history_tests;

#[cfg(test)]
#[path = "../tests/internal/exchange_reuse.rs"]
mod exchange_reuse_tests;

//! Conditional legal-history completion of an already weighted current cohort.
//! Q is a named model extension, not an inverse of the marginalized feature prior.
//! Every completed slot retains W(C); history sampling never reweights its cards.

use agent::{HistoryReplayError, HypothesizedPass, SeatView, Table};
use beliefs::DealWeights;
use engine::CardSet;
use knowledge::{Hands, World};
use observation::SeatEvent;

use crate::Rng;

pub(crate) const MODEL: &str = "legal-exchange-history-feature-bid-v1";
const RNG_DOMAIN: u64 = 0x4849_5354_4F52_5931;

#[derive(Clone, Copy, Debug)]
pub(crate) struct HistoryLimits {
    pub max_proposals_per_slot: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HistoryError {
    Interrupted,
    ProposalLimit { slot: usize, proposals: usize },
    InvalidHistory,
    InvalidWeight,
    InvalidModel,
    Replay(HistoryReplayError),
}

pub(crate) struct CompletedHistories {
    pub tables: Vec<(Table, f64)>,
    pub model: &'static str,
    pub proposals: usize,
}

pub(crate) fn has_hidden_passes(view: &SeatView<'_>) -> bool {
    view.events
        .iter()
        .any(|e| matches!(e, SeatEvent::Pass { cards: None, .. }))
}

fn check(keep_going: &mut impl FnMut() -> bool) -> Result<(), HistoryError> {
    if keep_going() {
        Ok(())
    } else {
        Err(HistoryError::Interrupted)
    }
}

/// Uniform bounded integer, without modulo bias. This stream is never shared
/// with the current-cohort draws or systematic-resampling offset.
fn below(rng: &mut Rng, n: u64) -> u64 {
    let ceiling = u64::MAX - u64::MAX % n;
    loop {
        let value = rng.next();
        if value < ceiling {
            return value % n;
        }
    }
}

fn four(mut available: CardSet, rng: &mut Rng) -> Result<CardSet, HistoryError> {
    if available.len() < 4 || !available.is_subset(CardSet::ALL) {
        return Err(HistoryError::InvalidHistory);
    }
    let mut selected = CardSet::EMPTY;
    for _ in 0..4 {
        let card = available
            .iter()
            .nth(below(rng, available.len() as u64) as usize)
            .unwrap();
        selected |= CardSet::single(card);
        available -= CardSet::single(card);
    }
    Ok(selected)
}

/// Reverse plays before transfers. Each hidden reverse transfer draws one
/// uniform four-subset of its then-recipient hand. For one hidden pass there
/// are C(13,4) equiprobable paths; after two, C(9,4)*C(13,4). Returned cards are
/// represented naturally. Only the supplied observer events are read.
fn propose(
    view: &SeatView<'_>,
    mut hands: Hands,
    rng: &mut Rng,
    keep_going: &mut impl FnMut() -> bool,
) -> Result<(Hands, Vec<HypothesizedPass>), HistoryError> {
    let mut assignments = Vec::new();
    for (event_index, event) in view.events.iter().enumerate().rev() {
        check(keep_going)?;
        match *event {
            SeatEvent::CardPlayed { seat, card } => {
                if seat >= 4 || card >= engine::CARDS as u8 || hands.iter().any(|hand| hand.contains(card)) {
                    return Err(HistoryError::InvalidHistory);
                }
                hands[seat as usize] |= CardSet::single(card);
            }
            SeatEvent::Pass { from, to, cards } => {
                if from >= 4 || to != engine::partner(from) {
                    return Err(HistoryError::InvalidHistory);
                }
                let cards = match cards {
                    Some(cards) => cards,
                    None => {
                        let cards = four(hands[to as usize], rng)?;
                        assignments.push(HypothesizedPass { event_index, cards });
                        cards
                    }
                };
                if cards.len() != 4 || !cards.is_subset(hands[to as usize]) {
                    return Err(HistoryError::InvalidHistory);
                }
                hands[to as usize] -= cards;
                hands[from as usize] |= cards;
            }
            SeatEvent::Undone => return Err(HistoryError::InvalidHistory),
            _ => {}
        }
    }
    if assignments.len() > 2 || hands.iter().any(|h| h.len() != engine::HAND_SIZE as u32) {
        return Err(HistoryError::InvalidHistory);
    }
    assignments.reverse();
    Ok((hands, assignments))
}

/// Complete every ordered slot or return no usable cohort. Duplicate current
/// hands must reach this function as separate slots: their sampled histories
/// may differ. No sorting, coalescing, renormalization or positive-mass removal.
///
/// Uniform reverse paths are accepted with DealWeights' bounded density. This
/// samples normalized Q without enumerating all histories or reapplying its
/// likelihood to W(C). A finite proposal cap is a completion limit, not pruning.
/// Failed exact replay rejects the entire cohort: no world may disappear.
/// The caller must label history sampling even when current deals were enumerated.
pub(crate) fn complete_slots(
    view: &SeatView<'_>,
    slots: &[World],
    weights: &DealWeights,
    history_seed: u64,
    limits: HistoryLimits,
    keep_going: &mut impl FnMut() -> bool,
) -> Result<CompletedHistories, HistoryError> {
    check(keep_going)?;
    let hidden = has_hidden_passes(view);
    let mut rng = Rng::new(history_seed ^ RNG_DOMAIN);
    let mut tables = Vec::with_capacity(slots.len());
    let mut proposals = 0;
    for (slot, world) in slots.iter().enumerate() {
        check(keep_going)?;
        if !world.weight.is_finite() || world.weight <= 0.0 {
            return Err(HistoryError::InvalidWeight);
        }
        let assignments = if hidden {
            let mut accepted = None;
            for _ in 0..limits.max_proposals_per_slot {
                check(keep_going)?;
                let (dealt, assignments) = propose(view, world.hands, &mut rng, keep_going)?;
                proposals += 1;
                let density = weights
                    .history_acceptance(&world.hands, &dealt)
                    .ok_or(HistoryError::InvalidModel)?;
                check(keep_going)?;
                let uniform = (rng.next() >> 11) as f64 / (1u64 << 53) as f64;
                if uniform < density {
                    accepted = Some(assignments);
                    break;
                }
            }
            accepted.ok_or(HistoryError::ProposalLimit {
                slot,
                proposals: limits.max_proposals_per_slot,
            })?
        } else {
            Vec::new()
        };
        let table =
            Table::from_view_with_passes_checked(view, world.hands, &assignments, keep_going).map_err(|error| {
                match error {
                    HistoryReplayError::Interrupted => HistoryError::Interrupted,
                    other => HistoryError::Replay(other),
                }
            })?;
        tables.push((table, world.weight));
    }
    check(keep_going)?;
    Ok(CompletedHistories {
        tables,
        model: MODEL,
        proposals,
    })
}

#[cfg(test)]
#[path = "../../tests/internal/history_completion.rs"]
mod tests;

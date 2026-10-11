//! Cooperative preparation of one complete, ordered cohort. No prefix escapes.
//! Checks neither consume RNG nor reorder likelihood calls or floating reductions.

use agent::{SeatView, Table};
use knowledge::{Belief, DealSpace, World};

use crate::concepts::value::CohortMethod;
use crate::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PreparationError {
    Deferred,
    InvalidCohort,
}

impl PreparationError {
    /// Operational interruption is not evidence of inconsistent beliefs.
    /// A bool checker cannot identify every stopping lane: do not invent one.
    pub(crate) fn reason(self) -> crate::concepts::sequence::SequenceError {
        use crate::concepts::sequence::SequenceError;
        match self {
            Self::Deferred => SequenceError::Budget,
            Self::InvalidCohort => SequenceError::InvalidCohort,
        }
    }
}

pub(super) fn check(keep_going: &mut impl FnMut() -> bool) -> Result<(), PreparationError> {
    if keep_going() {
        Ok(())
    } else {
        Err(PreparationError::Deferred)
    }
}

/// Complete proposals only. The small enumeration walker has no early-stop API;
/// after interruption it finishes traversing, but performs no further weights.
pub(super) fn proposal_pool(
    space: &DealSpace,
    weights: &(impl Belief + ?Sized),
    rng: &mut Rng,
    n: usize,
    keep_going: &mut impl FnMut() -> bool,
) -> Result<(Vec<World>, u64, usize), PreparationError> {
    let _timing = beliefs::profile::scope("forecast.cohort.proposals");
    check(keep_going)?;
    if n == 0 {
        return Err(PreparationError::InvalidCohort);
    }
    let count = space.count();
    let mut pool = Vec::new();
    let draws = if count <= n as u64 {
        let mut deferred = false;
        space.for_each_deal(|hands| {
            if deferred {
                return;
            }
            if !keep_going() {
                deferred = true;
                return;
            }
            let weight = weights.weight(hands);
            if !keep_going() {
                deferred = true;
                return;
            }
            pool.push(World { hands: *hands, weight });
        });
        if deferred {
            return Err(PreparationError::Deferred);
        }
        0
    } else {
        let draws = n * 256; // Preserve the existing proposal count exactly.
        for _ in 0..draws {
            check(keep_going)?;
            let Some(hands) = space.sample(rng) else {
                break;
            };
            check(keep_going)?;
            let weight = weights.weight(&hands);
            check(keep_going)?;
            pool.push(World { hands, weight });
        }
        draws
    };
    check(keep_going)?;
    Ok((pool, count, draws))
}

pub(super) struct SelectedWorlds {
    /// Legacy merged cohort, in the original encounter/reduction order.
    pub worlds: Vec<World>,
    /// Ordered selections before duplicate merging. Assign private histories
    /// to these slots before any coalescing; identical hands need not mean history.
    pub slots: Vec<World>,
    pub cohort: CohortMethod,
}

pub(super) fn select_worlds(
    mut pool: Vec<World>,
    allocation_count: u64,
    proposal_draws: usize,
    n: usize,
    rng: &mut Rng,
    keep_going: &mut impl FnMut() -> bool,
) -> Result<SelectedWorlds, PreparationError> {
    let _timing = beliefs::profile::scope("forecast.cohort.reduction");
    check(keep_going)?;
    if n == 0 || pool.iter().any(|w| !w.weight.is_finite() || w.weight < 0.0) {
        return Err(PreparationError::InvalidCohort);
    }
    pool.retain(|w| w.weight > 0.0);
    check(keep_going)?;
    let mass: f64 = pool.iter().map(|w| w.weight).sum();
    if !mass.is_finite() || mass <= 0.0 {
        return Err(PreparationError::InvalidCohort);
    }
    check(keep_going)?;
    let ess = mass * mass / pool.iter().map(|w| w.weight * w.weight).sum::<f64>();
    check(keep_going)?;
    let cohort = if allocation_count <= n as u64 {
        CohortMethod::Enumerated
    } else if pool.len() <= n {
        CohortMethod::ImportanceSampled {
            draws: proposal_draws,
            proposal_ess: ess,
        }
    } else {
        CohortMethod::Resampled {
            draws: proposal_draws,
            proposal_ess: ess,
        }
    };
    let (worlds, slots) = if pool.len() <= n {
        for w in &mut pool {
            check(keep_going)?;
            w.weight /= mass;
        }
        let slots = pool.clone();
        (pool, slots)
    } else {
        let step = mass / n as f64;
        check(keep_going)?;
        let start = (rng.next() >> 11) as f64 / (1u64 << 53) as f64 * step;
        let mut worlds: Vec<World> = Vec::new();
        let mut slots = Vec::with_capacity(n);
        let (mut index, mut sum) = (0, pool[0].weight);
        for i in 0..n {
            check(keep_going)?;
            let target = start + i as f64 * step;
            while sum <= target && index + 1 < pool.len() {
                check(keep_going)?;
                index += 1;
                sum += pool[index].weight;
            }
            let hands = pool[index].hands;
            slots.push(World {
                hands,
                weight: 1.0 / n as f64,
            });
            if let Some(w) = worlds.iter_mut().find(|w| w.hands == hands) {
                w.weight += 1.0 / n as f64;
            } else {
                worlds.push(World {
                    hands,
                    weight: 1.0 / n as f64,
                });
            }
        }
        (worlds, slots)
    };
    check(keep_going)?;
    Ok(SelectedWorlds { worlds, slots, cohort })
}

/// Each table is still replayed through the existing engine/private-view owner.
/// A single replay is atomic at this layer; failure drops all preceding tables.
pub(super) fn replay_worlds(
    view: &SeatView<'_>,
    worlds: Vec<World>,
    keep_going: &mut impl FnMut() -> bool,
) -> Result<Vec<(Table, f64)>, PreparationError> {
    let _timing = beliefs::profile::scope("forecast.cohort.replay");
    let mut tables = Vec::with_capacity(worlds.len());
    for world in worlds {
        check(keep_going)?;
        let table = Table::from_view(view, world.hands).ok_or(PreparationError::InvalidCohort)?;
        check(keep_going)?;
        tables.push((table, world.weight));
    }
    check(keep_going)?;
    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{ActionType, Card, CardSet};
    use knowledge::{Hands, Knowledge};
    use std::cell::RefCell;

    fn table() -> Table {
        Table::new(&std::array::from_fn(|seat| {
            (0..9).map(|i| Card::from_index((seat * 9 + i) as u8)).collect()
        }))
    }

    // Frozen original reduction, including the exact merge/addition order.
    fn original_select(mut pool: Vec<World>, n: usize, rng: &mut Rng) -> Vec<World> {
        pool.retain(|w| w.weight > 0.0);
        let mass: f64 = pool.iter().map(|w| w.weight).sum();
        if pool.len() <= n {
            for w in &mut pool {
                w.weight /= mass;
            }
            return pool;
        }
        let step = mass / n as f64;
        let start = (rng.next() >> 11) as f64 / (1u64 << 53) as f64 * step;
        let mut worlds: Vec<World> = Vec::new();
        let (mut index, mut sum) = (0, pool[0].weight);
        for i in 0..n {
            let target = start + i as f64 * step;
            while sum <= target && index + 1 < pool.len() {
                index += 1;
                sum += pool[index].weight;
            }
            let hands = pool[index].hands;
            if let Some(w) = worlds.iter_mut().find(|w| w.hands == hands) {
                w.weight += 1.0 / n as f64;
            } else {
                worlds.push(World {
                    hands,
                    weight: 1.0 / n as f64,
                });
            }
        }
        worlds
    }

    #[test]
    fn uninterrupted_draws_weights_resampling_and_rng_match_original_order() {
        let knowledge = Knowledge::new(0, CardSet::suit(engine::Suit::Acorns));
        let space = DealSpace::new(&knowledge);
        let seen = RefCell::new(Vec::new());
        let weight = |hands: &Hands| {
            seen.borrow_mut().push(*hands);
            0.25 + (hands[1].0 % 17) as f64 / 10.0
        };
        let n = 3;
        let mut original_rng = Rng::new(19);
        let original_pool = space.sample_worlds(&mut original_rng, n * 256, &weight);
        let original_order = seen.take();
        let original_worlds = original_select(original_pool.clone(), n, &mut original_rng);
        let mut checked_rng = Rng::new(19);
        let (pool, count, draws) = proposal_pool(&space, &weight, &mut checked_rng, n, &mut || true).unwrap();
        assert_eq!(pool, original_pool);
        assert_eq!(seen.take(), original_order);
        assert_eq!(draws, n * 256);
        let selected = select_worlds(pool, count, draws, n, &mut checked_rng, &mut || true).unwrap();
        assert_eq!(selected.worlds, original_worlds);
        assert_eq!(selected.slots.len(), n);
        assert_eq!(checked_rng.next(), original_rng.next());
    }

    #[test]
    fn interrupted_proposals_never_return_prefix_and_stop_weight_calls() {
        let space = DealSpace::new(&Knowledge::new(0, CardSet::suit(engine::Suit::Acorns)));
        let calls = std::cell::Cell::new(0);
        let weight = |_: &Hands| {
            calls.set(calls.get() + 1);
            1.0
        };
        let mut checks = 0;
        let result = proposal_pool(&space, &weight, &mut Rng::new(19), 3, &mut || {
            checks += 1;
            checks < 9
        });
        assert_eq!(result.unwrap_err(), PreparationError::Deferred);
        assert_eq!(calls.get(), 2); // No third likelihood after the next draw's check fails.
    }

    #[test]
    fn duplicate_slots_survive_until_history_assignment_and_legacy_merge_is_unchanged() {
        let hands = [CardSet::EMPTY; 4]; // Structural reduction fixture, no replay.
        let pool = vec![World { hands, weight: 1.0 }; 8];
        let mut original_rng = Rng::new(31);
        let expected = original_select(pool.clone(), 3, &mut original_rng);
        let mut rng = Rng::new(31);
        let selected = select_worlds(pool, 100, 768, 3, &mut rng, &mut || true).unwrap();
        assert_eq!(selected.worlds, expected);
        assert_eq!(selected.worlds.len(), 1);
        assert_eq!(selected.slots.len(), 3);
        assert!(selected.slots.iter().all(|w| w.hands == hands && w.weight == 1.0 / 3.0));
        assert_eq!(rng.next(), original_rng.next());
    }

    #[test]
    fn replay_checks_between_tables_and_drops_completed_prefix() {
        let original = table();
        let view = original.view(0);
        let hands = std::array::from_fn(|seat| CardSet::suit(engine::SUITS[seat]));
        let worlds = vec![World { hands, weight: 0.25 }, World { hands, weight: 0.75 }];
        let rebuilt = replay_worlds(&view, worlds.clone(), &mut || true).unwrap();
        for ((t, weight), w) in rebuilt.iter().zip(&worlds) {
            assert_eq!(*weight, w.weight);
            for seat in 0..4 {
                let actual = t.view(seat);
                let expected = original.view(seat);
                assert_eq!(actual.knowledge, expected.knowledge);
                assert_eq!(actual.events, expected.events);
                assert_eq!(actual.kinds, expected.kinds);
            }
        }
        let mut checks = 0;
        let result = replay_worlds(&view, worlds, &mut || {
            checks += 1;
            checks < 3
        });
        assert!(matches!(result, Err(PreparationError::Deferred)));
    }

    #[test]
    fn enumeration_order_and_rng_are_unchanged_and_interruption_keeps_no_weighted_prefix() {
        let mut table = table();
        for kind in [
            ActionType::NewBid(120),
            ActionType::StopBidding,
            ActionType::StopBidding,
            ActionType::StopBidding,
        ] {
            let action = *table
                .to_act()
                .unwrap()
                .1
                .iter()
                .find(|a| a.action_type == kind)
                .unwrap();
            table.apply(action);
        }
        for _ in 0..2 {
            let action = *table
                .to_act()
                .unwrap()
                .1
                .iter()
                .find(|a| matches!(a.action_type, ActionType::Pass(_)))
                .unwrap();
            table.apply(action);
        }
        for _ in 0..128 {
            if table.view(0).knowledge.hand_len(0) == 1 && (0..4).all(|s| table.view(0).knowledge.hand_len(s) == 1) {
                break;
            }
            let legal = table.to_act().unwrap().1;
            let action = *legal
                .iter()
                .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
                .unwrap_or(&legal[0]);
            table.apply(action);
        }
        let space = DealSpace::new(table.view(0).knowledge);
        assert!((1..=6).contains(&space.count()));
        let mut expected = Vec::new();
        space.for_each_deal(|hands| {
            expected.push(World {
                hands: *hands,
                weight: 1.0,
            })
        });
        let mut rng = Rng::new(2);
        let (pool, count, draws) = proposal_pool(&space, &knowledge::Uniform, &mut rng, 8, &mut || true).unwrap();
        assert_eq!(pool, expected);
        assert_eq!(count, space.count());
        assert_eq!(draws, 0);
        assert_eq!(rng.next(), Rng::new(2).next());
        let mut checks = 0;
        assert!(matches!(
            proposal_pool(&space, &knowledge::Uniform, &mut Rng::new(2), 8, &mut || {
                checks += 1;
                checks < 3
            }),
            Err(PreparationError::Deferred)
        ));
    }

    #[test]
    fn reduction_interruption_and_invalid_mass_never_publish_worlds() {
        let hands = [CardSet::EMPTY; 4];
        let pool = vec![World { hands, weight: 1.0 }; 8];
        // Inject interruption at every check, including after final resampling.
        let mut total = 0;
        select_worlds(pool.clone(), 100, 768, 3, &mut Rng::new(31), &mut || {
            total += 1;
            true
        })
        .unwrap();
        for stop in 1..=total {
            let mut checks = 0;
            assert!(matches!(
                select_worlds(pool.clone(), 100, 768, 3, &mut Rng::new(31), &mut || {
                    checks += 1;
                    checks < stop
                }),
                Err(PreparationError::Deferred)
            ));
        }
        for weight in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(matches!(
                select_worlds(vec![World { hands, weight }], 1, 0, 3, &mut Rng::new(31), &mut || true),
                Err(PreparationError::InvalidCohort)
            ));
        }
        assert_eq!(
            PreparationError::Deferred.reason(),
            crate::concepts::sequence::SequenceError::Budget
        );
        assert_eq!(
            PreparationError::InvalidCohort.reason(),
            crate::concepts::sequence::SequenceError::InvalidCohort
        );
    }
}

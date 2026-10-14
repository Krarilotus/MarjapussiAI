use super::*;
use crate::{BeliefParams, Concept, ConceptBudget, ConceptQuery};
use agent::SeatKind;
use observation::SeatEvent;
use rand::{rngs::StdRng, SeedableRng};

fn hidden_exchange() -> (Mind, Hands) {
    let hands: Hands =
        std::array::from_fn(|seat| (0..9).fold(CardSet::EMPTY, |set, i| set | CardSet::single((4 * i + seat) as u8)));
    let mut mind = Mind::new(1, hands[1], [SeatKind::Human; 4], BeliefParams::default());
    for event in [
        SeatEvent::Bid { seat: 0, value: 130 },
        SeatEvent::StopBidding { seat: 1 },
        SeatEvent::StopBidding { seat: 2 },
        SeatEvent::StopBidding { seat: 3 },
        SeatEvent::Pass {
            from: 2,
            to: 0,
            cards: None,
        },
        SeatEvent::Pass {
            from: 0,
            to: 2,
            cards: None,
        },
    ] {
        mind.apply(&event).unwrap();
    }
    // A legal completion returns the same four cards, so the current hands can
    // equal the dealt hands. The observer never receives the hidden transfers.
    (mind, hands)
}

fn fresh(mind: &Mind, start: CardSet) -> Mind {
    let mut fresh = Mind::new(mind.seat(), start, mind.kinds(), mind.params().clone());
    for event in mind.events() {
        fresh.apply(event).unwrap();
    }
    fresh
}

fn assert_parity(mind: &Mind, start: CardSet, hands: &Hands) {
    let fresh = fresh(mind, start);
    assert_eq!(mind.knowledge(), fresh.knowledge());
    let model = mind.deal_model();
    let independent = fresh.deal_weights();
    assert_eq!(
        model.weights().weight(hands).to_bits(),
        independent.weight(hands).to_bits()
    );
    let mut rng = StdRng::seed_from_u64(731);
    for _ in 0..16 {
        let sample = model.space().sample(&mut rng).unwrap();
        assert_eq!(
            model.weights().weight(&sample).to_bits(),
            independent.weight(&sample).to_bits()
        );
    }
    let query = ConceptQuery::new(Concept::Pair {
        seat: 0,
        suit: engine::SUITS[0],
    });
    let budget = ConceptBudget { max_nodes: 64 };
    assert_eq!(mind.concept(&query, budget), fresh.concept(&query, budget));
}

#[test]
fn exchange_reuse_refreshes_cards_undo_and_rights_but_preserves_weights_and_queries() {
    let (mut mind, initial) = hidden_exchange();
    let ace = (initial[0] & CardSet::ACES).iter().next().unwrap();
    let mut current = initial;
    mind.deal_model().weights().weight(&current);
    assert!(mind.deal_model().weights().exchange.borrow().cached_terms() > 0);
    assert_parity(&mind, initial[1], &current);
    for event in [
        SeatEvent::CardPlayed { seat: 0, card: ace },
        SeatEvent::Undone,
        SeatEvent::AskPair { seat: 0 },
    ] {
        let cached = mind.deal_model().weights().exchange.borrow().cached_terms();
        let before = mind.epochs();
        mind.apply(&event).unwrap();
        assert_ne!(mind.epochs(), before);
        match event {
            SeatEvent::CardPlayed { .. } => current[0] -= CardSet::single(ace),
            SeatEvent::Undone => current = initial,
            _ => {}
        }
        // Check before any new weight read: this must be actual transfer of
        // computed sums, not equivalent results from reconstructing the cache.
        assert_eq!(mind.deal_model().weights().exchange.borrow().cached_terms(), cached);
        assert_parity(&mind, initial[1], &current);
    }
    // Separate Minds must not share tables even for identical epochs/history.
    let other = fresh(&mind, initial[1]);
    assert_eq!(other.deal_model().weights().exchange.borrow().cached_terms(), 0);
}

#[test]
fn exchange_reuse_invalidates_each_sum_input_and_preserves_unrelated_model_changes() {
    let (mind, hands) = hidden_exchange();
    // Changes to private configuration cannot occur in-place through Mind's
    // public API. Exercise every transfer guard directly, including ordered
    // passers and a formerly absent likelihood, without relying on a bid encoder.
    for change in 0..8 {
        let edit = |weights: &mut DealWeights| match change {
            0 => weights.kernel.stay[0][0] = 0.25,
            1 => weights.likelihoods[0]
                .as_mut()
                .unwrap()
                .iter_mut()
                .for_each(|p| *p *= 0.25),
            2 => weights.likelihoods[2] = Some(Box::new([0.5; crate::conventions::N_CLASSES])),
            3 => weights.hidden_exchange = Some([0, 2]),
            4 => weights.hidden_exchange = Some([3, 1]),
            5 => weights.hidden_exchange = None,
            6 => weights.likelihoods[3] = Some(Box::new([0.5; crate::conventions::N_CLASSES])),
            7 => weights.factors[0][hands[0].iter().next().unwrap() as usize] = 1.5,
            _ => unreachable!(),
        };
        let mut previous = mind.deal_weights();
        previous.weight(&hands);
        let cached = previous.exchange.borrow().cached_terms();
        assert!(cached > 0);
        let mut next = mind.deal_weights();
        let mut independent = mind.deal_weights();
        edit(&mut next);
        edit(&mut independent);
        next.reuse_exchange(&mut previous);
        let reuse = change >= 6;
        assert_eq!(next.exchange.borrow().cached_terms(), if reuse { cached } else { 0 });
        assert_eq!(
            previous.exchange.borrow().cached_terms(),
            if reuse { 0 } else { cached }
        );
        assert_eq!(next.weight(&hands).to_bits(), independent.weight(&hands).to_bits());
    }
}

/// Root runs this separately from correctness tests. Both lanes refresh every
/// model and evaluate identical held feature counts. Only term reuse differs.
#[cfg(feature = "profile")]
#[test]
#[ignore = "manual exchange-cache microfixture; root owns resource-limited measurements"]
fn exchange_reuse_microfixture() {
    use std::{hint::black_box, time::Instant};
    const ROUNDS: usize = 16;
    let (mind, hands) = hidden_exchange();
    for reuse in [false, true] {
        crate::profile::reset();
        let mut previous = mind.deal_weights();
        let start = Instant::now();
        let mut expected = None;
        for _ in 0..ROUNDS {
            let mut next = mind.deal_weights();
            if reuse {
                next.reuse_exchange(&mut previous);
            }
            let weight = black_box(next.weight(black_box(&hands))).to_bits();
            assert_eq!(*expected.get_or_insert(weight), weight);
            previous = next;
        }
        let elapsed = start.elapsed();
        let profile = crate::profile::snapshot();
        let sums = profile.get("belief.exchange_sum").unwrap();
        assert_eq!(sums.calls, if reuse { 1 } else { ROUNDS as u64 });
        eprintln!("exchange_reuse reuse={reuse} rounds={ROUNDS} elapsed={elapsed:?} sums={sums:?}");
    }
}

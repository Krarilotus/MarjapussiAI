use super::*;
use crate::{BeliefParams, Concept, ConceptBudget, ConceptQuery, Epochs};
use agent::SeatKind;
use observation::SeatEvent;
use rand::{rngs::StdRng, SeedableRng};

fn cards(text: &str) -> CardSet {
    text.split_whitespace().fold(CardSet::EMPTY, |h, c| {
        h | CardSet::single(c.parse::<engine::Card>().unwrap().index())
    })
}

fn card(text: &str) -> u8 {
    text.parse::<engine::Card>().unwrap().index()
}

fn mind() -> Mind {
    Mind::new(
        0,
        cards("g-6 g-7 e-6 e-7 s-6 s-7 r-6 r-7 r-8"),
        [SeatKind::Human; 4],
        BeliefParams::default(),
    )
}

fn model_key(mind: &Mind) -> (Epochs, u32) {
    let model = mind.model.borrow();
    let (epochs, public, _) = model.as_ref().unwrap();
    (*epochs, *public)
}

fn assert_matches_legacy(mind: &Mind) {
    let uncached = mind.knowledge().deals();
    assert_eq!(mind.base(), crate::table::base(&uncached));
    let model = mind.deal_model();
    let legacy = mind.deal_weights();
    assert_eq!(model.space().count(), uncached.count());
    assert_eq!(model.space().holder_counts(), uncached.holder_counts());
    let mut rng = StdRng::seed_from_u64(37);
    let mut uncached_rng = StdRng::seed_from_u64(37);
    for _ in 0..32 {
        let hands = model.space().sample(&mut rng).unwrap();
        assert_eq!(Some(hands), uncached.sample(&mut uncached_rng));
        assert_eq!(model.weights().weight(&hands), legacy.weight(&hands));
    }
}

#[test]
fn fact_dp_reuse_exact_first_and_model_first_share_without_eager_weights() {
    for exact_first in [true, false] {
        let mind = mind();
        if exact_first {
            mind.base();
            assert!(mind.model.borrow().is_none());
            assert!(mind.cache.borrow().signals.is_none(), "exact reads need no likelihoods");
        } else {
            mind.deal_model().space();
        }
        let facts = mind.fact_space();
        assert!(facts.space.get().is_some());
        let expected = crate::table::base(&mind.knowledge().deals());
        for _ in 0..3 {
            assert_eq!(mind.base(), expected);
            assert!(Arc::ptr_eq(&facts, &mind.deal_model().facts));
            assert!(std::ptr::eq(facts.space(), mind.deal_model().space()));
        }
        assert_matches_legacy(&mind);
    }
}

#[test]
fn fact_dp_reuse_tracks_bid_play_undo_and_changed_history_like_fresh_reads() {
    let mut warm = mind();
    let start = warm.knowledge().certain(0);
    warm.base();
    let mut prior = warm.fact_space();
    for event in [
        SeatEvent::Bid { seat: 2, value: 130 },
        SeatEvent::Undone,
        SeatEvent::Bid { seat: 2, value: 125 },
        SeatEvent::CardPlayed {
            seat: 0,
            card: card("g-6"),
        },
        SeatEvent::Undone,
        SeatEvent::CardPlayed {
            seat: 0,
            card: card("e-6"),
        },
    ] {
        let before = (warm.epochs().fact, warm.public.fact_epoch());
        warm.apply(&event).unwrap();
        // Exercise exact-first invalidation while the previous model still exists.
        warm.base();
        let facts = warm.fact_space();
        let same_facts = before == (warm.epochs().fact, warm.public.fact_epoch());
        assert_eq!(Arc::ptr_eq(&prior, &facts), same_facts);
        assert!(Arc::ptr_eq(&facts, &warm.deal_model().facts));

        let mut fresh = Mind::new(0, start, warm.kinds(), warm.params().clone());
        for accepted in warm.events() {
            fresh.apply(accepted).unwrap();
        }
        assert_eq!(warm.knowledge(), fresh.knowledge());
        assert_eq!(warm.base(), fresh.base());
        let a = warm.deal_model();
        let b = fresh.deal_model();
        assert_eq!(a.space().holder_counts(), b.space().holder_counts());
        let mut rng = StdRng::seed_from_u64(91);
        for _ in 0..16 {
            let hands = a.space().sample(&mut rng).unwrap();
            assert_eq!(a.weights().weight(&hands), b.weights().weight(&hands));
        }
        assert_matches_legacy(&warm);
        prior = facts;
    }
}

#[test]
fn fact_dp_reuse_is_private_to_each_mind_and_parameter_configuration() {
    let original = mind();
    original.base();
    let owner = original.fact_space();
    let start = original.knowledge().certain(0);
    // Epoch zero in different seats, configurations and starting hands is not
    // the same semantic context. No epoch-keyed process-wide sharing is allowed.
    for (seat, hand, kinds, params) in [
        (0, start, [SeatKind::Human; 4], BeliefParams::base_only()),
        (0, start, [SeatKind::Heuristic; 4], BeliefParams::default()),
        (1, start, [SeatKind::Human; 4], BeliefParams::default()),
        (
            0,
            (start - CardSet::single(card("g-6"))) | CardSet::single(card("g-A")),
            [SeatKind::Human; 4],
            BeliefParams::default(),
        ),
    ] {
        let mut changed = Mind::new(seat, hand, kinds, params);
        changed.base();
        assert!(!Arc::ptr_eq(&owner, &changed.fact_space()));
        changed.apply(&SeatEvent::Bid { seat: 2, value: 130 }).unwrap();
        assert_matches_legacy(&changed);
    }
    assert_matches_legacy(&original);
}

/// Root runs this small measurement separately from correctness checks. Each
/// lane consumes identical DP counts, marginals and samples; only construction
/// reuse differs. Profile calls expose actual builds, not inferred cache hits.
#[cfg(feature = "profile")]
#[test]
#[ignore = "manual fact-DP construction microfixture; root owns resource-limited measurements"]
fn fact_dp_reuse_microfixture() {
    use std::{hint::black_box, time::Instant};
    const ROUNDS: usize = 16;
    for shared in [false, true] {
        crate::profile::reset();
        let started = Instant::now();
        for _ in 0..ROUNDS {
            let mind = mind();
            let model = mind.deal_model();
            let base = if shared {
                mind.base()
            } else {
                let facts = {
                    let _timing = crate::profile::scope("belief.deal_space");
                    mind.knowledge().deals()
                };
                crate::table::base(&facts)
            };
            let mut rng = StdRng::seed_from_u64(37);
            let hands = model.space().sample(&mut rng).unwrap();
            black_box((base, model.space().count(), model.weights().weight(&hands)));
        }
        let elapsed = started.elapsed();
        let profile = crate::profile::snapshot();
        let builds = profile.get("belief.deal_space").unwrap();
        assert_eq!(builds.calls, (ROUNDS * if shared { 1 } else { 2 }) as u64);
        eprintln!("fact_dp_reuse shared={shared} rounds={ROUNDS} elapsed={elapsed:?} builds={builds:?}");
    }
}

#[test]
fn weights_only_leave_space_unallocated_and_first_space_read_builds_once() {
    let mind = mind();
    let model = mind.deal_model();
    assert!(model.facts.space.get().is_none());
    // A concrete hard-consistent allocation needs no DP or sampler to construct.
    let mut hands = [CardSet::EMPTY; 4];
    hands[0] = mind.knowledge().certain(0);
    let mut unknown = (CardSet::ALL - hands[0]).iter();
    for hand in &mut hands[1..] {
        *hand = unknown
            .by_ref()
            .take(9)
            .fold(CardSet::EMPTY, |h, c| h | CardSet::single(c));
    }
    assert_eq!(model.weights().weight(&hands), mind.deal_weights().weight(&hands));
    assert!(
        model.facts.space.get().is_none(),
        "weighting must not construct a fact DP"
    );
    let first = model.space();
    assert!(first.count() > 0);
    assert!(model.facts.space.get().is_some());
    assert!(std::ptr::eq(first, model.space()));
    // Query grouping consumes exactly that already-constructed space.
    let query = ConceptQuery::new(Concept::Card {
        seat: 1,
        card: card("g-A"),
    });
    mind.concept(&query, ConceptBudget { max_nodes: 32 }).unwrap();
    assert!(std::ptr::eq(first, mind.deal_model().space()));
}

#[test]
fn model_reuse_preserves_send_and_has_a_separate_borrow_owner() {
    fn assert_send<T: Send>() {}
    assert_send::<Mind>();
    assert_send::<DealModel>();
    let mind = mind();
    let model = mind.deal_model();
    {
        // A same-epoch hit must not touch the independently borrowed query cache.
        let _queries = mind.cache.borrow_mut();
        let same = mind.deal_model();
        assert!(std::ptr::eq(&*model, &*same));
    }
    // Positive-budget concept reads can update their cache while this model
    // borrow stays live, without rebuilding evidence or borrowing that cell mutably.
    for card in [card("g-A"), card("e-A")] {
        let query = ConceptQuery::new(Concept::Card { seat: 1, card });
        mind.concept(&query, ConceptBudget { max_nodes: 32 }).unwrap();
        let same = mind.deal_model();
        assert!(std::ptr::eq(&*model, &*same));
    }
    assert_matches_legacy(&mind);
}

#[test]
fn bids_plays_and_undo_refresh_model_inputs_and_preserve_legacy_weights() {
    let mut mind = mind();
    let original_counts;
    {
        let model = mind.deal_model();
        assert!(model.weights().likelihoods[2].is_none());
        original_counts = model.space().holder_counts();
    }
    let original = model_key(&mind);
    mind.apply(&SeatEvent::Bid { seat: 2, value: 130 }).unwrap();
    {
        let model = mind.deal_model();
        assert!(model.weights().likelihoods[2].is_some());
        assert!(
            model.facts.space.get().is_some(),
            "bid-only refresh shares the existing fact DP"
        );
        assert_eq!(model.space().holder_counts(), original_counts);
    }
    let bid = model_key(&mind);
    assert_ne!(bid, original);
    assert_eq!(bid.0.fact, original.0.fact);
    assert_eq!(bid.1, original.1);
    assert_matches_legacy(&mind);
    mind.apply(&SeatEvent::Undone).unwrap();
    {
        let model = mind.deal_model();
        assert!(model.weights().likelihoods[2].is_none());
        assert!(
            model.facts.space.get().is_none(),
            "undo advances the fact epochs and discards the DP"
        );
    }
    assert_ne!(model_key(&mind), bid);
    assert_matches_legacy(&mind);
    let before_play = model_key(&mind);
    let played = card("g-6");
    mind.apply(&SeatEvent::CardPlayed { seat: 0, card: played }).unwrap();
    {
        let model = mind.deal_model();
        assert!(model.facts.space.get().is_none(), "a card changes allocation facts");
        assert_eq!(model.weights().played[0], CardSet::single(played));
        assert!(!model.weights().certain[0].contains(played));
        let hands = model.space().sample(&mut StdRng::seed_from_u64(38)).unwrap();
        assert_eq!(hands[0].len(), 8);
        assert!(hands.iter().all(|h| !h.contains(played)));
    }
    let after_play = model_key(&mind);
    assert_ne!(after_play, before_play);
    assert_matches_legacy(&mind);
    mind.apply(&SeatEvent::Undone).unwrap();
    {
        let model = mind.deal_model();
        assert!(
            model.facts.space.get().is_none(),
            "card undo must not retain the post-play DP"
        );
        assert!(model.weights().played[0].is_empty());
        assert!(model.weights().certain[0].contains(played));
    }
    assert_ne!(model_key(&mind), after_play);
    assert_matches_legacy(&mind);
}

#[test]
fn public_fact_epoch_is_part_of_the_model_key() {
    let mut mind = mind();
    mind.deal_model().space();
    let original = model_key(&mind);
    // Isolate the public component of the key; normal Mind::apply updates both trackers.
    mind.public
        .apply(&SeatEvent::CardPlayed {
            seat: 0,
            card: card("g-6"),
        })
        .unwrap();
    assert_eq!(mind.epochs(), original.0);
    assert!(
        mind.deal_model().facts.space.get().is_none(),
        "public fact changes discard the DP"
    );
    let refreshed = model_key(&mind);
    assert_eq!(refreshed.0, original.0);
    assert_ne!(refreshed.1, original.1);
}

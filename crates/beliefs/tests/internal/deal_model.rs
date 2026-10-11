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
    let model = mind.deal_model();
    let legacy = mind.deal_weights();
    assert_eq!(model.space().count(), mind.knowledge().deals().count());
    let mut rng = StdRng::seed_from_u64(37);
    for _ in 0..32 {
        let hands = model.space().sample(&mut rng).unwrap();
        assert_eq!(model.weights().weight(&hands), legacy.weight(&hands));
    }
}

#[test]
fn weights_only_leave_space_unallocated_and_first_space_read_builds_once() {
    let mind = mind();
    let model = mind.deal_model();
    assert!(model.space.get().is_none());
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
    assert!(model.space.get().is_none(), "weighting must not construct a fact DP");
    let first = model.space();
    assert!(first.count() > 0);
    assert!(model.space.get().is_some());
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
            model.space.get().is_some(),
            "bid-only refresh moves the existing fact DP"
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
            model.space.get().is_none(),
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
        assert!(model.space.get().is_none(), "a card changes allocation facts");
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
            model.space.get().is_none(),
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
        mind.deal_model().space.get().is_none(),
        "public fact changes discard the DP"
    );
    let refreshed = model_key(&mind);
    assert_eq!(refreshed.0, original.0);
    assert_ne!(refreshed.1, original.1);
}

//! Planning and feature reads must describe the same joint bidding model.

use agent::{SeatKind, Table};
use beliefs::{BeliefParams, Mind};
use engine::{ActionType, Card, CardSet, SUITS};
use knowledge::Belief;
use observation::SeatEvent;
use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};

fn cards(text: &str) -> CardSet {
    text.split_whitespace().fold(CardSet::EMPTY, |set, card| {
        set | CardSet::single(card.parse::<Card>().unwrap().index())
    })
}

#[test]
fn pair_signal_keeps_its_joint_probability_and_impossible_red_stays_impossible() {
    let mut mind = Mind::new(
        0,
        cards("r-K g-A g-6 g-7 e-6 e-7 s-6 r-6 r-7"),
        [SeatKind::Human; 4],
        BeliefParams::default(),
    );
    for event in [
        SeatEvent::Bid { seat: 0, value: 120 },
        SeatEvent::StopBidding { seat: 1 },
        SeatEvent::Bid { seat: 2, value: 135 },
    ] {
        mind.apply(&event).unwrap();
    }
    let worlds = mind
        .knowledge()
        .deals()
        .sample_worlds(&mut StdRng::seed_from_u64(108), 100_000, &mind.deal_weights());
    let mass: f64 = worlds.iter().map(|w| w.weight).sum();
    for suit in SUITS {
        let p = worlds
            .iter()
            .filter(|w| CardSet::halves(suit).is_subset(w.hands[2]))
            .map(|w| w.weight)
            .sum::<f64>()
            / mass;
        let expected = mind.features(2).pair[suit as usize] as f64;
        assert!(
            (p - expected).abs() < 0.015,
            "{suit:?}: sampled {p}, feature model {expected}"
        );
    }
    assert_eq!(mind.features(2).pair[engine::Suit::Red as usize], 0.0);
    assert!(worlds
        .iter()
        .all(|w| !CardSet::halves(engine::Suit::Red).is_subset(w.hands[2])));
}

#[test]
fn visible_and_hidden_exchanges_match_exact_enumerated_feature_probabilities() {
    let mut rng = StdRng::seed_from_u64(5831);
    let mut checks = [0; 2];
    let mut uncertain = 0;
    for _ in 0..4 {
        let mut deck = engine::get_all_cards();
        deck.shuffle(&mut rng);
        let mut table = Table::new(&engine::hands_from_deck(&deck));
        let bid = *table
            .to_act()
            .unwrap()
            .1
            .iter()
            .find(|a| matches!(a.action_type, ActionType::NewBid(130)))
            .unwrap();
        table.apply(bid);
        let declaring_team = bid.player.0 % 2;
        loop {
            let Some((_, legal)) = table.to_act() else { break };
            let cards: Vec<_> = legal
                .iter()
                .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
                .collect();
            let action = if !cards.is_empty() {
                *cards[rng.random_range(0..cards.len())]
            } else if let Some(a) = legal.iter().find(|a| a.action_type == ActionType::StopBidding) {
                *a
            } else {
                legal[rng.random_range(0..legal.len())]
            };
            table.apply(action);
            if !matches!(action.action_type, ActionType::CardPlayed(_)) {
                continue;
            }
            for seat in 0..4 {
                let view = table.view(seat);
                let space = view.knowledge.deals();
                if space.count() == 0 || space.count() > 5_000 {
                    continue;
                }
                let mut mind = Mind::new(
                    seat,
                    view.knowledge.dealt_certain(seat),
                    view.kinds,
                    BeliefParams::default(),
                );
                for event in view.events {
                    mind.apply(event).unwrap();
                }
                let weights = mind.deal_weights();
                let mut pairs = [[0.0; 4]; 4];
                let mut ace = [0.0; 4];
                let mut mass = 0.0;
                space.for_each_deal(|hands| {
                    let w = weights.weight(hands);
                    assert!(w.is_finite() && w > 0.0);
                    mass += w;
                    for s in 0..4 {
                        ace[s] += w * f64::from(!(hands[s] & CardSet::ACES).is_empty());
                        for suit in SUITS {
                            pairs[s][suit as usize] += w * f64::from(CardSet::halves(suit).is_subset(hands[s]));
                        }
                    }
                });
                for s in 0..4 {
                    let features = mind.features(s as u8);
                    assert!((ace[s] / mass - features.ace as f64).abs() < 1e-5);
                    for suit in SUITS {
                        let expected = features.pair[suit as usize] as f64;
                        uncertain += usize::from(expected > 0.0 && expected < 1.0);
                        assert!(
                            (pairs[s][suit as usize] / mass - expected).abs() < 1e-5,
                            "seat {seat} observes {s} {suit:?}: {} vs {expected}",
                            pairs[s][suit as usize] / mass
                        );
                    }
                }
                checks[usize::from(seat % 2 != declaring_team)] += 1;
            }
        }
    }
    assert!(
        checks.iter().all(|&n| n > 0),
        "both visible and hidden exchanges: {checks:?}"
    );
    assert!(uncertain > 0, "must compare uncertain pairs, not only settled facts");
}

#[test]
fn facts_only_weights_are_uniform_and_undo_restores_the_likelihoods() {
    let hand = cards("r-K g-A g-6 g-7 e-6 e-7 s-6 r-6 r-7");
    let mut mind = Mind::new(0, hand, [SeatKind::Human; 4], BeliefParams::default());
    let worlds = mind
        .knowledge()
        .deals()
        .sample_worlds(&mut StdRng::seed_from_u64(9), 100, &knowledge::Uniform);
    mind.apply(&SeatEvent::Bid { seat: 0, value: 120 }).unwrap();
    let before = mind.deal_weights();
    mind.apply(&SeatEvent::Bid { seat: 1, value: 135 }).unwrap();
    let after = mind.deal_weights();
    assert!(worlds.iter().any(|w| before.weight(&w.hands) != after.weight(&w.hands)));
    mind.apply(&SeatEvent::Undone).unwrap();
    let undone = mind.deal_weights();
    for world in &worlds {
        assert_eq!(before.weight(&world.hands), undone.weight(&world.hands));
    }
    let mut base = Mind::new(0, hand, [SeatKind::Human; 4], BeliefParams::base_only());
    base.apply(&SeatEvent::Bid { seat: 0, value: 120 }).unwrap();
    base.apply(&SeatEvent::Bid { seat: 1, value: 135 }).unwrap();
    for world in worlds {
        assert_eq!(base.deal_weights().weight(&world.hands), 1.0);
    }
}

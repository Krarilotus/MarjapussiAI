//! Reusing an agent must agree with reconstructing it from the incoming seat view.
mod common;

use common::Scene;
use heuristic::{HeuristicAgent, SeatKind};
use observation::SeatEvent;

const HAND: [&str; 9] = ["g-6", "e-7", "s-8", "r-9", "s-6", "g-7", "e-8", "r-8", "g-8"];

fn auction(bid: i32) -> Scene {
    let mut scene = Scene::new(2, &HAND);
    scene.auction(&[(0, Some(bid)), (1, None)]);
    scene
}

fn assert_fresh(agent: &mut HeuristicAgent, scene: &Scene) {
    let mut fresh = HeuristicAgent::new(agent.params.clone());
    let actual = agent.mind(&scene.view());
    let expected = fresh.mind(&scene.view());
    assert_eq!(actual.seat(), expected.seat());
    assert_eq!(actual.kinds(), expected.kinds());
    assert_eq!(actual.params(), expected.params());
    assert_eq!(actual.events(), expected.events());
    assert_eq!(actual.knowledge().value(), expected.knowledge().value());
    for seat in 0..4 {
        assert_eq!(actual.knowledge().certain(seat), expected.knowledge().certain(seat));
        assert_eq!(actual.holds_row(seat), expected.holds_row(seat));
    }
    assert_eq!(
        agent.decide(&scene.view(), &scene.bid_legal()),
        fresh.decide(&scene.view(), &scene.bid_legal())
    );
}

#[test]
fn forks_with_the_same_last_event_rebuild_before_reading_or_appending() {
    for append in [false, true] {
        let mut agent = HeuristicAgent::default();
        assert_fresh(&mut agent, &auction(120));
        let mut fork = auction(125);
        if append {
            fork.auction(&[(2, Some(130))]);
        }
        assert_fresh(&mut agent, &fork);
    }
}

#[test]
fn repeated_views_extensions_and_rewinds_match_fresh_agents() {
    let mut agent = HeuristicAgent::default();
    let mut scene = auction(120);
    assert_fresh(&mut agent, &scene);
    let epochs = agent.mind(&scene.view()).epochs();
    assert_fresh(&mut agent, &scene);
    assert_eq!(agent.mind(&scene.view()).epochs(), epochs);
    scene.auction(&[(2, Some(125)), (3, None)]);
    assert_fresh(&mut agent, &scene);
    assert_fresh(&mut agent, &auction(120));
    assert_fresh(&mut agent, &Scene::new(2, &HAND));
}

#[test]
fn belief_parameter_changes_rebuild_even_with_an_unchanged_history() {
    for append in [false, true] {
        let mut agent = HeuristicAgent::default();
        let mut scene = auction(120);
        assert_fresh(&mut agent, &scene); // Populate the lazy belief caches first.
        agent.params.beliefs = beliefs::BeliefParams::base_only();
        if append {
            scene.auction(&[(2, Some(125))]);
        }
        assert_fresh(&mut agent, &scene);
        agent.params.beliefs = beliefs::BeliefParams::default();
        assert_fresh(&mut agent, &scene);
    }
}

#[test]
fn undo_is_part_of_the_input_history_even_when_mind_retracts_the_event() {
    let mut agent = HeuristicAgent::default();
    let mut scene = auction(120);
    assert_fresh(&mut agent, &scene);
    scene.ev(SeatEvent::Undone);
    assert_fresh(&mut agent, &scene);
    assert_fresh(&mut agent, &scene);
    scene.auction(&[(1, Some(125))]);
    assert_fresh(&mut agent, &scene);
    // Same length and final event, but a different opening before the undo.
    let mut fork = auction(125);
    fork.ev(SeatEvent::Undone).auction(&[(1, Some(130))]);
    scene.auction(&[(2, None)]);
    fork.auction(&[(2, None)]);
    assert_fresh(&mut agent, &scene);
    assert_fresh(&mut agent, &fork);
}

#[test]
fn new_seats_hands_and_table_kinds_rebuild() {
    let mut agent = HeuristicAgent::default();
    assert_fresh(&mut agent, &auction(120));
    let mut scene = Scene::new(3, &HAND);
    scene.auction(&[(0, Some(120)), (1, None)]);
    assert_fresh(&mut agent, &scene);
    let mut different_hand = HAND;
    different_hand[0] = "g-A";
    let mut scene = Scene::new(3, &different_hand);
    scene.auction(&[(0, Some(120)), (1, None)]);
    assert_fresh(&mut agent, &scene);
    scene.kinds = [SeatKind::Heuristic; 4];
    assert_fresh(&mut agent, &scene);
}

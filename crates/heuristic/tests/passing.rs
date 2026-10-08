//! Independent pass scenarios and checks of the active C42 features.

mod common;

use common::{passed, set, Scene};
use engine::CardSet;
use engine::Suit;
use heuristic::passing::{back_plan, Partner};

/// MH is seat 2; VH is seat 0. Replay the supplied public auction.
fn mh_scene(hand: &[&str], my_bids: &[(u8, Option<i32>)]) -> Scene {
    let mut s = Scene::new(2, hand);
    s.auction(my_bids);
    s
}

fn plain_auction() -> Vec<(u8, Option<i32>)> {
    vec![(0, Some(120)), (1, None), (2, None), (3, None)]
}

#[test]
fn hp5_green_is_only_a_tie_breaker() {
    // Green, Acorns and Bells are interchangeable Lusche pairs: blank the two that are not green.
    let s = mh_scene(
        &["g-6", "g-7", "e-6", "e-7", "s-6", "s-7", "r-A", "r-Z", "r-K"],
        &plain_auction(),
    );
    let d = s.act(&s.pass_legal());
    assert_eq!(passed(&d), set(&["e-6", "e-7", "s-6", "s-7"]));
}

#[test]
fn hp6_passes_an_unknown_whole_suit_pair() {
    // C42c, the owner: "passing pair if its the whole suite is often adviced to get trump
    // consolidated, unless the partner knows of it and can play around it". My Bells are only
    // K and O, the partner doesn't know them: both go to VH.
    let s = mh_scene(
        &["g-6", "g-7", "e-8", "e-9", "s-K", "s-O", "r-9", "r-8", "r-7"],
        &plain_auction(),
    );
    let d = s.act(&s.pass_legal());
    assert!(
        CardSet::halves(Suit::Bells).is_subset(passed(&d)),
        "{:?} {:?}",
        passed(&d),
        d.why
    );
}

#[test]
fn hp6_passes_an_unknown_acorns_pair_when_the_partner_showed_no_halves() {
    let s = mh_scene(
        &["e-K", "e-O", "g-6", "g-7", "s-8", "s-9", "r-A", "r-Z", "r-9"],
        &plain_auction(),
    );
    let d = s.act(&s.pass_legal());
    assert!(CardSet::halves(Suit::Acorns).is_subset(passed(&d)), "{:?}", passed(&d));
}

// Passing back: VH = seat 0 with 13 cards.
const VH13: [&str; 13] = [
    "r-K", "r-O", "r-A", "r-9", "g-6", "g-7", "e-A", "e-8", "s-6", "s-7", "s-U", "g-8", "e-9",
];

#[test]
fn hk1_plans_the_red_pair() {
    assert_eq!(back_plan(set(&VH13), [0.0; 4], 0.5), Some(Suit::Red));
}

/// Named feature values in concrete giver scenarios.
mod features {
    use super::*;
    use heuristic::passing::{back_features, forth_features, known_pairs};
    use heuristic::{HeuristicAgent, Params as HParams};

    const PLAIN: &[(u8, Option<i32>)] = &[(0, Some(120)), (1, None), (2, None), (3, None)];

    fn feature(names: &[&str], f: &[f64], name: &str) -> f64 {
        f[names.iter().position(|&n| n == name).unwrap()]
    }

    #[test]
    fn forth_features_of_one_option() {
        // MH (2) passes Green (6, 7: blank with Lusche), the Bells ace (blank, no Lusche) and
        // the Acorns 8 from a suit it keeps.
        let s = mh_scene(&["g-6", "g-7", "s-A", "e-8", "e-K", "e-Z", "r-A", "r-Z", "r-6"], PLAIN);
        let mut agent = HeuristicAgent::new(HParams::default());
        let mind = agent.mind(&s.view());
        let known = known_pairs(&mind.record().seats[2].bids);
        let partner = Partner::of(mind);
        let f = forth_features(s.hand(), &partner, known, set(&["g-6", "g-7", "s-A", "e-8"]));
        let names = &heuristic::passing::FORTH_FEATURES;
        assert_eq!(feature(names, &f, "blank_with_lusche"), 1.0);
        assert_eq!(feature(names, &f, "blank_without_lusche"), 1.0);
        assert_eq!(feature(names, &f, "lusche_from_kept_suit"), 1.0);
        assert_eq!(feature(names, &f, "aces"), 1.0);
        assert_eq!(feature(names, &f, "green"), 2.0);
        assert_eq!(feature(names, &f, "points"), 11.0);
        // VH's 120 is read as an ace: the ace is worth less to it.
        assert!(feature(names, &f, "aces_x_vh_lacks_ace") < 0.5);
    }

    #[test]
    fn back_features_of_one_option() {
        // VH (0) with the red pair: gives back the Acorns ace with its 8 kept (an entry), the
        // whole Bells suit (a void) and the Green 6.
        let mut s = Scene::new(0, &["r-K", "r-O", "r-A", "r-9", "g-6", "g-7", "e-A", "e-8", "s-6"]);
        s.auction(PLAIN);
        s.pass(2, 0, Some(&["s-7", "s-U", "g-8", "e-9"]));
        let mut agent = HeuristicAgent::new(HParams::default());
        let mind = agent.mind(&s.view());
        let partner = Partner::of(mind);
        let plan = back_plan(s.hand(), partner.half, HParams::default().partner_half_conf);
        assert_eq!(plan, Some(Suit::Red));
        let f = back_features(s.hand(), &partner, plan, set(&["e-A", "s-6", "s-7", "s-U"]));
        let names = &heuristic::passing::BACK_FEATURES;
        assert_eq!(feature(names, &f, "aces"), 1.0);
        assert_eq!(feature(names, &f, "aces_entry_low_kept"), 1.0);
        assert_eq!(feature(names, &f, "voids_made"), 1.0);
        assert_eq!(feature(names, &f, "side_suits_complete"), 1.0);
        assert_eq!(feature(names, &f, "plan_trump_given"), 0.0);
        assert_eq!(feature(names, &f, "plan_top_trump_kept"), 1.0);
        // Kept: r-A r-K r-O r-9 g-6 g-7 g-8 e-8 e-9: the red A, K, O... the A stands, then the
        // Z is missing, so only the ace stands.
        assert_eq!(feature(names, &f, "standing_after"), 1.0);
        assert_eq!(feature(names, &f, "points"), 13.0);
    }
}

#[test]
fn back_plan_requires_a_held_half_and_enough_partner_evidence() {
    let hand = set(&["r-K", "g-O", "e-A", "s-6"]);
    let mut evidence = [0.0; 4];
    evidence[Suit::Red as usize] = 0.49;
    evidence[Suit::Bells as usize] = 1.0; // No half of my own: cannot be the plan.
    assert_eq!(back_plan(hand, evidence, 0.5), None);
    evidence[Suit::Red as usize] = 0.5;
    evidence[Suit::Green as usize] = 1.0;
    assert_eq!(back_plan(hand, evidence, 0.5), Some(Suit::Red));
    // A complete pair takes precedence over the more valuable possible completion.
    assert_eq!(back_plan(hand | set(&["g-K"]), evidence, 0.5), Some(Suit::Green));
}

#[test]
fn pair_messages_mark_only_the_suits_the_convention_covers() {
    use engine::SUITS;
    use heuristic::passing::known_pairs;
    use heuristic::{HeuristicAgent, Params};
    for (bid, suits) in [
        (120, vec![]),
        (125, vec![Suit::Green, Suit::Acorns]),
        (130, vec![Suit::Bells, Suit::Red]),
        (135, vec![Suit::Red]),
    ] {
        let mut s = Scene::new(2, &["g-6", "g-7", "e-6", "e-7", "s-6", "s-7", "r-A", "r-Z", "r-K"]);
        s.auction(&[(0, None), (1, None), (2, Some(bid))]);
        let mut agent = HeuristicAgent::new(Params::default());
        let mind = agent.mind(&s.view());
        assert_eq!(
            known_pairs(&mind.record().seats[2].bids),
            SUITS.map(|s| suits.contains(&s))
        );
    }
}

#[test]
fn tied_passes_follow_the_supplied_legal_order() {
    let s = mh_scene(
        &["g-6", "g-7", "g-8", "e-6", "e-7", "e-8", "r-A", "r-Z", "r-K"],
        &plain_auction(),
    );
    // Each option passes two low cards of each minor suit, keeping both suits.
    let choices = [set(&["g-6", "g-7", "e-6", "e-7"]), set(&["g-6", "g-8", "e-6", "e-8"])];
    let mut legal: Vec<_> = s
        .pass_legal()
        .into_iter()
        .filter(|a| match &a.action_type {
            engine::ActionType::Pass(cards) => choices.contains(&CardSet::from_cards(cards)),
            _ => false,
        })
        .collect();
    assert_eq!(legal.len(), 2);
    assert_eq!(s.act(&legal).action, legal[0]);
    legal.reverse();
    assert_eq!(s.act(&legal).action, legal[0]);
}

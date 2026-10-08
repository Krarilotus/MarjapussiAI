//! Passing. The store (C42): features, `passing_weights`, the argmax. And the old pass models
//! (`beliefs::conventions::passes`, HP forth / HK back), which the reading of a partner's pass
//! still inverts until R1–R4 are built.

mod common;

use beliefs::conventions::passes::{plan, BackCtx, ForthCtx, ForthModel, Plan};
use beliefs::conventions::Params;
use common::{passed, set, Scene};
use engine::CardSet;
use engine::Suit;
use heuristic::RuleId;

/// MH = seat 2; VH = seat 0 won at `vh_bid` after showing `vh_steps`.
fn mh_scene(hand: &[&str], my_bids: &[(u8, Option<i32>)]) -> Scene {
    let mut s = Scene::new(2, hand);
    s.auction(my_bids);
    s
}

fn plain_auction() -> Vec<(u8, Option<i32>)> {
    vec![(0, Some(120)), (1, None), (2, None), (3, None)]
}

#[test]
fn hp1_blanks_two_suits() {
    let p = Params::default();
    let hand = set(&["g-6", "g-7", "e-8", "e-9", "r-A", "r-Z", "r-K", "s-9", "s-U"]);
    let m = ForthModel::new(hand, &ForthCtx::default(), &p);
    let two = m.score(set(&["g-6", "g-7", "e-8", "e-9"]));
    let one = m.score(set(&["g-6", "g-7", "e-8", "s-9"]));
    assert!(two.terms[0] > one.terms[0]);
}

#[test]
fn hp2_an_unannounced_ace_is_news_for_the_partner() {
    let p = Params::default();
    let hand = set(&["g-6", "g-7", "e-8", "e-9", "r-A", "r-Z", "s-K", "s-9", "s-U"]);
    let x = set(&["r-A", "g-6", "g-7", "e-8"]);
    let silent = ForthModel::new(hand, &ForthCtx::default(), &p).score(x);
    let told = ForthCtx {
        partner_knows_ace: true,
        ..ForthCtx::default()
    };
    let told = ForthModel::new(hand, &told, &p).score(x);
    assert!(silent.terms[1] > told.terms[1]);
}

#[test]
fn hp3_better_cards_score_higher() {
    let p = Params::default();
    let hand = set(&["g-6", "g-7", "e-8", "e-9", "r-A", "r-Z", "s-K", "s-9", "s-U"]);
    let m = ForthModel::new(hand, &ForthCtx::default(), &p);
    assert!(
        m.score(set(&["r-A", "r-Z", "s-9", "s-U"])).terms[2] > m.score(set(&["g-6", "g-7", "e-8", "e-9"])).terms[2]
    );
}

#[test]
fn hp4_vh_without_an_ace_needs_one() {
    let p = Params::default();
    let hand = set(&["g-6", "g-7", "e-8", "e-9", "r-A", "r-Z", "s-K", "s-9", "s-U"]);
    let x = set(&["r-A", "g-6", "g-7", "e-8"]);
    let showed = ForthCtx {
        partner_showed_ace: true,
        ..ForthCtx::default()
    };
    let needs = ForthModel::new(hand, &ForthCtx::default(), &p).score(x);
    let has = ForthModel::new(hand, &showed, &p).score(x);
    assert!(needs.terms[3] > has.terms[3]);
}

#[test]
fn hp4_vh_bid_180_without_a_pair_needs_a_half() {
    let p = Params::default();
    let hand = set(&["g-6", "g-7", "e-8", "e-9", "s-O", "r-Z", "r-K", "s-9", "s-U"]);
    let ctx = ForthCtx {
        partner_bid: 220,
        partner_showed_ace: true,
        ..ForthCtx::default()
    };
    let m = ForthModel::new(hand, &ctx, &p);
    assert!(m.score(set(&["s-O", "g-6", "g-7", "e-8"])).terms[3] > 0.0);
    assert_eq!(m.score(set(&["g-6", "g-7", "e-8", "e-9"])).terms[3], 0.0);
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
    let p = Params::default();
    assert!(p.w_green <= 0.05 * p.w_blank);
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
    let p = Params::default();
    let hand = set(&["e-K", "e-O", "g-6", "g-7", "s-8", "s-9", "r-A", "r-Z", "r-9"]);
    let m = ForthModel::new(hand, &ForthCtx::default(), &p);
    let pass_pair = m.score(set(&["e-K", "e-O", "g-6", "g-7"]));
    assert!(pass_pair.terms[5] >= p.w_pass_acorns + p.w_pass_needed);
    let s = mh_scene(
        &["e-K", "e-O", "g-6", "g-7", "s-8", "s-9", "r-A", "r-Z", "r-9"],
        &plain_auction(),
    );
    let d = s.act(&s.pass_legal());
    assert!(CardSet::halves(Suit::Acorns).is_subset(passed(&d)), "{:?}", passed(&d));
}

#[test]
fn hp6_splitting_a_pair_is_penalised() {
    let p = Params::default();
    let hand = set(&["e-K", "e-O", "g-6", "g-7", "s-8", "s-9", "r-A", "r-Z", "r-9"]);
    let m = ForthModel::new(hand, &ForthCtx::default(), &p);
    assert_eq!(m.score(set(&["e-K", "g-6", "g-7", "s-8"])).terms[5], -p.w_split);
}

#[test]
fn hp7_awkward_hand_boosts_quality_and_need() {
    let p = Params::default();
    let hand = set(&["r-6", "r-7", "r-8", "r-9", "r-U", "s-6", "s-7", "s-8", "s-A"]);
    let m = ForthModel::new(hand, &ForthCtx::default(), &p);
    assert!(m.score(set(&["s-A", "r-6", "r-7", "r-8"])).terms[6] > 0.0);
}

#[test]
fn hp8_blanking_with_lusche_signals_the_void() {
    let p = Params::default();
    let hand = set(&["g-6", "g-7", "e-Z", "e-K", "s-8", "s-9", "r-A", "r-Z", "r-9"]);
    let m = ForthModel::new(hand, &ForthCtx::default(), &p);
    assert_eq!(
        m.score(set(&["g-6", "g-7", "s-8", "s-9"])).terms[7],
        4.0 * p.w_small_blank
    );
    assert_eq!(m.score(set(&["e-Z", "e-K", "s-8", "s-9"])).terms[7], 0.0);
}

// Passing back: VH = seat 0 with 13 cards.
const VH13: [&str; 13] = [
    "r-K", "r-O", "r-A", "r-9", "g-6", "g-7", "e-A", "e-8", "s-6", "s-7", "s-U", "g-8", "e-9",
];

#[test]
fn hk1_plans_the_red_pair() {
    let p = Params::default();
    assert_eq!(plan(set(&VH13), &BackCtx::default(), &p), Plan::Trump(Suit::Red));
}

/// HK-4 never decides in the old back model: HK-1's penalty per trump given back always
/// outweighs it (60,000 searched hands).
#[test]
fn hk4_is_dominated_by_hk1() {
    let p = Params::default();
    assert!(p.w_trump_keep * 2.0 > p.w_last_trick);
}

/// The store (C42): the features of one option, and the decision as the first argmax of
/// Σ passing_weights × features over the legal options.
mod store {
    use super::*;
    use beliefs::conventions::forth_ctx;
    use heuristic::passing::{back_features, back_plan, forth_features, score, PASSING_WEIGHTS};
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
        let ctx = forth_ctx(&mind.record().seats, 2, &Params::default());
        let f = forth_features(mind, &ctx, set(&["g-6", "g-7", "s-A", "e-8"]));
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
        let plan = back_plan(mind, &Params::default());
        assert_eq!(plan, Plan::Trump(Suit::Red));
        let f = back_features(mind, plan, set(&["e-A", "s-6", "s-7", "s-U"]));
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

    fn first_best(scores: &[f64]) -> usize {
        (0..scores.len())
            .max_by(|&a, &b| scores[a].total_cmp(&scores[b]).then(b.cmp(&a)))
            .unwrap()
    }

    #[test]
    fn the_pass_forth_is_the_first_argmax_of_the_weighted_features() {
        let s = mh_scene(&["g-6", "g-7", "s-A", "e-8", "e-K", "e-Z", "r-A", "r-Z", "r-6"], PLAIN);
        let legal = s.pass_legal();
        let d = s.act(&legal);
        let mut agent = HeuristicAgent::new(HParams::default());
        let mind = agent.mind(&s.view());
        let ctx = forth_ctx(&mind.record().seats, 2, &Params::default());
        let scores: Vec<f64> = legal
            .iter()
            .map(|a| score(&PASSING_WEIGHTS.forth, &forth_features(mind, &ctx, passed_of(a))))
            .collect();
        assert_eq!(passed(&d), passed_of(&legal[first_best(&scores)]));
        assert_eq!(d.rule, RuleId::PForth);
    }

    #[test]
    fn the_pass_back_is_the_first_argmax_of_the_weighted_features() {
        let mut s = Scene::new(0, &["r-K", "r-O", "r-A", "r-9", "g-6", "g-7", "e-A", "e-8", "s-6"]);
        s.auction(PLAIN);
        s.pass(2, 0, Some(&["s-7", "s-U", "g-8", "e-9"]));
        let legal = s.pass_legal();
        let d = s.act(&legal);
        let mut agent = HeuristicAgent::new(HParams::default());
        let mind = agent.mind(&s.view());
        let plan = back_plan(mind, &Params::default());
        let scores: Vec<f64> = legal
            .iter()
            .map(|a| score(&PASSING_WEIGHTS.back, &back_features(mind, plan, passed_of(a))))
            .collect();
        assert_eq!(passed(&d), passed_of(&legal[first_best(&scores)]));
        assert_eq!(d.rule, RuleId::PBack);
    }

    fn passed_of(a: &engine::GameAction) -> CardSet {
        match &a.action_type {
            engine::ActionType::Pass(c) => CardSet::from_cards(c),
            _ => CardSet::EMPTY,
        }
    }
}

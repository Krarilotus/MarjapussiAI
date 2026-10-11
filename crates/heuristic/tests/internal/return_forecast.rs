use super::*;
use crate::{HeuristicAgent, Params};
use std::cell::RefCell;

#[test]
fn combined_shortlist_keeps_pair_and_control_together_without_a_score_bonus() {
    let set = |text: &str| {
        text.split_whitespace().fold(CardSet::EMPTY, |h, c| {
            h | CardSet::single(c.parse::<engine::Card>().unwrap().index())
        })
    };
    let low = set("g-6 g-7 g-8 g-9 e-6 e-7 r-6 r-7 r-8");
    let resources = set("s-A s-K s-O e-A");
    let mut mind = Mind::new(0, low, [crate::SeatKind::Unknown; 4], beliefs::BeliefParams::base_only());
    mind.apply(&SeatEvent::Pass { from: 2, to: 0, cards: Some(resources) }).unwrap();
    let params = Params::default();
    let solver = RefCell::new(None);
    let ctx = Ctx {
        me: 0,
        k: mind.knowledge(),
        mind: &mind,
        params: &params,
        solver: &solver,
        seed: 7,
        routes: RefCell::new(None),
        concepts: RefCell::new(crate::concepts::Reads::default()),
        series: crate::SeriesScore::default(),
    };
    let action = |given: CardSet| GameAction {
        player: engine::PlaceAtTable(0),
        action_type: ActionType::Pass(given.cards().try_into().unwrap()),
    };
    let give_winners = set("s-A e-A g-6 g-7");
    let give_pair = set("s-K s-O g-6 g-7");
    let keep_connection = set("g-6 g-7 g-8 g-9");
    let ranked = vec![
        (action(give_winners), give_winners, 3.0),
        (action(give_pair), give_pair, 2.0),
        (action(keep_connection), keep_connection, 1.0),
    ];
    let mut selected = vec![ranked[0].0, ranked[1].0];
    add_connected_allocations(&ctx, &mut selected, &ranked);
    assert_eq!(selected, ranked.iter().map(|x| x.0).collect::<Vec<_>>());
    // C42 still prefers the first action: nomination only adds the missing
    // conjunction for completed continuation comparison, with no repricing.
    assert_eq!(ranked[0].2, 3.0);
    add_connected_allocations(&ctx, &mut selected, &ranked);
    assert_eq!(selected.len(), 3);
    assert!(ctx.routes.borrow().is_none(), "shape coverage needs no sampled entry evaluation");
}

#[test]
fn shortlist_preserves_a_joint_partner_known_resource_only_when_missing() {
    let set = |text: &str| {
        text.split_whitespace().fold(CardSet::EMPTY, |h, c| {
            h | CardSet::single(c.parse::<engine::Card>().unwrap().index())
        })
    };
    let low = set("g-6 g-7 g-8 g-9 e-6 e-7 r-6 r-7 r-8");
    let resources = set("s-A s-K s-O e-A");
    let whole = low | resources;
    let action = |given: CardSet| GameAction {
        player: engine::PlaceAtTable(0),
        action_type: ActionType::Pass(given.cards().try_into().unwrap()),
    };
    let give_winners = set("s-A e-A g-6 g-7");
    let give_pair = set("s-K s-O g-6 g-7");
    let keep_connection = set("g-6 g-7 g-8 g-9");
    let ranked = vec![
        (action(give_winners), give_winners, 3.0),
        (action(give_pair), give_pair, 2.0),
        (action(keep_connection), keep_connection, 1.0),
    ];
    for shared in [resources, keep_connection] {
        let mut mind = Mind::new(
            0,
            whole - shared,
            [crate::SeatKind::Unknown; 4],
            beliefs::BeliefParams::base_only(),
        );
        mind.apply(&SeatEvent::Pass {
            from: 2,
            to: 0,
            cards: Some(shared),
        })
        .unwrap();
        let params = Params::default();
        let ctx = Ctx {
            me: 0,
            k: mind.knowledge(),
            mind: &mind,
            params: &params,
            solver: &RefCell::new(None),
            seed: 7,
            routes: RefCell::new(None),
            concepts: RefCell::new(crate::concepts::Reads::default()),
            series: crate::SeriesScore::default(),
        };
        let mut options = vec![ranked[0].0, ranked[1].0];
        add_known_connection(&ctx, &mut options, &ranked);
        if shared == resources {
            assert_eq!(options, ranked.iter().map(|x| x.0).collect::<Vec<_>>());
            add_known_connection(&ctx, &mut options, &ranked);
            assert_eq!(options.len(), 3, "already represented connection is not duplicated");
        } else {
            assert_eq!(
                options.len(),
                2,
                "root's private pair must not become partner knowledge"
            );
        }
    }
}

#[test]
fn expired_comparison_retains_c42_without_publishing_partial_outcomes() {
    let hands = std::array::from_fn(|s| (0..9).map(|c| engine::Card::from_index((9 * s + c) as u8)).collect());
    let mut table = Table::new(&hands);
    let bid = *table
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| matches!(a.action_type, ActionType::NewBid(_)))
        .unwrap();
    table.apply(bid);
    while table.game.state.phase != engine::GamePhase::PassingBack {
        let a = table.to_act().unwrap().1[0]; // stop bidding, then a legal first pass
        table.apply(a);
    }
    let (seat, legal) = table.to_act().unwrap();
    let view = table.view(seat);
    let mut agent = HeuristicAgent::new(Params {
        return_worlds: 0,
        ..Params::default()
    });
    let baseline = agent.decide(&view, legal);
    let mind = agent.mind(&view);
    let params = Params::default();
    let ctx = Ctx {
        me: seat,
        k: mind.knowledge(),
        mind,
        params: &params,
        solver: &RefCell::new(None),
        seed: 7,
        routes: RefCell::new(None),
        concepts: RefCell::new(crate::concepts::Reads::default()),
        series: view.series,
    };
    let partner = Partner::of(mind);
    let trump = super::super::back_plan(ctx.k.certain(seat), partner.half, params.partner_half_conf);
    let (_, options) = super::super::choose(
        &ctx,
        legal,
        &PASSING_WEIGHTS.back,
        &BACK_FEATURES,
        crate::RuleId::PBack,
        |cards| back_features(ctx.k.certain(seat), &partner, trump, cards),
    );
    let selected = candidates(&ctx, options.clone(), baseline.action);
    assert!(selected.iter().all(|a| legal.contains(a)));
    for (action, given, _) in &options {
        let hand = ctx.k.certain(seat);
        for suit in SUITS {
            let shape = |cards: CardSet| (hand - cards) & CardSet::suit(suit);
            assert!(
                selected.iter().any(|a| {
                    let ActionType::Pass(cards) = a.action_type else {
                        return false;
                    };
                    shape(CardSet::from_cards(&cards)).len() == shape(*given).len()
                }),
                "missing retained length for {action:?} in {suit:?}"
            );
        }
    }
    let actual = choose_until(&ctx, options, &partner, trump, baseline.clone(), Instant::now());
    assert_eq!(actual.action, baseline.action);
    assert_eq!(actual.rule, baseline.rule);
    assert_eq!(&actual.why[..baseline.why.len()], &baseline.why);
    assert!(actual
        .why
        .iter()
        .any(|f| f.key == "forecast_complete" && f.value == 0.0));
    assert!(!actual.why.iter().any(|f| f.key == "forecast_score"));
}

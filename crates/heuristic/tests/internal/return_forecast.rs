use super::*;
use std::cell::RefCell;

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

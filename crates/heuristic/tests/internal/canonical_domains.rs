fn shuffled_hands() -> [Vec<Card>; 4] {
    let mut cards = hands();
    // Keep the first four positions so the existing recorded pass fixture stays
    // legal; make the surviving own-card order differ from reconstruction.
    for hand in &mut cards {
        hand[4..].rotate_left(2);
    }
    cards
}
fn reconstructed(table: &Table, root: u8) -> Table {
    let remaining = std::array::from_fn(|seat| table.view(seat as u8).knowledge.certain(seat as u8));
    Table::from_view(&table.view(root), remaining).expect("same recorded observation with sorted physical hands")
}
#[test]
fn shuffled_live_and_reconstructed_domains_share_values_actions_and_prepared_choices() {
    let live = at_lead_with(shuffled_hands());
    let (root, live_legal) = live.to_act().unwrap();
    let modeled = reconstructed(&live, root);
    let modeled_legal = modeled.to_act().unwrap().1;
    assert!(Information::of(&live.view(root)).matches(&modeled.view(root)));
    assert_ne!(
        live_legal, modeled_legal,
        "the fixture must exercise the physical-order difference"
    );
    let legal = crate::legal_actions::eligible(live_legal);
    assert_eq!(legal, crate::legal_actions::eligible(modeled_legal));
    let cards = legal
        .iter()
        .filter_map(|action| match action.action_type {
            ActionType::CardPlayed(card) => Some(card),
            _ => None,
        })
        .take(2)
        .collect::<Vec<_>>();
    assert_eq!(cards.len(), 2);
    let candidate = Candidate::new(Program {
        name: "canonical-domain-choice",
        version: 1,
        start: Stage(1),
        nodes: vec![
            Node::Tail,
            Node::Action {
                actions: Actions::Cards(CardSet::from_cards(&cards)),
                next: Stage(0),
            },
        ],
    });
    let context = context();
    let mut cache = CompletedCache::default();
    let compare = |table: &Table, cache: &mut CompletedCache| {
        evaluate_with_cache(
            std::slice::from_ref(&candidate),
            &[(table.clone(), 1.0)],
            root,
            provenance(),
            &context,
            Limits::default(),
            cache,
            &mut leaf,
        )
        .unwrap()
    };
    let initial = compare(&live, &mut cache);
    let replay = compare(&modeled, &mut cache);
    assert_eq!(initial.values, replay.values);
    assert_eq!(initial.executions, replay.executions);
    assert_eq!(initial.receipt, replay.receipt);
    assert_eq!(initial.prepared, replay.prepared);
    assert_eq!(
        initial.work.represented_transitions,
        replay.work.represented_transitions
    );
    assert_eq!(
        initial.work.represented_solver_nodes,
        replay.work.represented_solver_nodes
    );
    assert!(replay.work.response_calls < initial.work.response_calls);
    let combined = evaluate(
        std::slice::from_ref(&candidate),
        &[(live.clone(), 1.0 / 3.0), (modeled.clone(), 2.0 / 3.0)],
        root,
        provenance(),
        &context,
        Limits::default(),
        &mut leaf,
    )
    .unwrap();
    assert_eq!(combined.executions[0].first.action, initial.executions[0].first.action);
    for table in [&live, &modeled] {
        let view = table.view(root);
        let raw = table.to_act().unwrap().1;
        let membership = Membership::authored(&view, raw).unwrap();
        let receipt = Receipt::new(candidate.program.clone(), &view).unwrap();
        let prepared = prepared_lookup(
            &receipt,
            &view,
            raw,
            &mind(&view),
            &context,
            &membership,
            &initial.prepared,
        )
        .expect("exact canonical domain permits the same prepared offer");
        assert_eq!(prepared.action, initial.executions[0].first.action);
        assert!(
            raw.contains(&prepared.action),
            "canonicalization preserves exact engine actions"
        );
    }
    // The real policy boundary must also receive this order, including the
    // primitive response path. Disable planning only for this isolated parity
    // check; the graph above exercises actual finite authored choice/continuation.
    let params = crate::Params {
        plan_worlds: 0,
        ..crate::Params::default()
    };
    let mut first = crate::HeuristicAgent::new(params.clone());
    let mut second = crate::HeuristicAgent::new(params);
    assert_eq!(
        first.decide(&live.view(root), live_legal),
        second.decide(&modeled.view(root), modeled_legal)
    );
}
#[test]
fn live_and_reconstructed_pass_catalogues_have_the_same_canonical_exact_actions() {
    let mut live = Table::new(&shuffled_hands());
    let bid = *live
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| matches!(a.action_type, ActionType::NewBid(_)))
        .unwrap();
    live.apply(bid);
    while live.game.state.phase != GamePhase::PassingBack {
        live.apply(live.to_act().unwrap().1[0]);
    }
    let (root, legal) = live.to_act().unwrap();
    let modeled = reconstructed(&live, root);
    assert_ne!(legal, modeled.to_act().unwrap().1);
    let canonical = crate::legal_actions::eligible(legal);
    assert_eq!(canonical, crate::legal_actions::eligible(modeled.to_act().unwrap().1));
    assert_eq!(
        Membership::authored(&live.view(root), legal).unwrap(),
        Membership::authored(&modeled.view(root), modeled.to_act().unwrap().1).unwrap()
    );
    for action in canonical {
        let ActionType::Pass(cards) = action.action_type else {
            panic!("expected the exchange domain")
        };
        assert!(cards.windows(2).all(|pair| pair[0] > pair[1]));
        assert!(legal.contains(&action));
        assert!(modeled.to_act().unwrap().1.contains(&action));
    }
}

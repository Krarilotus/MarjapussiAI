//! Uncached continuation oracle and HO-1/HO-2 sharing tests. No production toggle.
use super::*;
use beliefs::Mind;
use observation::SeatEvent;
use std::cell::RefCell;

fn fixture() -> (crate::Params, Mind) {
    let params = crate::Params {
        outlook_worlds: 8,
        beliefs: beliefs::BeliefParams::base_only(),
        ..crate::Params::default()
    };
    let hand = CardSet::from_cards(
        &["r-A", "r-Z", "r-K", "r-O", "r-9", "r-8", "e-6", "g-6", "s-6"]
            .map(|s| s.parse().unwrap()),
    );
    let mut mind = Mind::new(0, hand, [crate::SeatKind::Unknown; 4], params.beliefs.clone());
    mind.apply(&SeatEvent::Bid { seat: 0, value: 120 }).unwrap();
    for (seat, name) in ["r-A", "r-6", "r-7", "r-U"].into_iter().enumerate() {
        mind.apply(&SeatEvent::CardPlayed {
            seat: seat as u8,
            card: name.parse::<engine::Card>().unwrap().index(),
        })
        .unwrap();
    }
    (params, mind)
}

fn context<'a>(params: &'a crate::Params, mind: &'a Mind, solver: &'a RefCell<Option<solver::Solver>>) -> Ctx<'a> {
    Ctx {
        me: 0,
        k: mind.knowledge(),
        mind,
        params,
        solver,
        seed: 7,
        deadline: None,
        deadline_hit: Default::default(),
        series: Default::default(),
        class: Default::default(),
        routes: RefCell::new(None),
        concepts: RefCell::new(crate::concepts::Reads::default()),
    }
}

/// Original cell evaluation without a store read or write; preserves option and
/// world order so bit equality covers aggregation, not only the chosen action.
fn uncached(ctx: &Ctx, phase: PlayPhase, moves: &[Move]) -> WorldValues<(Move, Plan)> {
    let public = ctx.k.public_state(ctx.me, phase);
    let known = Known::new(ctx);
    let options = moves.iter().flat_map(|&m| Plan::all(ctx.me).map(move |p| (m, p))).collect();
    ctx.with_worlds(|worlds| {
        WorldValues::over(ctx, worlds, options, |w, &(action, plan)| {
            let mut state = FastState::from_public(public, w.hands).ok()?;
            let mut k = known.in_world(ctx, &w.hands);
            match action {
                Move::Card(c) if state.legal_cards().contains(c) => {
                    k.play(&state, c);
                    state.play(c);
                }
                m if state.moves().any(|x| x == m) => apply(&mut state, &mut k, m),
                _ => return None,
            }
            playout(&mut state, &mut k, Some(plan));
            Some(score(&state, contract(ctx), ctx.me % 2))
        })
    })
    .unwrap()
}

fn same_values<K: Copy + PartialEq + std::fmt::Debug>(a: &WorldValues<K>, b: &WorldValues<K>) {
    assert_eq!(a.options, b.options);
    assert_eq!(a.p.iter().map(|x| x.to_bits()).collect::<Vec<_>>(), b.p.iter().map(|x| x.to_bits()).collect::<Vec<_>>());
    assert_eq!(a.values.len(), b.values.len());
    for i in 0..a.options.len() {
        assert_eq!(a.values[i].iter().map(|x| x.to_bits()).collect::<Vec<_>>(), b.values[i].iter().map(|x| x.to_bits()).collect::<Vec<_>>());
        assert_eq!(a.mean(i).to_bits(), b.mean(i).to_bits());
        for j in 0..a.options.len() {
            assert_eq!(a.clearly_better(i, j), b.clearly_better(i, j));
        }
    }
    assert_eq!(a.best(), b.best());
}

#[test]
fn cached_and_uncached_continuations_have_identical_cells_weights_plans_and_values() {
    let (params, mind) = fixture();
    let solver = RefCell::new(None);
    let ctx = context(&params, &mind, &solver);
    let card = "r-Z".parse::<engine::Card>().unwrap().index();
    let moves = [Move::Card(card), Move::Announce(engine::Suit::Red), Move::Question(QuestionType::Yours)];
    let oracle = uncached(&ctx, PlayPhase::Start, &moves);
    assert_eq!(ctx.concepts.borrow().continuation_counts(), (0, 0));
    let (cold, cold_plans) = route_values(&ctx, PlayPhase::Start, &moves).unwrap();
    // Check every authored-plan/world cell, including plans discarded by best_per.
    ctx.with_worlds(|worlds| {
        assert_eq!(worlds.len(), oracle.p.len());
        for (i, &(action, plan)) in oracle.options.iter().enumerate() {
            for (j, world) in worlds.iter().enumerate() {
                let phase = if matches!(action, Move::Card(_)) { PlayPhase::Cards } else { PlayPhase::Start };
                let query = ContinuationQuery { world: world.hands, phase, action, plan };
                let actual = ctx.concepts.borrow().continuation_score(&query).unwrap();
                assert_eq!((actual.value() as f64).to_bits(), oracle.values[i][j].to_bits());
            }
        }
    });
    let (expected, picked) = oracle.best_per(|&(m, _)| Some(m));
    assert_eq!(cold_plans, picked.into_iter().map(|(_, p)| p).collect::<Vec<_>>());
    same_values(&expected, &cold);
    let before = ctx.concepts.borrow().continuation_counts();
    let (warm, warm_plans) = route_values(&ctx, PlayPhase::Start, &moves).unwrap();
    let after = ctx.concepts.borrow().continuation_counts();
    assert_eq!(after.0, before.0);
    assert!(after.1 > before.1);
    assert_eq!(cold_plans, warm_plans);
    same_values(&cold, &warm);
}

#[test]
fn ho1_card_result_is_reused_by_ho2_start_without_weakening_phase_or_plan_identity() {
    let (params, mind) = fixture();
    let solver = RefCell::new(None);
    let ctx = context(&params, &mind, &solver);
    let card = "r-Z".parse::<engine::Card>().unwrap().index();
    let lead = lead_values(&ctx, CardSet::single(card)).unwrap();
    let before = ctx.concepts.borrow().continuation_counts();
    let legal = [engine::GameAction {
        player: engine::PlaceAtTable(0),
        action_type: engine::ActionType::CardPlayed(engine::Card::from_index(card)),
    }];
    assert!(route_choice(&ctx, &legal, PlayPhase::Start, &[Move::Card(card)], Move::Card(card)).is_none());
    let after = ctx.concepts.borrow().continuation_counts();
    assert_eq!(after.0, before.0);
    assert!(after.1 > before.1);
    let (activation, _) = route_values(&ctx, PlayPhase::Start, &[Move::Card(card)]).unwrap();
    assert_eq!(lead.mean(0).to_bits(), activation.mean(0).to_bits());
    ctx.with_worlds(|worlds| {
        let world = worlds[0];
        let public = ctx.k.public_state(ctx.me, PlayPhase::Start);
        let mut start = FastState::from_public(public, world.hands).unwrap();
        let mut cards = FastState::from_public(engine::PublicState { phase: PlayPhase::Cards, ..public }, world.hands).unwrap();
        let (mut a, mut b) = (Known::new(&ctx), Known::new(&ctx));
        assert_eq!(start.legal_cards(), cards.legal_cards());
        a.play(&start, card);
        b.play(&cards, card);
        start.play(card);
        cards.play(card);
        assert_eq!(start, cards);
        assert_eq!(
            (a.may, a.pair_possible, a.read, a.held),
            (b.may, b.pair_possible, b.read, b.held)
        );
        let query = ContinuationQuery {
            world: world.hands,
            phase: PlayPhase::Start,
            action: Move::Announce(engine::Suit::Red),
            plan: Plan::all(ctx.me).next().unwrap(),
        };
        assert!(continuation_score(&ctx, query, Known::new(&ctx)).is_some());
        let before = ctx.concepts.borrow().continuation_counts();
        // Same announcement is illegal in Cards; no result is reused or stored.
        assert!(continuation_score(&ctx, ContinuationQuery { phase: PlayPhase::Cards, ..query }, Known::new(&ctx)).is_none());
        assert_eq!(ctx.concepts.borrow().continuation_counts(), before);
        let changed = ContinuationQuery { plan: Plan { next: 1, ..query.plan }, ..query };
        assert!(ctx.concepts.borrow().continuation_score(&changed).is_none());
        assert!(continuation_score(&ctx, changed, Known::new(&ctx)).is_some());
        assert_eq!(ctx.concepts.borrow().continuation_counts().0, before.0 + 1);
    });
    let fresh = context(&params, &mind, &solver);
    assert_eq!(fresh.concepts.borrow().continuation_counts(), (0, 0));
    same_values(&lead, &lead_values(&fresh, CardSet::single(card)).unwrap());
}

#[test]
fn changed_public_pair_fact_uses_fresh_context_and_matches_uncached_continuations() {
    let (params, mind) = fixture();
    let (_, mut changed_mind) = fixture();
    changed_mind.apply(&SeatEvent::NoPair { seat: 2 }).unwrap();
    let solver = RefCell::new(None);
    let old = context(&params, &mind, &solver);
    let fresh = context(&params, &changed_mind, &solver);
    let before = Known::new(&old);
    let after = Known::new(&fresh);
    assert_eq!(before.may, after.may, "NoPair is a correlated fact, not a card exclusion");
    assert_ne!(before.pair_possible[2], after.pair_possible[2]);
    assert_eq!(after.pair_possible[2], 0);

    let action = Move::Card("r-Z".parse::<engine::Card>().unwrap().index());
    let query = fresh.with_worlds(|worlds| ContinuationQuery {
        world: worlds[0].hands,
        phase: PlayPhase::Cards,
        action,
        plan: Plan::all(fresh.me).next().unwrap(),
    });
    assert!(continuation_score(&old, query, before).is_some());
    assert!(old.concepts.borrow().continuation_score(&query).is_some());
    assert_eq!(fresh.concepts.borrow().continuation_counts(), (0, 0));
    assert!(fresh.concepts.borrow().continuation_score(&query).is_none());

    // The exact same query key belongs to different immutable fact contexts.
    // Recompute from the new facts and compare all cells with the existing oracle.
    let oracle = uncached(&fresh, PlayPhase::Cards, &[action]);
    let (cold, cold_plans) = route_values(&fresh, PlayPhase::Cards, &[action]).unwrap();
    let actual = fresh.concepts.borrow().continuation_score(&query).unwrap();
    assert_eq!((actual.value() as f64).to_bits(), oracle.values[0][0].to_bits());
    let (expected, picked) = oracle.best_per(|&(m, _)| Some(m));
    assert_eq!(cold_plans, picked.into_iter().map(|(_, p)| p).collect::<Vec<_>>());
    same_values(&expected, &cold);
    let counts = fresh.concepts.borrow().continuation_counts();
    let (warm, warm_plans) = route_values(&fresh, PlayPhase::Cards, &[action]).unwrap();
    assert_eq!(fresh.concepts.borrow().continuation_counts().0, counts.0);
    assert!(fresh.concepts.borrow().continuation_counts().1 > counts.1);
    assert_eq!(cold_plans, warm_plans);
    same_values(&cold, &warm);
    assert_eq!(Known::new(&old).pair_possible, before.pair_possible);
}

#[test]
fn expired_batch_does_not_start_uncached_work_or_change_existing_admission() {
    let (params, mind) = fixture();
    let solver = RefCell::new(None);
    let mut ctx = context(&params, &mind, &solver);
    ctx.deadline = Some(std::time::Instant::now());
    let card = "r-Z".parse::<engine::Card>().unwrap().index();
    assert!(lead_values(&ctx, CardSet::single(card)).is_none());
    assert_eq!(ctx.concepts.borrow().continuation_counts(), (0, 0));
}


#[test]
fn pending_answer_cache_matches_uncached_and_rejects_cards_even_with_a_warm_card_result() {
    let params = crate::Params {
        outlook_worlds: 8,
        beliefs: beliefs::BeliefParams::base_only(),
        ..crate::Params::default()
    };
    let hand = CardSet::from_cards(
        &["r-Z", "r-K", "r-O", "e-K", "e-O", "e-6", "g-6", "s-6", "s-7"]
            .map(|s| s.parse().unwrap()),
    );
    let mut mind = Mind::new(0, hand, [crate::SeatKind::Unknown; 4], params.beliefs.clone());
    mind.apply(&SeatEvent::Bid { seat: 0, value: 120 }).unwrap();
    // Partner wins the first trick, then asks us for a pair. We can choose
    // between two truthful answers; after either, the asker leads, not us.
    for (seat, name) in ["g-6", "g-7", "g-A", "g-8"].into_iter().enumerate() {
        mind.apply(&SeatEvent::CardPlayed {
            seat: seat as u8,
            card: name.parse::<engine::Card>().unwrap().index(),
        })
        .unwrap();
    }
    mind.apply(&SeatEvent::AskPair { seat: 2 }).unwrap();
    let solver = RefCell::new(None);
    let ctx = context(&params, &mind, &solver);
    let phase = PlayPhase::Answer(QuestionType::Yours);
    let moves = [
        Move::Answer(engine::AnswerType::YesPair(engine::Suit::Red)),
        Move::Answer(engine::AnswerType::YesPair(engine::Suit::Acorns)),
    ];
    let oracle = uncached(&ctx, phase, &moves);
    assert_eq!(ctx.concepts.borrow().continuation_counts(), (0, 0));
    let (cold, cold_plans) = route_values(&ctx, phase, &moves).unwrap();
    ctx.with_worlds(|worlds| {
        assert_eq!(worlds.len(), oracle.p.len());
        for (j, world) in worlds.iter().enumerate() {
            let state = FastState::from_public(ctx.k.public_state(ctx.me, phase), world.hands).unwrap();
            assert_eq!(state.to_move(), ctx.me);
            assert!(state.legal_cards().is_empty());
            assert_eq!(state.moves().count(), moves.len());
            assert!(moves.iter().all(|&action| state.moves().any(|m| m == action)));
            for action in moves {
                let mut answered = state;
                let mut known = Known::new(&ctx).in_world(&ctx, &world.hands);
                apply(&mut answered, &mut known, action);
                assert_eq!(answered.phase(), PlayPhase::Cards);
                assert_eq!(answered.to_move(), engine::partner(ctx.me));
            }
            for (i, &(action, plan)) in oracle.options.iter().enumerate() {
                assert_eq!(plan.actor, ctx.me);
                let query = ContinuationQuery { world: world.hands, phase, action, plan };
                let actual = ctx.concepts.borrow().continuation_score(&query).unwrap();
                assert_eq!((actual.value() as f64).to_bits(), oracle.values[i][j].to_bits());
            }
        }
    });
    let (expected, picked) = oracle.best_per(|&(m, _)| Some(m));
    assert_eq!(cold_plans, picked.into_iter().map(|(_, p)| p).collect::<Vec<_>>());
    same_values(&expected, &cold);
    let before = ctx.concepts.borrow().continuation_counts();
    let (warm, warm_plans) = route_values(&ctx, phase, &moves).unwrap();
    let after = ctx.concepts.borrow().continuation_counts();
    assert_eq!(after.0, before.0);
    assert!(after.1 > before.1);
    assert_eq!(cold_plans, warm_plans);
    same_values(&cold, &warm);

    let card = Move::Card("e-6".parse::<engine::Card>().unwrap().index());
    ctx.with_worlds(|worlds| {
        for world in worlds {
            // A valid counterfactual Cards query is warm. It must not satisfy
            // this same concrete card while an answer is still pending.
            let query = ContinuationQuery {
                world: world.hands,
                phase: PlayPhase::Cards,
                action: card,
                plan: Plan::all(ctx.me).next().unwrap(),
            };
            assert!(continuation_score(&ctx, query, Known::new(&ctx)).is_some());
            let before = ctx.concepts.borrow().continuation_counts();
            assert!(continuation_score(
                &ctx,
                ContinuationQuery { phase, ..query },
                Known::new(&ctx),
            )
            .is_none());
            assert_eq!(ctx.concepts.borrow().continuation_counts(), before);
        }
    });
    let before = ctx.concepts.borrow().continuation_counts();
    assert!(route_values(&ctx, phase, &[card]).is_none());
    assert_eq!(ctx.concepts.borrow().continuation_counts(), before);
}

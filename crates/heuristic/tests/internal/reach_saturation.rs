//! Eager pre-change Reach oracle; no policy toggle or alternate production lane.
use super::*;
use agent::{SeatView, Table};
use beliefs::Mind;
use engine::Card;
use std::cell::RefCell;
use std::time::{Duration, Instant};

fn eager_reach(ctx: &Ctx, legal: &[GameAction]) -> Reach {
    let _timing = beliefs::profile::scope("pair.reach");
    let k = ctx.k;
    let mate = partner(ctx.me);
    let hand = k.certain(ctx.me);
    let uncalled = SUITS.into_iter().filter(|&s| !k.is_called(s));
    let mine = k.pairs(ctx.me).certain;
    let my_pairs: Vec<Suit> = uncalled.clone().filter(|&s| mine & 1 << s as u8 != 0).collect();
    // The believed pair is 1 or 0 where the facts decide (`beliefs`).
    let belief = ctx.mind.features(mate);
    let pair_p = |s: Suit| belief.pair[s as usize];
    let no_partner_pair = ConceptQuery {
        event: uncalled
            .clone()
            .map(|suit| Literal {
                concept: Concept::Pair { seat: mate, suit },
                value: false,
            })
            .collect(),
        given: vec![],
    };
    let partner_pair = ctx
        .joint_probability(&no_partner_pair)
        .map(|p| 1.0 - p)
        .unwrap_or_else(|| 1.0 - uncalled.clone().map(|s| 1.0 - pair_p(s)).product::<f32>());
    let mut halves: Vec<(Suit, f32)> = uncalled
        .filter_map(|s| {
            let mine = hand & CardSet::halves(s);
            if mine.len() != 1 {
                return None;
            }
            let other = (CardSet::halves(s) - mine).iter().next()?;
            Some((s, ctx.mind.p_holds(mate, other)))
        })
        .collect();
    halves.sort_by(|a, b| {
        (b.1 * pair_value(b.0) as f32)
            .total_cmp(&(a.1 * pair_value(a.0) as f32))
            .then((a.0 as u8).cmp(&(b.0 as u8)))
    });
    let mut p_partner_lead = ctx.mind.features(mate).ace;
    let requires_partner = (!my_pairs.is_empty() && !k.rights(ctx.me).may_announce() && k.rights(mate).may_ask_pair())
        || (partner_pair > 0.0 && !k.rights(ctx.me).may_ask_pair() && k.rights(mate).may_announce());
    if requires_partner {
        // Establish the hard entry first. Its existence is sufficient for
        // this access proxy; do not build a sampled Routes DP to rediscover it.
        let known_entry = legal.iter().any(|action| match action.action_type {
            ActionType::CardPlayed(c) if !CardSet::HALVES.contains(c.index()) => {
                !(k.partner_entry_winners(ctx.me, c.index()) - CardSet::HALVES).is_empty()
            }
            _ => false,
        });
        if known_entry {
            p_partner_lead = 1.0;
        } else {
            for action in legal {
                if let ActionType::CardPlayed(c) = action.action_type {
                    p_partner_lead = p_partner_lead.max(ctx.entry(c.index()).lead[mate as usize]);
                }
            }
        }
    }
    Reach {
        my_pairs,
        partner_pair,
        halves,
        p_partner_lead,
    }
}

#[test]
fn entry_access_stops_only_at_exact_one_without_reordering_reads() {
    let below_one = f32::from_bits(1.0_f32.to_bits() - 1);
    for (initial, values, expected_reads) in [
        (1.0, vec![0.1, 0.8, 1.0], 0),
        (0.2, vec![0.1, below_one, 1.0, 0.9], 3),
        (0.2, vec![0.1, 0.8, below_one, 0.9], 4),
    ] {
        let mut visited = Vec::new();
        let lazy = entry_access(
            initial,
            values.iter().enumerate().map(|(index, &p)| {
                visited.push(index);
                p
            }),
        );
        let eager = values.iter().fold(initial, |p, &entry| p.max(entry));
        assert_eq!(lazy.to_bits(), eager.to_bits());
        assert_eq!(visited, (0..expected_reads).collect::<Vec<_>>());
    }
}

fn at_lead(tricks: usize) -> Table {
    let hands = std::array::from_fn(|seat| (0..9).map(|index| Card::from_index((seat * 9 + index) as u8)).collect());
    let mut table = Table::new(&hands);
    assert_eq!(table.to_act().unwrap().0, 0);
    let bid = *table
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| matches!(a.action_type, ActionType::NewBid(_)))
        .unwrap();
    table.apply(bid);
    // Root receives then returns mate's Bells Ace, so its continued possession
    // is actor-visible. Root trades its Green Ace and keeps Bells Nine to regain
    // the second lead. All transfers/actions below use actual engine legality.
    let pass = CardSet::single(18) | CardSet::single(19) | CardSet::single(21) | CardSet::single(26);
    let back = CardSet::single(8) | CardSet::single(18) | CardSet::single(19) | CardSet::single(26);
    for _ in 0..16 {
        let (actor, legal) = table.to_act().unwrap();
        if legal.iter().any(|a| matches!(a.action_type, ActionType::CardPlayed(_))) {
            break;
        }
        let action = *legal
            .iter()
            .find(|a| a.action_type == ActionType::StopBidding)
            .or_else(|| {
                legal.iter().find(|a| {
                    matches!(a.action_type, ActionType::Pass(cards)
                if CardSet::from_cards(&cards) == if actor == 0 { back } else { pass })
                })
            })
            .expect("fixture reaches its legal return");
        table.apply(action);
    }
    for _ in 0..80 {
        let (actor, legal) = table.to_act().unwrap();
        if table.game.state.all_tricks.len() == tricks && table.game.state.phase == engine::GamePhase::StartTrick {
            assert_eq!(actor, 0);
            // Spend root's rights without activating trump or exposing a mate
            // pair. Root retains its Green pair; mate retains the known Bells Ace.
            let ask = *legal
                .iter()
                .find(|a| a.action_type == ActionType::Question(QuestionType::YourHalf(Suit::Red)))
                .unwrap();
            table.apply(ask);
            let answer = *table
                .to_act()
                .unwrap()
                .1
                .iter()
                .find(|a| matches!(a.action_type, ActionType::Answer(_)))
                .unwrap();
            table.apply(answer);
            assert_eq!(table.to_act().unwrap().0, 0);
            return table;
        }
        let action = *legal
            .iter()
            .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
            .min_by_key(|a| match a.action_type {
                ActionType::CardPlayed(c) => (
                    actor == 0 && CardSet::HALVES.contains(c.index()),
                    CardSet::ACES.contains(c.index()),
                    c.index(),
                ),
                _ => unreachable!(),
            })
            .expect("fixture follows legal cards");
        table.apply(action);
    }
    panic!("fixture did not reach the requested lead");
}

fn assert_saturated_entry_branch(ctx: &Ctx, legal: &[GameAction], reach: &Reach, eager: bool) {
    assert!(!reach.my_pairs.is_empty());
    assert!(!ctx.k.rights(ctx.me).may_announce());
    assert!(ctx.k.rights(partner(ctx.me)).may_ask_pair());
    assert!(
        !legal.iter().any(|action| match action.action_type {
            ActionType::CardPlayed(c) if !CardSet::HALVES.contains(c.index()) =>
                !(ctx.k.partner_entry_winners(ctx.me, c.index()) - CardSet::HALVES).is_empty(),
            _ => false,
        }),
        "must reach the sampled-entry loop, not the hard-entry shortcut"
    );
    assert_eq!(ctx.mind.features(partner(ctx.me)).ace.to_bits(), 1.0_f32.to_bits());
    assert_eq!(reach.p_partner_lead.to_bits(), 1.0_f32.to_bits());
    assert!(
        legal
            .iter()
            .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
            .count()
            >= 2
    );
    assert_eq!(
        ctx.routes.borrow().is_none(),
        !eager,
        "lazy Reach must actually skip eager Entry access"
    );
}

fn params() -> crate::Params {
    crate::Params {
        endgame_worlds: 8,
        endgame_tricks: 2,
        ..crate::Params::default()
    }
}

fn with_ctx<T>(
    view: &SeatView<'_>,
    params: &crate::Params,
    f: impl FnOnce(&Ctx<'_>) -> T,
) -> T {
    let mut mind = Mind::new(
        view.seat,
        view.knowledge.dealt_certain(view.seat),
        view.kinds,
        params.beliefs.clone(),
    );
    for event in view.events {
        mind.apply(event).unwrap();
    }
    let solver = RefCell::new(None);
    let ctx = Ctx {
        me: view.seat,
        k: mind.knowledge(),
        mind: &mind,
        params,
        solver: &solver,
        seed: params.seed ^ (view.seat as u64) << 56 ^ view.events.len() as u64,
        deadline: Some(Instant::now() + Duration::from_secs(60)),
        deadline_hit: Default::default(),
        series: Default::default(),
        routes: RefCell::new(None),
        concepts: RefCell::new(crate::concepts::Reads::default()),
    };
    f(&ctx)
}

fn entry_bits(entry: crate::routes::Entry) -> ([u32; 4], [u32; 4], u32) {
    (
        entry.lead.map(f32::to_bits),
        entry.activation.map(f32::to_bits),
        entry.both_follow.to_bits(),
    )
}
fn reach_bits(reach: &Reach) -> (Vec<Suit>, u32, Vec<(Suit, u32)>, u32) {
    (
        reach.my_pairs.clone(),
        reach.partner_pair.to_bits(),
        reach.halves.iter().map(|&(s, p)| (s, p.to_bits())).collect(),
        reach.p_partner_lead.to_bits(),
    )
}

#[test]
fn eager_and_lazy_reach_preserve_shared_reads_and_later_full_entries() {
    let table = at_lead(3);
    let (seat, raw_legal) = table.to_act().unwrap();
    let legal = crate::legal_actions::eligible(raw_legal);
    let view = table.view(seat);
    let params = params();
    let run = |eager: bool| {
        with_ctx(&view, &params, |ctx| {
            let reach = if eager {
                eager_reach(ctx, &legal)
            } else {
                Reach::new(ctx, &legal)
            };
            assert_saturated_entry_branch(ctx, &legal, &reach, eager);
            let before = ctx.concepts.borrow().snapshot();
            // Spend a small positive remaining allowance on genuinely uncertain
            // events. This cap only decreases test scratch; it cannot refill it.
            let other = (seat + 1) % 4;
            let queries: Vec<_> = (ctx.k.possible(other) - ctx.k.certain(other))
                .iter()
                .map(|card| ConceptQuery::new(Concept::Card { seat: other, card }))
                .filter(|query| query.hard(ctx.k).unwrap().known.is_none())
                .take(2)
                .collect();
            assert_eq!(queries.len(), 2);
            assert!(ctx.concepts.borrow().snapshot().0 > 0);
            ctx.concepts.borrow_mut().cap_for_test(1);
            assert_eq!(ctx.concepts.borrow().snapshot().0, 1);
            assert_eq!(ctx.joint_probability(&queries[0]), None);
            let spent = ctx.concepts.borrow().snapshot();
            assert_eq!(spent.0, 0, "uncertain query must spend positive traversal allowance");
            assert_eq!(ctx.joint_probability(&queries[0]), None);
            assert_eq!(
                ctx.concepts.borrow().snapshot(),
                spent,
                "cached query must not reorder or recharge"
            );
            assert_eq!(ctx.joint_probability(&queries[1]), None);
            let after = ctx.concepts.borrow().snapshot();
            assert_eq!(after.1[after.1.len() - 2].0, queries[0]);
            assert_eq!(after.1[after.1.len() - 1].0, queries[1]);
            assert_eq!(
                ctx.routes.borrow().is_none(),
                !eager,
                "uncertain queries must precede delayed Routes construction"
            );
            let entries: Vec<_> = legal
                .iter()
                .filter_map(|a| match a.action_type {
                    ActionType::CardPlayed(c) => Some((c.index(), entry_bits(ctx.entry(c.index())))),
                    _ => None,
                })
                .collect();
            assert_eq!(
                ctx.concepts.borrow().snapshot(),
                after,
                "Entry cannot spend/query shared Reads"
            );
            (reach_bits(&reach), before, entries, after)
        })
    };
    assert_eq!(run(false), run(true));
}

#[test]
fn initially_saturated_access_leaves_full_entries_lazy_and_bitwise_equal() {
    let table = at_lead(7);
    let (seat, legal) = table.to_act().unwrap();
    let view = table.view(seat);
    let params = params();
    let cards: Vec<_> = legal
        .iter()
        .filter_map(|a| match a.action_type {
            ActionType::CardPlayed(c) => Some(c.index()),
            _ => None,
        })
        .collect();
    let run = |eager: bool| {
        with_ctx(&view, &params, |ctx| {
            let before = ctx.concepts.borrow().snapshot();
            let entries = cards.iter().map(|&c| ctx.entry(c).lead[partner(seat) as usize]);
            let lead = if eager {
                entries.fold(1.0_f32, f32::max)
            } else {
                entry_access(1.0, entries)
            };
            assert_eq!(lead.to_bits(), 1.0_f32.to_bits());
            assert_eq!(ctx.routes.borrow().is_none(), !eager);
            assert_eq!(ctx.concepts.borrow().snapshot(), before);
            // Demand all fields later in reverse order, not only the access projection.
            cards
                .iter()
                .rev()
                .map(|&c| (c, entry_bits(ctx.entry(c))))
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(run(false), run(true));
}


use super::*;
use agent::Table;
use beliefs::BeliefParams;
use engine::{Card, QuestionType};

fn cards(codes: &[&str]) -> CardSet {
    codes.iter().fold(CardSet::EMPTY, |set, code| {
        set | CardSet::single(code.parse::<Card>().unwrap().index())
    })
}

/// Legal engine auction/exchange. Returning exactly the received four cards
/// preserves the specified root hand; hiding a half keeps its ownership unsure.
fn first_play(own: CardSet, hide_from_exchange: CardSet) -> Table {
    assert_eq!(own.len(), engine::HAND_SIZE as u32);
    let mut hands: [Vec<Card>; 4] = std::array::from_fn(|_| Vec::new());
    hands[0] = own.iter().map(Card::from_index).collect();
    for (i, card) in (CardSet::ALL - own).iter().enumerate() {
        hands[1 + i / engine::HAND_SIZE].push(Card::from_index(card));
    }
    let mut table = Table::new(&hands);
    apply_kind(&mut table, ActionType::NewBid(120));
    while table.game.state.phase == engine::GamePhase::Bidding {
        apply_kind(&mut table, ActionType::StopBidding);
    }
    let pass = *table
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| match a.action_type {
            ActionType::Pass(cards) => cards.iter().all(|card| !hide_from_exchange.contains(card.index())),
            _ => false,
        })
        .unwrap();
    let ActionType::Pass(received) = pass.action_type else {
        unreachable!()
    };
    table.apply(pass);
    let returned = *table
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| match a.action_type {
            ActionType::Pass(cards) => CardSet::from_cards(&cards) == CardSet::from_cards(&received),
            _ => false,
        })
        .unwrap();
    table.apply(returned);
    apply_kind(&mut table, ActionType::NewBid(125));
    assert_eq!(table.to_act().unwrap().0, 0);
    assert_eq!(table.view(0).knowledge.certain(0), own);
    table
}

fn apply_kind(table: &mut Table, kind: ActionType) {
    let action = *table
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| a.action_type == kind)
        .expect("scripted engine action must be legal");
    table.apply(action);
}

fn reader(table: &Table) -> Mind {
    let view = table.view(0);
    let mut mind = Mind::new(
        0,
        view.knowledge.dealt_certain(0),
        view.kinds,
        BeliefParams::base_only(),
    );
    for event in view.events {
        mind.apply(event).unwrap();
    }
    assert_eq!(mind.knowledge(), view.knowledge);
    mind
}

#[derive(Clone)]
struct Bound {
    offer: Offer,
    actions: Actions,
    reads: Vec<Read>,
}

/// Enumerate common-dispatcher offers in one permitted observation. This is a
/// test witness search, not an alternate strategic evaluator or value oracle.
fn available(program: &Program, stage: Stage, table: &Table) -> Vec<Bound> {
    fn walk(
        program: &Program,
        stage: Stage,
        table: &Table,
        mind: &Mind,
        prior: &[Read],
        seen: &mut Vec<Stage>,
        result: &mut Vec<Bound>,
    ) {
        if seen.contains(&stage) {
            return;
        }
        seen.push(stage);
        let view = table.view(0);
        let reading = inspect(program, stage, &view, table.to_act().unwrap().1, mind).unwrap();
        let mut reads = prior.to_vec();
        reads.extend(reading.consumed);
        match reading.frontier {
            Frontier::Actions(offers) => {
                let actions = reads
                    .iter()
                    .rev()
                    .find_map(|read| match &read.meaning {
                        ReadMeaning::Binding { actions, .. } => Some(actions.clone()),
                        _ => None,
                    })
                    .unwrap();
                result.extend(offers.into_iter().map(|offer| Bound {
                    offer,
                    actions: actions.clone(),
                    reads: reads.clone(),
                }));
            }
            Frontier::Choice(alternatives) => {
                for next in alternatives {
                    walk(program, next, table, mind, &reads, seen, result);
                }
            }
            Frontier::Tail | Frontier::Interrupted => {}
            Frontier::Reconsider { .. } => panic!("routine route must not require global reconsideration"),
        }
    }
    let mut result = Vec::new();
    walk(program, stage, table, &reader(table), &[], &mut Vec::new(), &mut result);
    result
}

fn advance_other_actors(table: &mut Table) {
    for _ in 0..128 {
        let Some((seat, legal)) = table.to_act() else {
            return;
        };
        if seat == 0 {
            return;
        }
        let action = *legal
            .iter()
            .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
            .or_else(|| legal.iter().find(|a| matches!(a.action_type, ActionType::Answer(_))))
            .expect("fixture response is an actual legal card or truthful answer");
        table.apply(action);
    }
    panic!("fixture response did not terminate")
}

#[test]
fn no_side_card_activation_and_uncertain_half_inquiry_have_authored_successors() {
    let mut table = first_play(CardSet::suit(Suit::Green), CardSet::EMPTY);
    let program = resource_program(&table.view(0));
    program.validate().unwrap();
    let announce = available(&program, program.start, &table)
        .into_iter()
        .find(|bound| {
            bound.offer.action.action_type == ActionType::AnnounceTrump(Suit::Green)
                && program.suffix(bound.offer.next).unwrap().nodes.iter().any(|node| {
                    matches!(
                        node,
                        Node::Guard {
                            predicate: Guard::Trump(Suit::Green),
                            ..
                        }
                    )
                })
        })
        .expect("activation must not need a side card");
    table.apply(announce.offer.action);
    let next = available(&program, announce.offer.next, &table);
    assert!(
        next.iter().any(|bound| matches!(bound.actions, Actions::Standing(_))),
        "same-suit cash/control survives activation"
    );
    assert!(next.iter().flat_map(|b| &b.reads).any(|read| matches!(
        read.meaning,
        ReadMeaning::Guard {
            predicate: Guard::Trump(Suit::Green),
            truth: Some(true)
        }
    )));

    let own = cards(&["g-K", "e-A", "r-A", "s-6", "s-7", "r-6", "e-6", "g-6", "g-7"]);
    let half = cards(&["g-O"]);
    let mut table = first_play(own, half);
    assert!(!(half & table.view(0).knowledge.may_hold(2)).is_empty());
    assert!((half & table.view(0).knowledge.certain(2)).is_empty());
    let program = resource_program(&table.view(0));
    let ask = available(&program, program.start, &table)
        .into_iter()
        .find(|bound| {
            bound.offer.action.action_type == ActionType::Question(QuestionType::YourHalf(Suit::Green))
                && program.suffix(bound.offer.next).unwrap().nodes.iter().any(|node| {
                    matches!(
                        node,
                        Node::Guard {
                            predicate: Guard::Trump(Suit::Green),
                            ..
                        }
                    )
                })
        })
        .expect("uncertain split route stays in the legal domain");
    table.apply(ask.offer.action);
    advance_other_actors(&mut table);
    let actual = table.view(0).knowledge.trump() == Some(Suit::Green);
    let next = available(&program, ask.offer.next, &table);
    assert!(
        !next.is_empty(),
        "negative or positive answer preserves the continuation"
    );
    assert!(next.iter().flat_map(|b| &b.reads).any(|read| matches!(read.meaning,
        ReadMeaning::Guard { predicate: Guard::Trump(Suit::Green), truth: Some(truth) } if truth == actual)));
}

#[test]
fn two_cashes_activation_draw_transfer_and_follow_exist_without_global_rebinding() {
    let own = cards(&["e-A", "r-A", "g-K", "g-O", "g-A", "g-Z", "s-6", "e-6", "r-6"]);
    let mut table = first_play(own, CardSet::EMPTY);
    let program = resource_program(&table.view(0));
    program.validate().unwrap();
    let card = |code: &str| ActionType::CardPlayed(code.parse::<Card>().unwrap());
    let script = [
        card("e-A"),
        card("r-A"),
        ActionType::AnnounceTrump(Suit::Green),
        card("g-A"),
        card("s-6"),
    ];
    let mut states = vec![table.clone()];
    for kind in script {
        apply_kind(&mut table, kind);
        advance_other_actors(&mut table);
        states.push(table.clone());
    }
    fn seek(
        program: &Program,
        stage: Stage,
        states: &[Table],
        script: &[ActionType],
        index: usize,
        failed: &mut Vec<(Stage, usize)>,
    ) -> Option<Vec<Offer>> {
        if index == script.len() {
            return program
                .suffix(stage)
                .unwrap()
                .nodes
                .iter()
                .any(|node| {
                    matches!(
                        node,
                        Node::Guard {
                            predicate: Guard::Leader(2),
                            ..
                        }
                    )
                })
                .then(Vec::new);
        }
        if failed.contains(&(stage, index)) {
            return None;
        }
        for bound in available(program, stage, &states[index]) {
            if bound.offer.action.action_type != script[index] {
                continue;
            }
            if [0, 1, 3].contains(&index) && !matches!(bound.actions, Actions::Standing(_)) {
                continue;
            }
            if let Some(mut rest) = seek(program, bound.offer.next, states, script, index + 1, failed) {
                rest.insert(0, bound.offer);
                return Some(rest);
            }
        }
        failed.push((stage, index));
        None
    }
    let path =
        seek(&program, program.start, &states, &script, 0, &mut Vec::new()).expect("explicit routine resource chain");
    let mut receipt = Receipt::new(program.clone(), &states[0].view(0)).unwrap();
    for (i, offer) in path.iter().enumerate() {
        receipt
            .propose(offer.clone(), &states[i].view(0), states[i].to_act().unwrap().1)
            .unwrap();
        receipt.synchronize(&states[i + 1].view(0)).unwrap();
        assert_eq!(receipt.stage, offer.next);
    }
    let last = states.last().unwrap();
    assert_ne!(
        last.view(0)
            .knowledge
            .public_state(0, PlayPhase::from_actions(last.to_act().unwrap().1))
            .leader,
        0
    );
    let follows = available(&program, receipt.stage, last);
    let follow = follows
        .iter()
        .find(|bound| {
            matches!(bound.offer.action.action_type, ActionType::CardPlayed(_))
                && bound.reads.iter().any(|r| {
                    matches!(
                        r.meaning,
                        ReadMeaning::Guard {
                            predicate: Guard::Leader(2),
                            ..
                        }
                    )
                })
                && program.suffix(bound.offer.next).unwrap().nodes.iter().any(|node| {
                    matches!(
                        node,
                        Node::Action {
                            actions: Actions::Exact(ActionType::Question(_)),
                            ..
                        }
                    )
                })
        })
        .expect("observed next leader drives a follow retaining the later activation goal");
    receipt
        .propose(follow.offer.clone(), &last.view(0), last.to_act().unwrap().1)
        .unwrap();
    let mut after = last.clone();
    after.apply(follow.offer.action);
    advance_other_actors(&mut after);
    receipt.synchronize(&after.view(0)).unwrap();
    assert_eq!(receipt.stage, follow.offer.next);
    assert!(receipt.pending.is_none());
    assert_eq!(receipt.program, program);
}

#[test]
fn routine_graph_is_finite_shared_and_raise_keeps_the_same_play_reconsideration() {
    let own = cards(&["e-A", "r-A", "g-K", "g-O", "g-A", "g-Z", "s-6", "e-6", "r-6"]);
    let table = first_play(own, CardSet::EMPTY);
    let program = resource_program(&table.view(0));
    program.validate().unwrap();
    assert!(
        program.nodes.len() < 512,
        "bounded stages, not a card/suit path product"
    );
    for (i, node) in program.nodes.iter().enumerate() {
        assert!(!program.nodes[..i].contains(node));
    }
    assert!(!program.nodes.iter().any(|node| matches!(node, Node::Reconsider { .. })));
    let mut raised = program.clone();
    raised.nodes.push(Node::Action {
        actions: Actions::Raises,
        next: raised.start,
    });
    raised.start = Stage(raised.nodes.len() - 1);
    let raised = with_reconsideration(&raised, 1).unwrap();
    let opening = with_reconsideration(&program, 1).unwrap();
    let Node::Action {
        actions: Actions::Raises,
        next,
    } = &raised.nodes[raised.start.0]
    else {
        panic!("raising family")
    };
    assert_eq!(raised.suffix(*next).unwrap(), opening.suffix(opening.start).unwrap());
    let bounded = with_reconsideration(&raised, 0).unwrap();
    assert!(!bounded.nodes.iter().any(|node| matches!(node, Node::Reconsider { .. })));
    // A non-standing investment shares recovery, with an actual surviving-entry
    // guard; it is not excluded merely because Standing does not prove a win.
    assert!(program.nodes.iter().any(|node| matches!(node,
        Node::Action { actions: Actions::Cards(mask), next }
        if mask.len() > 1 && program.suffix(*next).unwrap().nodes.iter().any(|node| matches!(node, Node::Guard { predicate: Guard::OwnLead, .. })))));
    assert!(program.nodes.iter().any(|node| matches!(node,
        Node::Guard { predicate: Guard::Known(query), yes, no, .. }
        if yes != no && query.given.is_empty() && query.event.iter().any(|term| matches!(term.concept, beliefs::Concept::Count { seat: 0, min: 1, .. })))));
}

//! A post-return raise is a real contract event, not a second auction message.

use agent::{SeatKind, Table};
use beliefs::{conventions::walk_auction, BeliefParams, Mind};
use engine::{ActionType, AnswerType, Card, CardSet, GamePhase, QuestionType, Suit, SUITS};
use knowledge::Belief;
use observation::SeatEvent;

fn apply(table: &mut Table, action_type: ActionType) {
    // Use the engine list directly: undo accepts may belong to either responder.
    let action = table
        .game
        .legal_actions()
        .into_iter()
        .find(|a| a.action_type == action_type)
        .unwrap_or_else(|| panic!("missing legal {action_type:?}"));
    table.apply(action);
}

fn undo(table: &mut Table) {
    apply(table, ActionType::UndoRequest);
    apply(table, ActionType::UndoAccept);
    apply(table, ActionType::UndoAccept);
}

fn auction() -> Table {
    let mut table = Table::new(&SUITS.map(|s| CardSet::suit(s).cards()));
    table.kinds = [SeatKind::Heuristic; 4];
    apply(&mut table, ActionType::NewBid(120));
    apply(&mut table, ActionType::StopBidding);
    apply(&mut table, ActionType::NewBid(150));
    table
}

fn returned() -> (Table, Vec<(u8, Option<i32>)>) {
    let mut table = auction();
    while table.game.state.phase == GamePhase::Bidding {
        apply(&mut table, ActionType::StopBidding);
    }
    let prefix = mind(&table, 0).record().auction.clone();
    // Give and return the same four low Green cards, restoring all original hands.
    let pass = ActionType::Pass([3, 2, 1, 0].map(Card::from_index));
    apply(&mut table, pass);
    apply(&mut table, pass);
    assert_eq!(table.game.state.phase, GamePhase::Raising);
    (table, prefix)
}

fn mind(table: &Table, seat: u8) -> Mind {
    let mut mind = Mind::new(
        seat,
        CardSet::suit(SUITS[seat as usize]),
        table.kinds,
        BeliefParams::default(),
    );
    for event in table.view(seat).events {
        mind.apply(event).unwrap();
    }
    mind
}

#[test]
fn post_return_raises_keep_contract_history_without_another_auction_likelihood() {
    let (before, prefix) = returned();
    for increment in [5, 10, 100] {
        let mut raised = before.clone();
        apply(&mut raised, ActionType::NewBid(150 + increment));
        let mut undone = raised.clone();
        undo(&mut undone);
        for seat in 0..4 {
            let mut warm = mind(&before, seat);
            let record = warm.record().clone();
            assert_eq!(record.auction, prefix);
            assert_eq!(record.passes.len(), 2);
            assert!(record.passes.iter().all(|p| p.cards.is_some() == (seat % 2 == 0)));
            let likelihoods = std::array::from_fn::<_, 4, _>(|speaker| warm.class_likelihood(speaker as u8, seat));
            let weights = warm.deal_weights();
            let events = warm.events().to_vec();
            let epoch = warm.epochs();

            for event in &raised.view(seat).events[events.len()..] {
                warm.apply(event).unwrap();
            }
            assert_eq!(warm.knowledge().value(), 150 + increment);
            assert_eq!(warm.public().value(), 150 + increment);
            assert_eq!(warm.knowledge().declarer(), Some(2));
            assert_eq!(
                warm.events().last(),
                Some(&SeatEvent::Bid {
                    seat: 2,
                    value: 150 + increment
                })
            );
            assert_ne!(warm.epochs().signal, epoch.signal);
            assert_eq!(*warm.record(), record);
            for (speaker, likelihood) in likelihoods.iter().enumerate() {
                // Includes the observer's own class read, not just other seats.
                assert_eq!(&warm.class_likelihood(speaker as u8, seat), likelihood);
            }
            let after_weights = warm.deal_weights();
            let original = SUITS.map(CardSet::suit);
            let mut alternate = original;
            let a = (seat as usize + 1) % 4;
            let b = (seat as usize + 3) % 4;
            let ace = (original[a] & CardSet::ACES).iter().next().unwrap();
            let low = original[b].iter().next().unwrap();
            alternate[a] = (alternate[a] - CardSet::single(ace)) | CardSet::single(low);
            alternate[b] = (alternate[b] - CardSet::single(low)) | CardSet::single(ace);
            for hands in [original, alternate] {
                assert!(weights.weight(&hands) > 0.0);
                assert_eq!(weights.weight(&hands), after_weights.weight(&hands));
            }

            for event in &undone.view(seat).events[raised.view(seat).events.len()..] {
                warm.apply(event).unwrap();
            }
            assert_eq!(warm.events(), events);
            assert_eq!(warm.knowledge().value(), 150);
            assert_eq!(warm.public().value(), 150);
            assert_eq!(*warm.record(), record);
            let fresh = mind(&undone, seat);
            assert_eq!(warm.knowledge(), fresh.knowledge());
            assert_eq!(*warm.record(), *fresh.record());
            for (speaker, likelihood) in likelihoods.iter().enumerate() {
                assert_eq!(&warm.class_likelihood(speaker as u8, seat), likelihood);
            }
            for hands in [original, alternate] {
                assert_eq!(weights.weight(&hands), warm.deal_weights().weight(&hands));
            }
        }
    }
}

#[test]
fn auction_bids_still_have_their_original_context_and_undo_replays_the_prefix() {
    for increment in [5, 10, 100] {
        let mut table = auction();
        let prefix = mind(&table, 0).record().auction.clone();
        apply(&mut table, ActionType::NewBid(150 + increment));
        let during = mind(&table, 0);
        let mut expected = prefix.clone();
        expected.push((3, Some(150 + increment)));
        assert_eq!(during.record().auction, expected);
        let observations = walk_auction(&expected, &during.params().conventions);
        for seat in 0..4 {
            assert_eq!(
                during.record().seats[seat].bids,
                observations
                    .iter()
                    .copied()
                    .filter(|o| o.ctx.speaker as usize == seat)
                    .collect::<Vec<_>>()
            );
        }
        undo(&mut table);
        let replayed = mind(&table, 0);
        assert_eq!(replayed.record().auction, prefix);
        assert_eq!(replayed.knowledge().value(), 150);
        // A pre-exchange history fork remains open to an alternative auction bid.
        apply(&mut table, ActionType::NewBid(160));
        assert_eq!(mind(&table, 0).record().auction.last(), Some(&(3, Some(160))));
    }
}

#[test]
fn later_plays_announcements_and_questions_remain_observed() {
    let (mut table, prefix) = returned();
    apply(&mut table, ActionType::NewBid(160));
    for lead in [26, 25] {
        if lead == 25 {
            apply(&mut table, ActionType::AnnounceTrump(Suit::Bells));
        }
        apply(&mut table, ActionType::CardPlayed(Card::from_index(lead)));
        for _ in 0..3 {
            let action = table
                .to_act()
                .unwrap()
                .1
                .iter()
                .filter_map(|a| match a.action_type {
                    ActionType::CardPlayed(card) => Some((card.index(), *a)),
                    _ => None,
                })
                .min_by_key(|(card, _)| *card)
                .unwrap()
                .1;
            table.apply(action);
        }
    }
    apply(&mut table, ActionType::Question(QuestionType::Yours));
    apply(&mut table, ActionType::Answer(AnswerType::YesPair(Suit::Green)));
    for seat in 0..4 {
        let mind = mind(&table, seat);
        let record = mind.record();
        assert_eq!(record.auction, prefix);
        assert_eq!(record.passes.len(), 2);
        assert_eq!(record.seats[2].melds, 1 << Suit::Bells as u8);
        assert_eq!(record.seats[0].melds, 1 << Suit::Green as u8);
        assert_eq!(record.seats[2].asks, vec![None]);
        assert_eq!(mind.knowledge().value(), 160);
        assert!(mind.public().is_called(Suit::Bells));
        assert!(mind.public().is_called(Suit::Green));
        assert_eq!(
            mind.events()
                .iter()
                .filter(|e| matches!(e, SeatEvent::CardPlayed { .. }))
                .count(),
            8
        );
        assert!(mind.events().contains(&SeatEvent::Bid { seat: 2, value: 160 }));
    }
}

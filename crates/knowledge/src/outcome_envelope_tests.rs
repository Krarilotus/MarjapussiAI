use super::*;
use engine::{Card, CardSet, Clock, FastState, Game, QuestionType, Suit};

fn eligible(game: &Game) -> Vec<GameAction> {
    game.legal_actions()
        .into_iter()
        .filter(|a| {
            !matches!(
                a.action_type,
                ActionType::UndoRequest | ActionType::UndoAccept | ActionType::UndoDecline
            )
        })
        .collect()
}

/// Full legal prefix; one suit per initial seat. Preserve K/O until the final
/// two tricks when requested, so the oracle covers announcements/questions too.
fn fixture(contract: Option<i32>, keep_pairs: bool) -> Game {
    let hands = std::array::from_fn(|s| (s * 9..s * 9 + 9).map(|i| Card::from_index(i as u8)).collect());
    let mut game = Game::new(
        "public envelope fixture".into(),
        std::array::from_fn(|_| "fixture".into()),
        Some(hands),
    )
    .with_clock(Clock::Off);
    let mut returned = None;
    while game.state.all_tricks.len() < (TRICKS - 2) as usize {
        let legal = eligible(&game);
        let action = match game.state.phase {
            GamePhase::WaitingForStart => legal[0],
            GamePhase::Bidding if game.state.value == NO_BID && contract.is_some() => *legal
                .iter()
                .find(|a| a.action_type == ActionType::NewBid(contract.unwrap()))
                .unwrap(),
            GamePhase::Bidding => *legal.iter().find(|a| a.action_type == ActionType::StopBidding).unwrap(),
            GamePhase::PassingForth => {
                let action = legal[0];
                if let ActionType::Pass(cards) = action.action_type {
                    returned = Some(cards);
                }
                action
            }
            GamePhase::PassingBack => *legal
                .iter()
                .find(|a| a.action_type == ActionType::Pass(returned.unwrap()))
                .unwrap(),
            _ => *legal
                .iter()
                .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
                .min_by_key(|a| match a.action_type {
                    ActionType::CardPlayed(card) => {
                        (keep_pairs && CardSet::HALVES.contains(card.index()), card.index())
                    }
                    _ => unreachable!(),
                })
                .unwrap(),
        };
        assert!(game.is_legal(&action));
        game.apply_action_mut(action);
    }
    assert!(matches!(game.state.phase, GamePhase::StartTrick));
    game
}

fn observed(game: &Game, observer: u8) -> (Knowledge, Vec<SeatEvent>) {
    let events = observation::seat_events(game, observer);
    let mut tracker = crate::Tracker::new(observer, observation::start_hand(game, observer));
    for event in &events {
        tracker.apply(event).unwrap();
    }
    (*tracker.knowledge(), events)
}

fn envelope<'a>(game: &Game, k: &'a Knowledge, events: &'a [SeatEvent]) -> PublicOutcomeEnvelope<'a> {
    PublicOutcomeEnvelope::from_forward_play(k, &game.state.phase, &eligible(game), events).unwrap()
}

/// Enumerate EVERY legal compact-engine continuation, including all truthful
/// answers, questions, trump changes and card orders; no policy/model oracle.
fn oracle(state: FastState, bounds: &PublicOutcomeEnvelope<'_>, leaves: &mut usize) {
    if state.is_over() {
        *leaves += 1;
        assert!(*leaves <= 100_000, "keep this oracle tiny; never truncate its support");
        let outcome = GameOutcome {
            playing_team: bounds.playing_team,
            game_value: bounds.game_value,
            team_points: [state.points(0), state.points(1)],
            team_tricks: [state.tricks_won(0), state.tricks_won(1)],
        };
        assert!(bounds.contains(&outcome), "{outcome:?}");
        let scores = game_scores(&outcome, SCHWARZ_FACTOR_FIFTHS);
        let intervals = bounds.score_intervals();
        for root in 0..2 {
            assert!(intervals.teams[root].contains(i64::from(scores[root])));
            for coefficient in [-3, -1, 0, 1, 2, i32::MIN, i32::MAX] {
                let value = i64::from(scores[root]) - i64::from(coefficient) * i64::from(scores[1 - root]);
                assert!(intervals.difference(root, coefficient).unwrap().contains(value));
            }
        }
        return;
    }
    for action in state.moves() {
        oracle(state.after(action), bounds, leaves);
    }
}

#[test]
fn exhaustive_small_legal_terminals_fit_public_boxes_and_signed_scores() {
    for (contract, keep_pairs) in [(None, false), (Some(140), false), (Some(420), false), (Some(140), true)] {
        let game = fixture(contract, keep_pairs);
        let (k, events) = observed(&game, 0);
        let bounds = envelope(&game, &k, &events);
        let state = FastState::from_game(&game).unwrap();
        assert_eq!(state.hand(state.to_move()).len(), 2);
        let mut leaves = 0;
        oracle(state, &bounds, &mut leaves);
        assert!(leaves > 0);
        assert!(bounds.score_intervals().difference(2, 1).is_none());
        let impossible = GameOutcome {
            playing_team: bounds.playing_team,
            game_value: bounds.game_value,
            team_points: bounds.points.map(|range| range[0]),
            team_tricks: bounds.tricks.map(|range| range[1]),
        };
        assert!(
            bounds.contains(&impossible),
            "independent relaxed boxes intentionally include impossible tuples"
        );
        assert!(impossible.team_tricks.iter().sum::<u8>() > TRICKS);
        let changed_context = GameOutcome {
            game_value: bounds.game_value + 5,
            ..impossible
        };
        assert!(!bounds.contains(&changed_context));
    }
}

#[test]
fn score_mapping_covers_threshold_rounding_and_conditional_schwarz_regions() {
    for points in [[[136, 142], [0, 9]], [[0, 9], [136, 142]]] {
        for playing_team in [None, Some(0), Some(1)] {
            let bounds = score_box(points, [[0, 9], [0, 9]], playing_team, 140);
            let mut exact = [IntegerInterval {
                lower: i64::MAX,
                upper: i64::MIN,
            }; 2];
            for a in points[0][0]..=points[0][1] {
                for b in points[1][0]..=points[1][1] {
                    for x in 0..=9 {
                        for y in 0..=9 {
                            let scores = game_scores(
                                &GameOutcome {
                                    playing_team,
                                    game_value: 140,
                                    team_points: [a, b],
                                    team_tricks: [x, y],
                                },
                                SCHWARZ_FACTOR_FIFTHS,
                            );
                            for t in 0..2 {
                                assert!(bounds.teams[t].contains(i64::from(scores[t])));
                                exact[t].lower = exact[t].lower.min(i64::from(scores[t]));
                                exact[t].upper = exact[t].upper.max(i64::from(scores[t]));
                            }
                        }
                    }
                }
            }
            assert_eq!(bounds.teams, exact, "extrema of the RELAXED box, not reachable games");
            if playing_team.is_none() {
                let rounded = points.map(|[lo, hi]| IntegerInterval {
                    lower: i64::from(engine::round5(lo)),
                    upper: i64::from(engine::round5(hi)),
                });
                assert_eq!(bounds.teams, rounded);
            }
        }
    }
    // A known prior trick rules out zero-trick declarer Schwarz.
    let known_trick = score_box([[0, 139], [0, 9]], [[1, 9], [0, 9]], Some(0), 140);
    assert_eq!(
        known_trick.teams[0],
        IntegerInterval {
            lower: -140,
            upper: -140
        }
    );
}

#[test]
fn spent_uncalled_half_removes_future_award_without_banking_current_trick_points() {
    let mut game = fixture(Some(140), true);
    let (k, events) = observed(&game, 0);
    let before = envelope(&game, &k, &events);
    assert_eq!(before.future_pair_budget(), PAIR_POINTS.iter().sum());
    let card = *eligible(&game)
        .iter()
        .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
        .unwrap();
    assert!(matches!(card.action_type, ActionType::CardPlayed(c) if CardSet::HALVES.contains(c.index())));
    game.apply_action_mut(card);
    let (after_k, after_events) = observed(&game, 0);
    let after = envelope(&game, &after_k, &after_events);
    assert_eq!(
        after.future_pair_budget(),
        before.future_pair_budget() - PAIR_POINTS[Suit::Green as usize]
    );
    assert_eq!(
        after.remaining_card_budget(),
        before.remaining_card_budget(),
        "played in unfinished trick is unbanked"
    );
    assert_eq!(after.points[0][0], before.points[0][0]);
    let mut leaves = 0;
    oracle(FastState::from_game(&game).unwrap(), &after, &mut leaves);
}

#[test]
fn called_pair_credit_survives_spending_its_half_and_public_bounds_ignore_private_view() {
    let mut game = fixture(Some(140), true);
    let announce = *eligible(&game)
        .iter()
        .find(|a| a.action_type == ActionType::AnnounceTrump(Suit::Green))
        .unwrap();
    game.apply_action_mut(announce);
    let (k, events) = observed(&game, 0);
    let before = envelope(&game, &k, &events);
    let banked = before.points[0][0];
    let card = eligible(&game)[0];
    assert!(matches!(card.action_type, ActionType::CardPlayed(c) if CardSet::HALVES.contains(c.index())));
    game.apply_action_mut(card);
    let (k0, events0) = observed(&game, 0);
    let (k1, events1) = observed(&game, 1);
    let a = envelope(&game, &k0, &events0);
    let b = envelope(&game, &k1, &events1);
    assert_eq!(a.points[0][0], banked);
    assert_eq!(a.future_pair_budget(), before.future_pair_budget());
    assert_eq!(a.points, b.points);
    assert_eq!(a.tricks, b.tricks);
    assert_eq!(a.score_intervals(), b.score_intervals());
}

#[test]
fn phase_domain_and_supplied_undo_rejection_are_explicit() {
    let game = fixture(Some(140), true);
    let (k, events) = observed(&game, 0);
    let legal = eligible(&game);
    for phase in [
        GamePhase::WaitingForStart,
        GamePhase::Bidding,
        GamePhase::PassingForth,
        GamePhase::PassingBack,
        GamePhase::Raising,
        GamePhase::Ended,
        GamePhase::PendingUndo(Box::new(GamePhase::Trick)),
    ] {
        assert!(matches!(
            PublicOutcomeEnvelope::from_forward_play(&k, &phase, &legal, &events),
            Err(EnvelopeError::UnsupportedPhase)
        ));
    }
    assert!(matches!(
        PublicOutcomeEnvelope::from_forward_play(&k, &game.state.phase, &[], &events),
        Err(EnvelopeError::InvalidDomain)
    ));
    for invalid in [
        ActionType::NewBid(145),
        ActionType::StopBidding,
        ActionType::Start,
        ActionType::Pass([Card::from_index(0); 4]),
        ActionType::UndoRequest,
        ActionType::UndoAccept,
        ActionType::UndoDecline,
    ] {
        let mut mixed = legal.clone();
        mixed.push(GameAction {
            player: legal[0].player,
            action_type: invalid,
        });
        assert!(matches!(
            PublicOutcomeEnvelope::from_forward_play(&k, &game.state.phase, &mixed, &events),
            Err(EnvelopeError::InvalidDomain)
        ));
    }
    let mut mixed_actor = legal.clone();
    mixed_actor[0].player.0 = (mixed_actor[0].player.0 + 1) % 4;
    assert!(matches!(
        PublicOutcomeEnvelope::from_forward_play(&k, &game.state.phase, &mixed_actor, &events),
        Err(EnvelopeError::InvalidDomain)
    ));
    let mut undone = events.clone();
    undone.push(SeatEvent::Undone);
    assert!(matches!(
        PublicOutcomeEnvelope::from_forward_play(&k, &game.state.phase, &legal, &undone),
        Err(EnvelopeError::UndoHistory)
    ));
}

#[test]
fn answer_kind_suit_and_empty_logical_trick_are_checked() {
    let mut game = fixture(None, true);
    let ask = *eligible(&game)
        .iter()
        .find(|a| a.action_type == ActionType::Question(QuestionType::YourHalf(Suit::Red)))
        .unwrap();
    game.apply_action_mut(ask);
    let (k, events) = observed(&game, 0);
    let legal = eligible(&game);
    assert!(PublicOutcomeEnvelope::from_forward_play(&k, &game.state.phase, &legal, &events).is_ok());
    assert!(matches!(
        PublicOutcomeEnvelope::from_forward_play(&k, &GamePhase::AnsweringPair, &legal, &events),
        Err(EnvelopeError::InvalidDomain)
    ));
    assert!(matches!(
        PublicOutcomeEnvelope::from_forward_play(&k, &GamePhase::AnsweringHalf(Suit::Bells), &legal, &events),
        Err(EnvelopeError::InvalidDomain)
    ));
    let mut leaves = 0;
    oracle(
        FastState::from_game(&game).unwrap(),
        &envelope(&game, &k, &events),
        &mut leaves,
    );
    let mut pair_game = fixture(None, true);
    let ask = *eligible(&pair_game)
        .iter()
        .find(|a| a.action_type == ActionType::Question(QuestionType::Yours))
        .unwrap();
    pair_game.apply_action_mut(ask);
    let (pair_k, pair_events) = observed(&pair_game, 0);
    let pair_legal = eligible(&pair_game);
    assert!(
        PublicOutcomeEnvelope::from_forward_play(&pair_k, &GamePhase::AnsweringPair, &pair_legal, &pair_events).is_ok()
    );
    assert!(matches!(
        PublicOutcomeEnvelope::from_forward_play(
            &pair_k,
            &GamePhase::AnsweringHalf(Suit::Red),
            &pair_legal,
            &pair_events
        ),
        Err(EnvelopeError::InvalidDomain)
    ));
    let mut pair_leaves = 0;
    oracle(
        FastState::from_game(&pair_game).unwrap(),
        &envelope(&pair_game, &pair_k, &pair_events),
        &mut pair_leaves,
    );
    assert!(pair_leaves > 0);
    let mut game = fixture(None, true);
    let card = *eligible(&game)
        .iter()
        .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
        .unwrap();
    game.apply_action_mut(card);
    let (k, events) = observed(&game, 0);
    assert!(matches!(
        PublicOutcomeEnvelope::from_forward_play(&k, &GamePhase::StartTrick, &eligible(&game), &events),
        Err(EnvelopeError::InvalidDomain)
    ));
}

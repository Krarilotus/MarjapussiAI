//! Owner rule: only asking moves that player's rights along own -> pair -> halves.
//! Complete legal traces, including the next trick; answers never spend rights.

use marjapussi_engine::{
    get_all_cards, ActionType, AnswerType, Card, Game, GameAction, GamePhase, PlaceAtTable,
    PlayerTrumpPossibilities as Right, QuestionType, Suit,
};

fn act(game: &mut Game, action_type: ActionType) {
    let fast = marjapussi_engine::FastState::from_game(game);
    let action = GameAction {
        action_type,
        player: PlaceAtTable(game.state.player_at_turn.0),
    };
    *game = game.apply_action(action).expect("fixture action must be legal");
    if let (Some(fast), Some(mv)) = (fast, marjapussi_engine::Move::from_action(&action_type)) {
        assert_eq!(Some(fast.after(mv)), marjapussi_engine::FastState::from_game(game));
    }
}

fn allows(game: &Game, action_type: ActionType) -> bool {
    game.legal_actions().iter().any(|a| a.action_type == action_type)
}

fn rights(game: &Game) -> [Right; 4] {
    [0, 1, 2, 3].map(|i| game.state.players[i].trump)
}

fn trick(game: &mut Game, lead: &str) {
    act(game, ActionType::CardPlayed(lead.parse().unwrap()));
    for _ in 0..3 {
        let card = game
            .legal_actions()
            .into_iter()
            .filter_map(|a| match a.action_type {
                ActionType::CardPlayed(c) => Some(c),
                _ => None,
            })
            .min_by_key(|c| c.index())
            .unwrap();
        act(game, ActionType::CardPlayed(card));
    }
    assert_eq!(game.state.phase, GamePhase::StartTrick);
}

fn after_opening(swaps: &[(&str, &str)]) -> Game {
    after_opening_at(swaps, 0)
}

fn after_opening_at(swaps: &[(&str, &str)], opening: u8) -> Game {
    let mut hands = [Suit::Red, Suit::Bells, Suit::Acorns, Suit::Green]
        .map(|s| get_all_cards().into_iter().filter(|c| c.suit == s).collect::<Vec<_>>());
    for &(a, b) in swaps {
        let locate = |text: &str| {
            let card: Card = text.parse().unwrap();
            hands
                .iter()
                .enumerate()
                .find_map(|(seat, hand)| hand.iter().position(|&c| c == card).map(|i| (seat, i)))
                .unwrap()
        };
        let ((x, i), (y, j)) = (locate(a), locate(b));
        let b = hands[y][j];
        hands[y][j] = std::mem::replace(&mut hands[x][i], b);
    }
    let mut game = Game::new(
        "question rights".into(),
        ["S0", "S1", "S2", "S3"].map(String::from),
        Some(std::array::from_fn(|seat| {
            hands[(seat + 4 - opening as usize) % 4].clone()
        })),
    );
    for seat in 0..4 {
        game = game
            .apply_action(GameAction {
                player: PlaceAtTable(seat),
                action_type: ActionType::Start,
            })
            .unwrap();
    }
    assert_eq!(game.state.player_at_turn.0, 0);
    for _ in 0..opening {
        act(&mut game, ActionType::StopBidding);
    }
    act(&mut game, ActionType::NewBid(120));
    for _ in 0..3 - opening {
        act(&mut game, ActionType::StopBidding);
    }
    let pass = game
        .legal_actions()
        .into_iter()
        .find_map(|a| match a.action_type {
            ActionType::Pass(cards) => Some(cards),
            _ => None,
        })
        .unwrap();
    act(&mut game, ActionType::Pass(pass));
    act(&mut game, ActionType::Pass(pass)); // Return the same cards: restore the fixture hands.
    trick(&mut game, "r-A");
    assert_eq!(game.state.player_at_turn.0, opening);
    game
}

#[test]
fn repeated_half_answers_do_not_restore_or_rescore_a_called_trump() {
    use marjapussi_engine::{team_points, GameCallback};
    for opening in [0, 1] {
        let mut game = after_opening_at(
            &[
                ("e-K", "r-6"),
                ("g-K", "r-7"),
                ("g-O", "e-6"),
                ("e-9", "r-8"),
                ("g-7", "r-9"),
            ],
            opening,
        );
        act(&mut game, ActionType::Question(QuestionType::YourHalf(Suit::Acorns)));
        act(&mut game, ActionType::Answer(AnswerType::YesHalf(Suit::Acorns)));
        assert_eq!(game.state.trump, Some(Suit::Acorns));
        trick(&mut game, "e-9");
        assert_eq!(game.state.player_at_turn.0, (opening + 2) % 4);

        // Repeating the current trump remains legal and confirms the halves only.
        let mut current = game.clone();
        let points = team_points(&current.state.all_tricks, &current.all_events).pairs;
        act(&mut current, ActionType::Question(QuestionType::YourHalf(Suit::Acorns)));
        act(&mut current, ActionType::Answer(AnswerType::YesHalf(Suit::Acorns)));
        assert_eq!(current.state.trump, Some(Suit::Acorns));
        assert_eq!(
            team_points(&current.state.all_tricks, &current.all_events).pairs,
            points
        );

        act(&mut game, ActionType::Question(QuestionType::YourHalf(Suit::Green)));
        act(&mut game, ActionType::Answer(AnswerType::YesHalf(Suit::Green)));
        assert_eq!(game.state.trump, Some(Suit::Green));
        trick(&mut game, "r-8");
        assert_eq!(game.state.player_at_turn.0, opening);
        let points = team_points(&game.state.all_tricks, &game.all_events).pairs;
        let called = game.state.trump_called.clone();
        act(&mut game, ActionType::Question(QuestionType::YourHalf(Suit::Acorns)));
        act(&mut game, ActionType::Answer(AnswerType::YesHalf(Suit::Acorns)));
        assert!(matches!(
            game.all_events.last().unwrap().callback,
            Some(GameCallback::StillTrump(Suit::Acorns))
        ));
        assert_eq!(game.state.trump, Some(Suit::Green));
        assert_eq!(game.state.trump_called, called);
        assert_eq!(team_points(&game.state.all_tricks, &game.all_events).pairs, points);
        // `act` compares every answer and card transition against FastState.
    }
}

#[test]
fn no_pair_answer_leaves_repeat_pair_questions_legal() {
    let mut game = after_opening(&[("e-K", "s-6")]); // Partner has no complete pair.
    act(&mut game, ActionType::Question(QuestionType::Yours));
    act(&mut game, ActionType::Answer(AnswerType::NoPair));
    assert_eq!(rights(&game), [Right::Yours, Right::Own, Right::Own, Right::Own]);
    assert!(
        !allows(&game, ActionType::Question(QuestionType::Yours)),
        "one question per lead"
    );
    trick(&mut game, "r-Z");
    assert_eq!(game.state.player_at_turn.0, 0);
    assert!(!allows(&game, ActionType::AnnounceTrump(Suit::Red)));
    for suit in [Suit::Red, Suit::Bells, Suit::Acorns, Suit::Green] {
        assert!(allows(&game, ActionType::Question(QuestionType::YourHalf(suit))));
    }
    act(&mut game, ActionType::Question(QuestionType::Yours));
    act(&mut game, ActionType::Answer(AnswerType::NoPair));
    assert_eq!(rights(&game), [Right::Yours, Right::Own, Right::Own, Right::Own]);
}

#[test]
fn a_positive_half_without_the_askers_half_keeps_the_existing_trump() {
    use marjapussi_engine::{team_points, GameCallback};
    let mut game = after_opening(&[("g-K", "e-7")]);
    act(&mut game, ActionType::AnnounceTrump(Suit::Red));
    trick(&mut game, "r-Z");
    let points = team_points(&game.state.all_tricks, &game.all_events).pairs;
    act(&mut game, ActionType::Question(QuestionType::YourHalf(Suit::Green)));
    act(&mut game, ActionType::Answer(AnswerType::YesHalf(Suit::Green)));
    assert!(matches!(
        game.all_events.last().unwrap().callback,
        Some(GameCallback::OnlyHalf(Suit::Green))
    ));
    assert_eq!(game.state.trump, Some(Suit::Red));
    assert!(!game.state.trump_called.contains(&Suit::Green));
    assert_eq!(team_points(&game.state.all_tricks, &game.all_events).pairs, points);
}

#[test]
fn even_an_unsuccessful_half_question_restricts_only_its_asker() {
    let mut game = after_opening(&[]);
    act(&mut game, ActionType::Question(QuestionType::YourHalf(Suit::Red)));
    act(&mut game, ActionType::Answer(AnswerType::NoHalf(Suit::Red)));
    assert_eq!(rights(&game), [Right::Ours, Right::Own, Right::Own, Right::Own]);
    trick(&mut game, "r-Z");
    assert!(!allows(&game, ActionType::AnnounceTrump(Suit::Red)));
    assert!(!allows(&game, ActionType::Question(QuestionType::Yours)));
    for suit in [Suit::Red, Suit::Bells, Suit::Acorns, Suit::Green] {
        assert!(allows(&game, ActionType::Question(QuestionType::YourHalf(suit))));
    }
}

#[test]
fn a_restricted_player_can_still_answer_with_its_pair() {
    for split in [false, true] {
        let mut game = after_opening(if split { &[("e-K", "r-6")] } else { &[] });
        let (question, answer, expected) = if split {
            (
                QuestionType::YourHalf(Suit::Acorns),
                AnswerType::YesHalf(Suit::Acorns),
                Right::Ours,
            )
        } else {
            (QuestionType::Yours, AnswerType::YesPair(Suit::Acorns), Right::Yours)
        };
        act(&mut game, ActionType::Question(question));
        act(&mut game, ActionType::Answer(answer));
        assert_eq!(rights(&game), [expected, Right::Own, Right::Own, Right::Own]);
        trick(&mut game, "r-Z"); // Partner trumps and gains a lead of its own.
        assert_eq!(game.state.player_at_turn.0, 2);
        act(&mut game, ActionType::Question(QuestionType::Yours));
        act(&mut game, ActionType::Answer(AnswerType::YesPair(Suit::Red)));
        assert_eq!(game.state.trump, Some(Suit::Red));
        assert_eq!(rights(&game), [expected, Right::Own, Right::Yours, Right::Own]);
    }
}

#[test]
fn answering_a_half_does_not_remove_ones_own_announcement_right() {
    let mut game = after_opening(&[("e-6", "r-6")]);
    act(&mut game, ActionType::Question(QuestionType::YourHalf(Suit::Acorns)));
    act(&mut game, ActionType::Answer(AnswerType::YesHalf(Suit::Acorns)));
    assert_eq!(game.state.trump, None, "the asker has no matching half");
    trick(&mut game, "e-6");
    assert_eq!(game.state.player_at_turn.0, 2);
    act(&mut game, ActionType::AnnounceTrump(Suit::Acorns));
    assert_eq!(rights(&game), [Right::Ours, Right::Own, Right::Own, Right::Own]);
}

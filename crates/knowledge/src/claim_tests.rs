use super::*;
use engine::{AnswerType, Card, FastState, PlayPhase, PlayerTrumpPossibilities, QuestionType, TRICKS};

#[path = "../tests/claim_oracle/mod.rs"]
mod oracle;

fn set(codes: &str) -> CardSet {
    CardSet::from_cards(
        &codes
            .split_whitespace()
            .map(|s| s.parse::<Card>().unwrap())
            .collect::<Vec<_>>(),
    )
}

/// Exact end fixtures: the missing deck cards were played, all live hands have
/// equal sizes, and public trick counts match a real compact engine state.
fn scene(codes: [&str; 4], trump: Option<Suit>, start: bool) -> (Knowledge, FastState) {
    let hands = codes.map(set);
    let n = hands[0].len() as u8;
    assert!(hands.iter().all(|h| h.len() == n as u32));
    let live = hands.into_iter().fold(CardSet::EMPTY, |a, h| a | h);
    assert_eq!(live.len(), n as u32 * 4);
    let mut k = Knowledge::new(0, hands[0]);
    k.certain = hands;
    k.possible = [CardSet::EMPTY; 4];
    k.hand_len = [n; 4];
    k.played = !live;
    k.tricks_done = TRICKS - n;
    k.trump = trump;
    k.called = trump.map_or(0, |s| 1 << s as u8);
    let phase = if start { PlayPhase::Start } else { PlayPhase::Cards };
    let state = FastState::from_public(k.public_state(0, phase), hands).unwrap();
    (k, state)
}

fn actions(state: FastState) -> Vec<GameAction> {
    state.moves().map(|mv| mv.to_action(state.to_move())).collect()
}

fn certified(k: &Knowledge, state: FastState) -> Vec<GameAction> {
    let plan = remaining_claim(k, &actions(state)).expect("fixture must have a certificate");
    oracle::check(state, &plan, state.hand(0).len() <= 3);
    plan
}

#[test]
fn played_rank_gaps_and_highest_first() {
    let (k, state) = scene(["e-A e-K e-U", "g-6 g-7 g-8", "s-6 s-7 s-8", "r-6 r-7 r-8"], None, true);
    let plan = certified(&k, state);
    assert_eq!(
        plan,
        ["e-A", "e-K", "e-U"].map(|s| Move::Card(set(s).iter().next().unwrap()).to_action(0))
    );
}

#[test]
fn currently_retaining_cards_can_become_ruffable() {
    let (k, state) = scene(["e-A e-Z", "e-6 r-6", "e-7 e-8", "e-9 e-U"], Some(Suit::Red), true);
    assert_eq!(k.retaining_lead(0), k.certain(0));
    assert!(remaining_claim(&k, &actions(state)).is_none());
    let (k, state) = scene(["e-A e-Z", "g-6 r-6", "e-7 e-8", "e-9 e-U"], Some(Suit::Red), true);
    assert!(remaining_claim(&k, &actions(state)).is_none(), "already void");
}

#[test]
fn draws_trump_before_the_side_winner() {
    let (k, state) = scene(["e-A r-A", "g-6 r-6", "s-6 r-7", "g-7 r-8"], Some(Suit::Red), true);
    let plan = certified(&k, state);
    assert_eq!(plan[0], Move::Card(set("r-A").iter().next().unwrap()).to_action(0));
}

#[test]
fn partner_higher_card_and_partner_ruff_are_refused() {
    let (k, state) = scene(["e-Z", "e-6", "e-A", "e-7"], None, true);
    assert!(k.standing(0).contains(set("e-Z").iter().next().unwrap()));
    assert!(remaining_claim(&k, &actions(state)).is_none());
    let (k, state) = scene(["e-A", "e-6", "r-6", "e-7"], Some(Suit::Red), true);
    assert!(remaining_claim(&k, &actions(state)).is_none());
}

#[test]
fn both_own_pairs_are_announced_in_separate_tricks() {
    let (k, state) = scene(
        [
            "g-K g-O e-K e-O",
            "g-6 g-7 e-6 e-7",
            "g-8 g-9 e-8 e-9",
            "g-U e-U s-6 s-7",
        ],
        None,
        true,
    );
    let plan = certified(&k, state);
    let declarations: Vec<_> = plan
        .iter()
        .enumerate()
        .filter(|(_, a)| matches!(a.action_type, ActionType::AnnounceTrump(_)))
        .collect();
    assert_eq!(declarations.len(), 2);
    assert!(declarations[1].0 > declarations[0].0 + 1);
    assert_eq!(plan[0].action_type, ActionType::AnnounceTrump(Suit::Green));
    assert!(plan
        .iter()
        .any(|a| a.action_type == ActionType::AnnounceTrump(Suit::Acorns)));
}

#[test]
fn cards_phase_cannot_consume_an_undeclared_own_pair() {
    let (k, state) = scene(["e-K e-O", "e-6 e-7", "e-8 e-9", "e-U g-6"], None, false);
    assert!(remaining_claim(&k, &actions(state)).is_none());
    let mut spent = k;
    spent.rights[0] = PlayerTrumpPossibilities::Yours;
    let state = FastState::from_public(spent.public_state(0, PlayPhase::Cards), state.hands()).unwrap();
    assert!(certified(&spent, state)
        .iter()
        .all(|a| matches!(a.action_type, ActionType::CardPlayed(_))));
}

#[test]
fn cards_phase_can_wait_for_the_next_declaration_window() {
    let (k, state) = scene(
        ["e-A e-K e-O", "g-6 g-7 g-8", "s-6 s-7 s-8", "r-6 r-7 r-8"],
        None,
        false,
    );
    let plan = certified(&k, state);
    assert!(matches!(plan[0].action_type, ActionType::CardPlayed(_)));
    assert_eq!(plan[1].action_type, ActionType::AnnounceTrump(Suit::Acorns));
}

#[test]
fn possible_partner_pairs_and_split_pairs_preserve_points() {
    let (mut k, state) = scene(["g-A g-Z", "r-6 r-7", "e-K e-O", "s-6 s-7"], None, true);
    // Neither pair half's owner is known, but partner can jointly hold both.
    let hidden = state.in_hands() - k.certain(0);
    for seat in 1..4 {
        k.certain[seat] = CardSet::EMPTY;
        k.possible[seat] = hidden;
    }
    assert_eq!(k.team_pairs(0).yours.certain, 0);
    assert_ne!(k.team_pairs(0).yours.possible, 0);
    assert!(remaining_claim(&k, &actions(state)).is_none());
    k.rights[0] = PlayerTrumpPossibilities::Ours;
    let state = FastState::from_public(k.public_state(0, PlayPhase::Start), state.hands()).unwrap();
    certified(&k, state);
    let (mut k, state) = scene(["g-A e-K", "r-6 r-7", "e-O s-6", "s-7 s-8"], None, true);
    k.rights[0] = PlayerTrumpPossibilities::Ours;
    let state = FastState::from_public(k.public_state(0, PlayPhase::Start), state.hands()).unwrap();
    assert!(
        remaining_claim(&k, &actions(state)).is_none(),
        "half questions survive spent rights"
    );
}

#[test]
fn safe_cards_are_not_enough_when_the_required_trump_switch_is_unsafe() {
    let (k, state) = scene(
        [
            "e-K e-O g-K g-O s-A",
            "e-6 e-7 e-8 e-9 r-6",
            "g-6 g-7 g-8 g-9 r-7",
            "e-U g-U r-8 r-9 s-6",
        ],
        Some(Suit::Red),
        true,
    );
    assert!(remaining_claim(&k, &actions(state)).is_none());
}

#[test]
fn full_hand_certificate_respects_the_first_trick_ace_rule() {
    let (k, state) = scene(
        [
            "g-6 g-7 g-8 g-9 g-U g-O g-K g-Z g-A",
            "e-6 e-9 e-K s-6 s-9 s-K r-6 r-9 r-K",
            "e-7 e-U e-Z s-7 s-U s-Z r-7 r-U r-Z",
            "e-8 e-O e-A s-8 s-O s-A r-8 r-O r-A",
        ],
        None,
        false,
    );
    let plan = certified(&k, state);
    assert_eq!(plan[0], Move::Card(set("g-A").iter().next().unwrap()).to_action(0));
    assert_eq!(plan[1].action_type, ActionType::AnnounceTrump(Suit::Green));
}

#[test]
fn rejects_wrong_actor_answers_nonplay_and_inexact_or_nonempty_hands() {
    let (k, state) = scene(["e-A", "e-6", "e-7", "e-8"], None, true);
    let legal = actions(state);
    for action in [
        Move::Card(set("e-A").iter().next().unwrap()).to_action(1),
        Move::Answer(AnswerType::NoPair).to_action(0),
        GameAction {
            action_type: ActionType::NewBid(140),
            player: engine::PlaceAtTable(0),
        },
        GameAction {
            action_type: ActionType::UndoAccept,
            player: engine::PlaceAtTable(0),
        },
    ] {
        assert!(remaining_claim(&k, &[action]).is_none());
    }
    assert!(remaining_claim(&k, &[Move::Question(QuestionType::Yours).to_action(0)]).is_none());
    let mut changed = k;
    changed.possible[0] = set("g-6");
    assert!(remaining_claim(&changed, &legal).is_none());
    changed = k;
    changed.hand_len[0] = 0;
    assert!(remaining_claim(&changed, &legal).is_none());
    changed = k;
    changed.trick_len = 1;
    assert!(remaining_claim(&changed, &legal).is_none());
    assert!(remaining_claim(&Knowledge::public(), &legal).is_none());
}

#[test]
fn first_lead_must_be_in_the_supplied_legal_set() {
    let (k, state) = scene(["e-A e-Z", "g-6 g-7", "s-6 s-7", "r-6 r-7"], None, true);
    let low_only = [Move::Card(set("e-Z").iter().next().unwrap()).to_action(0)];
    assert!(remaining_claim(&k, &low_only).is_none());
    certified(&k, state);
}

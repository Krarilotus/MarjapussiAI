use engine::{Card, CardSet, FastState, Move, PlayPhase, PlayerTrumpPossibilities as Rights, QuestionType, Suit};
use solver::trick_control;

fn c(s: &str) -> u8 {
    s.parse::<Card>().unwrap().index()
}
fn position<const N: usize>(cards: [[&str; N]; 4], trump: Option<Suit>) -> FastState {
    let hands = cards.map(|h| {
        h.into_iter()
            .fold(CardSet::EMPTY, |s, card| s | CardSet::single(c(card)))
    });
    let mut p = FastState::new(hands, 0).public();
    p.tricks_done = 9 - N as u8;
    p.tricks = [p.tricks_done, 0];
    p.phase = PlayPhase::Start;
    p.trump = trump;
    p.called = trump.map_or(0, |s| 1 << s as u8);
    FastState::from_public(p, hands).unwrap()
}

#[test]
fn ruff_entry_needs_to_survive_the_players_who_can_use_their_trumps() {
    let hands = [
        ["e-6", "e-7", "g-A"],
        ["e-A", "s-A", "g-6"],
        ["s-K", "s-7", "r-6"],
        ["e-K", "s-Z", "g-9"],
    ];
    let lead = Move::Card(c("e-6"));
    // Both opponents follow; their Ace and Ten cannot stop partner's King.
    assert!(trick_control(position(hands, Some(Suit::Bells)), lead).lead(2));
    let mut void = hands;
    void[3][0] = "r-A";
    // A later round has exhausted the opponent's last Acorn: now it overruffs.
    assert!(!trick_control(position(void, Some(Suit::Bells)), lead).lead(2));
    void[1][1] = "s-K";
    void[2][0] = "s-A";
    // Partner can deliberately choose its Ace, even though its Seven loses.
    assert!(trick_control(position(void, Some(Suit::Bells)), lead).lead(2));
    // The same hands with a different active trump do not give that ruff entry.
    assert!(!trick_control(position(void, Some(Suit::Green)), lead).lead(2));
}

#[test]
fn activation_requires_the_winners_rights_and_surviving_halves() {
    let hands = [
        ["e-A", "r-K", "r-O"],
        ["e-6", "g-6", "s-6"],
        ["e-7", "g-7", "s-7"],
        ["e-8", "g-8", "s-8"],
    ];
    let s = position(hands, None);
    let lead = Move::Card(c("e-A"));
    assert!(trick_control(s, lead).activation(0, Suit::Red));
    for right in [Rights::Yours, Rights::Ours] {
        let mut p = s.public();
        p.rights[0] = right;
        // Partner's untouched Own right is useless while seat 0 has the lead.
        let goals = trick_control(FastState::from_public(p, s.hands()).unwrap(), lead);
        assert!(goals.lead(0));
        assert!(!goals.activation(0, Suit::Red));
    }
    let split = position(
        [
            ["e-A", "r-K", "g-6"],
            ["e-6", "g-7", "s-6"],
            ["e-7", "r-O", "s-7"],
            ["e-8", "g-8", "s-8"],
        ],
        None,
    );
    for right in [Rights::Own, Rights::Yours, Rights::Ours] {
        let mut p = split.public();
        p.rights[0] = right;
        assert!(trick_control(FastState::from_public(p, split.hands()).unwrap(), lead).activation(0, Suit::Red));
    }
    let forced_half = position(
        [
            ["r-A", "r-K", "g-6"],
            ["r-6", "g-7", "s-6"],
            ["r-O", "e-A", "s-7"],
            ["r-8", "g-8", "s-8"],
        ],
        None,
    );
    let goals = trick_control(forced_half, Move::Card(c("r-A")));
    assert!(goals.lead(0));
    assert!(!goals.activation(0, Suit::Red));
    let last = position([["r-A"], ["r-6"], ["r-O"], ["r-8"]], None);
    assert!(!trick_control(last, Move::Card(c("r-A"))).activation(0, Suit::Red));
}

#[test]
fn a_team_winner_is_not_necessarily_the_leaders_next_opportunity() {
    let s = position(
        [
            ["e-K", "r-K", "r-O"],
            ["e-6", "g-6", "s-6"],
            ["e-A", "g-7", "s-7"],
            ["e-8", "g-8", "s-8"],
        ],
        None,
    );
    let lead = Move::Card(c("e-K"));
    assert!(trick_control(s, lead).team(0));
    assert!(trick_control(s, lead).lead(2));
    assert!(!trick_control(s, lead).lead(0));
    assert!(trick_control(s, lead).activation(0, Suit::Red)); // Partner can ask Yours.
    let mut p = s.public();
    p.rights[2] = Rights::Ours;
    assert!(!trick_control(FastState::from_public(p, s.hands()).unwrap(), lead).activation(0, Suit::Red));
}

#[test]
fn questions_can_create_or_restore_the_trump_needed_for_entry() {
    let s = position(
        [
            ["r-K", "s-O", "e-6"],
            ["e-A", "g-6", "g-7"],
            ["s-K", "e-8", "g-A"],
            ["e-K", "g-K", "g-O"],
        ],
        Some(Suit::Red),
    );
    let lead = Move::Card(c("r-K"));
    assert!(!trick_control(s, lead).lead(2));
    let question = Move::Question(QuestionType::YourHalf(Suit::Bells));
    assert!(trick_control(s, question).lead(2));
    let mut p = s.public();
    p.called |= 1 << Suit::Bells as u8;
    let restored = FastState::from_public(p, s.hands()).unwrap();
    assert!(trick_control(restored, question).lead(2));
    let answered = restored
        .after(question)
        .after(Move::Answer(engine::AnswerType::YesHalf(Suit::Bells)));
    assert_eq!(answered.trump(), Some(Suit::Bells));
    assert_eq!(answered.pair_points(0), restored.pair_points(0));
}

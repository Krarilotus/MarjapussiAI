//! The cheap actor does not cash over the partner's only possible card when that card is the
//! other half of my split pair (port of live 9f85768; invented Green example, not a private deal).
use super::*;

fn cards(names: &str) -> CardSet {
    CardSet::from_cards(&names.split(' ').map(|s| s.parse().unwrap()).collect::<Vec<_>>())
}

fn card(name: &str) -> u8 {
    name.parse::<engine::Card>().unwrap().index()
}

/// Seat 0 leads trick 4 with g-A g-Z standing over g-K g-6 and holds g-O; the partner's only
/// possible Green card is g-K. The partner may hold an Acorns pair, so Yours is the question.
fn split_position() -> (FastState, Known) {
    let hands = [
        cards("g-A g-Z g-O e-9 e-6 s-6"),
        cards("g-6 e-U s-U e-K s-A r-A"),
        cards("g-K e-7 e-8 s-7 r-7 r-8"),
        cards("e-A e-O s-K s-O r-9 s-9"),
    ];
    let remaining = hands.into_iter().fold(CardSet::EMPTY, |a, h| a | h);
    let mut public = FastState::new(hands, 0).public();
    public.phase = PlayPhase::Start;
    public.tricks_done = 3;
    public.tricks = [3, 0];
    public.card_points = [(120 - engine::points_set(remaining)) as i16, 0];
    let state = FastState::from_public(public, hands).unwrap();
    let mut known = Known {
        may: [remaining; 4],
        pair_possible: [0b1111; 4],
        read: [0; 4],
        held: [CardSet::EMPTY; 4],
    };
    known.may[2] -= CardSet::suit(Suit::Green) - cards("g-K");
    (state, known)
}

#[test]
fn the_cheap_actor_asks_instead_of_cashing_over_a_forced_split_half() {
    let (state, known) = split_position();
    assert_eq!(lead0(&state, &known), card("g-A"));
    // Without the split, cashing g-A first is the cheap actor's choice (the guard alone decides).
    let others = known.may[1] | known.may[2] | known.may[3];
    assert!(crate::leads::cash_before_question(state.hand(0), None, others - state.hand(0), card("g-A")));
    assert_eq!(activation(&state, &known, None), Some(Move::Question(QuestionType::Yours)));
}

#[test]
fn a_possible_green_spare_with_the_partner_restores_the_cash() {
    let (state, mut known) = split_position();
    known.may[2] |= cards("g-6");
    assert_eq!(activation(&state, &known, None), None);
}

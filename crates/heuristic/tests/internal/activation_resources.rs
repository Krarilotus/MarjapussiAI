use super::*;
use beliefs::{BeliefParams, Mind};
use observation::SeatEvent;
use std::cell::RefCell;

fn set(cards: &str) -> CardSet {
    cards.split_whitespace().fold(CardSet::EMPTY, |hand, text| {
        hand | CardSet::single(text.parse::<engine::Card>().unwrap().index())
    })
}

fn spend_half(mind: &mut Mind, seat: u8, suit: Suit) {
    mind.apply(&SeatEvent::AskHalf { seat, suit }).unwrap();
    mind.apply(&SeatEvent::NoHalf {
        seat: engine::partner(seat),
        suit,
    })
    .unwrap();
}

fn access(mind: &Mind, suit: Suit, own: f32, partner: f32) -> f32 {
    let params = crate::Params::default();
    let solver = RefCell::new(None);
    let ctx = Ctx {
        me: 0,
        k: mind.knowledge(),
        mind,
        params: &params,
        solver: &solver,
        seed: 7,
        deadline: None,
        deadline_hit: Default::default(),
        series: Default::default(),
        class: Default::default(),
        routes: RefCell::new(None),
        concepts: RefCell::new(crate::concepts::Reads::default()),
    };
    // No inferred fields are used by this conditional ownership calculation.
    let reach = Reach {
        my_pairs: vec![],
        partner_pair: 0.0,
        halves: vec![],
        p_partner_lead: 0.0,
    };
    reach.p_activate(&ctx, suit, Control { own, partner })
}

#[test]
fn own_control_cannot_replace_the_eligible_partners_lead() {
    let mut mind = Mind::new(
        0,
        set("r-K r-O e-A e-6 e-7 g-6 g-7 s-6 s-7"),
        [crate::SeatKind::Unknown; 4],
        BeliefParams::base_only(),
    );
    spend_half(&mut mind, 0, Suit::Bells);
    assert_eq!(access(&mind, Suit::Red, 8.0, 0.0), 0.0);
    // Root can truthfully answer Yours even though its own right is Ours.
    assert_eq!(access(&mind, Suit::Red, 0.0, 1.0), 1.0);
    let own = mind.knowledge().certain(0);
    let other: Vec<_> = (!own).iter().collect();
    let hands = std::array::from_fn(|seat| {
        if seat == 0 {
            own
        } else {
            other[(seat - 1) * 9..seat * 9]
                .iter()
                .fold(CardSet::EMPTY, |h, &c| h | CardSet::single(c))
        }
    });
    let hands = hands.map(|h| h - CardSet::single((h - CardSet::HALVES).iter().next().unwrap()));
    let mut public = engine::FastState::new(hands, 2).public();
    public.tricks_done = 1;
    public.tricks = [1, 0];
    public.phase = engine::PlayPhase::Start;
    public.rights[0] = engine::PlayerTrumpPossibilities::Ours;
    let asked = engine::FastState::from_public(public, hands)
        .unwrap()
        .after(engine::Move::Question(QuestionType::Yours));
    assert!(asked
        .moves()
        .any(|answer| answer == engine::Move::Answer(engine::AnswerType::YesPair(Suit::Red))));
    spend_half(&mut mind, 2, Suit::Green);
    assert_eq!(access(&mind, Suit::Red, 8.0, 8.0), 0.0);
}

#[test]
fn partner_control_cannot_replace_a_spent_own_announcement() {
    let mut mind = Mind::new(
        0,
        set("r-K r-O e-A e-6 e-7 g-6 g-7 s-6 s-7"),
        [crate::SeatKind::Unknown; 4],
        BeliefParams::base_only(),
    );
    mind.apply(&SeatEvent::Pass {
        from: 0,
        to: 2,
        cards: Some(set("r-K r-O s-6 s-7")),
    })
    .unwrap();
    spend_half(&mut mind, 0, Suit::Bells);
    assert_eq!(access(&mind, Suit::Red, 8.0, 0.0), 0.0);
    assert_eq!(access(&mind, Suit::Red, 0.0, 1.0), 1.0);
    mind.apply(&SeatEvent::AskPair { seat: 2 }).unwrap();
    mind.apply(&SeatEvent::NoPair { seat: 0 }).unwrap();
    assert_eq!(access(&mind, Suit::Red, 8.0, 8.0), 0.0);
}

#[test]
fn split_activation_does_not_require_either_players_own_or_yours_right() {
    let mut mind = Mind::new(
        0,
        set("r-K r-O e-A e-6 e-7 g-6 g-7 s-6 s-7"),
        [crate::SeatKind::Unknown; 4],
        BeliefParams::base_only(),
    );
    mind.apply(&SeatEvent::Pass {
        from: 0,
        to: 2,
        cards: Some(set("r-O g-7 s-6 s-7")),
    })
    .unwrap();
    for seat in [0, 2] {
        spend_half(&mut mind, seat, Suit::Bells);
    }
    assert_eq!(access(&mind, Suit::Red, 1.0, 0.0), 1.0);
    assert_eq!(access(&mind, Suit::Red, 0.0, 1.0), 1.0);
}

#[test]
fn one_actor_exclusive_opening_is_not_reused_for_every_known_pair() {
    let mut mind = Mind::new(
        0,
        set("r-K r-O e-K e-O e-A g-6 g-7 s-6 s-7"),
        [crate::SeatKind::Unknown; 4],
        BeliefParams::base_only(),
    );
    spend_half(&mut mind, 2, Suit::Bells);
    assert!(pair_value(Suit::Red) > pair_value(Suit::Acorns));
    assert_eq!(access(&mind, Suit::Red, 1.0, 0.0), 1.0);
    assert!(access(&mind, Suit::Acorns, 1.0, 0.0) < 1.0);
    assert_eq!(access(&mind, Suit::Acorns, 2.0, 0.0), 1.0);
}

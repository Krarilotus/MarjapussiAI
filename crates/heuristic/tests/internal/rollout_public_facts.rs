//! Public response facts survive rollout transitions without private hand inference.
use super::*;
use beliefs::Mind;
use observation::SeatEvent;
use std::cell::RefCell;

fn cards(names: &[&str]) -> CardSet {
    CardSet::from_cards(&names.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>())
}

fn opening(hands: [CardSet; 4]) -> (FastState, Known) {
    let mut state = FastState::new(hands, 0);
    let mut known = Known {
        may: [state.in_hands(); 4],
        pair_possible: [0b1111; 4],
        read: [0; 4],
        held: [CardSet::EMPTY; 4],
    };
    for name in ["s-A", "s-6", "s-7", "s-8"] {
        let c = name.parse::<engine::Card>().unwrap().index();
        known.play(&state, c);
        state.play(c);
    }
    (state, known)
}

fn negative_opening() -> (FastState, Known) {
    opening([
        cards(&["s-A", "r-A", "e-O", "g-6"]),
        cards(&["s-6", "r-K", "e-K", "g-7"]),
        cards(&["s-7", "r-6", "e-8", "g-8"]),
        cards(&["s-8", "r-7", "e-9", "g-9"]),
    ])
}

fn same_public(a: &Known, b: &Known) {
    assert_eq!(a.may, b.may);
    assert_eq!(a.pair_possible, b.pair_possible);
    assert_eq!(a.read, b.read);
    assert_eq!(a.held, b.held);
}

fn answer_both(question: QuestionType, answer: AnswerType) -> (FastState, Known) {
    let (mut automatic, mut a) = negative_opening();
    let (mut direct, mut b) = (automatic, a);
    apply(&mut automatic, &mut a, Move::Question(question));
    direct.apply(Move::Question(question));
    assert!(direct.moves().any(|m| m == Move::Answer(answer)));
    apply(&mut direct, &mut b, Move::Answer(answer));
    assert_eq!(automatic, direct);
    same_public(&a, &b);
    (direct, b)
}

#[test]
fn no_pair_direct_and_automatic_exclude_pairs_but_preserve_half_questions() {
    let (mut state, mut known) = answer_both(QuestionType::Yours, AnswerType::NoPair);
    assert_eq!(known.pair_possible[2], 0);
    assert!(CardSet::halves(Suit::Acorns).is_subset(known.may[2]));
    assert!(
        state.rights(0).may_ask_pair(),
        "NoPair does not spend all question rights"
    );
    // Regain a real opening: engine still permits another Yours, but facts must
    // reject that futile question while preserving a possible split-half route.
    for name in ["r-A", "r-K", "r-6", "r-7"] {
        let c = name.parse::<engine::Card>().unwrap().index();
        known.play(&state, c);
        state.play(c);
    }
    assert!(state.moves().any(|m| m == Move::Question(QuestionType::Yours)));
    assert_eq!(
        ask(&state, &known),
        Some(Move::Question(QuestionType::YourHalf(Suit::Acorns)))
    );
}

#[test]
fn no_half_direct_and_automatic_exclude_both_halves_publicly() {
    let (_, known) = answer_both(QuestionType::YourHalf(Suit::Acorns), AnswerType::NoHalf(Suit::Acorns));
    assert!((known.may[2] & CardSet::halves(Suit::Acorns)).is_empty());
    assert_eq!(known.pair_possible[2] & (1 << Suit::Acorns as u8), 0);
    assert!(CardSet::halves(Suit::Acorns).is_subset(known.may[1]));
}

#[test]
fn negative_public_answer_is_invariant_under_unobserved_half_allocation() {
    let (mut a, mut ka) = negative_opening();
    // Seat 3's cards and public prefix are unchanged; the asker privately has
    // the other half here. Public NoHalf must imply the same two exclusions.
    let (mut b, mut kb) = opening([
        cards(&["s-A", "r-A", "e-K", "g-6"]),
        cards(&["s-6", "r-K", "e-O", "g-7"]),
        cards(&["s-7", "r-6", "e-8", "g-8"]),
        cards(&["s-8", "r-7", "e-9", "g-9"]),
    ]);
    for state in [&mut a, &mut b] {
        state.apply(Move::Question(QuestionType::YourHalf(Suit::Acorns)));
    }
    apply(&mut a, &mut ka, Move::Answer(AnswerType::NoHalf(Suit::Acorns)));
    apply(&mut b, &mut kb, Move::Answer(AnswerType::NoHalf(Suit::Acorns)));
    same_public(&ka, &kb);
}

#[test]
fn positive_pair_and_announcement_reveal_ownership_without_private_allocations() {
    let (mut automatic, mut a) = opening([
        cards(&["s-A", "r-A", "e-6", "g-6"]),
        cards(&["s-6", "r-K", "e-7", "g-7"]),
        cards(&["s-7", "e-K", "e-O", "g-8"]),
        cards(&["s-8", "r-7", "e-9", "g-9"]),
    ]);
    let (mut direct, mut b) = (automatic, a);
    let (mut swapped, mut kswap) = opening([
        cards(&["s-A", "r-A", "e-6", "g-6"]),
        cards(&["s-6", "r-K", "e-9", "g-7"]),
        cards(&["s-7", "e-K", "e-O", "g-8"]),
        cards(&["s-8", "r-7", "e-7", "g-9"]),
    ]);
    let bit = 1 << Suit::Acorns as u8;
    for known in [&mut a, &mut b, &mut kswap] {
        known.read = [bit; 4];
    }
    apply(&mut swapped, &mut kswap, Move::Question(QuestionType::Yours));
    apply(&mut automatic, &mut a, Move::Question(QuestionType::Yours));
    same_public(&a, &kswap);
    direct.apply(Move::Question(QuestionType::Yours));
    apply(&mut direct, &mut b, Move::Answer(AnswerType::YesPair(Suit::Acorns)));
    same_public(&a, &b);
    let halves = CardSet::halves(Suit::Acorns);
    assert_eq!(
        a.read,
        [bit, bit, 0, bit],
        "only the publicly revealed owner overrides its reading"
    );
    assert!(halves.is_subset(a.may[2]));
    for seat in [0, 1, 3] {
        assert!((a.may[seat] & halves).is_empty());
    }
    let (mut announced, mut c) = opening([
        cards(&["s-A", "e-K", "e-O", "g-6"]),
        cards(&["s-6", "r-K", "e-7", "g-7"]),
        cards(&["s-7", "r-A", "e-6", "g-8"]),
        cards(&["s-8", "r-7", "e-9", "g-9"]),
    ]);
    c.read = [bit; 4];
    apply(&mut announced, &mut c, Move::Announce(Suit::Acorns));
    assert_eq!(c.read, [0, bit, bit, bit]);
    assert!(halves.is_subset(c.may[0]));
    for seat in [1, 2, 3] {
        assert!((c.may[seat] & halves).is_empty());
    }
    assert!(c.pair_possible.iter().all(|p| p & (1 << Suit::Acorns as u8) == 0));
}

#[test]
fn positive_half_does_not_assign_a_hidden_half() {
    let (mut a, mut ka) = opening([
        cards(&["s-A", "r-A", "e-K", "g-6"]),
        cards(&["s-6", "r-K", "e-7", "g-7"]),
        cards(&["s-7", "r-6", "e-O", "g-8"]),
        cards(&["s-8", "r-7", "e-9", "g-9"]),
    ]);
    let before = ka.may;
    let bit = 1 << Suit::Acorns as u8;
    ka.read = [bit; 4];
    let (mut direct, mut kd) = (a, ka);
    apply_observed(
        &mut direct,
        &mut kd,
        Move::Question(QuestionType::YourHalf(Suit::Acorns)),
    );
    assert_eq!(kd.read, [bit; 4], "a question alone proves no ownership");
    apply(&mut direct, &mut kd, Move::Answer(AnswerType::YesHalf(Suit::Acorns)));
    apply(&mut a, &mut ka, Move::Question(QuestionType::YourHalf(Suit::Acorns)));
    assert_eq!(a, direct);
    same_public(&ka, &kd);
    assert_eq!(a.trump(), Some(Suit::Acorns));
    assert_eq!(
        ka.read,
        [0, bit, 0, bit],
        "a newly called pair proves both partners have a half"
    );
    assert_eq!(ka.may, before, "positive half keeps individual allocation unresolved");
}

#[test]
fn half_question_without_asker_half_only_clears_the_positive_respondent_read() {
    let bit = 1 << Suit::Acorns as u8;
    let other_bit = 1 << Suit::Green as u8;
    let mut results = Vec::new();
    // Swap the two hidden halves while preserving the same positive answer and
    // public callback. Neither question nor answer can expose that allocation.
    for (respondent, opponent) in [("e-K", "e-O"), ("e-O", "e-K")] {
        let (mut automatic, mut a) = opening([
            cards(&["s-A", "r-A", "e-6", "g-6"]),
            cards(&["s-6", "r-K", opponent, "g-7"]),
            cards(&["s-7", "r-6", respondent, "g-8"]),
            cards(&["s-8", "r-7", "e-9", "g-9"]),
        ]);
        a.read = [bit | other_bit; 4];
        let before = a.may;
        let (mut direct, mut d) = (automatic, a);
        apply_observed(
            &mut direct,
            &mut d,
            Move::Question(QuestionType::YourHalf(Suit::Acorns)),
        );
        assert_eq!(d.read, [bit | other_bit; 4]);
        assert!(direct
            .moves()
            .any(|m| m == Move::Answer(AnswerType::YesHalf(Suit::Acorns))));
        apply(&mut direct, &mut d, Move::Answer(AnswerType::YesHalf(Suit::Acorns)));
        apply(
            &mut automatic,
            &mut a,
            Move::Question(QuestionType::YourHalf(Suit::Acorns)),
        );
        assert_eq!(automatic, direct);
        same_public(&a, &d);
        assert_eq!(automatic.called() & bit, 0);
        assert_eq!(a.read, [bit | other_bit, bit | other_bit, other_bit, bit | other_bit]);
        assert_eq!(a.may, before);
        results.push(a);
    }
    same_public(&results[0], &results[1]);
}

#[test]
fn card_play_narrows_pairs_and_only_hypothetical_exchange_resets_exclusions() {
    let (mut state, mut known) = negative_opening();
    let c = "e-O".parse::<engine::Card>().unwrap().index();
    known.play(&state, c);
    state.play(c);
    assert!(known.pair_possible.iter().all(|p| p & (1 << Suit::Acorns as u8) == 0));
    let mut known = Known {
        may: [
            cards(&["e-K"]),
            CardSet::halves(Suit::Green),
            cards(&["e-O"]),
            CardSet::halves(Suit::Red),
        ],
        pair_possible: [0; 4],
        read: [0; 4],
        held: [CardSet::EMPTY; 4],
    };
    known.exchanged(0, cards(&["e-K"]), cards(&["e-O"]), [CardSet::EMPTY; 2]);
    assert_eq!(
        known.pair_possible, [0; 4],
        "historical transfer cannot erase later NoPair"
    );
    known.exchanging(0);
    assert_ne!(known.pair_possible[0] & (1 << Suit::Acorns as u8), 0);
    assert_ne!(known.pair_possible[2] & (1 << Suit::Acorns as u8), 0);
    assert_eq!(known.pair_possible[1], 0);
    assert_eq!(known.pair_possible[3], 0);
}

#[test]
fn initialization_uses_public_pair_constraints_not_observers_private_hand() {
    let params = crate::Params::default();
    let hand = cards(&["r-K", "r-O", "s-A", "s-Z", "s-9", "g-6", "g-7", "e-6", "e-7"]);
    let mut mind = Mind::new(0, hand, [crate::SeatKind::Unknown; 4], params.beliefs.clone());
    mind.apply(&SeatEvent::NoPair { seat: 2 }).unwrap();
    let solver = RefCell::new(None);
    let ctx = Ctx {
        me: 0,
        k: mind.knowledge(),
        mind: &mind,
        params: &params,
        solver: &solver,
        seed: 0,
        deadline: None,
        deadline_hit: Default::default(),
        series: Default::default(),
        routes: RefCell::new(None),
        concepts: RefCell::new(crate::concepts::Reads::default()),
    };
    let known = Known::new(&ctx);
    assert_eq!(known.pair_possible[2], 0);
    assert!(CardSet::halves(Suit::Acorns).is_subset(known.may[2]));
    assert_eq!(ctx.k.pairs(1).possible & (1 << Suit::Red as u8), 0);
    assert_ne!(known.pair_possible[1] & (1 << Suit::Red as u8), 0);
}

fn exchanged_mind(kept: &[&str]) -> (crate::Params, Mind) {
    let params = crate::Params::default();
    let forth = cards(&["e-6", "g-6", "s-6", "r-6"]);
    let back = cards(&["g-A", "s-A", "r-A", "r-Z"]);
    let mut mind = Mind::new(
        0,
        forth | cards(kept),
        [crate::SeatKind::Unknown; 4],
        params.beliefs.clone(),
    );
    mind.apply(&SeatEvent::Pass {
        from: 0,
        to: 2,
        cards: Some(forth),
    })
    .unwrap();
    mind.apply(&SeatEvent::Pass {
        from: 2,
        to: 0,
        cards: Some(back),
    })
    .unwrap();
    (params, mind)
}

fn known_from(params: &crate::Params, mind: &Mind) -> Known {
    let solver = RefCell::new(None);
    let ctx = Ctx {
        me: 0,
        k: mind.knowledge(),
        mind,
        params,
        solver: &solver,
        seed: 0,
        deadline: None,
        deadline_hit: Default::default(),
        series: Default::default(),
        routes: RefCell::new(None),
        concepts: RefCell::new(crate::concepts::Reads::default()),
    };
    Known::new(&ctx)
}

#[test]
fn historical_exchange_loading_preserves_later_no_pair_and_private_half_does_not_clear_read() {
    let bit = 1 << Suit::Acorns as u8;
    let mut results = Vec::new();
    for half in ["e-K", "e-O"] {
        let (params, mut mind) = exchanged_mind(&[half, "e-7", "e-8", "g-7", "s-7"]);
        mind.apply(&SeatEvent::NoPair { seat: 2 }).unwrap();
        assert_eq!(mind.public().count_bounds(0, Suit::Acorns).0, 0);
        assert!(mind.knowledge().count_bounds(0, Suit::Acorns).0 > 0);
        let known = known_from(&params, &mind);
        assert_eq!(
            known.pair_possible[2], 0,
            "loading historical exchange preserves later NoPair"
        );
        assert_ne!(
            known.read[0] & bit,
            0,
            "observer's private kept cards cannot clear a public reading"
        );
        results.push(known);
    }
    same_public(&results[0], &results[1]);
}

#[test]
fn historical_exchange_read_yields_to_public_positive_half_constraints_without_allocation_leak() {
    let bit = 1 << Suit::Acorns as u8;
    let mut results = Vec::new();
    for half in ["e-K", "e-O"] {
        let (params, mut mind) = exchanged_mind(&[half, "e-7", "e-8", "g-7", "s-7"]);
        assert_ne!(known_from(&params, &mind).read[0] & bit, 0);
        mind.apply(&SeatEvent::YesHalf {
            seat: 0,
            suit: Suit::Acorns,
            asker_has_half: false,
        })
        .unwrap();
        assert!((mind.public().certain(0) & CardSet::suit(Suit::Acorns)).is_empty());
        assert_eq!(mind.public().count_bounds(0, Suit::Acorns).0, 1);
        let known = known_from(&params, &mind);
        assert_eq!(known.read[0] & bit, 0);
        assert!(CardSet::halves(Suit::Acorns).is_subset(known.may[0]));
        results.push(known);
    }
    same_public(&results[0], &results[1]);
}

#[test]
fn historical_exchange_read_yields_to_public_pair_ownership() {
    let bit = 1 << Suit::Acorns as u8;
    for event in [
        SeatEvent::AnnounceTrump {
            seat: 0,
            suit: Suit::Acorns,
        },
        SeatEvent::YesPair {
            seat: 0,
            suit: Suit::Acorns,
        },
    ] {
        let (params, mut mind) = exchanged_mind(&["e-K", "e-O", "e-7", "g-7", "s-7"]);
        assert_ne!(known_from(&params, &mind).read[0] & bit, 0);
        mind.apply(&event).unwrap();
        assert!(CardSet::halves(Suit::Acorns).is_subset(mind.public().certain(0)));
        let known = known_from(&params, &mind);
        assert_eq!(known.read[0] & bit, 0);
        assert_eq!(known.pair_possible[0] & bit, 0);
        assert!(CardSet::halves(Suit::Acorns).is_subset(known.may[0]));
    }
}

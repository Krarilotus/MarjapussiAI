//! The public rule transitions (E8): each rule once, as a pure function at the crate root.

use marjapussi_engine::{
    bid_range, is_bid, trick_points, trump_points, winning_seat, Card, PlayerTrumpPossibilities as Right, Points,
    QuestionType, Suit, CARDS, LAST_TRICK_BONUS, MAX_BID, NO_BID, PAIR_POINTS, SUITS, TRICKS,
};

#[test]
fn question_rights() {
    let rights = [Right::Own, Right::Yours, Right::Ours];
    let after_yours = [Right::Yours, Right::Yours, Right::Ours];
    for (r, want) in rights.into_iter().zip(after_yours) {
        assert_eq!(r.after_question(QuestionType::Yours), want);
        for s in SUITS {
            assert_eq!(r.after_question(QuestionType::YourHalf(s)), Right::Ours);
        }
    }
    assert_eq!(rights.map(Right::may_announce), [true, false, false]);
    assert_eq!(rights.map(Right::may_ask_pair), [true, true, false]);
}

#[test]
fn trick_points_and_last_trick_bonus() {
    let trick = ["r-A", "r-Z", "g-K", "e-6"].map(|c| c.parse::<Card>().unwrap().index());
    for done in 0..TRICKS - 1 {
        assert_eq!(trick_points(trick, done), 25);
    }
    assert_eq!(trick_points(trick, TRICKS - 1), 25 + LAST_TRICK_BONUS);
    let deck: Vec<u8> = (0..CARDS as u8).collect();
    let all: i32 = deck.chunks(4).map(|t| trick_points([t[0], t[1], t[2], t[3]], 0)).sum();
    assert_eq!(all, 120);
}

#[test]
fn pair_points_once_per_suit() {
    for s in SUITS {
        assert_eq!(trump_points(s, false), PAIR_POINTS[s as usize]);
        assert_eq!(trump_points(s, true), 0);
    }
    assert_eq!(trump_points(Suit::Red, false), 100);
}

#[test]
fn is_bid_is_bid_range_membership() {
    for value in NO_BID.0 - 20..=MAX_BID + 10 {
        let range: Vec<i32> = bid_range(Points(value)).collect();
        for bid in -10..=MAX_BID + 20 {
            assert_eq!(
                is_bid(Points(value), bid),
                range.contains(&bid),
                "value {value} bid {bid}"
            );
        }
    }
}

#[test]
fn winning_seat_counts_from_the_leader() {
    let idx = |c: &str| c.parse::<marjapussi_engine::Card>().unwrap().index();
    let trick = [idx("g-6"), idx("g-A"), idx("e-A"), idx("g-Z")];
    for leader in 0..4 {
        assert_eq!(winning_seat(leader, &trick, None), (leader + 1) % 4);
        assert_eq!(winning_seat(leader, &trick, Some(Suit::Acorns)), (leader + 2) % 4);
    }
}

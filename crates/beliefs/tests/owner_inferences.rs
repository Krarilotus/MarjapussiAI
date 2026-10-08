//! KB batch 1b (`findings/kb/concepts.md`): the pair inferences the owner expects, from the
//! steps combined with my own cards. The exact feature DP over the facts produces them; each
//! is pinned here with a position. Seat 0 is me; seat 2 my partner; 1 and 3 the opponents.

use agent::SeatKind;
use beliefs::{BeliefParams, Mind};
use engine::{Card, CardSet, Suit};
use observation::{Seat, SeatEvent};

fn mind(hand: &[&str], auction: &[(Seat, Option<i32>)]) -> Mind {
    let set = hand.iter().fold(CardSet::EMPTY, |a, c| {
        a | CardSet::single(c.parse::<Card>().unwrap().index())
    });
    let mut m = Mind::new(0, set, [SeatKind::Human; 4], BeliefParams::default());
    for &(seat, bid) in auction {
        let e = match bid {
            Some(value) => SeatEvent::Bid { seat, value },
            None => SeatEvent::StopBidding { seat },
        };
        m.apply(&e).unwrap();
    }
    m
}

#[test]
fn an_opponents_big_pair_is_the_other_big_suit_when_i_hold_one_big_half() {
    // I hold the red king: an opponent's +15 (a big pair) can only be Bells. Since C39 (with a
    // pair, the ace first) the +15 also says "no ace", a rarer hand, so the Bells pair is read
    // well above its prior but no longer above 0.5 (0.34 at C39).
    let hand = ["r-K", "g-6", "g-7", "e-6", "e-7", "s-6", "r-6", "r-7", "e-8"];
    let m = mind(&hand, &[(0, None), (1, Some(130))]);
    let prior = mind(&hand, &[(0, None)]).has_pair(1, Suit::Bells);
    assert_eq!(m.has_pair(1, Suit::Red), 0.0);
    let bells = m.has_pair(1, Suit::Bells);
    assert!(bells > 2.0 * prior, "{bells} vs prior {prior}");
}

#[test]
fn an_opponents_big_pair_while_i_hold_both_big_halves_is_a_bluff() {
    let m = mind(
        &["r-K", "s-O", "g-6", "g-7", "e-6", "e-7", "s-6", "r-6", "r-7"],
        &[(0, None), (1, Some(130))],
    );
    assert!(m.bluffing(1));
    assert!(!mind(
        &["r-K", "g-6", "g-7", "e-6", "e-7", "s-6", "r-6", "r-7", "e-8"],
        &[(0, None), (1, Some(130))]
    )
    .bluffing(1));
}

#[test]
fn an_opponents_plus_10_while_i_hold_both_small_halves_means_three_halves() {
    // I hold a half of Acorns and of Green: the opponent's +10 is no small pair, so 3+ halves.
    let m = mind(
        &["e-K", "g-O", "g-6", "g-7", "e-6", "e-7", "s-6", "r-6", "r-7"],
        &[(0, None), (1, Some(125))],
    );
    assert_eq!(m.has_pair(1, Suit::Acorns), 0.0);
    assert_eq!(m.has_pair(1, Suit::Green), 0.0);
    assert!(m.has_halves(1, 3) > 0.5, "{}", m.has_halves(1, 3));
}

#[test]
fn the_partners_strict_two_halves_connect_with_my_two_big_halves() {
    // I told my ace (120); my partner's second +5 (125) is the strict two halves: their pair
    // values reach 140, so one of them is red or Bells. I hold the red and the Bells king.
    let m = mind(
        &["r-K", "s-K", "g-A", "g-6", "e-6", "e-7", "s-6", "r-6", "r-7"],
        &[(0, Some(120)), (1, None), (2, Some(125))],
    );
    let cover = m.team_pair_cover(145);
    assert!(cover > 0.5, "team_pair_cover(145) = {cover}");
}

#[test]
fn derived_pair_value_counts_my_single_halves_except_green() {
    // Batch 1c: the partner (2) +10; I hold the Red king, the Acorns ober and the Green king.
    // Red and Acorns count with P(the partner holds the other half) × the pair value; Green
    // is left out.
    let m = mind(
        &["r-K", "e-O", "g-K", "g-6", "e-6", "e-7", "s-6", "s-7", "r-6"],
        &[(0, None), (1, None), (2, Some(125))],
    );
    let expected = m.team_pair(Suit::Red) * 100.0 + m.team_pair(Suit::Acorns) * 60.0;
    assert!((m.derived_pair_value() - expected).abs() < 1e-3);
    assert!(m.derived_pair_value() > 60.0, "{}", m.derived_pair_value());
    assert!(m.team_pair(Suit::Green) > 0.0);
}

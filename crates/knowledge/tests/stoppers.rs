//! KB batch 1b: `stopper` and `trick_confidence` on the owner's examples (addendum 4).

use engine::{Card, CardSet, Suit};
use knowledge::Tracker;

fn confidence(hand: &[&str]) -> f32 {
    let set = hand.iter().fold(CardSet::EMPTY, |a, c| {
        a | CardSet::single(c.parse::<Card>().unwrap().index())
    });
    Tracker::new(0, set).knowledge().trick_confidence(0)
}

#[test]
fn stoppers_follow_the_owners_order() {
    let close = |a: f32, b: f32| (a - b).abs() < 1e-6;
    assert!(close(
        confidence(&["s-A", "s-8", "g-6", "g-7", "g-8", "e-7", "e-8", "r-8", "r-9"]),
        1.0
    ));
    assert!(close(
        confidence(&["s-Z", "s-8", "g-6", "g-7", "g-8", "e-7", "e-8", "r-8", "r-9"]),
        0.8
    ));
    assert!(close(
        confidence(&["s-K", "s-8", "s-7", "g-6", "g-7", "e-7", "e-8", "r-8", "r-9"]),
        0.5
    ));
    assert!(close(
        confidence(&["s-O", "s-8", "s-7", "s-6", "g-7", "e-7", "e-8", "r-8", "r-9"]),
        0.2
    ));
    // The unter never stops a suit; a ten alone is no stopper and multiplies the miss by 1.1.
    assert!(close(
        confidence(&["s-U", "s-8", "s-7", "s-6", "s-9", "g-7", "e-7", "e-8", "r-8"]),
        0.0
    ));
    assert!(close(
        confidence(&["s-Z", "g-6", "g-7", "g-8", "e-7", "e-8", "r-8", "r-9", "r-7"]),
        0.0
    ));
    assert!(close(
        confidence(&["s-O", "s-8", "s-7", "s-6", "g-O", "g-9", "g-8", "g-7", "r-Z"]),
        0.296
    ));
    assert!(close(
        confidence(&["s-K", "s-8", "s-7", "r-Z", "g-6", "g-7", "e-7", "e-8", "e-6"]),
        0.45
    ));
    let hand = ["s-K", "s-8", "g-6", "g-7", "g-8", "e-7", "e-8", "r-8", "r-9"]
        .iter()
        .fold(CardSet::EMPTY, |a, c| {
            a | CardSet::single(c.parse::<Card>().unwrap().index())
        });
    // A king with only 1 more does not stop Bells.
    assert_eq!(Tracker::new(0, hand).knowledge().stopper(0, Suit::Bells), None);
}

fn against(hand: &[&str]) -> u32 {
    let set = hand.iter().fold(CardSet::EMPTY, |a, c| {
        a | CardSet::single(c.parse::<Card>().unwrap().index())
    });
    Tracker::new(0, set).knowledge().standing_against_us(0)
}

#[test]
fn standing_against_us_on_the_owners_examples() {
    // Three blank tens and a suit 6 to O: 27.
    assert_eq!(
        against(&["s-Z", "g-Z", "e-Z", "r-6", "r-7", "r-8", "r-9", "r-U", "r-O"]),
        27
    );
    // Only 6, 7, 8 (one suit void): 27.
    assert_eq!(
        against(&["s-6", "s-7", "s-8", "g-6", "g-7", "g-8", "e-6", "e-7", "e-8"]),
        27
    );
    // A king with 2 more and junk: 23 (A and Z stand against the king, then it wins).
    assert_eq!(
        against(&["s-K", "s-8", "s-7", "g-6", "g-7", "g-8", "e-6", "e-7", "e-8"]),
        23
    );
    // One ace and junk: 20.
    assert_eq!(
        against(&["s-A", "s-6", "g-6", "g-7", "g-8", "e-6", "e-7", "e-8", "r-6"]),
        20
    );
}

#[test]
fn an_ace_hand_can_have_21_against_it() {
    // The B-defend test's ace hand: 0 + 3 + 9 + 9.
    assert_eq!(
        against(&["s-A", "s-6", "s-7", "g-6", "g-7", "g-8", "g-9", "g-U", "g-O"]),
        21
    );
}

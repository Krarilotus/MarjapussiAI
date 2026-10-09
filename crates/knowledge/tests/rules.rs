//! One small scenario per deduction rule (IDs from Investigator A's catalogue).
//! Seat 0 is "me"; it holds only low cards, so every K/O/A is hidden.

use engine::CardSet;
use engine::{Card, Suit, Value};
use knowledge::Knowledge;
use observation::SeatEvent;

use Suit::{Acorns as E, Bells as S, Green as G, Red as R};
use Value::*;

fn c(suit: Suit, value: Value) -> u8 {
    Card { suit, value }.index()
}

fn set(cards: &[u8]) -> CardSet {
    cards.iter().fold(CardSet::EMPTY, |s, &i| s | CardSet::single(i))
}

fn me_low() -> Knowledge {
    let hand = set(&[
        c(G, Six),
        c(G, Seven),
        c(E, Six),
        c(E, Seven),
        c(S, Six),
        c(S, Seven),
        c(R, Six),
        c(R, Seven),
        c(R, Eight),
    ]);
    Knowledge::new(0, hand)
}

fn play(k: &mut Knowledge, seat: u8, card: u8) {
    k.apply(&SeatEvent::CardPlayed { seat, card }).unwrap();
}

/// Trick 1 with no deductions beyond the acorns lead: 1 leads E-A, 2 E-10, 3 E-K, 0 E-6.
fn after_first_trick() -> Knowledge {
    let mut k = me_low();
    for (seat, card) in [(1, c(E, Ace)), (2, c(E, Ten)), (3, c(E, King)), (0, c(E, Six))] {
        play(&mut k, seat, card);
    }
    k
}

#[test]
fn s4_trick_one_lead_without_ace_or_green() {
    let mut k = me_low();
    play(&mut k, 1, c(E, Nine));
    assert!((k.may_hold(1) & (CardSet::value(Ace) | CardSet::suit(G))).is_empty());
}

#[test]
fn s5_trick_one_follower_without_the_led_ace() {
    let mut k = me_low();
    play(&mut k, 1, c(E, Nine));
    play(&mut k, 2, c(E, Unter));
    assert!(!k.may_hold(2).contains(c(E, Ace)));
}

#[test]
fn s6_followed_without_overtaking_and_no_trump() {
    let mut k = after_first_trick();
    play(&mut k, 1, c(S, Nine));
    play(&mut k, 2, c(S, Eight));
    let higher_bells = set(&[c(S, Unter), c(S, Ober), c(S, King), c(S, Ten), c(S, Ace)]);
    assert!((k.may_hold(2) & higher_bells).is_empty());
}

#[test]
fn s7_s9_off_suit_without_trump_means_void_in_led_suit_and_trump() {
    let mut k = after_first_trick();
    k.apply(&SeatEvent::AnnounceTrump { seat: 1, suit: R }).unwrap();
    play(&mut k, 1, c(G, Ace));
    play(&mut k, 3, c(E, Eight));
    assert!(k.is_void(3, G) && k.is_void(3, R));
}

#[test]
fn s8_off_suit_trump_that_does_not_win_means_no_higher_trump() {
    let mut k = after_first_trick();
    k.apply(&SeatEvent::AnnounceTrump { seat: 1, suit: R }).unwrap();
    // Acorns led: only 4 acorns are hidden, so two voids stay consistent with the hand sizes.
    play(&mut k, 1, c(E, Unter));
    play(&mut k, 2, c(R, Ten));
    play(&mut k, 3, c(R, Nine));
    assert!(k.is_void(3, E));
    assert!(!k.may_hold(3).contains(c(R, Ace)));
}

#[test]
fn s10_s11_announcing_or_answering_a_pair_shows_both_halves() {
    let mut k = after_first_trick();
    k.apply(&SeatEvent::AnnounceTrump { seat: 1, suit: R }).unwrap();
    assert!(CardSet::halves(R).is_subset(k.certain(1)));
    k.apply(&SeatEvent::YesPair { seat: 3, suit: G }).unwrap();
    assert!(CardSet::halves(G).is_subset(k.certain(3)));
    assert_eq!(k.trump(), Some(G));
}

#[test]
fn s12_no_pair_then_one_half_played_excludes_the_other() {
    let mut k = after_first_trick();
    k.apply(&SeatEvent::NoPair { seat: 2 }).unwrap();
    play(&mut k, 2, c(G, King));
    assert!(!k.may_hold(2).contains(c(G, Ober)));
}

#[test]
fn no_pair_answer_updates_facts_without_spending_any_players_rights() {
    use engine::PlayerTrumpPossibilities::{Own, Yours};
    let mut k = after_first_trick();
    k.apply(&SeatEvent::AskPair { seat: 1 }).unwrap();
    k.apply(&SeatEvent::NoPair { seat: 3 }).unwrap();
    assert_eq!([0, 1, 2, 3].map(|seat| k.rights(seat)), [Own, Yours, Own, Own]);
    assert_eq!(k.pairs(3).possible, 0);
}

#[test]
fn s13_s15_no_half_and_only_half() {
    let mut k = after_first_trick();
    k.apply(&SeatEvent::NoHalf { seat: 3, suit: S }).unwrap();
    assert!((k.may_hold(3) & CardSet::halves(S)).is_empty());
    k.apply(&SeatEvent::YesHalf {
        seat: 2,
        suit: G,
        asker_has_half: false,
    })
    .unwrap();
    assert!((k.may_hold(0) & CardSet::halves(G)).is_empty());
}

#[test]
fn s14_yes_half_then_own_half_played_does_not_certify_the_other() {
    let mut k = after_first_trick();
    k.apply(&SeatEvent::YesHalf {
        seat: 2,
        suit: G,
        asker_has_half: false,
    })
    .unwrap();
    play(&mut k, 2, c(G, King));
    assert!(!k.certain(2).contains(c(G, Ober)), "the old marjapussi-rs bug");
    assert!(k.may_hold(2).contains(c(G, Ober)));
}

#[test]
fn s17_yes_half_then_other_seat_plays_one_half_certifies_the_other() {
    let mut k = after_first_trick();
    k.apply(&SeatEvent::YesHalf {
        seat: 2,
        suit: G,
        asker_has_half: false,
    })
    .unwrap();
    play(&mut k, 1, c(G, King));
    assert!(k.certain(2).contains(c(G, Ober)));
}

#[test]
fn s16_joint_half_gives_exactly_one_each() {
    let hand = set(&[
        c(G, King),
        c(G, Seven),
        c(E, Six),
        c(E, Seven),
        c(S, Six),
        c(S, Seven),
        c(R, Six),
        c(R, Seven),
        c(R, Eight),
    ]);
    let mut k = Knowledge::new(0, hand);
    for (seat, card) in [(1, c(E, Ace)), (2, c(E, Ten)), (3, c(E, King)), (0, c(E, Six))] {
        play(&mut k, seat, card);
    }
    k.apply(&SeatEvent::YesHalf {
        seat: 2,
        suit: G,
        asker_has_half: true,
    })
    .unwrap();
    assert!(k.certain(2).contains(c(G, Ober)));
    assert!(!k.may_hold(1).contains(c(G, Ober)) && !k.may_hold(3).contains(c(G, Ober)));
    assert_eq!(k.trump(), Some(G));
}

#[test]
fn s19_a_card_possible_for_one_seat_only_is_certain_there() {
    let mut k = after_first_trick();
    // Seats 1 and 3 show they are void in bells; the hidden bells must all be with seat 2.
    play(&mut k, 2, c(S, Nine));
    play(&mut k, 3, c(G, Eight));
    play(&mut k, 0, c(S, Six));
    play(&mut k, 1, c(G, Nine));
    let hidden_bells = CardSet::suit(S) - k.played() - k.certain(0);
    assert!(!hidden_bells.is_empty() && hidden_bells.is_subset(k.certain(2)));
}

// S18 (hand-size counting) is guarded in tests/soundness.rs by the information floor; see the
// mutation result there.

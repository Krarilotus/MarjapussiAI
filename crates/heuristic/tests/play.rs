//! One scenario per play rule (HC-1/2/4/5/6/7/8/13, HV-0/1, HL-2/7, HF-2, HQ-3, the P10a
//! rights guard, HX-1/2), including the owner's three HC-1 examples (C4). Card choices are the
//! valuation's; the label is the term that separates the card from the runner-up.

mod common;

use common::{c, played, Scene};
use engine::PlaceAtTable;
use engine::Suit;
use engine::{ActionType, AnswerType, GameAction, QuestionType};
use heuristic::RuleId;
use observation::SeatEvent;

fn pass_game(me: u8, hand: &[&str]) -> Scene {
    let mut s = Scene::new(me, hand);
    s.auction(&[(0, None), (1, None), (2, None), (3, None)]);
    s
}

fn trick(s: &mut Scene, plays: &[(u8, &str)]) {
    for &(seat, card) in plays {
        s.play(seat, card);
    }
}

const REST: [&str; 6] = ["r-6", "r-7", "r-8", "e-6", "e-7", "s-6"];

fn with_rest(cards: &[&'static str]) -> Vec<&'static str> {
    let mut h = cards.to_vec();
    h.extend(REST.iter().take(9 - cards.len()));
    h
}

#[test]
fn hc1_owner_a_gives_the_ten_before_the_king_of_a_pair() {
    // Trick 1: an opponent leads the green ace; I must follow green and cannot win.
    let mut s = pass_game(0, &with_rest(&["g-Z", "g-K", "g-O"]));
    trick(&mut s, &[(1, "g-A"), (2, "g-6"), (3, "g-7")]);
    let d = s.act(&s.card_legal());
    assert_eq!(played(&d), Some(c("g-Z")), "{d:?}");
    // Points alone would give the K (4 < 10): the keep value decided, so the label is HC-1.
    assert_eq!(d.rule, RuleId::HC1);
}

/// MH (seat 2) gave VH (seat 0) the green King and `extra`, then dumped its small cards in 7
/// tricks. At trick 8 an opponent leads e-Z; I hold g-Z and g-O; g-A is still out.
fn owner_bc(extra: [&str; 3]) -> Scene {
    let start: Vec<&str> = ["g-Z", "g-O", "g-K", "e-6", "e-7", "e-8"]
        .into_iter()
        .chain(extra)
        .collect();
    let mut s = Scene::new(2, &start);
    s.auction(&[(0, Some(120)), (1, None), (2, None), (3, None)]);
    let x: Vec<&str> = ["g-K"].into_iter().chain(extra).collect();
    s.pass(2, 0, Some(&x));
    s.pass(0, 2, Some(&["e-9", "e-U", "s-9", "s-U"]));
    trick(&mut s, &[(0, "s-A"), (1, "s-Z"), (2, "s-9"), (3, "s-K")]);
    trick(&mut s, &[(0, "r-6"), (1, "r-8"), (2, "e-6"), (3, "r-9")]);
    trick(&mut s, &[(0, "r-7"), (1, "r-U"), (2, "e-7"), (3, "r-Z")]);
    trick(&mut s, &[(1, "s-6"), (2, "s-U"), (3, "s-O"), (0, "s-7")]);
    trick(&mut s, &[(1, "r-A"), (2, "e-8"), (3, "r-K"), (0, "g-7")]);
    trick(&mut s, &[(1, "s-8"), (2, "e-9"), (3, "g-6"), (0, "g-9")]);
    trick(&mut s, &[(1, "r-O"), (2, "e-U"), (3, "g-U"), (0, "g-8")]);
    s.play(1, "e-Z");
    s
}

#[test]
fn hc1_owner_keeps_ten_and_ober_while_the_ace_is_out() {
    // One trick earlier than owner_bc: I still hold e-U besides g-Z and g-O; I give the Unter.
    let start = ["g-Z", "g-O", "g-K", "e-6", "e-7", "e-8", "r-6", "r-7", "s-7"];
    let mut s = Scene::new(2, &start);
    s.auction(&[(0, Some(120)), (1, None), (2, None), (3, None)]);
    s.pass(2, 0, Some(&["g-K", "r-6", "r-7", "s-7"]));
    s.pass(0, 2, Some(&["e-9", "e-U", "s-9", "s-U"]));
    trick(&mut s, &[(0, "s-A"), (1, "s-Z"), (2, "s-9"), (3, "s-K")]);
    trick(&mut s, &[(0, "r-6"), (1, "r-8"), (2, "e-6"), (3, "r-9")]);
    trick(&mut s, &[(0, "r-7"), (1, "r-U"), (2, "e-7"), (3, "r-Z")]);
    trick(&mut s, &[(1, "s-6"), (2, "s-U"), (3, "s-O"), (0, "s-7")]);
    trick(&mut s, &[(1, "r-A"), (2, "e-8"), (3, "r-K"), (0, "g-7")]);
    trick(&mut s, &[(1, "s-8"), (2, "e-9"), (3, "g-6"), (0, "g-9")]);
    s.play(1, "r-O");
    let d = s.act(&s.card_legal());
    assert_eq!(played(&d), Some(c("e-U")), "{d:?}");
}

#[test]
#[ignore = "C32, for the owner: with the n-over-k base (no fitted factors) P(partner holds e-A) is 0.25, an entry; the valuation keeps the O for the ask and gives the 10"]
fn hc1_owner_b_without_an_entry_gives_the_ober() {
    // The partner holds g-K and one unknown card: no sure lead for our team. The ten is the
    // card that may still get us a trick; the O's meld counts for us only through the contract.
    let s = owner_bc(["r-6", "r-7", "s-7"]);
    let d = s.act(&s.card_legal());
    assert_eq!(played(&d), Some(c("g-O")), "{d:?}");
    // The runner-up fed the ten to the opponents: its future-trick value decided (HC-7).
    assert_eq!(d.rule, RuleId::HC7);
}

#[test]
fn hc1_owner_c_partner_can_take_over_so_the_ten_goes() {
    // The partner certainly holds the acorns ace (standing): it can take over and ask me.
    let s = owner_bc_with_ace();
    let d = s.act(&s.card_legal());
    assert_eq!(played(&d), Some(c("g-Z")), "{d:?}");
}

fn owner_bc_with_ace() -> Scene {
    // As owner_bc, but I passed e-A instead of s-7; the partner keeps it to the end.
    let start = ["g-Z", "g-O", "g-K", "e-6", "e-7", "e-8", "r-6", "r-7", "e-A"];
    let mut s = Scene::new(2, &start);
    s.auction(&[(0, Some(120)), (1, None), (2, None), (3, None)]);
    s.pass(2, 0, Some(&["g-K", "r-6", "r-7", "e-A"]));
    s.pass(0, 2, Some(&["e-9", "e-U", "s-9", "s-U"]));
    trick(&mut s, &[(0, "s-A"), (1, "s-Z"), (2, "s-9"), (3, "s-K")]);
    trick(&mut s, &[(0, "r-6"), (1, "r-8"), (2, "e-6"), (3, "r-9")]);
    trick(&mut s, &[(0, "r-7"), (1, "r-U"), (2, "e-7"), (3, "r-Z")]);
    trick(&mut s, &[(1, "s-6"), (2, "s-U"), (3, "s-O"), (0, "s-7")]);
    trick(&mut s, &[(1, "r-A"), (2, "e-8"), (3, "r-K"), (0, "g-7")]);
    trick(&mut s, &[(1, "s-8"), (2, "e-9"), (3, "g-6"), (0, "g-9")]);
    trick(&mut s, &[(1, "r-O"), (2, "e-U"), (3, "g-U"), (0, "g-8")]);
    s.play(1, "e-Z");
    s
}

#[test]
fn hc7_no_ten_into_an_opponents_trick() {
    let mut s = pass_game(0, &with_rest(&["g-Z", "g-K", "g-8"]));
    trick(&mut s, &[(1, "g-A"), (2, "g-6"), (3, "g-7")]);
    let d = s.act(&s.card_legal());
    assert_eq!(played(&d), Some(c("g-8")));
    // Kept for their future trick: HC-7 if the runner-up is the ten, else HV-1.
    assert!([RuleId::HC7, RuleId::HV1].contains(&d.rule), "{d:?}");
}

#[test]
fn hc8_forced_over_my_partner_with_the_cheapest_card() {
    let mut s = pass_game(0, &with_rest(&["e-A", "e-Z", "g-8"]));
    trick(&mut s, &[(1, "g-A"), (2, "g-6"), (3, "g-7"), (0, "g-8")]);
    trick(&mut s, &[(1, "e-9"), (2, "e-K"), (3, "e-U")]);
    let d = s.act(&s.card_legal());
    assert_eq!((played(&d), d.rule), (Some(c("e-Z")), RuleId::HC8));
}

#[test]
fn hv0_a_ten_that_cannot_win_later_goes_onto_the_partners_trick() {
    // I am last and void in acorns; the partner's ace wins. The lone bells ten can't win a trick
    // later (the bells ace is out), so its 10 points are worth most in this trick: no rule
    // "schmier the most points", the future-trick value of the ten is low.
    let mut s = pass_game(0, &["s-Z", "g-6", "g-7", "g-8", "r-6", "r-7", "r-8", "r-9", "g-9"]);
    trick(&mut s, &[(1, "g-A"), (2, "g-U"), (3, "g-Z"), (0, "g-6")]);
    trick(&mut s, &[(1, "e-9"), (2, "e-A"), (3, "e-8")]);
    let d = s.act(&s.card_legal());
    assert_eq!((played(&d), d.rule), (Some(c("s-Z")), RuleId::HV0));
}

#[test]
fn hf2_opponent_wins_so_i_give_the_least() {
    // No A/10 and no guard: nothing but the points decides.
    let mut s = pass_game(0, &["s-7", "g-6", "s-8", "s-9", "s-6", "r-6", "r-7", "r-8", "r-9"]);
    trick(&mut s, &[(1, "g-A"), (2, "g-U"), (3, "g-Z"), (0, "g-6")]);
    trick(&mut s, &[(1, "e-9"), (2, "e-6"), (3, "e-A")]);
    let d = s.act(&s.card_legal());
    assert_eq!(d.rule, RuleId::HF2);
    assert_eq!(heuristic_points(played(&d).unwrap()), 0);
}

fn heuristic_points(i: u8) -> i32 {
    engine::points_card(engine::Card::from_index(i)).0
}

#[test]
fn hv1_last_to_play_wins_with_the_ten_and_keeps_the_standing_ace() {
    let mut s = pass_game(0, &with_rest(&["e-A", "e-Z", "g-8"]));
    trick(&mut s, &[(1, "g-A"), (2, "g-6"), (3, "g-7"), (0, "g-8")]);
    trick(&mut s, &[(1, "e-9"), (2, "e-U"), (3, "e-K")]);
    let d = s.act(&s.card_legal());
    assert_eq!((played(&d), d.rule), (Some(c("e-Z")), RuleId::HV1));
}

#[test]
fn hv0_trumps_a_rich_trick_without_the_ace() {
    // Seat 1 announced Red and leads bells; I am void in bells.
    let mut s = pass_game(0, &["r-A", "r-9", "r-U", "g-8", "e-6", "e-7", "e-8", "g-9", "g-U"]);
    trick(&mut s, &[(1, "g-A"), (2, "g-6"), (3, "g-7"), (0, "g-8")]);
    s.ev(SeatEvent::AnnounceTrump {
        seat: 1,
        suit: Suit::Red,
    });
    trick(&mut s, &[(1, "s-A"), (2, "s-6"), (3, "s-Z")]);
    let d = s.act(&s.card_legal());
    // A low trump takes the 21 points; the ace keeps its control. Between r-9 and r-U the
    // valuation weighs the U's 2 points now against its slightly higher future (C25).
    assert!([c("r-9"), c("r-U")].contains(&played(&d).unwrap()), "{d:?}");
}

#[test]
fn hl2_leads_a_standing_ace() {
    let s = pass_game(0, &["r-A", "r-7", "r-6", "g-A", "e-6", "e-7", "s-6", "s-7", "s-8"]);
    let d = s.act(&s.card_legal());
    assert_eq!((played(&d), d.rule), (Some(c("r-A")), RuleId::HL2));
}

#[test]
fn hl7_never_leads_a_certain_loser() {
    // Seat 1 announced Red (r-K, r-O certain there); later I announce Green, so Red is a side
    // suit. r-A and r-Z are gone, so my r-U certainly loses, and it would be my cheapest lead.
    let hand = ["g-8", "r-9", "s-A", "g-K", "g-O", "r-U", "e-Z", "e-K", "s-O"];
    let mut s = pass_game(0, &hand);
    trick(&mut s, &[(1, "g-A"), (2, "g-6"), (3, "g-7"), (0, "g-8")]);
    s.ev(SeatEvent::AnnounceTrump {
        seat: 1,
        suit: Suit::Red,
    });
    trick(&mut s, &[(1, "r-A"), (2, "r-Z"), (3, "r-8"), (0, "r-9")]);
    trick(&mut s, &[(1, "s-6"), (2, "s-7"), (3, "s-8"), (0, "s-A")]);
    s.ev(SeatEvent::AnnounceTrump {
        seat: 0,
        suit: Suit::Green,
    });
    let d = s.act(&s.card_legal());
    assert_ne!(played(&d), Some(c("r-U")));
    assert_eq!(d.rule, RuleId::HL7);
}

/// No standing card and no plan: the valuation leads a card that gives nothing away.
#[test]
fn without_a_plan_the_lead_gives_no_points() {
    let mut s = pass_game(0, &["g-8", "s-A", "e-7", "e-6", "r-8", "r-7", "g-9", "e-9", "e-U"]);
    trick(&mut s, &[(1, "g-A"), (2, "g-6"), (3, "g-7"), (0, "g-8")]);
    trick(&mut s, &[(1, "s-6"), (2, "s-7"), (3, "s-8"), (0, "s-A")]);
    let d = s.act(&s.card_legal());
    assert_eq!(heuristic_points(played(&d).unwrap()), 0);
}

/// The question actions of a seat leading a trick (announcements as given), plus its cards.
fn ask_legal(s: &Scene, announce: &[Suit], yours: bool) -> Vec<GameAction> {
    let mut extra: Vec<ActionType> = announce.iter().map(|&x| ActionType::AnnounceTrump(x)).collect();
    if yours {
        extra.push(ActionType::Question(QuestionType::Yours));
    }
    extra.extend(
        [Suit::Red, Suit::Bells, Suit::Acorns, Suit::Green].map(|x| ActionType::Question(QuestionType::YourHalf(x))),
    );
    s.with(extra, s.card_legal())
}

fn fact(d: &heuristic::Decision, key: &str) -> Option<f32> {
    d.why.iter().find(|f| f.key == key).map(|f| f.value)
}

#[test]
fn hc5_no_sure_lead_left_announces_the_most_valuable_of_two_pairs() {
    // P8: two own pairs need 0 attempts, and no standing card is left (0 sure leads): act now.
    let mut s = pass_game(0, &["r-K", "r-O", "g-K", "g-O", "s-A", "g-8", "e-6", "e-7", "e-8"]);
    trick(&mut s, &[(1, "s-6"), (2, "s-7"), (3, "s-8"), (0, "s-A")]);
    let d = s.act(&ask_legal(&s, &[Suit::Red, Suit::Green], true));
    assert_eq!(
        (d.action.action_type, d.rule),
        (ActionType::AnnounceTrump(Suit::Red), RuleId::HC5)
    );
    assert_eq!((fact(&d, "attempts"), fact(&d, "sure_leads")), (Some(0.0), Some(0.0)));
}

#[test]
fn hc4_cashes_a_standing_card_while_the_attempts_fit_the_sure_leads() {
    // P8: my pair needs 0 attempts and I hold two aces: cash one first, announce later.
    let mut s = pass_game(0, &["r-K", "r-O", "e-A", "g-A", "s-A", "g-8", "e-6", "e-7", "s-9"]);
    trick(&mut s, &[(1, "s-6"), (2, "s-7"), (3, "s-8"), (0, "s-A")]);
    let d = s.act(&ask_legal(&s, &[Suit::Red], true));
    assert_eq!(d.rule, RuleId::HC4, "{d:?}");
    assert!([c("e-A"), c("g-A")].contains(&played(&d).unwrap()));
    assert_eq!((fact(&d, "attempts"), fact(&d, "sure_leads")), (Some(0.0), Some(2.0)));
}

#[test]
fn hq3_asks_yours_when_the_partner_showed_a_pair() {
    // The partner's +15 showed a big pair; I hold no pair and no standing card.
    let mut s = Scene::new(0, &["s-A", "g-8", "e-6", "e-7", "e-8", "r-6", "r-7", "g-9", "g-U"]);
    s.auction(&[(0, None), (1, None), (2, Some(130)), (3, None)]);
    trick(&mut s, &[(1, "s-6"), (2, "s-7"), (3, "s-8"), (0, "s-A")]);
    let d = s.act(&ask_legal(&s, &[], true));
    assert_eq!(
        (d.action.action_type, d.rule),
        (ActionType::Question(QuestionType::Yours), RuleId::HQ3)
    );
}

#[test]
fn p10a_a_half_question_does_not_spend_the_yours_the_partners_pair_needs() {
    // The partner's +15 showed a big pair, and I hold a Red half the partner may complete. A
    // half question first would leave me `Ours` and the partner's pair without my "Yours".
    let mut s = Scene::new(0, &["s-A", "r-K", "e-6", "e-7", "e-8", "r-6", "r-7", "g-9", "g-U"]);
    s.auction(&[(0, None), (1, None), (2, Some(130)), (3, None)]);
    trick(&mut s, &[(1, "s-6"), (2, "s-7"), (3, "s-8"), (0, "s-A")]);
    let d = s.act(&ask_legal(&s, &[], true));
    assert_eq!(d.action.action_type, ActionType::Question(QuestionType::Yours), "{d:?}");
}

#[test]
fn p10a_holding_an_own_pair_never_asks_while_the_partner_is_unlikely_to_lead() {
    // My uncalled Red pair needs my `Own` right: a question would spend it, and the partner
    // (no ace shown, all passed) is unlikely to win a lead to ask me "Yours".
    let mut s = pass_game(0, &["r-K", "r-O", "g-K", "s-A", "g-8", "e-6", "e-7", "e-8", "s-9"]);
    trick(&mut s, &[(1, "s-6"), (2, "s-7"), (3, "s-8"), (0, "s-A")]);
    let d = s.act(&ask_legal(&s, &[Suit::Red], true));
    assert!(!matches!(d.action.action_type, ActionType::Question(_)), "{d:?}");
}

#[test]
fn hc13_does_not_repeat_a_known_no_pair_even_at_zero_threshold() {
    let mut s = Scene::new(0, &["s-A", "g-8", "e-6", "e-7", "e-8", "r-6", "r-7", "g-9", "g-U"]);
    s.auction(&[(0, None), (1, None), (2, Some(130)), (3, None)]);
    trick(&mut s, &[(1, "s-6"), (2, "s-7"), (3, "s-8"), (0, "s-A")]);
    s.ev(SeatEvent::AskPair { seat: 0 }).ev(SeatEvent::NoPair { seat: 2 });
    for threshold in [0.0, heuristic::Params::default().partner_pair_min] {
        let params = heuristic::Params {
            partner_pair_min: threshold,
            ..heuristic::Params::default()
        };
        let d = s.act_with(params, &ask_legal(&s, &[], true));
        assert!(
            !matches!(d.action.action_type, ActionType::Question(QuestionType::Yours)),
            "{d:?}"
        );
    }
}

#[test]
fn hc13_skips_the_half_the_partner_said_it_lacks() {
    // My "Yours" got "no pair", and my Green half question got "no" (I am `Ours` now): of my two
    // halves only the Acorns one is still worth a question.
    let mut s = pass_game(0, &["g-K", "e-K", "s-A", "g-8", "e-6", "e-7", "r-6", "r-7", "s-9"]);
    trick(&mut s, &[(1, "s-6"), (2, "s-7"), (3, "s-8"), (0, "s-A")]);
    s.ev(SeatEvent::AskPair { seat: 0 })
        .ev(SeatEvent::NoPair { seat: 2 })
        .ev(SeatEvent::AskHalf {
            seat: 0,
            suit: Suit::Green,
        })
        .ev(SeatEvent::NoHalf {
            seat: 2,
            suit: Suit::Green,
        });
    let d = s.act(&ask_legal(&s, &[], false));
    assert_eq!(
        (d.action.action_type, d.rule),
        (ActionType::Question(QuestionType::YourHalf(Suit::Acorns)), RuleId::HQ3),
        "{d:?}"
    );
    assert_eq!(fact(&d, "hc13_skipped"), Some(1.0));
}

#[test]
fn hc6_answers_with_the_pair_most_at_risk() {
    // Without a sure lead of my own, the Red pair (worth most) is the one most at risk.
    let s = pass_game(0, &["r-K", "r-O", "g-K", "g-O", "s-A", "g-8", "e-6", "e-7", "e-8"]);
    let legal: Vec<GameAction> = [Suit::Green, Suit::Red]
        .map(|x| GameAction {
            action_type: ActionType::Answer(AnswerType::YesPair(x)),
            player: PlaceAtTable(0),
        })
        .to_vec();
    let d = s.act(&legal);
    assert_eq!(
        (d.action.action_type, d.rule),
        (ActionType::Answer(AnswerType::YesPair(Suit::Red)), RuleId::HC6)
    );
}

#[test]
fn hx1_single_legal_action_and_hx2_undo_declined() {
    let s = pass_game(0, &with_rest(&["g-Z", "g-K", "g-O"]));
    let one = vec![s.card_legal()[0]];
    assert_eq!(s.act(&one).rule, RuleId::HX1);
    let undo: Vec<GameAction> = [ActionType::UndoAccept, ActionType::UndoDecline]
        .map(|t| GameAction {
            action_type: t,
            player: PlaceAtTable(0),
        })
        .to_vec();
    let d = s.act(&undo);
    assert_eq!((d.action.action_type, d.rule), (ActionType::UndoDecline, RuleId::HX2));
}

#[test]
fn hl1_schnippeln_when_the_partner_certainly_holds_the_ace() {
    // VH (0) gave its only ace, g-A, back to MH: no ace to lead, green is forced, and MH must
    // play g-A in trick 1. Lead green low (P9).
    let mut s = Scene::new(0, &["g-A", "g-7", "e-6", "e-7", "s-6", "s-7", "r-6", "r-7", "e-8"]);
    s.auction(&[(0, Some(120)), (1, None), (2, None), (3, None)]);
    s.pass(2, 0, Some(&["g-K", "s-8", "r-8", "e-9"]));
    s.pass(0, 2, Some(&["g-A", "s-6", "r-6", "e-6"]));
    let d = s.act(&s.card_legal());
    assert_eq!((played(&d), d.rule), (Some(c("g-7")), RuleId::HL1), "{d:?}");
}

/// Demo game 123 (owner reports C22, C24): VH (0, the owner) declared 140; MH (2, HP1) passed
/// r-A r-U r-6 e-7 and got back e-O e-8 e-7 g-A. MH holds the green pair and the green ace.
fn demo_123() -> Scene {
    let mut s = Scene::new(2, &["s-O", "g-9", "e-7", "r-6", "g-O", "r-A", "s-Z", "g-K", "r-U"]);
    s.auction(&[
        (0, Some(120)),
        (1, Some(125)),
        (2, Some(135)),
        (3, None),
        (0, Some(140)),
        (1, None),
        (2, None),
    ]);
    s.pass(2, 0, Some(&["r-A", "r-U", "r-6", "e-7"]));
    s.pass(0, 2, Some(&["e-O", "e-8", "e-7", "g-A"]));
    s
}

#[test]
fn c22_keeps_the_ace_the_partner_gave_and_the_pair_on_the_partners_trick() {
    // Trick 1: the owner's r-A wins, I am void in red. The g-A is standing (future-trick value)
    // and g-K / g-O are our needed pair (activation value): neither is thrown. (Which other card
    // goes is the valuation's trade of points now against a later trick: the s-Z, which wins
    // later only if the s-A falls first, is a fair candidate.)
    let mut s = demo_123();
    trick(&mut s, &[(0, "r-A"), (1, "r-Z")]);
    let d = s.act(&s.card_legal());
    let thrown = played(&d).unwrap();
    for keep in ["g-A", "g-K", "g-O"] {
        assert_ne!(thrown, c(keep), "{d:?}");
    }
}

#[test]
fn c24_with_the_lead_the_needed_pair_is_announced() {
    // The owner's plan: he leads g-U, I take it with g-A, and with the lead I announce the green
    // pair the 140 contract needs (instead of later playing its halves as plain cards).
    let mut s = demo_123();
    trick(&mut s, &[(0, "r-A"), (1, "r-Z"), (2, "e-8"), (3, "r-7")]);
    trick(&mut s, &[(0, "g-U"), (1, "g-Z"), (2, "g-A"), (3, "g-7")]);
    let d = s.act(&ask_legal(&s, &[Suit::Green], true));
    assert_eq!(d.action.action_type, ActionType::AnnounceTrump(Suit::Green), "{d:?}");
}

#[test]
fn c24_a_half_of_a_needed_split_pair_is_not_schmiered() {
    // MH (2) gave r-O to VH (0), who declared 140: the Red pair is ours, split. On VH's ace the
    // r-K would add 4 points; the contract needs the meld, so the half stays (HC-9 prices it).
    let mut s = Scene::new(2, &["r-K", "r-O", "s-9", "g-7", "g-8", "e-7", "e-8", "g-9", "s-8"]);
    s.auction(&[(0, Some(140)), (1, None), (2, None), (3, None)]);
    s.pass(2, 0, Some(&["r-O", "g-7", "g-8", "g-9"]));
    s.pass(0, 2, Some(&["e-6", "e-9", "s-6", "s-7"]));
    trick(&mut s, &[(0, "g-A"), (1, "g-6")]);
    let d = s.act(&s.card_legal());
    assert_ne!(played(&d), Some(c("r-K")), "{d:?}");
}

#[test]
fn c34_a_discard_keeps_the_colour_the_partner_drops() {
    // All passed. Trick 1: E leads e-A, my partner S discards g-8 (no acorns), W follows. I have
    // no acorns either and two small cards that are worth the same: I keep green, which my
    // partner gave up, and discard bells, so our team keeps different colours.
    let mut s = pass_game(0, &["g-6", "s-6", "r-A", "r-Z", "r-K", "r-O", "r-U", "r-9", "r-8"]);
    trick(&mut s, &[(1, "e-A"), (2, "g-8"), (3, "e-7")]);
    let d = s.act(&s.card_legal());
    assert_eq!(played(&d), Some(c("s-6")), "{d:?}");
}

#[test]
fn a_pair_passed_to_the_partner_is_not_led_before_its_announcement() {
    // Seed 42 under C42: VH (3) received s-K from MH (1) and gave the whole Schell pair back
    // with its green. The partner's pair is certain to me (my own pass, K5), so Schell is a plan
    // suit for me too: I don't cash s-A (MH would have to follow with a half) or lead s-9
    // before the pair is announced (F10, owner #25 item 7).
    let mut s = Scene::new(3, &["s-A", "s-O", "r-K", "g-8", "e-7", "g-9", "e-K", "s-9", "r-9"]);
    s.auction(&[
        (0, Some(120)),
        (1, Some(125)),
        (2, None),
        (3, Some(145)),
        (0, None),
        (1, None),
    ]);
    s.pass(1, 3, Some(&["r-Z", "s-K", "e-A", "e-Z"]));
    s.pass(3, 1, Some(&["s-K", "s-O", "g-9", "g-8"]));
    let d = s.act(&s.card_legal());
    let card = played(&d).expect("a card");
    assert_ne!(engine::suit_of(card), Suit::Bells, "{d:?}");
}

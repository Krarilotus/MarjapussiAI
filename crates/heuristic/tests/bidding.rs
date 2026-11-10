//! KB batch 1 (`findings/kb/concepts.md`): one position per bidding rule (B-tell, B-over140,
//! B-block, B-quiet) and the `bluffing(y)` reading. Seat 0 opens.

mod common;

use common::{bid_of, Scene, RULE_FIXTURE_ENDGAME_TRICKS};
use heuristic::{HeuristicAgent, Params, RuleId};

const LOW: [&str; 9] = ["g-6", "e-7", "s-8", "r-9", "s-6", "g-7", "e-8", "r-8", "g-8"];

fn hand(extra: &[&'static str]) -> Vec<&'static str> {
    let mut h: Vec<&str> = extra.to_vec();
    for l in ["g-6", "g-7", "e-6", "e-7", "s-6", "s-7", "r-6", "r-7", "e-8"] {
        if h.len() < 9 && !h.contains(&l) {
            h.push(l);
        }
    }
    h
}

fn tell(s: &Scene) -> (Option<i32>, RuleId) {
    let d = s.act(&s.bid_legal());
    (bid_of(&d), d.rule)
}

/// `Scene::act`'s parameters with B-take on (off by default, the owner 2026-11-10; on in the
/// hill climb's checkpoints 4 and 5).
fn b_take_on() -> Params {
    Params {
        b_take: true,
        endgame_tricks: RULE_FIXTURE_ENDGAME_TRICKS,
        ..Params::default()
    }
}

/// B-take's bid, recorded where it deviates from the owner's examples, which predate it.
fn b_take_tell(s: &Scene) -> (Option<i32>, RuleId) {
    let d = s.act_with(b_take_on(), &s.bid_legal());
    (bid_of(&d), d.rule)
}

#[test]
fn b_tell_red_pair_on_the_first_step_is_plus_20() {
    assert_eq!(tell(&Scene::new(0, &hand(&["r-K", "r-O"]))), (Some(135), RuleId::BTell));
}

#[test]
fn b_tell_big_pair_is_plus_15() {
    assert_eq!(tell(&Scene::new(0, &hand(&["s-K", "s-O"]))), (Some(130), RuleId::BTell));
}

#[test]
fn b_tell_a_green_pair_waits_for_the_ace() {
    // C43: with only a Grün pair, the ace (+5) first ([O] "not valuable enough to forgo the
    // ace"), then the pair (+10).
    let pair_ace = hand(&["g-K", "g-O", "r-A"]);
    assert_eq!(tell(&Scene::new(0, &pair_ace)), (Some(120), RuleId::BTell));
    let mut s = Scene::new(0, &pair_ace);
    s.auction(&[(0, Some(120)), (1, Some(125)), (2, None), (3, None)]);
    assert_eq!(tell(&s), (Some(135), RuleId::BTell));
}

#[test]
fn b_tell_a_red_pair_from_120_goes_to_145() {
    // C43c ([O] "red pair also applies so goes to 145"): pair first, +20 from 120 lands on 140
    // and shifts to 145.
    let mut s = Scene::new(1, &hand(&["r-K", "r-O", "s-A"]));
    s.auction(&[(0, Some(120))]);
    assert_eq!(tell(&s), (Some(145), RuleId::BTell));
}

#[test]
fn b_tell_a_schell_pair_goes_before_the_ace() {
    // C43: [O] "announce the pair first, your partner then has the option to announce his ace".
    assert_eq!(
        tell(&Scene::new(0, &hand(&["s-K", "s-O", "r-A"]))),
        (Some(130), RuleId::BTell)
    );
}

#[test]
fn seed_42_s3_tells_its_halves_without_a_jump() {
    // C43, the owner's seed-42 game: S3 at 125 holds three halves (no pair) and an ace: +10,
    // no over-140 on base rates (its halves message is still to tell).
    let mut s = Scene::new(3, &["s-A", "s-O", "r-K", "g-8", "e-7", "g-9", "e-K", "s-9", "r-9"]);
    s.auction(&[(0, Some(120)), (1, Some(125)), (2, None)]);
    assert_eq!(tell(&s), (Some(135), RuleId::BTell));
}

#[test]
fn b_quiet_runs_the_tie_break_with_strong_cards() {
    // C43: nothing left to tell (the partner told the team's ace), seven cards standing: 140
    // by p_take_140 (C43b) instead of a pass.
    let strong = ["g-A", "g-Z", "g-K", "e-A", "e-Z", "s-A", "s-Z", "r-6", "r-7"];
    let mut s = Scene::new(2, &strong);
    s.auction(&[(0, Some(120)), (1, Some(130))]);
    let d = s.act(&s.bid_legal());
    assert_eq!((bid_of(&d), d.rule), (Some(140), RuleId::BGate), "{:?}", d.why);
    assert!(d.why.iter().any(|f| f.key == "p_take_140" && f.value >= 0.5));
}

#[test]
fn b_tell_three_halves_go_before_the_ace() {
    assert_eq!(
        tell(&Scene::new(0, &hand(&["g-K", "e-O", "s-K", "r-A"]))),
        (Some(125), RuleId::BTell)
    );
}

#[test]
fn b_tell_an_ace_is_plus_5() {
    assert_eq!(tell(&Scene::new(0, &hand(&["r-A"]))), (Some(120), RuleId::BTell));
}

#[test]
fn b_tell_two_halves_after_the_partners_ace_only_if_their_pairs_reach_140() {
    // Strict (batch 1b, [O] "Strict 140+ variant is better"): red + acorns = 160 is told,
    // green + acorns = 100 is not.
    let mut s = Scene::new(2, &hand(&["r-K", "e-O"]));
    s.auction(&[(0, Some(120)), (1, None)]);
    assert_eq!(tell(&s), (Some(125), RuleId::BTell));
    let mut t = Scene::new(2, &hand(&["g-K", "e-O"]));
    t.auction(&[(0, Some(120)), (1, None)]);
    assert_eq!(tell(&t), (None, RuleId::BQuiet));
}

#[test]
fn b_tell_skips_140_by_5() {
    // My partner (0) passed, an opponent bid 125. My big pair: +15 lands on 140, so it goes
    // to 145; my own pair covers the excess.
    let mut s = Scene::new(2, &hand(&["s-K", "s-O"]));
    s.auction(&[(0, None), (1, Some(125))]);
    assert_eq!(tell(&s), (Some(145), RuleId::BTell));
}

#[test]
fn b_tell_a_second_ace_is_not_told() {
    let mut s = Scene::new(2, &hand(&["r-A"]));
    s.auction(&[(0, Some(120)), (1, None)]);
    assert_eq!(tell(&s), (None, RuleId::BQuiet));
}

#[test]
fn b_over140_holds_back_a_message_the_team_pair_cannot_cover() {
    // Three halves at 130 after the partner's pass: +10 lands on 140 and goes to 145, and
    // P(our team holds a pair) is below 0.75, so nothing is told.
    let mut s = Scene::new(2, &hand(&["g-K", "e-O", "s-K"]));
    s.auction(&[(0, None), (1, Some(130))]);
    let d = s.act(&s.bid_legal());
    let cover = d.why.iter().find(|f| f.key == "team_pair_cover").map(|f| f.value);
    assert_eq!((bid_of(&d), d.rule), (None, RuleId::BOver140), "cover {cover:?}");
    assert!(cover.unwrap() < Params::default().over_140_cover);
}

#[test]
fn b_tell_a_plus_5_landing_on_140_stays_140() {
    // Three halves at 135 after the partner's ace: the +10 would cross 140 without cover, the
    // two halves' +5 lands on 140 and stays there (owner, C39 addendum).
    let mut s = Scene::new(2, &hand(&["g-K", "e-O", "s-K"]));
    s.auction(&[(0, Some(120)), (1, Some(135))]);
    let d = s.act(&s.bid_legal());
    assert_eq!((bid_of(&d), d.rule), (Some(140), RuleId::BTell));
    assert!(d.why.iter().any(|f| f.key == "step" && f.value == 5.0));
}

#[test]
fn b_defend_overbids_against_schwarz_after_the_partners_pass() {
    // 27 against us: first 140, then +5 steps up to 240.
    let hopeless = ["s-Z", "g-Z", "e-Z", "r-6", "r-7", "r-8", "r-9", "r-U", "r-O"];
    let at = |hand: [&'static str; 9], partner: Option<i32>, value: i32| {
        let mut s = Scene::new(2, &hand);
        s.auction(&[(0, partner), (1, Some(value))]);
        tell(&s)
    };
    assert_eq!(at(hopeless, None, 120), (Some(140), RuleId::BDefend));
    // No pair signal from them: they can't reach above 140, so no defence above it.
    assert_eq!(at(hopeless, None, 140), (None, RuleId::BQuiet));
    // After their +15 (a big pair; my red ober makes it Bells, 80): up to min(240, 220). (Under
    // C39 a +15 also said "no ace" and the reach was 200; with the pair first again, C43, the
    // Bells pair reads at ≥ 0.75.)
    let after_big = |value: i32| {
        let mut s = Scene::new(2, &hopeless);
        s.auction(&[(0, None), (1, Some(130)), (2, Some(140)), (3, None), (1, Some(value))]);
        tell(&s)
    };
    assert_eq!(after_big(145), (Some(150), RuleId::BDefend));
    assert_eq!(after_big(215), (Some(220), RuleId::BDefend));
    assert_eq!(after_big(220), (None, RuleId::BQuiet));
    // A king with 2 more and junk, 23: the limit 180 caps it below their reach.
    let king = ["s-K", "s-8", "s-7", "g-6", "g-7", "g-8", "e-6", "e-7", "e-8"];
    let king_after_big = |value: i32| {
        let mut s = Scene::new(2, &king);
        s.auction(&[(0, None), (1, Some(130)), (2, Some(140)), (3, None), (1, Some(value))]);
        tell(&s)
    };
    assert_eq!(king_after_big(175), (Some(180), RuleId::BDefend));
    assert_eq!(king_after_big(180), (None, RuleId::BQuiet));
    // A hand with an ace never defends, even on a later call after its ace was told and with
    // 21 against it (a blank-ish ace suit, six low greens, two void suits).
    let ace = ["s-A", "s-6", "s-7", "g-6", "g-7", "g-8", "g-9", "g-U", "g-O"];
    let mut later = Scene::new(2, &ace);
    later.auction(&[
        (0, None),
        (1, Some(120)),
        (2, Some(125)),
        (3, Some(130)),
        (0, None),
        (1, Some(135)),
    ]);
    assert_eq!(tell(&later), (None, RuleId::BQuiet));
    // Without the partner's first pass: none either.
    assert_eq!(at(hopeless, Some(120), 125), (None, RuleId::BQuiet));
}

#[test]
fn b_gate_part_1_strong_cards_with_nothing_left_to_tell() {
    // My second call at 125: my ace is told, no pair, two aces with A+10: 140.
    let strong = ["g-A", "g-Z", "e-A", "e-6", "s-6", "s-7", "r-6", "r-7", "e-7"];
    let mut s = Scene::new(0, &strong);
    s.auction(&[(0, Some(120)), (1, None), (2, None), (3, Some(125))]);
    let d = s.act(&s.bid_legal());
    assert_eq!((bid_of(&d), d.rule), (Some(140), RuleId::BGate));
    assert!(d.why.iter().any(|f| f.key == "gate_part" && f.value == 1.0));
}

#[test]
fn b_gate_part_2_hides_the_pair_once_both_teams_told() {
    // The partner (0) told its ace, an opponent bid 125: my big pair would go to 145 and show
    // it to the opponents; I play it anyway, so 140.
    let mut s = Scene::new(2, &hand(&["s-K", "s-O"]));
    s.auction(&[(0, Some(120)), (1, Some(125))]);
    let d = s.act(&s.bid_legal());
    assert_eq!((bid_of(&d), d.rule), (Some(140), RuleId::BGate));
    assert!(d.why.iter().any(|f| f.key == "gate_part" && f.value == 2.0));
}

#[test]
fn b_quiet_passes_with_nothing_to_tell() {
    assert_eq!(tell(&Scene::new(0, &LOW)), (None, RuleId::BQuiet));
    // Without the opponents' exchange there is no block either.
    let mut s = Scene::new(2, &LOW);
    s.auction(&[(0, None), (1, None)]);
    assert_eq!(tell(&s), (None, RuleId::BQuiet));
}

#[test]
fn bluffing_an_ace_step_nobody_can_hold() {
    // I hold all four aces: seat 1's +5 "ace" has no support under the facts; its +10 would.
    let mut s = Scene::new(0, &["g-A", "e-A", "s-A", "r-A", "g-6", "e-6", "s-6", "r-6", "g-7"]);
    s.auction(&[(0, None), (1, Some(120))]);
    let mut agent = HeuristicAgent::new(Params::default());
    assert!(agent.mind(&s.view()).bluffing(1));
    let mut t = Scene::new(0, &["g-A", "e-A", "s-A", "r-A", "g-6", "e-6", "s-6", "r-6", "g-7"]);
    t.auction(&[(0, None), (1, Some(125))]);
    assert!(!HeuristicAgent::new(Params::default()).mind(&t.view()).bluffing(1));
}

#[test]
fn tie_break_over_140_with_a_derived_team_pair() {
    // Batch 1c, the owner's example: the partner +10 (3+ halves or a small pair), I hold the
    // Red and Acorns halves and an ace to tell: our team likely holds a pair, so over 140.
    let mut s = Scene::new(2, &["r-K", "e-O", "g-A", "g-6", "s-6", "s-7", "e-6", "e-7", "g-7"]);
    s.auction(&[(0, Some(125)), (1, None)]);
    let d = s.act(&s.bid_legal());
    assert_eq!((bid_of(&d), d.rule), (Some(145), RuleId::BOver140), "{:?}", d.why);
    let fact = |k: &str| d.why.iter().find(|f| f.key == k).map(|f| f.value).unwrap();
    assert!(fact("team_pair_cover") >= Params::default().over_140_cover);
    assert!(fact("derived_pair_value") > 60.0);
}

#[test]
fn tie_break_140_with_a_strong_hand_after_the_partners_pass() {
    // Only the ace to tell, six cards standing and two A+10 suits, the partner passed:
    // game_drive ≥ 0.5, so 140 instead of +5.
    let mut s = Scene::new(2, &["g-A", "g-Z", "g-K", "e-A", "e-Z", "s-A", "s-6", "r-6", "r-7"]);
    s.auction(&[(0, None), (1, None)]);
    let d = s.act(&s.bid_legal());
    assert_eq!((bid_of(&d), d.rule), (Some(140), RuleId::BGate), "{:?}", d.why);
    assert!(d.why.iter().any(|f| f.key == "game_drive" && f.value >= 0.5));
}

#[test]
fn tie_break_steps_with_a_pair_still_to_tell_and_the_partner_yet_to_call() {
    // The ace first (+5), the green pair still to tell, the partner hasn't called: the step.
    let s = Scene::new(0, &["g-K", "g-O", "r-A", "g-6", "e-6", "e-7", "s-6", "s-7", "e-8"]);
    let d = s.act(&s.bid_legal());
    assert_eq!((bid_of(&d), d.rule), (Some(120), RuleId::BTell), "{:?}", d.why);
    assert!(d.why.iter().any(|f| f.key == "game_drive" && f.value < 0.5));
}

#[test]
fn seed_42_s1_takes_the_game_at_140() {
    // C43b, the owner's seed-42 game: S1 told its ace (125), S3 +10 (135), nothing left to
    // tell: p_take_140 ≈ 0.69 → 140 ([O] "should have gone for the 140 to take the game");
    // without standing_potential (Eichel A Z U 9) it would be below 0.5.
    let mut s = Scene::new(1, &["e-9", "g-K", "e-Z", "e-U", "s-K", "g-6", "g-U", "e-A", "r-Z"]);
    s.auction(&[(0, Some(120)), (1, Some(125)), (2, None), (3, Some(135)), (0, None)]);
    let d = s.act(&s.bid_legal());
    let fact = |k: &str| d.why.iter().find(|f| f.key == k).map(|f| f.value);
    assert_eq!((bid_of(&d), d.rule), (Some(140), RuleId::BGate), "{:?}", d.why);
    let p = fact("p_take_140").unwrap();
    assert!((p - 0.69).abs() < 0.03, "{p}");
    let without = 1.0 / (1.0 + ((1.0 - p) / p).ln().exp() * fact("standing_potential").unwrap().exp());
    assert!(without < 0.5, "{without}");
}

fn cards(c: &[&str]) -> engine::CardSet {
    engine::CardSet::from_cards(&c.iter().map(|c| c.parse().unwrap()).collect::<Vec<_>>())
}

/// `control_run` (Astra's review 2026-10-13): deal 573's Schell A Z K O 9 7 (four on top, three
/// outside), deal 576's Rot A Z O 9 8 7 6 (two on top, two outside); without the ace, or with a
/// gap the outside cards can use, no run.
#[test]
fn a_control_run_needs_the_ace_and_a_top_sequence_covering_the_outside_cards() {
    use engine::Suit;
    let run = |c: &[&str], s: Suit| knowledge::control_run(cards(c), s);
    assert!(run(&["s-A", "s-Z", "s-K", "s-O", "s-9", "s-7"], Suit::Bells));
    assert!(run(&["r-A", "r-Z", "r-O", "r-9", "r-8", "r-7", "r-6"], Suit::Red));
    assert!(!run(&["s-U", "s-Z", "s-K", "s-O", "s-9", "s-7"], Suit::Bells));
    assert!(!run(&["s-A", "s-K", "s-O", "s-9", "s-7"], Suit::Bells));
}

/// The owner's deal 573: S2 told its Schell pair (150), the opponents went 155 after its partner
/// passed at its first call; with the run it bids 160 once (B-control).
#[test]
fn deal_573_takes_one_step_with_a_control_run() {
    let auction = [(0, None), (1, Some(135)), (2, Some(150)), (3, None), (1, Some(155))];
    let mut s = Scene::new(2, &["e-K", "e-U", "s-A", "s-Z", "s-K", "s-O", "s-9", "s-7", "r-8"]);
    s.auction(&auction);
    assert_eq!(tell(&s), (Some(160), RuleId::BControl));
    // Without the Schell ace there is no run to own.
    let mut s = Scene::new(2, &["e-K", "e-U", "s-U", "s-Z", "s-K", "s-O", "s-9", "s-7", "r-8"]);
    s.auction(&auction);
    assert_eq!(tell(&s).0, None);
    assert_eq!(b_take_tell(&s).1, RuleId::BTake);
    // Once: after its 160 the opponents' 165 is defended (B-control's one challenge is spent);
    // with B-take on, it bids on instead.
    let mut s = Scene::new(2, &["e-K", "e-U", "s-A", "s-Z", "s-K", "s-O", "s-9", "s-7", "r-8"]);
    let mut longer = auction.to_vec();
    longer.extend([(2, Some(160)), (1, Some(165))]);
    s.auction(&longer);
    assert_eq!(tell(&s).0, None);
    assert_eq!(b_take_tell(&s), (Some(170), RuleId::BTake));
}

/// The partner's first pass at 140 or more is censored, not weak (the owner 2026-10-13): no
/// B-control on it.
#[test]
fn a_first_pass_over_140_does_not_make_the_partner_weak() {
    let hand = ["e-K", "e-U", "s-A", "s-Z", "s-K", "s-O", "s-9", "s-7", "r-8"];
    let mut s = Scene::new(0, &hand);
    s.auction(&[(0, Some(130)), (1, Some(145)), (2, None), (3, None)]);
    assert_eq!(tell(&s).0, None);
    assert_eq!(b_take_tell(&s).1, RuleId::BTake);
    let mut s = Scene::new(0, &hand);
    s.auction(&[(0, Some(130)), (1, Some(135)), (2, None), (3, Some(145))]);
    assert_eq!(tell(&s), (Some(150), RuleId::BControl));
}

/// The owner's deal 848: S1 showed the Red pair (145) without an ace; S3 hasn't bid and holds
/// aces: it offers 150 (B-offer). Without an ace, no offer.
#[test]
fn deal_848_offers_an_ace_to_the_partners_red_pair() {
    let auction = [(0, Some(120)), (1, Some(145)), (2, None)];
    let mut s = Scene::new(3, &["g-Z", "g-U", "g-7", "e-A", "e-U", "e-6", "s-Z", "r-A", "r-Z"]);
    s.auction(&auction);
    assert_eq!(tell(&s), (Some(150), RuleId::BOffer));
    let mut s = Scene::new(3, &["g-Z", "g-U", "g-7", "e-K", "e-U", "e-6", "s-Z", "r-9", "r-Z"]);
    s.auction(&auction);
    assert_eq!(tell(&s).0, None);
}

/// The keep (the owner on deal 848): after S3's offer over its Red pair, S1 keeps the game with
/// +5 while it holds the pair, so it receives the offered ace.
#[test]
fn deal_848_the_pair_holder_keeps_after_the_offer() {
    let mut s = Scene::new(1, &["e-Z", "e-K", "e-8", "s-9", "r-K", "r-O", "r-9", "r-8", "r-6"]);
    s.auction(&[(0, Some(120)), (1, Some(145)), (2, None), (3, Some(150)), (0, None)]);
    assert_eq!(tell(&s), (Some(155), RuleId::BOffer));
}

#[test]
fn information_above_140_does_not_spend_the_ownership_challenge() {
    // A recorded truthful Red message from 140 to 160 is information even
    // though the ordinary current encoder closes its telling channel at 140.
    let mut s = Scene::new(1, &hand(&["r-K", "r-O"]));
    s.auction(&[(0, Some(140)), (1, Some(160)), (2, None), (3, Some(165)), (0, None)]);
    let d = s.act(&s.bid_legal());
    assert_eq!((bid_of(&d), d.rule), (Some(170), RuleId::BOffer));
    assert!(d.why.iter().any(|f| f.key == "own_challenged" && f.value == 0.0));
    s.auction(&[(1, Some(170)), (3, Some(175))]);
    let d = s.act(&s.bid_legal());
    assert_eq!(bid_of(&d), None);
    assert!(d.why.iter().any(|f| f.key == "own_challenged" && f.value == 1.0));
}

#[test]
fn keep_survives_intervention_but_requires_the_offered_red_pair() {
    let auction = [(0, Some(120)), (1, Some(145)), (2, None), (3, Some(150)), (0, Some(155))];
    let mut s = Scene::new(1, &hand(&["r-K", "r-O"]));
    s.auction(&auction);
    assert_eq!(tell(&s), (Some(160), RuleId::BOffer));
    // A bluffing Red message followed by the same offer cannot justify keeping
    // an unrelated Bells pair under the Red-pair prerequisite.
    let mut other_pair = Scene::new(1, &hand(&["s-K", "s-O"]));
    other_pair.auction(&auction);
    assert_eq!(tell(&other_pair).0, None);
    assert_eq!(b_take_tell(&other_pair).1, RuleId::BTake);
}

#[test]
fn an_offering_hand_with_its_own_pair_still_sends_only_plus_five() {
    let mut s = Scene::new(3, &hand(&["e-A", "g-K", "g-O"]));
    s.auction(&[(0, Some(120)), (1, Some(145)), (2, None)]);
    assert_eq!(tell(&s), (Some(150), RuleId::BOffer));
}

#[test]
fn b_take_bids_on_over_the_opponents_140_with_a_pair_and_aces() {
    // The partner passed, the opponents hold the game at 140; I hold the Bells pair, three aces
    // and two more standing cards: P(take) and P(make 145) both clear 0.5 (B-take on, as in
    // checkpoint 2026-11-01). A weak hand lets them play.
    let strong = ["s-A", "s-Z", "s-K", "s-O", "g-A", "e-A", "e-Z", "r-6", "g-7"];
    for (h, take) in [(&strong[..], true), (&LOW[..], false)] {
        let mut s = Scene::new(2, h);
        s.auction(&[(0, None), (1, Some(140))]);
        let d = s.act_with(b_take_on(), &s.bid_legal());
        let expected = if take { (Some(145), RuleId::BTake) } else { (None, RuleId::BQuiet) };
        assert_eq!((bid_of(&d), d.rule), expected, "{:?}", d.why);
    }
}

#[test]
fn b_take_crosses_140_only_with_a_likely_pair() {
    // Three aces, two tens and two single halves, the partner not yet heard: strong enough for
    // the fitted P(take) and P(make), but a bid above 140 tells a pair (HR-4) and the team's
    // derivable pair cover stays below `over_140_cover` (the Fable review's example; B-take on).
    let mut s = Scene::new(1, &["g-A", "e-A", "s-A", "e-Z", "g-Z", "r-K", "s-O", "g-6", "e-6"]);
    s.auction(&[(0, Some(140))]);
    let d = s.act_with(b_take_on(), &s.bid_legal());
    assert_ne!(d.rule, RuleId::BTake, "{:?}", d.why);
    assert_eq!(bid_of(&d), None, "{:?}", d.why);
    // The fitted gates pass; only the cover vetoes.
    let fact = |k: &str| d.why.iter().find(|f| f.key == k).map(|f| f.value).unwrap();
    assert!(fact("p_take") >= 0.5 && fact("p_make") >= 0.5, "{:?}", d.why);
    assert!(fact("team_pair_cover") < Params::default().over_140_cover, "{:?}", d.why);
}

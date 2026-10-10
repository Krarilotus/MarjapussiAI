//! KB batch 1 (`findings/kb/concepts.md`): one position per bidding rule (B-tell, B-over140,
//! B-block, B-quiet) and the `bluffing(y)` reading. Seat 0 opens.

mod common;

use common::{bid_of, Scene};
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

//! Certified raising must be realized by legal play, including fresh agents.

mod common;

use agent::Table;
use common::{bid_of, c, set, Scene};
use engine::{ActionType, Card, CardSet, GameFinishedInfo, GamePhase, Suit};
use heuristic::{Decision, HeuristicAgent, Params, RuleId};

// Certificates have stronger guarantees than the estimated raise (HZ-1). These oracle checks
// isolate that guarantee and its follow-through.
fn certified_agent() -> HeuristicAgent {
    HeuristicAgent::new(Params {
        estimated_raise: false,
        ..Params::default()
    })
}

const PARTIAL: [&str; 9] = ["r-A", "r-Z", "r-K", "r-O", "g-A", "g-Z", "e-A", "s-A", "e-6"];
const ALL_NINE: [&str; 9] = ["r-A", "r-Z", "r-K", "r-O", "g-A", "g-Z", "e-A", "s-A", "s-Z"];

fn value(d: &Decision, key: &str) -> f32 {
    d.why
        .iter()
        .find(|f| f.key == key)
        .unwrap_or_else(|| panic!("missing {key}: {d:?}"))
        .value
}

#[test]
fn suit_exhaustion_certificate_raises_and_follows_the_complete_claim() {
    let cards = ["r-A", "r-Z", "r-K", "r-O", "r-9", "r-6", "g-A", "g-Z", "e-A"];
    for rotation in [0, 5, 13] {
        let mut table = preplay(&cards, 140, rotation);
        let (seat, legal) = table.to_act().unwrap();
        let d = certified_agent().decide(&table.view(seat), legal);
        assert_eq!(bid_of(&d), Some(240), "{d:?}");
        assert_eq!(value(&d, "certified_claim"), 1.0);
        table.apply(d.action);
        for _ in 0..70 {
            let Some((seat, legal)) = table.to_act() else { break };
            let action = if seat == 0 {
                let d = certified_agent().decide(&table.view(seat), legal);
                assert_eq!(value(&d, "certified_claim"), 1.0);
                d.action
            } else {
                legal[0]
            };
            table.apply(action);
        }
        assert!(table.game.ended());
        let info = GameFinishedInfo::from(table.game);
        assert_eq!(info.team_points[0], 240);
        assert_eq!(info.outcome().team_tricks, [9, 0]);
    }
}

#[test]
fn a_contract_already_at_the_claim_floor_keeps_the_certified_route() {
    let cards = ["r-A", "r-Z", "r-K", "r-O", "r-9", "r-6", "g-A", "g-Z", "e-A"];
    let mut table = preplay(&cards, 240, 5);
    for _ in 0..70 {
        let Some((seat, legal)) = table.to_act() else { break };
        let action = if seat == 0 {
            let d = certified_agent().decide(&table.view(seat), legal);
            assert_eq!(value(&d, "certified_claim"), 1.0, "{d:?}");
            d.action
        } else {
            legal[0]
        };
        table.apply(action);
    }
    assert!(table.game.ended());
    assert_eq!(GameFinishedInfo::from(table.game).scores(), [240, -480]);
}

fn apply(table: &mut Table, kind: ActionType) {
    let (_, legal) = table.to_act().unwrap();
    let a = *legal
        .iter()
        .find(|a| a.action_type == kind)
        .unwrap_or_else(|| panic!("{kind:?}: {legal:?}"));
    table.apply(a);
}

fn preplay(cards: &[&str; 9], contract: i32, rotation: usize) -> Table {
    let mine = set(cards);
    let mut rest: Vec<Card> = (CardSet::ALL - mine).iter().map(Card::from_index).collect();
    let n = rest.len();
    rest.rotate_left(rotation % n);
    let hands = [
        mine.cards(),
        rest[..9].to_vec(),
        rest[9..18].to_vec(),
        rest[18..].to_vec(),
    ];
    let mut table = Table::new(&hands);
    apply(&mut table, ActionType::NewBid(contract));
    for _ in 0..3 {
        apply(&mut table, ActionType::StopBidding);
    }
    let (_, legal) = table.to_act().unwrap();
    let pass = legal
        .iter()
        .find(|a| matches!(a.action_type, ActionType::Pass(_)))
        .unwrap()
        .action_type;
    apply(&mut table, pass);
    apply(&mut table, pass);
    assert_eq!(table.game.state.phase, GamePhase::Raising);
    assert_eq!(table.hand(0), mine);
    table
}

#[test]
fn raises_only_within_the_partial_floor_and_explains_the_bound() {
    let table = preplay(&PARTIAL, 140, 0);
    let (seat, legal) = table.to_act().unwrap();
    let d = certified_agent().decide(&table.view(seat), legal);
    assert_eq!(bid_of(&d), Some(170), "{d:?}");
    assert_eq!(d.rule, RuleId::HZ2);
    assert_eq!(value(&d, "raise_floor"), 171.0); // 71 own card points + one red pair.
    assert_eq!(value(&d, "certified_tricks"), 8.0);
    assert_eq!(value(&d, "own_score_floor"), 170.0);

    let table = preplay(&PARTIAL, 175, 0);
    let (seat, legal) = table.to_act().unwrap();
    let d = certified_agent().decide(&table.view(seat), legal);
    assert!(matches!(d.action.action_type, ActionType::CardPlayed(_)), "{d:?}");
    assert_eq!(value(&d, "raise_floor"), 171.0);
}

#[test]
fn missing_own_ace_or_a_blocked_trump_run_does_not_become_a_guarantee() {
    for cards in [
        ["r-Z", "r-K", "r-O", "g-6", "g-7", "e-6", "e-7", "s-6", "s-7"],
        ["r-A", "r-K", "r-O", "g-6", "g-7", "e-6", "e-7", "s-6", "s-7"],
        // Holding two pairs does not certify that both can be activated.
        ["r-K", "r-O", "s-K", "s-O", "e-A", "g-6", "g-7", "e-6", "e-7"],
    ] {
        let table = preplay(&cards, 140, 0);
        let (seat, legal) = table.to_act().unwrap();
        let d = certified_agent().decide(&table.view(seat), legal);
        assert!(matches!(d.action.action_type, ActionType::CardPlayed(_)), "{d:?}");
        assert!(value(&d, "raise_floor") <= 140.0);
    }
}

#[test]
fn every_legal_opening_ace_realizes_the_floor_against_other_legal_hands() {
    for cards in [&PARTIAL, &ALL_NINE] {
        for opening in ["g-A", "e-A", "s-A", "r-A"] {
            for rotation in [0, 5, 13] {
                let mut table = preplay(cards, 140, rotation);
                let (seat, legal) = table.to_act().unwrap();
                let d = certified_agent().decide(&table.view(seat), legal);
                let floor = value(&d, "raise_floor") as i32;
                let target = bid_of(&d).unwrap();
                assert_eq!(target, if cards == &ALL_NINE { 240 } else { 170 });
                table.apply(d.action);
                apply(&mut table, ActionType::CardPlayed(Card::from_index(c(opening))));
                let mut certified = 0;
                for _ in 0..100 {
                    let Some((seat, legal)) = table.to_act() else {
                        break;
                    };
                    let action = if seat == 0 {
                        // A fresh agent must reconstruct the certificate; default
                        // four-trick sampled search cannot override its cashing.
                        let d = certified_agent().decide(&table.view(seat), legal);
                        assert!(legal.contains(&d.action), "{d:?}");
                        certified += d.why.iter().any(|f| f.key == "certified_raise_route") as usize;
                        d.action
                    } else {
                        *legal
                            .iter()
                            .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
                            .unwrap_or(&legal[0])
                    };
                    table.apply(action);
                }
                assert!(table.game.ended());
                let info = GameFinishedInfo::from(table.game);
                assert!(info.team_points[0] >= floor, "{opening}, rotation {rotation}: {info:?}");
                assert!(certified > 0);
                if cards == &ALL_NINE {
                    assert_eq!(floor, 240);
                    assert_eq!(info.outcome().team_tricks, [9, 0]);
                    assert_eq!(info.scores(), [240, -480]);
                }
            }
        }
    }
}

fn raised_scene() -> Scene {
    let mut s = Scene::new(0, &PARTIAL);
    s.auction(&[(0, Some(140)), (1, None), (2, None), (3, None)]);
    let transfer = ["g-6", "g-7", "e-7", "e-8"];
    s.pass(2, 0, Some(&transfer));
    s.pass(0, 2, Some(&transfer));
    s.auction(&[(0, Some(170))]);
    for (seat, card) in [(0, "r-A"), (1, "r-6"), (2, "r-7"), (3, "r-8")] {
        s.play(seat, card);
    }
    s
}

#[test]
fn interrupted_routes_are_not_relabelled_as_certified() {
    let mut spent = raised_scene();
    spent.ev(observation::SeatEvent::AskPair { seat: 0 });
    let d = spent.act(&spent.card_legal());
    assert!(!d.why.iter().any(|f| f.key == "certified_raise_route"));

    let mut changed = raised_scene();
    changed.ev(observation::SeatEvent::AnnounceTrump {
        seat: 2,
        suit: Suit::Green,
    });
    let d = changed.act(&changed.card_legal());
    assert!(!d.why.iter().any(|f| f.key == "certified_raise_route"));

    let mut consumed = raised_scene();
    for (seat, card) in [(0, "r-O"), (1, "r-9"), (2, "r-U"), (3, "s-6")] {
        consumed.play(seat, card);
    }
    let d = consumed.act(&consumed.card_legal());
    assert!(!d.why.iter().any(|f| f.key == "certified_raise_route"));

    let mut lost = raised_scene();
    for (seat, card) in [(0, "e-6"), (1, "e-U"), (2, "e-7"), (3, "e-9")] {
        lost.play(seat, card);
    }
    assert_eq!(lost.tracker.knowledge().trick_winners(), &[0, 1]);
    let d = lost.act(&lost.card_legal());
    assert!(!d.why.iter().any(|f| f.key == "certified_raise_route"));
}

#[test]
fn two_pairs_raise_by_the_expected_contract_score() {
    // HZ-1 with the fitted contract_estimate (the route raise is off here).
    let cards = ["r-K", "r-O", "s-K", "s-O", "e-A", "g-6", "g-7", "e-6", "e-7"];
    let table = preplay(&cards, 140, 0);
    let (seat, legal) = table.to_act().unwrap();
    let params = Params {
        route_raise: false,
        ..Params::default()
    };
    let d = HeuristicAgent::new(params).decide(&table.view(seat), legal);
    assert_eq!(d.rule, RuleId::HZ1, "{d:?}");
    let target = bid_of(&d).unwrap();
    assert!(target > 140, "{d:?}");
    assert!(value(&d, "p_make") > 0.5);
    assert!(value(&d, "expected_score") > value(&d, "expected_score_keep"));
}

#[test]
fn no_certain_team_pair_never_raises_across_140() {
    let cards = ["r-A", "r-Z", "g-A", "g-Z", "e-A", "e-Z", "s-A", "s-Z", "r-9"];
    let table = preplay(&cards, 120, 0);
    let (seat, legal) = table.to_act().unwrap();
    let d = HeuristicAgent::new(Params::default()).decide(&table.view(seat), legal);
    assert!(bid_of(&d).is_none_or(|v| v <= 140), "{d:?}");
}

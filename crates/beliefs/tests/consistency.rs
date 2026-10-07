//! Guard tests of the believed forms (S9 acceptance): known ⇒ believed (a read is exactly 1 or
//! 0 precisely where the facts decide it, and then true), privacy (hidden cards change no
//! read), undo (a mind after an undo reads as a fresh mind replaying the remaining events).
//! Random legal games through the engine, including undos; checked after every event.

use agent::SeatKind;
use beliefs::{BeliefParams, Mind, SeatFeatures};
use engine::{get_all_cards, hands_from_deck, partner, takes, ActionType, Card, CardSet, Game, GameAction, SUITS};
use observation::{seat_events, start_hand, Seat, SeatFilter};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

const GAMES: u64 = 150;

fn game_from(hands: &[Vec<Card>; 4]) -> Game {
    Game::new(
        "beliefs".into(),
        ["S0", "S1", "S2", "S3"].map(String::from),
        Some(hands.clone()),
    )
}

fn deal(rng: &mut StdRng) -> [Vec<Card>; 4] {
    let mut deck = get_all_cards();
    deck.shuffle(rng);
    hands_from_deck(&deck)
}

fn pick_action(game: &Game, rng: &mut StdRng) -> GameAction {
    let legal = &game.legal_actions();
    let (undo, normal): (Vec<&GameAction>, Vec<&GameAction>) = legal
        .iter()
        .partition(|a| matches!(a.action_type, ActionType::UndoRequest));
    if normal.is_empty() || (!undo.is_empty() && rng.random_bool(0.05)) {
        return legal[rng.random_range(0..legal.len())];
    }
    *normal[rng.random_range(0..normal.len())]
}

fn new_mind(game: &Game, seat: Seat) -> Mind {
    mind_with(game, seat, [SeatKind::Unknown; 4], BeliefParams::default())
}

fn mind_with(game: &Game, seat: Seat, kinds: [SeatKind; 4], params: BeliefParams) -> Mind {
    Mind::new(seat, start_hand(game, seat), kinds, params)
}

/// `BaseOnly` reads the exact n-over-k base for every card.
fn check_base_only(m: &Mind, ctx: &str) {
    let space = m.knowledge().deals();
    let (counts, total) = (space.holder_counts(), space.count() as f64);
    for y in 0..4u8 {
        for c in 0..36u8 {
            let exact = counts[y as usize][c as usize] as f64 / total;
            let p = m.p_holds(y, c) as f64;
            assert!(
                (p - exact).abs() < 1e-5,
                "{ctx}: base-only p_holds({y}, {c}) = {p}, exact {exact}"
            );
        }
    }
}

/// Every read of the API, in a fixed order.
fn reads(m: &Mind) -> Vec<f32> {
    let mut out = vec![];
    for x in 0..4u8 {
        out.extend(m.holds_row(x));
        for s in SUITS {
            out.push(m.p_void(x, s));
            out.extend(m.count(x, s).p);
        }
        out.extend(feature_values(m.features(x)));
        out.extend(feature_values(m.dealt_features(x)));
        out.extend((0..4).flat_map(|y| m.view(x).row(y)));
    }
    for c in 0..36 {
        out.push(m.p_stands(c));
        out.push(m.future_win(c));
    }
    for team in 0..2 {
        for s in SUITS {
            out.push(m.p_team_pair(team, s));
            out.push(m.p_meldable(team, s));
        }
    }
    out
}

fn feature_values(f: SeatFeatures) -> Vec<f32> {
    let mut v = vec![f.ace];
    v.extend(f.halves_at_least);
    v.extend(f.half);
    v.extend(f.pair);
    v
}

/// The features of a hand, in `feature_values` order (each grows with the hand).
fn feature_truth(hand: CardSet) -> Vec<bool> {
    let halves: Vec<u32> = SUITS.iter().map(|&s| (hand & CardSet::halves(s)).len()).collect();
    let total: u32 = halves.iter().sum();
    let mut v = vec![!(hand & CardSet::ACES).is_empty()];
    v.extend((1..=3).map(|k| total >= k));
    v.extend(halves.iter().map(|&h| h >= 1));
    v.extend(halves.iter().map(|&h| h >= 2));
    v
}

/// A feature that is a fact is true of `hand`; a feature the sets `sure ⊆ hand ⊆ may` decide is
/// that fact.
fn check_features(f: SeatFeatures, hand: CardSet, sure: CardSet, may: CardSet, ctx: &str) {
    let (truth, yes, maybe) = (feature_truth(hand), feature_truth(sure), feature_truth(may));
    for (i, p) in feature_values(f).into_iter().enumerate() {
        assert!(
            !is_fact(p) || (p == 1.0) == truth[i],
            "{ctx}: feature {i} = {p} is false"
        );
        assert!(!yes[i] || p == 1.0, "{ctx}: feature {i} = {p}, but the facts say yes");
        assert!(maybe[i] || p == 0.0, "{ctx}: feature {i} = {p}, but the facts say no");
    }
}

fn is_fact(p: f32) -> bool {
    p == 0.0 || p == 1.0
}

/// Known ⇒ believed, and every fact true, for one mind.
fn check(m: &Mind, game: &Game, ctx: &str) {
    let k = m.knowledge();
    let me = m.seat();
    let truth = |y: u8| CardSet::from_cards(&game.state.players[y as usize].cards);
    let exact = k.deals().exact_sets();
    for p in reads(m) {
        assert!(
            (0.0..=1.0).contains(&p) && p.is_finite(),
            "{ctx}: read {p} out of range"
        );
        assert_eq!(m.band(p).is_known(), is_fact(p), "{ctx}: band of {p}");
    }
    for x in 0..4u8 {
        // Cards: 1 exactly on the exact certain set, 0 exactly off the exact possible set.
        for c in 0..36u8 {
            let p = m.p_holds(x, c);
            assert_eq!(
                p == 1.0,
                exact.certain[x as usize].contains(c),
                "{ctx}: p_holds({x}, {c}) = {p}"
            );
            assert_eq!(
                p == 0.0,
                !exact.may_hold(x).contains(c),
                "{ctx}: p_holds({x}, {c}) = {p}"
            );
            assert!(
                !is_fact(p) || (p == 1.0) == truth(x).contains(c),
                "{ctx}: p_holds({x}, {c}) false"
            );
        }
        for s in SUITS {
            let n = (truth(x) & CardSet::suit(s)).len() as usize;
            let (lo, hi) = k.count_bounds(x, s);
            let d = m.count(x, s).p;
            assert!(
                (d.iter().sum::<f32>() - 1.0).abs() < 1e-4,
                "{ctx}: count of {x} in {s:?} sums wrong"
            );
            assert!(d[n] > 0.0, "{ctx}: count of {x} in {s:?} rules out the truth {n}");
            assert!(
                d.iter()
                    .enumerate()
                    .all(|(i, &q)| q == 0.0 || (lo as usize..=hi as usize).contains(&i)),
                "{ctx}: count of {x} in {s:?} outside K4 {lo}..{hi}"
            );
            let v = m.p_void(x, s);
            if lo > 0 {
                assert_eq!(v, 0.0, "{ctx}: p_void({x}, {s:?}) with K4 lo {lo}");
            }
            if hi == 0 {
                assert_eq!(v, 1.0, "{ctx}: p_void({x}, {s:?}) with K4 hi 0");
            }
            assert!(
                !is_fact(v) || (v == 1.0) == (n == 0),
                "{ctx}: p_void({x}, {s:?}) = {v} false"
            );
        }
        // Features now: decided by the exact sets; dealt: decided by K5.
        let f = m.features(x);
        check_features(
            f,
            truth(x),
            exact.certain[x as usize],
            exact.may_hold(x),
            &format!("{ctx}: features of {x}"),
        );
        let dealt = CardSet::from_cards(&game.info.player_start_cards[x as usize]);
        let f = m.dealt_features(x);
        check_features(
            f,
            dealt,
            k.dealt_certain(x),
            k.dealt_possible(x),
            &format!("{ctx}: dealt features of {x}"),
        );
        // Views: a cell the viewer is said to know is true.
        let v = m.view(x);
        for y in 0..4u8 {
            for c in (CardSet::ALL - k.played()).iter() {
                let p = v.p_holds(y, c);
                assert!(
                    !is_fact(p) || (p == 1.0) == truth(y).contains(c),
                    "{ctx}: view of {x}: ({y}, {c}) = {p}"
                );
            }
        }
    }
    // K1: stands exactly where standing; a 0 means an opponent holds a higher card.
    let standing = k.standing(me);
    let opponents = truth((me + 1) % 4) | truth((me + 3) % 4);
    for c in 0..36u8 {
        let (p, w) = (m.p_stands(c), m.future_win(c));
        if !k.certain(me).contains(c) {
            assert!(p == 0.0 && w == 0.0, "{ctx}: a card not mine stands");
            continue;
        }
        assert_eq!(p == 1.0, standing.contains(c), "{ctx}: p_stands({c}) = {p}");
        assert_eq!(w == 1.0, standing.contains(c), "{ctx}: future_win({c}) = {w}");
        if p == 0.0 {
            let higher = (CardSet::suit(engine::suit_of(c)) - k.played())
                .iter()
                .any(|b| takes(b, c, None) && opponents.contains(b));
            assert!(higher, "{ctx}: p_stands({c}) = 0 without a higher card against it");
        }
        assert!(w >= p, "{ctx}: future_win({c}) below p_stands");
    }
    // K3: team pairs.
    for team in 0..2u8 {
        let (a, b) = (team, team + 2);
        let tp = k.team_pairs(a);
        for s in SUITS {
            let bit = 1u8 << s as u8;
            let p = m.p_team_pair(team, s);
            assert_eq!(
                p == 1.0,
                tp.certain() & bit != 0,
                "{ctx}: p_team_pair({team}, {s:?}) = {p}"
            );
            assert_eq!(
                p == 0.0,
                tp.possible() & bit == 0,
                "{ctx}: p_team_pair({team}, {s:?}) = {p}"
            );
            let held = CardSet::halves(s).is_subset(truth(a) | truth(b));
            if !k.is_called(s) && (CardSet::halves(s) & k.played()).is_empty() {
                assert!(
                    !is_fact(p) || (p == 1.0) == held,
                    "{ctx}: p_team_pair({team}, {s:?}) false"
                );
            }
            let meld = m.p_meldable(team, s);
            if k.is_called(s) || tp.possible() & bit == 0 {
                assert_eq!(meld, 0.0, "{ctx}: p_meldable({team}, {s:?})");
            }
            assert!(
                meld != 1.0 || held,
                "{ctx}: p_meldable({team}, {s:?}) = 1 without the pair"
            );
        }
    }
}

#[test]
fn known_implies_believed_on_random_games() {
    let mut checked = 0u32;
    let mut undos = 0u32;
    for seed in 0..GAMES {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut game = game_from(&deal(&mut rng));
        let mut filters: Vec<SeatFilter> = (0..4).map(SeatFilter::new).collect();
        // Odd seeds: every seat plays HP1, so the agent-kind reading (HP1's encoder) is checked.
        let kinds = [if seed % 2 == 1 {
            SeatKind::Heuristic
        } else {
            SeatKind::Unknown
        }; 4];
        let mut minds: Vec<Mind> = (0..4)
            .map(|s| mind_with(&game, s, kinds, BeliefParams::default()))
            .collect();
        let mut base: Vec<Mind> = (0..4)
            .map(|s| mind_with(&game, s, kinds, BeliefParams::base_only()))
            .collect();
        for step in 0..600 {
            if game.ended() {
                break;
            }
            let action = pick_action(&game, &mut rng);
            game.apply_action_mut(action);
            let event = game.all_events.last().unwrap();
            for seat in 0..4u8 {
                let Some(e) = filters[seat as usize].filter(event) else {
                    continue;
                };
                minds[seat as usize].apply(&e).unwrap();
                base[seat as usize].apply(&e).unwrap();
                if e == observation::SeatEvent::Undone {
                    undos += 1;
                    // Undo: the same reads as a fresh mind replaying what is left.
                    let mut fresh = mind_with(&game, seat, kinds, BeliefParams::default());
                    for e in minds[seat as usize].events() {
                        fresh.apply(e).unwrap();
                    }
                    assert_eq!(
                        reads(&minds[seat as usize]),
                        reads(&fresh),
                        "seed {seed}: undo of seat {seat}"
                    );
                }
            }
            if step % 3 == 0 {
                for (seat, (m, b)) in minds.iter().zip(&base).enumerate() {
                    let ctx = format!("seed {seed} step {step} seat {seat}");
                    check(m, &game, &ctx);
                    check(b, &game, &format!("{ctx} (base only)"));
                    check_base_only(b, &ctx);
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 10_000, "only {checked} checks");
    assert!(undos > 20, "only {undos} undos");
}

fn mind_of(game: &Game, seat: Seat) -> Mind {
    let mut m = new_mind(game, seat);
    for e in seat_events(game, seat) {
        m.apply(&e).unwrap();
    }
    m
}

/// Privacy: swapping two cards the seat cannot see changes none of its reads (as in
/// `knowledge/tests/privacy_swap.rs`).
#[test]
fn hidden_cards_do_not_change_beliefs() {
    let mut swaps = 0;
    for seed in 0..20_000u64 {
        if swaps >= 200 {
            break;
        }
        let mut rng = StdRng::seed_from_u64(seed);
        let hands = deal(&mut rng);
        let mut game = game_from(&hands);
        let mut actions = vec![];
        let stop = rng.random_range(1..60);
        while !game.ended() && actions.len() < stop {
            let legal = game.legal_actions();
            let action = legal[rng.random_range(0..legal.len())];
            game.apply_action_mut(action);
            actions.push(action);
        }
        let seat: Seat = rng.random_range(0..4);
        let others: Vec<u8> = (0..4).filter(|&o| o != seat).collect();
        let i = rng.random_range(0..3);
        let (x, y) = (others[i], others[(i + 1 + rng.random_range(0..2)) % 3]);
        let touched = actions.iter().fold(CardSet::EMPTY, |set, a| match &a.action_type {
            ActionType::Pass(cards) => set | CardSet::from_cards(cards),
            ActionType::CardPlayed(card) => set | CardSet::of(card),
            _ => set,
        });
        let mind = mind_of(&game, seat);
        let candidates = |from: u8, to: u8| -> Vec<Card> {
            hands[from as usize]
                .iter()
                .filter(|c| !touched.contains(c.index()) && mind.knowledge().may_hold(to).contains(c.index()))
                .cloned()
                .collect()
        };
        let (from_x, from_y) = (candidates(x, y), candidates(y, x));
        if from_x.is_empty() || from_y.is_empty() {
            continue;
        }
        let (a, b) = (
            from_x[rng.random_range(0..from_x.len())],
            from_y[rng.random_range(0..from_y.len())],
        );
        let mut swapped = hands.clone();
        for card in swapped.iter_mut().flatten() {
            *card = if *card == a {
                b
            } else if *card == b {
                a
            } else {
                *card
            };
        }
        let Some(other) = actions
            .iter()
            .try_fold(game_from(&swapped), |g, &act| g.apply_action(act).ok())
        else {
            continue;
        };
        let public = |g: &Game| -> Vec<String> {
            g.all_events
                .iter()
                .map(|e| format!("{:?} {:?}", e.last_action, e.callback))
                .collect()
        };
        if public(&other) != public(&game) || seat_events(&other, seat) != seat_events(&game, seat) {
            continue;
        }
        assert_eq!(
            reads(&mind_of(&other, seat)),
            reads(&mind),
            "seed {seed}: seat {seat} believes hidden cards"
        );
        swaps += 1;
    }
    assert!(swaps >= 200, "only {swaps} valid swaps");
}

/// Judge J20-I1 item 2: after a "no pair" answer (K2), the answerer's believed pair is exactly 0
/// in every uncalled suit, for every viewer; and for the asker, in a suit it holds no half of,
/// the believed team pair and meldability are exactly 0.
#[test]
fn no_pair_answer_leaves_no_believed_pair() {
    let (mut answers, mut asker_suits) = (0, 0);
    for seed in 0..2_000u64 {
        if answers >= 60 && asker_suits >= 60 {
            break;
        }
        let mut rng = StdRng::seed_from_u64(seed);
        let mut game = game_from(&deal(&mut rng));
        let mut filters: Vec<SeatFilter> = (0..4).map(SeatFilter::new).collect();
        let mut minds: Vec<Mind> = (0..4).map(|s| new_mind(&game, s)).collect();
        while !game.ended() {
            let action = pick_action(&game, &mut rng);
            game.apply_action_mut(action);
            let event = game.all_events.last().unwrap();
            let mut answerer = None;
            for seat in 0..4u8 {
                if let Some(e) = filters[seat as usize].filter(event) {
                    minds[seat as usize].apply(&e).unwrap();
                    if let observation::SeatEvent::NoPair { seat: a } = e {
                        answerer = Some(a);
                    }
                }
            }
            let Some(a) = answerer else { continue };
            answers += 1;
            let asker = partner(a);
            for m in &minds {
                let k = m.knowledge();
                for s in SUITS.into_iter().filter(|&s| !k.is_called(s)) {
                    let p = m.features(a).pair[s as usize];
                    assert_eq!(
                        p,
                        0.0,
                        "seed {seed}: seat {} believes seat {a}'s {s:?} pair at {p}",
                        m.seat()
                    );
                    if m.seat() == asker && (k.certain(asker) & CardSet::halves(s)).is_empty() {
                        asker_suits += 1;
                        let team = asker % 2;
                        assert_eq!(m.p_team_pair(team, s), 0.0, "seed {seed}: team pair in {s:?}");
                        assert_eq!(m.p_meldable(team, s), 0.0, "seed {seed}: meldable {s:?}");
                    }
                }
            }
        }
    }
    assert!(
        answers >= 60 && asker_suits >= 60,
        "only {answers} answers, {asker_suits} asker suits"
    );
}

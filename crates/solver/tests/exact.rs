//! Guard tests: the solver equals brute-force minimax on random positions (1–4 tricks left,
//! trick starts and mid-trick, announcements and trump rights included), and the engine's
//! `Game` played out by brute force (1–3 tricks left).

use engine::get_all_cards;
use engine::ActionType;
use engine::Game;
use engine::GameFinishedInfo;
use engine::GamePhase;
use engine::{CardSet, SUITS};
use engine::{FastState, PublicState, CARDS, TRICKS};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use solver::Solver;

/// Plain minimax: `team` maximises its future points, the other team minimises them.
fn brute(s: &FastState, team: u8) -> i32 {
    if s.is_over() {
        return 0;
    }
    let values = s.moves().map(|m| {
        let child = s.after(m);
        child.points(team) - s.points(team) + brute(&child, team)
    });
    if s.to_move() % 2 == team {
        values.max().unwrap()
    } else {
        values.min().unwrap()
    }
}

/// Plain minimax on tricks: `team` maximises its future tricks, the other team minimises them.
fn brute_tricks(s: &FastState, team: u8) -> i32 {
    if s.is_over() {
        return 0;
    }
    let values = s.moves().map(|m| {
        let child = s.after(m);
        child.tricks_won(team) as i32 - s.tricks_won(team) as i32 + brute_tricks(&child, team)
    });
    if s.to_move() % 2 == team {
        values.max().unwrap()
    } else {
        values.min().unwrap()
    }
}

/// A team seat can still announce a pair.
fn pair_alive(s: &FastState) -> bool {
    SUITS.iter().any(|&suit| {
        s.called() >> suit as u8 & 1 == 0
            && (0..4).any(|x| s.may_announce() >> x & 1 == 1 && CardSet::halves(suit).is_subset(s.hand(x)))
    })
}

/// A random position with `left` tricks to go and `extra` cards already in the trick, built
/// from parts: random trump, called suits, trump rights, open trick start and score so far.
/// Often one seat holds both halves of a suit, so announcements are covered.
fn position(rng: &mut StdRng, left: u8, extra: u8) -> FastState {
    let mut deck: Vec<u8> = (0..CARDS as u8).collect();
    deck.shuffle(rng);
    let leader = rng.random_range(0..4u8);
    let (trick, rest) = deck.split_at(extra as usize);
    let mut rest = rest.to_vec();
    let mut hands = [CardSet::EMPTY; 4];
    let mut sizes = [left; 4];
    for i in 0..extra {
        sizes[((leader + i) % 4) as usize] -= 1;
    }
    let pair_seat = rng.random_range(0..4u8);
    if rng.random_bool(0.6) && sizes[pair_seat as usize] >= 2 {
        let pair = CardSet::halves(SUITS[rng.random_range(0..4)]);
        if (trick.iter().fold(CardSet::EMPTY, |a, &c| a | CardSet::single(c)) & pair).is_empty() {
            rest.retain(|c| !pair.contains(*c));
            hands[pair_seat as usize] = pair;
        }
    }
    let mut cards = rest.into_iter();
    for x in 0..4 {
        while hands[x].len() < sizes[x] as u32 {
            hands[x] |= CardSet::single(cards.next().unwrap());
        }
    }
    let trump = [None, Some(SUITS[0]), Some(SUITS[1]), Some(SUITS[2]), Some(SUITS[3])][rng.random_range(0..5)];
    let tricks_done = TRICKS - left;
    let won = rng.random_range(0..=tricks_done);
    let mut t = [0; 4];
    t[..trick.len()].copy_from_slice(trick);
    let public = PublicState {
        trick: t,
        trick_len: extra,
        leader,
        trump,
        called: rng.random_range(0..16u8) & rng.random_range(0..16u8) | trump.map_or(0, |s| 1 << s as u8),
        may_announce: rng.random_range(0..16),
        announce_open: extra == 0 && rng.random_bool(0.7),
        tricks_done,
        tricks: [won, tricks_done - won],
        card_points: [rng.random_range(0..60), rng.random_range(0..60)],
        pair_points: [0, 40 * rng.random_range(0..2)],
    };
    FastState::from_public(public, hands).unwrap()
}

#[test]
fn solver_equals_brute_force() {
    let mut rng = StdRng::seed_from_u64(3);
    // A small table forces collisions and replacements; it is reused across positions.
    let mut solver = Solver::with_tt_bits(10);
    let (mut checked, mut with_pairs) = (0, 0);
    for (left, count) in [(1, 800), (2, 800), (3, 600), (4, 100)] {
        for i in 0..count {
            let s = position(&mut rng, left, (i % 4) as u8);
            for team in 0..2 {
                let exact = brute(&s, team);
                assert_eq!(solver.value(&s, team), exact, "value, {left} tricks left: {s:?}");
                for target in [exact - 7, exact - 1, exact, exact + 1, exact + 12] {
                    assert_eq!(
                        solver.reaches(&s, team, target),
                        exact >= target,
                        "reaches {target}: {s:?}"
                    );
                }
                let tricks = brute_tricks(&s, team);
                assert_eq!(solver.tricks(&s, team), tricks, "tricks: {s:?}");
                for target in [tricks - 1, tricks, tricks + 1] {
                    assert_eq!(
                        solver.reaches_tricks(&s, team, target),
                        tricks >= target,
                        "tricks {target}: {s:?}"
                    );
                }
                for (m, v) in solver.move_values(&s, team) {
                    let child = s.after(m);
                    let gain = child.points(team) - s.points(team);
                    assert_eq!(v, gain + brute(&child, team), "move {m:?}: {s:?}");
                }
            }
            checked += 1;
            with_pairs += pair_alive(&s) as u32;
        }
    }
    eprintln!("{checked} positions, {with_pairs} with a pair that can still be announced");
    assert!(with_pairs >= 200, "too little announcement coverage: {with_pairs}");
}

/// Minimax over the engine's own `Game` (card plays and announcements; questions are not
/// solver moves), scored by `GameFinishedInfo::team_points`.
fn game_minimax(game: &Game, team: u8) -> i32 {
    if game.ended() {
        return GameFinishedInfo::from(game.clone()).team_points[team as usize];
    }
    let legal = game.legal_actions();
    let values = legal
        .iter()
        .filter(|a| matches!(a.action_type, ActionType::CardPlayed(_) | ActionType::AnnounceTrump(_)))
        .map(|a| game_minimax(&game.apply_action(*a).unwrap(), team));
    if game.state.player_at_turn.0 % 2 == team {
        values.max().unwrap()
    } else {
        values.min().unwrap()
    }
}

#[test]
fn solver_equals_engine_minimax() {
    let mut rng = StdRng::seed_from_u64(5);
    let mut solver = Solver::new();
    let (mut checked, mut seed) = (0, 0u64);
    while checked < 150 {
        seed += 1;
        let mut deck = get_all_cards();
        deck.shuffle(&mut rng);
        let hands = [0, 9, 18, 27].map(|i| deck[i..i + 9].to_vec());
        let mut game = Game::new("ref".into(), ["S0", "S1", "S2", "S3"].map(String::from), Some(hands));
        let left = 1 + (seed % 3) as usize;
        let extra = (seed / 3 % 4) as usize;
        // Random bidding, passing, questions and play (no undo) down to the position.
        loop {
            if game.ended() {
                break;
            }
            let s = &game.state;
            let in_play = matches!(s.phase, GamePhase::Trick | GamePhase::StartTrick);
            let cards_left = s.players.iter().map(|p| p.cards.len()).sum::<usize>();
            if in_play && cards_left == 4 * left - extra {
                break;
            }
            let legal: Vec<_> = game
                .legal_actions()
                .iter()
                .filter(|a| !matches!(a.action_type, ActionType::UndoRequest))
                .cloned()
                .collect();
            game.apply_action_mut(legal[rng.random_range(0..legal.len())]);
        }
        if game.ended() {
            continue;
        }
        let fast = FastState::from_game(&game).unwrap();
        for team in 0..2 {
            assert_eq!(
                fast.points(team) + solver.value(&fast, team),
                game_minimax(&game, team),
                "seed {seed}, team {team}: {fast:?}"
            );
        }
        checked += 1;
    }
}

/// `s` with one public field changed (hands, trick and score unchanged).
fn vary(s: &FastState, change: impl FnOnce(&mut PublicState)) -> FastState {
    let mut p = s.public();
    change(&mut p);
    FastState::from_public(p, s.hands()).unwrap()
}

/// Solves `a` then `b` through one shared solver, then `b` then `a` through another, and checks
/// every value against brute force. Returns whether the two positions differ in value.
fn check_pair(a: &FastState, b: &FastState, team: (u8, u8)) -> bool {
    let (ea, eb) = (brute(a, team.0), brute(b, team.1));
    for order in [[(a, team.0, ea), (b, team.1, eb)], [(b, team.1, eb), (a, team.0, ea)]] {
        let mut solver = Solver::new();
        for (s, t, exact) in order {
            assert_eq!(solver.value(s, t), exact, "shared table, team {t}: {s:?}");
            assert!(solver.reaches(s, t, exact), "{s:?}");
            assert!(!solver.reaches(s, t, exact + 1), "{s:?}");
        }
    }
    ea != eb
}

#[test]
fn transposition_key_separates_every_public_field() {
    // Each field of the key, changed alone on identical hands: if the key dropped it, the second
    // solve would reuse the first one's bounds. Cases are drawn per field until 40 pairs really
    // differ in value (at most 6,000 tries per field).
    let names = ["trump", "called suits", "trump rights", "open trick start", "team"];
    let mut differ = [0u32; 5];
    let mut tries = [0u32; 5];
    let mut rng = StdRng::seed_from_u64(21);
    for i in 0..30_000usize {
        let field = i % 5;
        if differ[field] >= 40 || tries[field] >= 6_000 {
            if (0..5).all(|f| differ[f] >= 40 || tries[f] >= 6_000) {
                break;
            }
            continue;
        }
        tries[field] += 1;
        let mut s = position(
            &mut rng,
            if field == 1 || field == 2 { 3 } else { 2 + (i % 2) as u8 },
            0,
        );
        // Called suits, rights and the open trick start matter only through an announceable
        // pair: give the lead to a seat holding a pair, with its right and the suit uncalled.
        if (1..=3).contains(&field) {
            let pair = (0..4u8)
                .flat_map(|x| SUITS.iter().map(move |&t| (x, t)))
                .find(|&(x, t)| CardSet::halves(t).is_subset(s.hand(x)));
            let Some((x, t)) = pair else {
                continue;
            };
            s = vary(&s, |p| {
                p.leader = x;
                p.may_announce |= 1 << x;
                p.called &= !(1 << t as u8);
                // Called suits and rights: the start stays closed, so the root's announcements are
                // equal in both positions and only the field itself tells them apart (the pair
                // counts at a later trick start). The open start is the field itself.
                p.announce_open = field == 3;
                if p.trump == Some(t) {
                    p.trump = None;
                }
            });
            if field == 1 {
                let b = vary(&s, |p| p.called |= 1 << t as u8);
                differ[1] += check_pair(&s, &b, (0, 0)) as u32;
                continue;
            }
            if field == 2 {
                let b = vary(&s, |p| p.may_announce &= !(1 << x));
                differ[2] += check_pair(&s, &b, (0, 0)) as u32;
                continue;
            }
        }
        let mut other_suit = |t: Option<_>| {
            let pick = SUITS[rng.random_range(0..4)];
            if t == Some(pick) {
                None
            } else {
                Some(pick)
            }
        };
        let (b, teams) = match field {
            0 => {
                let t = other_suit(s.trump());
                (vary(&s, |p| p.trump = t), (0, 0))
            }
            1 => {
                let bit = 1u8 << rng.random_range(0..4);
                (vary(&s, |p| p.called ^= bit), (0, 0))
            }
            2 => {
                // Toggle the right of a seat that holds an uncalled pair, if any.
                let seat = (0..4u8)
                    .find(|&x| SUITS.iter().any(|&t| CardSet::halves(t).is_subset(s.hand(x))))
                    .unwrap_or(rng.random_range(0..4));
                (vary(&s, |p| p.may_announce ^= 1 << seat), (0, 0))
            }
            3 => (vary(&s, |p| p.announce_open = !p.announce_open), (0, 0)),
            _ => (s, (0, 1)),
        };
        if check_pair(&s, &b, teams) {
            differ[field] += 1;
        }
    }
    eprintln!("discriminating pairs / tries per field: {names:?} = {differ:?} / {tries:?}");
    for (name, n) in names.iter().zip(differ) {
        assert!(n >= 40, "too few discriminating pairs for {name}: {n}");
    }
}

#[test]
fn shared_solver_is_exact_over_random_public_variants() {
    // Many public variants of the same hands through one solver, in random order.
    let mut rng = StdRng::seed_from_u64(22);
    for _ in 0..60 {
        let base = position(&mut rng, 2, 0);
        let mut solver = Solver::with_tt_bits(12);
        for _ in 0..20 {
            let trump = [None, Some(SUITS[0]), Some(SUITS[1]), Some(SUITS[2]), Some(SUITS[3])][rng.random_range(0..5)];
            let s = vary(&base, |p| {
                p.trump = trump;
                p.called = rng.random_range(0..16u8) | trump.map_or(0, |t| 1 << t as u8);
                p.may_announce = rng.random_range(0..16);
                p.announce_open = rng.random_bool(0.5);
            });
            let team = rng.random_range(0..2);
            assert_eq!(solver.value(&s, team), brute(&s, team), "{s:?}");
            assert_eq!(solver.tricks(&s, team), brute_tricks(&s, team), "{s:?}");
        }
    }
}

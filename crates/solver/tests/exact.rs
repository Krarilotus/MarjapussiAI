//! Guard tests: the solver equals brute-force minimax on random positions (1â€“4 tricks left,
//! trick starts and mid-trick, announcements and trump rights included), and the engine's
//! `Game` played out by brute force (1â€“3 tricks left).

use engine::get_all_cards;
use engine::ActionType;
use engine::Game;
use engine::GameFinishedInfo;
use engine::GamePhase;
use engine::{CardSet, SUITS};
use engine::{FastState, PlayPhase, PlayerTrumpPossibilities, PublicState, CARDS, TRICKS};
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
            && (0..4).any(|x| s.rights(x).may_announce() && CardSet::halves(suit).is_subset(s.hand(x)))
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
        rights: std::array::from_fn(|_| {
            [
                PlayerTrumpPossibilities::Own,
                PlayerTrumpPossibilities::Own,
                PlayerTrumpPossibilities::Yours,
                PlayerTrumpPossibilities::Ours,
            ][rng.random_range(0..4)]
        }),
        phase: if extra == 0 && rng.random_bool(0.7) {
            PlayPhase::Start
        } else {
            PlayPhase::Cards
        },
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

/// Minimax over all engine play actions, including every question and answer;
/// scored by `GameFinishedInfo::team_points`.
fn game_minimax(game: &Game, team: u8) -> i32 {
    if game.ended() {
        return GameFinishedInfo::from(game.clone()).team_points[team as usize];
    }
    let legal = game.legal_actions();
    let values = legal
        .iter()
        .filter(|a| engine::Move::from_action(&a.action_type).is_some())
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
                p.rights[x as usize] = PlayerTrumpPossibilities::Own;
                p.called &= !(1 << t as u8);
                // Called suits and rights: the start stays closed, so the root's announcements are
                // equal in both positions and only the field itself tells them apart (the pair
                // counts at a later trick start). The open start is the field itself.
                p.phase = if field == 3 { PlayPhase::Start } else { PlayPhase::Cards };
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
                let b = vary(&s, |p| p.rights[x as usize] = PlayerTrumpPossibilities::Ours);
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
                (
                    vary(&s, |p| {
                        p.rights[seat as usize] =
                            p.rights[seat as usize].after_question(engine::QuestionType::YourHalf(SUITS[0]))
                    }),
                    (0, 0),
                )
            }
            3 => (
                vary(&s, |p| {
                    p.phase = if p.phase == PlayPhase::Start {
                        PlayPhase::Cards
                    } else {
                        PlayPhase::Start
                    }
                }),
                (0, 0),
            ),
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
                p.rights = std::array::from_fn(|_| {
                    [
                        PlayerTrumpPossibilities::Own,
                        PlayerTrumpPossibilities::Own,
                        PlayerTrumpPossibilities::Yours,
                        PlayerTrumpPossibilities::Ours,
                    ][rng.random_range(0..4)]
                });
                p.phase = if rng.random_bool(0.5) {
                    PlayPhase::Start
                } else {
                    PlayPhase::Cards
                };
            });
            let team = rng.random_range(0..2);
            assert_eq!(solver.value(&s, team), brute(&s, team), "{s:?}");
            assert_eq!(solver.tricks(&s, team), brute_tricks(&s, team), "{s:?}");
        }
    }
}

#[test]
fn partner_pair_requires_the_askers_pair_right_but_not_the_respondents() {
    use engine::{Card, Suit};
    let hands = ["e-6 g-6", "s-A s-Z", "r-K r-O", "g-A e-A"].map(|cards| {
        cards
            .split_whitespace()
            .map(|s| CardSet::of(&s.parse::<Card>().unwrap()))
            .fold(CardSet::EMPTY, |a, b| a | b)
    });
    let mut parts = FastState::new(hands, 0).parts();
    parts.public.tricks_done = 7;
    parts.public.tricks = [4, 3];
    parts.public.phase = PlayPhase::Start;
    parts.public.trump = Some(Suit::Bells);
    parts.public.called = 1 << Suit::Bells as u8;
    parts.public.rights = [PlayerTrumpPossibilities::Ours; 4];
    parts.public.rights[0] = PlayerTrumpPossibilities::Yours;
    let can_ask = FastState::from_parts(parts).unwrap();
    let cannot_ask = vary(&can_ask, |p| p.rights[0] = PlayerTrumpPossibilities::Ours);
    // Neither position permits an own announcement. Only Yours can reach the
    // partner's whole pair, turn its cards into winners and score the 100-point pair.
    let mut solver = Solver::with_tt_bits(8);
    for order in [[(can_ask, 170), (cannot_ask, 0)], [(cannot_ask, 0), (can_ask, 170)]] {
        for (s, expected) in order {
            assert_eq!(brute(&s, 0), expected);
            assert_eq!(
                solver.value(&s, 0),
                expected,
                "all rights levels belong in the cache key"
            );
        }
    }
}

#[test]
fn split_pair_question_changes_the_last_trick_only_for_an_uncalled_suit() {
    use engine::{AnswerType, Card, Move, QuestionType, Suit};
    let hands = ["r-K", "s-A", "r-O", "g-A"].map(|s| CardSet::of(&s.parse::<Card>().unwrap()));
    let mut solver = Solver::with_tt_bits(8);
    for already_called in [false, true] {
        let state = FastState::from_public(
            PublicState {
                trick: [0; 4],
                trick_len: 0,
                leader: 0,
                trump: Some(Suit::Bells),
                called: (1 << Suit::Bells as u8) | if already_called { 1 << Suit::Red as u8 } else { 0 },
                rights: [PlayerTrumpPossibilities::Ours; 4],
                phase: PlayPhase::Start,
                tricks_done: 8,
                tricks: [4, 4],
                card_points: [0; 2],
                pair_points: [0; 2],
            },
            hands,
        )
        .unwrap();
        let red = state.after(Move::Question(QuestionType::YourHalf(Suit::Red)));
        let green = state.after(Move::Question(QuestionType::YourHalf(Suit::Green)));
        let value = if already_called { 0 } else { 149 };
        for order in [[(red, value), (green, 0)], [(green, 0), (red, value)]] {
            for (s, expected) in order {
                assert_eq!(
                    solver.value(&s, 0),
                    expected,
                    "pending question must be in the cache key"
                );
                assert_eq!(brute(&s, 0), expected);
            }
        }
        assert_eq!(solver.value(&state, 0), value);
        assert!(
            solver.reaches(&state, 0, value),
            "split pair belongs in the upper bound"
        );
        let answered = red.after(Move::Answer(AnswerType::YesHalf(Suit::Red)));
        assert_eq!(
            answered.trump(),
            Some(if already_called { Suit::Bells } else { Suit::Red })
        );
        assert_eq!(answered.pair_points(0), if already_called { 0 } else { 100 });
        assert_eq!(answered.to_move(), 0);
        assert_eq!(answered.phase(), PlayPhase::Cards);
    }
}

/// Reuse bounds between order-preserving renamings of the zero-point ranks.
/// Both point and trick optima are checked independently by exhaustive minimax;
/// scoring ranks, pair cards, suit identities and public state remain unchanged.
#[test]
fn zero_point_rank_gaps_preserve_values_and_cached_bounds() {
    let mut rng = StdRng::seed_from_u64(0x6007_8009);
    let mut solver = Solver::with_tt_bits(10);
    let mut changed = 0;
    for i in 0..300usize {
        let original = position(&mut rng, 1 + (i % 3) as u8, 0);
        let mut parts = original.parts();
        for suit in SUITS {
            let base = suit as u8 * 9;
            let live = original.in_hands().0 >> base & 15;
            let old: Vec<_> = (0..4).filter(|r| live >> r & 1 != 0).collect();
            let mut new = vec![0, 1, 2, 3];
            new.shuffle(&mut rng);
            new.truncate(old.len());
            new.sort_unstable();
            for hand in &mut parts.hands {
                let ranks = hand.0 >> base & 15;
                hand.0 &= !(15 << base);
                for (&from, &to) in old.iter().zip(&new) {
                    if ranks >> from & 1 != 0 {
                        hand.0 |= 1 << (base + to);
                    }
                }
            }
        }
        let renamed = FastState::from_parts(parts).unwrap();
        changed += usize::from(original != renamed);
        for team in 0..2 {
            let points = brute(&original, team);
            let tricks = brute_tricks(&original, team);
            assert_eq!(brute(&renamed, team), points);
            assert_eq!(brute_tricks(&renamed, team), tricks);
            for s in [&original, &renamed, &original] {
                assert_eq!(solver.value(s, team), points, "{s:?}");
                for target in [points - 1, points, points + 1] {
                    assert_eq!(solver.reaches(s, team, target), points >= target);
                }
                assert_eq!(solver.tricks(s, team), tricks);
                for target in [tricks, tricks + 1] {
                    assert_eq!(solver.reaches_tricks(s, team, target), tricks >= target);
                }
            }
        }
    }
    assert!(changed >= 150, "too few renamed positions: {changed}");
}

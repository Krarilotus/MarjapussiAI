//! Guard test: the knowledge of every seat never contradicts the true hands.
//! Random legal play through the engine, including undos; checked after every event.

use engine::get_all_cards;
use engine::Game;
use engine::{hands_from_deck, partner, ActionType, CardSet, GameAction, SUITS};
use knowledge::Tracker;
use observation::{start_hand, SeatFilter};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

const GAMES: u64 = 3_000;

fn new_game(rng: &mut StdRng) -> Game {
    let mut deck = get_all_cards();
    deck.shuffle(rng);
    let hands = hands_from_deck(&deck);
    let names = ["S0", "S1", "S2", "S3"].map(String::from);
    Game::new("soundness".into(), names, Some(hands))
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

#[derive(Default)]
struct Coverage {
    undone: u32,
    yes_half_joint: u32,
    yes_half_only: u32,
    no_half: u32,
    no_pair: u32,
    yes_pair_or_announce: u32,
    hidden_pass: u32,
    standing: u32,
    standing_trump: u32,
    partner_entries: u32,
    dealt_certain: u32,
}

/// The fact queries K1–K5 (C26) against the true hands and the true dealt hands.
fn check_facts(k: &knowledge::Knowledge, x: u8, game: &Game, seed: u64, cov: &mut Coverage) {
    let truth = |y: u8| CardSet::from_cards(&game.state.players[y as usize].cards);
    let dealt = CardSet::from_cards(&game.info.player_start_cards[x as usize]);
    let mate = partner(x);
    // K5: the dealt hand.
    assert!(
        k.dealt_certain(x).is_subset(dealt) && dealt.is_subset(k.dealt_possible(x)),
        "seed {seed}: seat {} wrong about the dealt hand of {x}",
        k.me()
    );
    cov.dealt_certain += (k.dealt_certain(x) - k.certain(x)).len();
    // K6: the public facts equal the engine's.
    let state = &game.state;
    assert_eq!(
        k.rights(x),
        state.players[x as usize].trump,
        "seed {seed}: rights of {x}"
    );
    if x == 0 {
        assert_eq!(k.declarer(), state.declarer().map(|p| p.0), "seed {seed}: declarer");
        assert_eq!(k.value(), state.value.0, "seed {seed}: value");
        let winners: Vec<u8> = state.all_tricks.iter().map(|t| t.winner.0).collect();
        assert_eq!(k.trick_winners(), &winners[..], "seed {seed}: trick winners");
        if let Some(leader) = state.trick_leader() {
            assert_eq!(k.trick_leader(), leader.0, "seed {seed}: trick leader");
        }
        let points = engine::team_points(&state.all_tricks, &game.all_events).total();
        assert_eq!(k.team_points(), points, "seed {seed}: team points");
        // The public card-play state the solver builds its worlds on.
        if let Some(fast) = engine::FastState::from_game(game) {
            let public = fast.public();
            assert_eq!(
                k.public_state(state.player_at_turn.0, public.phase),
                public,
                "seed {seed}: public state"
            );
        }
        // The per-trick context: every lead in order, and the trump of the trick in progress.
        let mut leads: Vec<(u8, u8)> = state
            .all_tricks
            .iter()
            .map(|t| (t.leader.0, t.cards[0].index()))
            .collect();
        if let (Some(leader), Some(first)) = (state.trick_leader(), state.current_trick.first()) {
            leads.push((leader.0, first.index()));
        }
        let known: Vec<(u8, u8)> = k.leads().map(|l| (l.seat, l.card)).collect();
        assert_eq!(known, leads, "seed {seed}: leads");
        // (The engine keeps a finished trick's cards until the next lead.)
        if (1..4).contains(&state.current_trick.len()) {
            assert_eq!(
                k.leads().last().unwrap().trump,
                k.trump(),
                "seed {seed}: trump of the trick"
            );
        }
    }
    for s in SUITS {
        // K4: count bounds.
        let (lo, hi) = k.count_bounds(x, s);
        let n = (truth(x) & CardSet::suit(s)).len() as u8;
        assert!(
            lo <= n && n <= hi,
            "seed {seed}: count bounds of seat {x} in {s:?}: {lo}..{hi}, truth {n}"
        );
        // K2, K3: pairs, certain and possible.
        let halves = CardSet::halves(s);
        if let Some(known) = k.known_half(x, s) {
            assert_eq!(
                known,
                !(truth(x) & halves).is_empty(),
                "seed {seed}: half presence of seat {x} in {s:?}"
            );
        }
        let bit = 1u8 << s as u8;
        if k.is_called(s) || !(halves & k.played()).is_empty() {
            continue;
        }
        let tp = k.team_pairs(x);
        let split = !(halves & truth(x)).is_empty() && !(halves & truth(mate)).is_empty();
        for (held, pairs) in [
            (halves.is_subset(truth(x)), tp.mine),
            (halves.is_subset(truth(mate)), tp.yours),
            (split, tp.ours),
        ] {
            assert!(
                !held || pairs.possible & bit != 0,
                "seed {seed}: a held pair of seat {x} not possible"
            );
            assert!(
                held || pairs.certain & bit == 0,
                "seed {seed}: a certain pair of seat {x} not held"
            );
        }
    }
    // K1: standing cards win if led now.
    for c in k.standing(x).iter() {
        let suit = engine::Card::from_index(c).suit;
        let higher = (CardSet::suit(suit) - k.played())
            .iter()
            .filter(|&b| engine::takes(b, c, None))
            .fold(CardSet::EMPTY, |a, b| a | CardSet::single(b));
        let ruffable = k.trump().is_some_and(|t| {
            t != suit
                && [(x + 1) % 4, (x + 3) % 4]
                    .iter()
                    .any(|&o| (truth(o) & CardSet::suit(suit)).is_empty() && !(truth(o) & CardSet::suit(t)).is_empty())
        });
        assert!(
            truth(x).contains(c) && higher.is_subset(truth(x) | truth(mate)) && !ruffable,
            "seed {seed}: seat {} calls {c} of seat {x} standing",
            k.me()
        );
        cov.standing += 1;
        cov.standing_trump += (Some(suit) == k.trump()) as u32;
    }
    for c in k.retaining_lead(x).iter() {
        let suit = engine::suit_of(c);
        assert!(k.standing(x).contains(c));
        assert!(!(truth(mate) & CardSet::suit(suit))
            .iter()
            .any(|b| engine::takes(b, c, None)));
        assert!(!k.trump().is_some_and(|t| t != suit
            && (truth(mate) & CardSet::suit(suit)).is_empty()
            && !(truth(mate) & CardSet::suit(t)).is_empty()));
    }
    // A reported entry uses one fixed partner card against every legal defence.
    // Verify with the engine on the true deal, independently of the certificate.
    if x == k.me() && k.trick().is_empty() {
        if let Some(state) = engine::FastState::from_game(game).filter(|s| s.to_move() == x) {
            for lead in state.moves().filter_map(|m| match m {
                engine::Move::Card(c) => Some(c),
                _ => None,
            }) {
                for winner in k.partner_entry_winners(x, lead).iter() {
                    let led = state.after(engine::Move::Card(lead));
                    for left in led.moves() {
                        let reply = led.after(left);
                        let entry = engine::Move::Card(winner);
                        assert!(reply.moves().any(|m| m == entry), "seed {seed}: illegal partner entry");
                        let winning = reply.after(entry);
                        for right in winning.moves() {
                            assert_eq!(winning.after(right).leader(), mate, "seed {seed}: beaten partner entry");
                        }
                    }
                    cov.partner_entries += 1;
                }
            }
        }
    }
}

/// Knowledge facts as (certain cards of others at the last trick, events checked).
fn play_and_check(seed: u64, cov: &mut Coverage) -> (u32, u32) {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut game = new_game(&mut rng);
    let mut filters: Vec<SeatFilter> = (0..4).map(SeatFilter::new).collect();
    let mut trackers: Vec<Tracker> = (0..4).map(|s| Tracker::new(s, start_hand(&game, s))).collect();
    let mut events = 0;
    let mut last_trick_certain = 0;

    for _ in 0..600 {
        if game.ended() {
            break;
        }
        let action = pick_action(&game, &mut rng);
        game.apply_action_mut(action);
        let event = game.all_events.last().unwrap();
        for seat in 0..4u8 {
            let Some(seat_event) = filters[seat as usize].filter(event) else {
                continue;
            };
            if seat == 0 {
                match seat_event {
                    observation::SeatEvent::Undone => cov.undone += 1,
                    observation::SeatEvent::YesHalf {
                        asker_has_half: true, ..
                    } => cov.yes_half_joint += 1,
                    observation::SeatEvent::YesHalf {
                        asker_has_half: false, ..
                    } => cov.yes_half_only += 1,
                    observation::SeatEvent::NoHalf { .. } => cov.no_half += 1,
                    observation::SeatEvent::NoPair { .. } => cov.no_pair += 1,
                    observation::SeatEvent::YesPair { .. } | observation::SeatEvent::AnnounceTrump { .. } => {
                        cov.yes_pair_or_announce += 1
                    }
                    observation::SeatEvent::Pass { cards: None, .. } => cov.hidden_pass += 1,
                    _ => {}
                }
            }
            if let Err(e) = trackers[seat as usize].apply(&seat_event) {
                panic!("seed {seed}, seat {seat}: {e:?} after {seat_event:?}");
            }
        }
        for seat in 0..4u8 {
            let k = trackers[seat as usize].knowledge();
            for x in 0..4u8 {
                let truth = CardSet::from_cards(&game.state.players[x as usize].cards);
                assert!(
                    k.certain(x).is_subset(truth) && truth.is_subset(k.may_hold(x)),
                    "seed {seed}: seat {seat} wrong about seat {x}\n truth {truth:?}\n certain {:?}\n possible {:?}",
                    k.certain(x),
                    k.possible(x),
                );
                assert_eq!(k.hand_len(x) as u32, truth.len(), "seed {seed}: hand size of seat {x}");
                check_facts(k, x, &game, seed, cov);
            }
        }
        events += 1;
        if game.state.players.iter().all(|p| p.cards.len() == 1) {
            last_trick_certain = (0..4u8)
                .map(|s| {
                    let k = trackers[s as usize].knowledge();
                    (0..4u8).filter(|&x| x != s).map(|x| k.certain(x).len()).sum::<u32>()
                })
                .sum();
        }
    }
    (last_trick_certain, events)
}

#[test]
fn knowledge_is_sound_in_random_games() {
    let mut certain_at_last_trick = 0;
    let mut events = 0;
    let mut cov = Coverage::default();
    for seed in 0..GAMES {
        let (c, e) = play_and_check(seed, &mut cov);
        certain_at_last_trick += c;
        events += e;
    }
    let known_at_last_trick = certain_at_last_trick as f64 / (GAMES as f64 * 4.0);
    eprintln!(
        "{GAMES} games, {events} events checked x 4 seats; at the last trick a seat knows the owner of {known_at_last_trick:.2} of the 3 hidden cards on average"
    );
    // Information floor (guards S18/S19 against silent weakening; v0 measures 1.29).
    assert!(
        known_at_last_trick >= 1.2,
        "knowledge got weaker: {known_at_last_trick:.2}"
    );
    eprintln!(
        "coverage (seat 0 view): undone {}, joint YesHalf {}, YesHalf+OnlyHalf {}, NoHalf {}, NoPair {}, YesPair/Announce {}, hidden passes {}",
        cov.undone, cov.yes_half_joint, cov.yes_half_only, cov.no_half, cov.no_pair, cov.yes_pair_or_announce, cov.hidden_pass
    );
    for (name, n) in [
        ("undo", cov.undone),
        ("joint half", cov.yes_half_joint),
        ("only half", cov.yes_half_only),
        ("no half", cov.no_half),
        ("no pair", cov.no_pair),
        ("pair", cov.yes_pair_or_announce),
        ("hidden pass", cov.hidden_pass),
    ] {
        assert!(n >= 50, "too little coverage of {name}: {n}");
    }
    eprintln!(
        "facts: {} standing cards checked ({} trumps), {} dealt cards known beyond the current hands, {} partner entries",
        cov.standing, cov.standing_trump, cov.dealt_certain, cov.partner_entries
    );
    assert!(cov.standing_trump >= 50 && cov.dealt_certain >= 50);
    assert!(cov.partner_entries >= 50);
}

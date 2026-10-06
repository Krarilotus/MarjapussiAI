//! Guard test: the knowledge of every seat never contradicts the true hands.
//! Random legal play through the engine, including undos; checked after every event.

use engine::bits::CardSet;
use engine::game::cards::get_all_cards;
use engine::game::gameevent::ActionType;
use engine::game::Game;
use knowledge::Tracker;
use observation::{start_hand, SeatFilter};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

const GAMES: u64 = 3_000;

fn new_game(rng: &mut StdRng) -> Game {
    let mut deck = get_all_cards();
    deck.shuffle(rng);
    let hands = [deck[0..9].to_vec(), deck[9..18].to_vec(), deck[18..27].to_vec(), deck[27..36].to_vec()];
    let names = ["S0", "S1", "S2", "S3"].map(String::from);
    Game::new("soundness".into(), names, Some(hands))
}

fn pick_action(game: &Game, rng: &mut StdRng) -> engine::game::gameevent::GameAction {
    let legal = &game.legal_actions;
    let (undo, normal): (Vec<_>, Vec<_>) =
        legal.iter().partition(|a| matches!(a.action_type, ActionType::UndoRequest));
    if normal.is_empty() || (!undo.is_empty() && rng.random_bool(0.05)) {
        return legal[rng.random_range(0..legal.len())].clone();
    }
    normal[rng.random_range(0..normal.len())].clone()
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
            let Some(seat_event) = filters[seat as usize].filter(event) else { continue };
            if seat == 0 {
                match seat_event {
                    observation::SeatEvent::Undone => cov.undone += 1,
                    observation::SeatEvent::YesHalf { asker_has_half: true, .. } => cov.yes_half_joint += 1,
                    observation::SeatEvent::YesHalf { asker_has_half: false, .. } => cov.yes_half_only += 1,
                    observation::SeatEvent::NoHalf { .. } => cov.no_half += 1,
                    observation::SeatEvent::NoPair { .. } => cov.no_pair += 1,
                    observation::SeatEvent::YesPair { .. } | observation::SeatEvent::AnnounceTrump { .. } => cov.yes_pair_or_announce += 1,
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
    eprintln!(
        "{GAMES} games, {events} events checked x 4 seats; at the last trick a seat knows the owner of {:.2} of the 3 hidden cards on average",
        certain_at_last_trick as f64 / (GAMES as f64 * 4.0)
    );
    eprintln!(
        "coverage (seat 0 view): undone {}, joint YesHalf {}, YesHalf+OnlyHalf {}, NoHalf {}, NoPair {}, YesPair/Announce {}, hidden passes {}",
        cov.undone, cov.yes_half_joint, cov.yes_half_only, cov.no_half, cov.no_pair, cov.yes_pair_or_announce, cov.hidden_pass
    );
    for (name, n) in [("undo", cov.undone), ("joint half", cov.yes_half_joint), ("only half", cov.yes_half_only), ("no half", cov.no_half), ("no pair", cov.no_pair), ("pair", cov.yes_pair_or_announce), ("hidden pass", cov.hidden_pass)] {
        assert!(n >= 50, "too little coverage of {name}: {n}");
    }
}

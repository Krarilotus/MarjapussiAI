//! Guard tests for the deal sampler: every sample satisfies all facts; counts, marginals and
//! sample frequencies match full enumeration on small states; sound over random games.

use std::collections::HashMap;

use engine::bits::CardSet;
use engine::game::cards::get_all_cards;
use engine::game::gameevent::{ActionType, GameAction};
use engine::game::Game;
use knowledge::{DealSpace, Hands, Knowledge, Tracker};
use observation::SeatFilter;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

fn new_game(rng: &mut StdRng) -> Game {
    let mut deck = get_all_cards();
    deck.shuffle(rng);
    let hands = [
        deck[0..9].to_vec(),
        deck[9..18].to_vec(),
        deck[18..27].to_vec(),
        deck[27..36].to_vec(),
    ];
    Game::new("sample".into(), ["S0", "S1", "S2", "S3"].map(String::from), Some(hands))
}

fn pick_action(game: &Game, rng: &mut StdRng) -> GameAction {
    let legal = &game.legal_actions;
    let (undo, normal): (Vec<_>, Vec<_>) = legal
        .iter()
        .partition(|a| matches!(a.action_type, ActionType::UndoRequest));
    if normal.is_empty() || (!undo.is_empty() && rng.random_bool(0.05)) {
        return legal[rng.random_range(0..legal.len())].clone();
    }
    normal[rng.random_range(0..normal.len())].clone()
}

/// Plays one random game and calls `visit(tracker, true hands)` for every seat after every event.
fn for_each_state(seed: u64, mut visit: impl FnMut(&Tracker, &Hands)) {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut game = new_game(&mut rng);
    let mut filters: Vec<SeatFilter> = (0..4).map(SeatFilter::new).collect();
    let mut trackers: Vec<Tracker> = (0..4)
        .map(|s| Tracker::new(s, observation::start_hand(&game, s)))
        .collect();
    for _ in 0..600 {
        if game.ended() {
            break;
        }
        game.apply_action_mut(pick_action(&game, &mut rng));
        let event = game.all_events.last().unwrap();
        for seat in 0..4 {
            if let Some(e) = filters[seat].filter(event) {
                trackers[seat].apply(&e).unwrap();
            }
        }
        let truth: Hands = std::array::from_fn(|x| CardSet::from_cards(&game.state.players[x].cards));
        for t in &trackers {
            visit(t, &truth);
        }
    }
}

/// The facts, checked directly: certain ⊆ hand ⊆ may_hold, hand sizes, disjoint hands
/// covering the live cards, every constraint.
fn consistent(k: &Knowledge, hands: &Hands) -> bool {
    let mut all = CardSet::EMPTY;
    for x in 0..4u8 {
        let h = hands[x as usize];
        if !k.certain(x).is_subset(h) || !h.is_subset(k.may_hold(x)) || h.len() != k.hand_len(x) as u32 {
            return false;
        }
        if !(all & h).is_empty() {
            return false;
        }
        all |= h;
    }
    all == !k.played()
        && k.constraints().iter().all(|c| {
            let n = (hands[c.seat as usize] & c.mask).len() as u8;
            c.lo <= n && n <= c.hi
        })
}

fn unknown(k: &Knowledge) -> CardSet {
    let known = (0..4).fold(k.played(), |a, x| a | k.certain(x));
    !known
}

/// Every consistent deal, by brute force: each unknown card to any seat that may hold it.
fn enumerate(k: &Knowledge) -> Vec<Hands> {
    fn go(k: &Knowledge, cards: &[u8], hands: &mut Hands, out: &mut Vec<Hands>) {
        let Some((&c, more)) = cards.split_first() else {
            if consistent(k, hands) {
                out.push(*hands);
            }
            return;
        };
        for x in 0..4u8 {
            let h = hands[x as usize];
            if k.possible(x).contains(c) && h.len() < k.hand_len(x) as u32 {
                hands[x as usize] = h | CardSet::single(c);
                go(k, more, hands, out);
                hands[x as usize] = h;
            }
        }
    }
    let cards: Vec<u8> = unknown(k).iter().collect();
    let mut hands: Hands = std::array::from_fn(|x| k.certain(x as u8));
    let mut out = vec![];
    go(k, &cards, &mut hands, &mut out);
    out
}

/// A live constraint on a hidden seat still ties unknown cards (outer enumeration in use).
fn has_tied_cards(k: &Knowledge) -> bool {
    k.constraints()
        .iter()
        .any(|c| c.seat != k.me() && !(c.mask & k.possible(c.seat) & unknown(k)).is_empty())
}

#[test]
fn sampler_is_sound_in_random_games() {
    let mut rng = StdRng::seed_from_u64(7);
    let (mut states, mut tied) = (0u64, 0u64);
    let (mut possible_bits, mut exact_bits) = (0u64, 0u64);
    for seed in 0..300 {
        for_each_state(seed, |t, truth| {
            let k = t.knowledge();
            assert!(consistent(k, truth), "seed {seed}: the true deal breaks the facts");
            let space = DealSpace::new(k);
            let total = space.count();
            assert!(total >= 1, "seed {seed}: no deal although the true one is consistent");
            let counts = space.holder_counts();
            for c in (!k.played()).iter() {
                let sum: u64 = (0..4).map(|x| counts[x][c as usize]).sum();
                assert_eq!(sum, total, "seed {seed}: card {c} not held exactly once");
            }
            for x in 0..4 {
                let sum: u64 = counts[x].iter().sum();
                assert_eq!(sum, k.hand_len(x as u8) as u64 * total, "seed {seed}: seat {x} size");
                for c in truth[x].iter() {
                    assert!(counts[x][c as usize] > 0, "seed {seed}: true card ruled out");
                }
            }
            for _ in 0..3 {
                let hands = space.sample(&mut rng).unwrap();
                assert!(consistent(k, &hands), "seed {seed}: sample breaks the facts");
            }
            // The cached accessor is never stale (the first call after each event) and stays sound.
            let exact = t.exact_possible();
            assert_eq!(exact, space.exact_possible(), "seed {seed}: stale exact_possible cache");
            assert_eq!(t.exact_possible(), exact);
            for x in 0..4u8 {
                assert!(
                    exact[x as usize].is_subset(k.possible(x)),
                    "seed {seed}: exact adds a card"
                );
                possible_bits += k.possible(x).len() as u64;
                exact_bits += exact[x as usize].len() as u64;
            }
            states += 1;
            tied += has_tied_cards(k) as u64;
        });
    }
    eprintln!("{states} states checked, {tied} with constrained cards");
    eprintln!(
        "exact_possible removes {} of {possible_bits} possible bits ({:.3} %)",
        possible_bits - exact_bits,
        100.0 * (possible_bits - exact_bits) as f64 / possible_bits as f64
    );
    assert!(tied >= 1_000, "too little constraint coverage: {tied}");
}

#[test]
fn counts_and_marginals_match_enumeration() {
    let mut rng = StdRng::seed_from_u64(11);
    let (mut states, mut tied, mut deals) = (0u64, 0u64, 0u64);
    for seed in 0..200 {
        for_each_state(seed, |t, _| {
            let k = t.knowledge();
            if unknown(k).len() > 12 {
                return;
            }
            let all = enumerate(k);
            let space = DealSpace::new(k);
            assert_eq!(space.count(), all.len() as u64, "seed {seed}: count");
            let mut counts = [[0u64; 36]; 4];
            for hands in &all {
                for x in 0..4 {
                    for c in hands[x].iter() {
                        counts[x][c as usize] += 1;
                    }
                }
            }
            assert_eq!(space.holder_counts(), counts, "seed {seed}: holder counts");
            let exact: Hands = std::array::from_fn(|x| {
                (0..36u8)
                    .filter(|&c| (1..all.len() as u64).contains(&counts[x][c as usize]))
                    .fold(CardSet::EMPTY, |set, c| set | CardSet::single(c))
            });
            assert_eq!(space.exact_possible(), exact, "seed {seed}: exact possible sets");
            let marginals = space.marginals();
            for x in 0..4 {
                for c in 0..36 {
                    let exact = counts[x][c] as f64 / all.len() as f64;
                    assert!((marginals[x][c] - exact).abs() <= 1e-12, "seed {seed}: marginal");
                }
            }
            for _ in 0..5 {
                let hands = space.sample(&mut rng).unwrap();
                assert!(all.contains(&hands), "seed {seed}: sample outside the enumeration");
            }
            states += 1;
            tied += has_tied_cards(k) as u64;
            deals += all.len() as u64;
        });
    }
    eprintln!("{states} states with <= 12 unknown cards, {tied} with constrained cards, {deals} deals enumerated");
    assert!(tied >= 200, "too little constraint coverage: {tied}");
}

/// Chi-square of sample frequencies against the uniform distribution over the enumeration.
fn chi_square(space: &DealSpace, all: &[Hands], per_deal: u64, rng: &mut StdRng) -> (f64, f64) {
    let index: HashMap<Hands, usize> = all.iter().enumerate().map(|(i, h)| (*h, i)).collect();
    let mut seen = vec![0u64; all.len()];
    for _ in 0..per_deal * all.len() as u64 {
        seen[index[&space.sample(rng).unwrap()]] += 1;
    }
    let e = per_deal as f64;
    let chi2 = seen.iter().map(|&o| (o as f64 - e).powi(2) / e).sum();
    (chi2, (all.len() - 1) as f64)
}

#[test]
fn samples_are_exactly_uniform() {
    let mut rng = StdRng::seed_from_u64(13);
    let (mut plain, mut tied) = (vec![], vec![]);
    for seed in 0..400 {
        for_each_state(seed, |t, _| {
            let k = t.knowledge();
            if unknown(k).len() > 12 {
                return;
            }
            let n = DealSpace::new(k).count();
            let bucket = if has_tied_cards(k) { &mut tied } else { &mut plain };
            if (20..=400).contains(&n) && bucket.len() < 25 {
                bucket.push(*k);
            }
        });
    }
    assert_eq!((plain.len(), tied.len()), (25, 25), "not enough small states");
    let (mut chi2_sum, mut df_sum) = (0.0, 0.0);
    for k in plain.iter().chain(&tied) {
        let all = enumerate(k);
        let (chi2, df) = chi_square(&DealSpace::new(k), &all, 200, &mut rng);
        let z = (chi2 - df) / (2.0 * df).sqrt();
        assert!(z.abs() < 5.0, "{} deals: chi2 {chi2:.1}, df {df}, z {z:.2}", all.len());
        chi2_sum += chi2;
        df_sum += df;
    }
    let z = (chi2_sum - df_sum) / (2.0 * df_sum).sqrt();
    eprintln!("50 states, 200 samples per deal: chi2 {chi2_sum:.0} on {df_sum} df, z {z:.2}");
    assert!(z.abs() < 4.0, "sample frequencies not uniform: z {z:.2}");
}

#[test]
fn belief_weights_are_importance_weights() {
    // Belief: deals where the next seat holds the lowest unknown card count 3 times.
    let mut state = None;
    for_each_state(5, |t, _| {
        let k = t.knowledge();
        if state.is_none() && unknown(k).len() <= 12 && DealSpace::new(k).count() >= 100 {
            state = Some(*k);
        }
    });
    let k = state.unwrap();
    let card = unknown(&k).iter().next().unwrap();
    let seat = ((k.me() + 1) % 4) as usize;
    let belief = |h: &Hands| if h[seat].contains(card) { 3.0 } else { 1.0 };
    let all = enumerate(&k);
    let mass: f64 = all.iter().map(belief).sum();
    let exact = all.iter().filter(|h| h[seat].contains(card)).map(belief).sum::<f64>() / mass;

    let mut rng = StdRng::seed_from_u64(17);
    let worlds = DealSpace::new(&k).sample_worlds(&mut rng, 200_000, &belief);
    assert_eq!(worlds.len(), 200_000);
    assert!(worlds.iter().all(|w| w.weight == belief(&w.hands)));
    let total: f64 = worlds.iter().map(|w| w.weight).sum();
    let estimate = worlds
        .iter()
        .filter(|w| w.hands[seat].contains(card))
        .map(|w| w.weight)
        .sum::<f64>()
        / total;
    eprintln!("weighted P(card at seat): exact {exact:.4}, estimate {estimate:.4}");
    assert!((estimate - exact).abs() < 0.01, "exact {exact}, estimate {estimate}");
}

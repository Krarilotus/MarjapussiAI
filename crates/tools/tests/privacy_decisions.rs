//! Guard test (I11, spec §9.3): HP1's decisions do not change when two cards the deciding seat
//! cannot see are swapped in the deal (the `privacy_swap.rs` logic of `crates/knowledge`,
//! extended to `act()`).

use agent::Agent;
use engine::Card;
use engine::CardSet;
use engine::{ActionType, GameAction};
use eval::{deal, Rng, Table};
use heuristic::{HeuristicAgent, Params};

const REQUIRED_SWAPS: u32 = 500;

fn replay(hands: &[Vec<Card>; 4], actions: &[GameAction]) -> Option<Table> {
    let mut table = Table::new(hands);
    for a in actions {
        let (_, legal) = table.to_act()?;
        if !legal.contains(a) {
            return None;
        }
        table.apply(*a);
    }
    Some(table)
}

fn public_events(t: &Table) -> Vec<String> {
    t.game
        .all_events
        .iter()
        .map(|e| format!("{:?} {:?}", e.last_action, e.callback))
        .collect()
}

#[test]
fn hidden_cards_do_not_change_hp1_decisions() {
    // Keep comparisons complete: wall-clock interruption is not a hidden input.
    // Solver/worlds separately compares four-card hidden swaps without deadlines.
    let params = Params {
        endgame_tricks: 2,
        endgame_worlds: 8,
        ..Params::default()
    };
    let mut swaps = 0;
    for seed in 0..20_000u64 {
        if swaps >= REQUIRED_SWAPS {
            break;
        }
        let mut rng = Rng::new(seed);
        let hands = deal(seed);
        let mut table = Table::new(&hands);
        // Cheap legal histories suffice; the compared decisions below enable forecasts.
        let mut players: Vec<HeuristicAgent> = (0..4)
            .map(|_| {
                HeuristicAgent::new(Params {
                    return_worlds: 0,
                    ..params.clone()
                })
            })
            .collect();
        let mut actions = vec![];
        let stop = 1 + rng.below(50);
        while actions.len() < stop {
            let Some((seat, legal)) = table.to_act() else { break };
            let action = players[seat as usize].act(&table.view(seat), legal).action;
            table.apply(action);
            actions.push(action);
        }
        let Some((seat, legal)) = table.to_act() else { continue };
        let others: Vec<u8> = (0..4).filter(|&o| o != seat).collect();
        let i = rng.below(3);
        let (x, y) = (others[i], others[(i + 1 + rng.below(2)) % 3]);
        let touched = actions.iter().fold(CardSet::EMPTY, |set, a| match &a.action_type {
            ActionType::Pass(cards) => set | CardSet::from_cards(cards),
            ActionType::CardPlayed(card) => set | CardSet::of(card),
            _ => set,
        });
        let known = *table.view(seat).knowledge;
        let candidates = |from: u8, to: u8| -> Vec<Card> {
            hands[from as usize]
                .iter()
                .filter(|c| !touched.contains(c.index()) && known.may_hold(to).contains(c.index()))
                .cloned()
                .collect()
        };
        let (from_x, from_y) = (candidates(x, y), candidates(y, x));
        if from_x.is_empty() || from_y.is_empty() {
            continue;
        }
        let a = from_x[rng.below(from_x.len())];
        let b = from_y[rng.below(from_y.len())];
        let mut swapped = hands.clone();
        for card in swapped[x as usize].iter_mut().filter(|c| **c == a) {
            *card = b;
        }
        for card in swapped[y as usize].iter_mut().filter(|c| **c == b) {
            *card = a;
        }
        let Some(other) = replay(&swapped, &actions) else {
            continue;
        };
        if public_events(&other) != public_events(&table) {
            continue;
        }
        let Some((other_seat, other_legal)) = other.to_act() else {
            continue;
        };
        assert_eq!(
            (other_seat, &other_legal),
            (seat, &legal),
            "seed {seed}: legal actions leak"
        );
        let mine = HeuristicAgent::new(params.clone()).act(&table.view(seat), legal);
        let theirs = HeuristicAgent::new(params.clone()).act(&other.view(seat), other_legal);
        for d in [&mine, &theirs] {
            assert!(
                d.explanation
                    .facts
                    .iter()
                    .filter(|f| matches!(f.key, "complete" | "forecast_complete"))
                    .all(|f| f.value == 1.0),
                "seed {seed}: privacy comparison must finish"
            );
        }
        assert_eq!(mine, theirs, "seed {seed}: seat {seat} decides on hidden cards");
        swaps += 1;
    }
    assert!(swaps >= REQUIRED_SWAPS, "only {swaps} valid swaps");
}

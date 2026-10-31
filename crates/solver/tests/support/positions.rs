//! Fixed random-game decisions shared by timing and value-regression fixtures.
use engine::{get_all_cards, ActionType, FastState, Game, GamePhase, PublicState};
use knowledge::{Knowledge, Tracker};
use observation::{seat_events, start_hand};
use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};

pub fn decisions(k: usize, count: usize) -> Vec<(Knowledge, PublicState, usize, i32)> {
    let mut decisions = vec![];
    let mut seed = 0u64;
    while decisions.len() < count {
        seed += 1;
        let mut rng = StdRng::seed_from_u64(seed);
        let mut deck = get_all_cards();
        deck.shuffle(&mut rng);
        let hands = [0, 9, 18, 27].map(|i| deck[i..i + 9].to_vec());
        let mut game = Game::new("speed".into(), ["S0", "S1", "S2", "S3"].map(String::from), Some(hands));
        let stop = rng.random_range(0..4);
        while !game.ended() {
            let s = &game.state;
            let me = s.player_at_turn.0;
            let in_play = matches!(s.phase, GamePhase::Trick | GamePhase::StartTrick);
            if in_play && s.players[me as usize].cards.len() == k && s.current_trick.len() % 4 == stop {
                let mut tracker = Tracker::new(me, start_hand(&game, me));
                for e in seat_events(&game, me) {
                    tracker.apply(&e).unwrap();
                }
                let fast = FastState::from_game(&game).unwrap();
                decisions.push((
                    *tracker.knowledge(),
                    fast.public(),
                    fast.moves().count(),
                    fast.points(me % 2),
                ));
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
    }
    decisions
}

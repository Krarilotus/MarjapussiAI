// `Game` benchmarks, written as a macro over the engine crate so that the same code can be
// compiled against `main`'s engine for A/B runs (see docs/design/performance.md).

/// A deterministic xorshift for deals and action choice (no `rand` version coupling).
pub struct Rng(pub u64);

impl Rng {
    pub fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

/// Best of `reps` runs of `f`, in nanoseconds per `units`.
pub fn best_ns(reps: usize, units: u64, mut f: impl FnMut()) -> f64 {
    (0..reps)
        .map(|_| {
            let t = std::time::Instant::now();
            f();
            t.elapsed().as_nanos() as f64 / units as f64
        })
        .fold(f64::INFINITY, f64::min)
}

/// Expands to `scripts` and `run` for one engine crate; the caller imports `best_ns` and `Rng`.
#[macro_export]
macro_rules! game_bench {
    ($engine:ident) => {
        use std::hint::black_box;

        use $engine::Game;
        use $engine::{get_all_cards, Card};
        use $engine::{ActionType, GameAction};

        pub struct Script {
            hands: [Vec<Card>; 4],
            picks: Vec<usize>,
        }

        /// The `i`-th legal action, through a reused buffer (legal actions are computed on demand).
        fn nth(game: &Game, i: usize, buf: &mut Vec<GameAction>) -> GameAction {
            game.legal_actions_into(buf);
            buf[i]
        }

        fn new_game(hands: &[Vec<Card>; 4]) -> Game {
            Game::new(
                "bench".into(),
                ["S0", "S1", "S2", "S3"].map(String::from),
                Some(hands.clone()),
            )
        }

        /// Random games without undo, recorded as indices into `legal_actions`.
        pub fn scripts(n: usize, seed: u64) -> Vec<Script> {
            let mut rng = Rng(seed | 1);
            (0..n)
                .map(|_| {
                    let mut deck = get_all_cards();
                    for i in (1..deck.len()).rev() {
                        deck.swap(i, rng.below(i + 1));
                    }
                    let hands = [
                        deck[0..9].to_vec(),
                        deck[9..18].to_vec(),
                        deck[18..27].to_vec(),
                        deck[27..36].to_vec(),
                    ];
                    let mut game = new_game(&hands);
                    let mut picks = vec![];
                    while !game.ended() {
                        let legal = game.legal_actions();
                        let normal: Vec<usize> = (0..legal.len())
                            .filter(|&i| legal[i].action_type != ActionType::UndoRequest)
                            .collect();
                        let pick = normal[rng.below(normal.len())];
                        picks.push(pick);
                        game.apply_action_mut(legal[pick]);
                    }
                    Script { hands, picks }
                })
                .collect()
        }

        /// ns per action with `apply_action_mut`, ns per action with `apply_action`, ns per full
        /// game with `apply_action_mut`, ns per `legal_actions()` call (all phases, card play),
        /// ns per random card playout from the second trick (clone included).
        pub fn run(scripts: &[Script], reps: usize) -> [f64; 6] {
            let actions: u64 = scripts.iter().map(|s| s.picks.len() as u64).sum();
            let games = scripts.len() as u64;
            let mut total_mut = 0.0;
            let per_action_mut = best_ns(reps, actions, || {
                for s in scripts {
                    let mut game = new_game(&s.hands);
                    let mut buf = Vec::new();
                    for &p in &s.picks {
                        let a = nth(&game, p, &mut buf);
                        game.apply_action_mut(a);
                    }
                    black_box(&game);
                }
            });
            total_mut += per_action_mut * actions as f64 / games as f64;
            let per_action_imm = best_ns(reps, actions, || {
                for s in scripts {
                    let mut game = new_game(&s.hands);
                    let mut buf = Vec::new();
                    for &p in &s.picks {
                        let a = nth(&game, p, &mut buf);
                        game = game.apply_action(a).unwrap();
                    }
                    black_box(&game);
                }
            });
            // Snapshots of every state of the first games, for legal_actions().
            let mut all = vec![];
            let mut cards = vec![];
            for s in scripts.iter().take(50) {
                let mut game = new_game(&s.hands);
                let mut buf = Vec::new();
                for &p in &s.picks {
                    let a = nth(&game, p, &mut buf);
                    game.apply_action_mut(a);
                    if game.ended() {
                        break;
                    }
                    if game
                        .legal_actions()
                        .iter()
                        .all(|a| matches!(a.action_type, ActionType::CardPlayed(_) | ActionType::UndoRequest))
                    {
                        cards.push(game.clone());
                    }
                    all.push(game.clone());
                }
            }
            let legal_all = best_ns(reps, all.len() as u64, || {
                for g in &all {
                    black_box(g.legal_actions());
                }
            });
            let legal_cards = best_ns(reps, cards.len() as u64, || {
                for g in &cards {
                    black_box(g.legal_actions());
                }
            });
            let positions = playout_positions(scripts);
            let playout = best_ns(reps, positions.len() as u64, || {
                let mut rng = Rng(99);
                for g in &positions {
                    black_box(card_playout(g.clone(), &mut rng));
                }
            });
            [
                per_action_mut,
                per_action_imm,
                total_mut,
                legal_all,
                legal_cards,
                playout,
            ]
        }

        /// The state after the first trick (8 tricks left) of each script.
        pub fn playout_positions(scripts: &[Script]) -> Vec<Game> {
            scripts
                .iter()
                .filter_map(|s| {
                    let mut game = new_game(&s.hands);
                    let mut buf = Vec::new();
                    for &p in &s.picks {
                        if game.state.all_tricks.len() == 1 {
                            return Some(game);
                        }
                        let a = nth(&game, p, &mut buf);
                        game.apply_action_mut(a);
                    }
                    None
                })
                .collect()
        }

        /// Plays random legal cards (no questions) to the end; returns the tricks played.
        pub fn card_playout(mut game: Game, rng: &mut Rng) -> usize {
            while !game.ended() {
                let legal = game.legal_actions();
                let cards: Vec<usize> = (0..legal.len())
                    .filter(|&i| matches!(legal[i].action_type, ActionType::CardPlayed(_)))
                    .collect();
                let a = legal[cards[rng.below(cards.len())]];
                game.apply_action_mut(a);
            }
            game.state.all_tricks.len()
        }
    };
}

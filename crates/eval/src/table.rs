//! The table: runs agents on the engine. Only this module touches `Game`; every agent gets
//! exactly its seat's `SeatView` (its `SeatEvent`s and its `Knowledge`) and its legal actions.

pub use agent::Table;
use agent::{Agent, Decision, Explanation, SeatKind, SeatView};

use crate::detectors::{self, Flags};
use engine::GameFinishedInfo;
use engine::HAND_SIZE;
use engine::{get_all_cards, Card};
use engine::{ActionType, GameAction};
use observation::Seat;

/// SplitMix64: a tiny deterministic RNG for deals and the random player.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// A deal from a seed (Fisher–Yates over the engine's deck).
pub fn deal(seed: u64) -> [Vec<Card>; 4] {
    let mut deck = get_all_cards();
    let mut rng = Rng::new(seed);
    for i in (1..deck.len()).rev() {
        deck.swap(i, rng.below(i + 1));
    }
    [0, 1, 2, 3].map(|s| deck[s * HAND_SIZE..(s + 1) * HAND_SIZE].to_vec())
}

/// Uniform over the legal actions.
pub struct RandomAgent(pub Rng);

impl Agent for RandomAgent {
    fn act(&mut self, _view: &SeatView, legal: &[GameAction]) -> Decision {
        Decision {
            action: legal[self.0.below(legal.len())],
            explanation: Explanation {
                label: "random",
                facts: vec![],
            },
        }
    }
}

/// One agent for the auction, the exchange and the raise, another for the card play
/// (questions and answers included): baselines such as HP1's bidding with random play
/// (Judge S10), which isolate what each part contributes.
pub struct Composite {
    pub auction: Box<dyn Agent>,
    pub play: Box<dyn Agent>,
}

impl Agent for Composite {
    fn act(&mut self, view: &SeatView, legal: &[GameAction]) -> Decision {
        let auction = legal.iter().any(|a| {
            matches!(
                a.action_type,
                ActionType::NewBid(_) | ActionType::StopBidding | ActionType::Pass(_)
            )
        });
        if auction {
            self.auction.act(view, legal)
        } else {
            self.play.act(view, legal)
        }
    }

    fn kind(&self) -> SeatKind {
        self.auction.kind()
    }
}

/// One decision of a player during a game, for the logs.
#[derive(Clone, Debug)]
pub struct Logged {
    pub seat: Seat,
    pub decision: Decision,
    pub nanos: u128,
    /// The intention detectors at this decision (`detectors.rs`).
    pub flags: Flags,
}

/// Plays one game to the end. Returns the outcome and every decision.
pub fn play_game(hands: &[Vec<Card>; 4], players: &mut [Box<dyn Agent>]) -> (GameFinishedInfo, Vec<Logged>) {
    play_game_with(hands, players, |_, _, _| {})
}

/// `play_game`, calling `hook(table, seat, decision)` before each decision is applied.
pub fn play_game_with(
    hands: &[Vec<Card>; 4],
    players: &mut [Box<dyn Agent>],
    mut hook: impl FnMut(&Table, Seat, &Decision),
) -> (GameFinishedInfo, Vec<Logged>) {
    let mut table = Table::new(hands);
    table.kinds = std::array::from_fn(|s| players[s].kind());
    let mut log = Vec::with_capacity(64);
    while let Some((seat, legal)) = table.to_act() {
        let view = table.view(seat);
        let t0 = std::time::Instant::now();
        let decision = players[seat as usize].act(&view, legal);
        let nanos = t0.elapsed().as_nanos();
        let flags = detectors::flags(&table, seat, legal, &decision.action);
        hook(&table, seat, &decision);
        table.apply(decision.action);
        log.push(Logged {
            seat,
            decision,
            nanos,
            flags,
        });
    }
    (GameFinishedInfo::from(table.game), log)
}

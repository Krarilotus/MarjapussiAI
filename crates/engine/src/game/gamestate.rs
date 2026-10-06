use serde::Serialize;

use crate::game::bidding::NO_BID;
use crate::game::cards::{Card, Suit};
use crate::game::gameevent::ActionType;
use crate::game::player::{PlaceAtTable, Player};
use crate::game::points::Points;
use crate::inline::{Filler, InlineVec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GamePhase {
    WaitingForStart,
    Bidding,
    PassingForth,
    PassingBack,
    Raising,
    Trick,
    StartTrick,
    AnsweringPair,
    AnsweringHalf(Suit),
    Ended,
    PendingUndo(Box<GamePhase>),
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct FinishedTrick {
    /// In play order: `cards[i]` was played by `seat_of_card(i)`.
    pub cards: [Card; 4],
    /// The seat that led the trick.
    pub leader: PlaceAtTable,
    pub winner: PlaceAtTable,
    /// Card points, plus the last-trick bonus on the ninth trick.
    pub points: Points,
}

impl FinishedTrick {
    /// The seat that played `cards[i]`.
    pub fn seat_of_card(&self, i: usize) -> PlaceAtTable {
        PlaceAtTable((self.leader.0 + i as u8) % 4)
    }

    /// The seats in play order.
    pub fn seats(&self) -> [PlaceAtTable; 4] {
        [0, 1, 2, 3].map(|i| self.seat_of_card(i))
    }
}

impl Filler for FinishedTrick {
    const FILL: FinishedTrick = FinishedTrick {
        cards: [Card::FILL; 4],
        leader: PlaceAtTable(0),
        winner: PlaceAtTable(0),
        points: Points(0),
    };
}

/// The state of one game. All lists except the bidding history are inline (`InlineVec`), so
/// a clone (the undo snapshot on every bid and card) is one copy plus the bidding history.
#[derive(Debug)]
pub struct GameState {
    pub phase: GamePhase,
    pub started: bool,
    pub players_started: InlineVec<PlaceAtTable, 4>,
    pub players_accept_undo: InlineVec<PlaceAtTable, 4>,
    pub bidding_players: u8, //starts at 4
    pub bidding_history: Vec<(ActionType, PlaceAtTable)>,
    pub trump: Option<Suit>,
    pub trump_called: InlineVec<Suit, 4>,
    pub player_at_turn: PlaceAtTable,
    pub players: [Player; 4],
    pub value: Points,
    pub all_tricks: InlineVec<FinishedTrick, 9>,
    pub current_trick: InlineVec<Card, 4>,
}

impl Clone for GameState {
    fn clone(&self) -> Self {
        GameState {
            phase: self.phase.clone(),
            bidding_history: self.bidding_history.clone(),
            players: self.players.clone(),
            ..*self
        }
    }

    /// Reuses the bidding history's buffer: the undo snapshot is refreshed on every bid and card.
    fn clone_from(&mut self, source: &Self) {
        let mut history = std::mem::take(&mut self.bidding_history);
        history.clone_from(&source.bidding_history);
        *self = GameState {
            phase: source.phase.clone(),
            bidding_history: history,
            players: source.players.clone(),
            ..*source
        };
    }
}

impl GameState {
    /// The declarer: the player whose bid set the game value; `None` if all passed.
    pub fn declarer(&self) -> Option<PlaceAtTable> {
        if self.value == NO_BID {
            return None;
        }
        self.bidding_history
            .iter()
            .rev()
            .find(|(action, _)| matches!(action, ActionType::NewBid(_)))
            .map(|&(_, player)| player)
    }

    /// The declarer's team (team 0 = seats 0 and 2); `None` if all passed.
    pub fn playing_team(&self) -> Option<u8> {
        self.declarer().map(|p| p.0 % 2)
    }

    /// The seat that led the current trick, once its first card is played.
    pub fn trick_leader(&self) -> Option<PlaceAtTable> {
        let n = self.current_trick.len();
        (1..4)
            .contains(&n)
            .then(|| PlaceAtTable((self.player_at_turn.0 + 4 - n as u8) % 4))
    }

    pub fn create(players: [Player; 4]) -> Self {
        GameState {
            started: false,
            players_started: InlineVec::new(),
            players_accept_undo: InlineVec::new(),
            phase: GamePhase::WaitingForStart,
            trump: None,
            trump_called: InlineVec::new(),
            player_at_turn: PlaceAtTable(0),
            value: NO_BID,
            bidding_players: 4,
            bidding_history: vec![],
            players,
            all_tricks: InlineVec::new(),
            current_trick: InlineVec::new(),
        }
    }
    pub fn player_at_turn(&self) -> &Player {
        &self.players[self.player_at_turn.0 as usize]
    }

    pub fn player_at_turn_mut(&mut self) -> &mut Player {
        &mut self.players[self.player_at_turn.0 as usize]
    }

    pub fn partner(&self) -> &Player {
        &self.players[self.player_at_turn.partner().0 as usize]
    }

    pub fn partner_mut(&mut self) -> &mut Player {
        let partner_idx = self.player_at_turn.partner().0 as usize;
        &mut self.players[partner_idx]
    }

    pub fn prev_player(&self) -> &Player {
        &self.players[self.player_at_turn.prev().0 as usize]
    }

    pub fn player_at_place(&self, place: PlaceAtTable) -> &Player {
        &self.players[place.0 as usize]
    }

    pub fn player_at_place_mut(&mut self, place: PlaceAtTable) -> &mut Player {
        &mut self.players[place.0 as usize]
    }

    pub fn players_perspective(&self, place: PlaceAtTable) -> [String; 4] {
        [
            self.player_at_place(place).name.to_string(),
            self.player_at_place(place.next()).name.to_string(),
            self.player_at_place(place.partner()).name.to_string(),
            self.player_at_place(place.prev()).name.to_string(),
        ]
    }

    pub fn players_perspective_cards(&self, place: PlaceAtTable) -> [u8; 4] {
        [
            self.player_at_place(place).cards.len() as u8,
            self.player_at_place(place.next()).cards.len() as u8,
            self.player_at_place(place.partner()).cards.len() as u8,
            self.player_at_place(place.prev()).cards.len() as u8,
        ]
    }

    pub fn players_started(&self) -> Vec<String> {
        let mut started = vec![];
        for player in &self.players {
            if self.players_started.contains(&player.place_at_table) {
                started.push(player.name.to_string());
            }
        }
        started
    }
}

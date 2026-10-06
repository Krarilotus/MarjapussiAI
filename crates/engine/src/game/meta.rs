//! Facts about one game that are not rules: name, times, names, start hands.

use serde::Serialize;

use crate::game::cards::Card;
use crate::game::clock::current_time_string;
use crate::game::player::Player;

#[derive(Debug, Clone, Serialize)]
pub struct GameMetaInfo {
    pub name: String,
    pub create_time: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub player_names: [String; 4],
    pub player_start_cards: [Vec<Card>; 4],
}

impl GameMetaInfo {
    pub fn create(name: String, player_names: [String; 4], players: [Player; 4]) -> Self {
        GameMetaInfo {
            name,
            create_time: current_time_string(),
            start_time: None,
            end_time: None,
            player_names,
            player_start_cards: [
                players[0].cards.to_vec(),
                players[1].cards.to_vec(),
                players[2].cards.to_vec(),
                players[3].cards.to_vec(),
            ],
        }
    }
}

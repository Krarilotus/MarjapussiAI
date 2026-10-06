#![allow(unused)]

use crate::game::clock::current_time_string;
use crate::game::gameevent::GameAction;
use crate::game::gameinfo::GameInfoPlayer;
use crate::game::player::PlaceAtTable;
use crate::game::scoring::SeriesSettings;
use crate::game::table::Game;

struct Series {
    name: String,
    created: String,
    finished: Option<String>,
    num_of_games: u32,
    games: Vec<Game>,
    players_names: [String; 4],
    settings: SeriesSettings,
}

impl Series {
    pub fn new(name: String, players_names: [String; 4], num_of_games: u32, settings: Option<SeriesSettings>) -> Self {
        Series {
            name: name.clone(),
            created: current_time_string(),
            finished: None,
            players_names: players_names.clone(),
            num_of_games,
            games: vec![Game::new(name, players_names, None)],
            settings: settings.unwrap_or_default(),
        }
    }

    pub fn active_game_info(&mut self, player: PlaceAtTable) -> Option<GameInfoPlayer> {
        if self.games.len() >= (self.num_of_games as usize) {
            return None;
        } else if self.games.last().unwrap().clone().ended() {
            self.games
                .push(Game::new(self.name.clone(), self.players_names.clone(), None))
        }
        self.games
            .last()
            .map(|game| GameInfoPlayer::from_game(game.clone(), player))
    }
    pub fn active_game_apply(&mut self, action: GameAction) {
        if let Some(game) = self.games.last_mut() {
            game.apply_action_mut(action);
        }
    }
}

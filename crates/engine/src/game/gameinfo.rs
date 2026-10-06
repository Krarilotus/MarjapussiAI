use serde::Serialize;

use crate::game::cards::Card;
use crate::game::gameevent::{ActionType, GameEvent};
use crate::game::gamestate::{FinishedTrick, GamePhase};
use crate::game::meta::GameMetaInfo;
use crate::game::player::PlaceAtTable;
use crate::game::points::Points;
use crate::game::scoring::{game_scores, team_points, GameOutcome, SCHWARZ_FACTOR_FIFTHS};
use crate::game::table::Game;

/// Everything the database needs to know
#[derive(Debug, Clone, Serialize)]
pub struct GameFinishedInfo {
    pub info: GameMetaInfo,
    pub game_value: Points,
    /// None if no_one_played
    pub won: Option<bool>,
    pub no_one_played: bool,
    pub schwarz_game: bool,
    /// The declarer's party as seat 0 or 1 (team).
    pub playing_party: Option<PlaceAtTable>,
    /// The declarer (`GameState::declarer`).
    pub declarer: Option<PlaceAtTable>,
    pub after_passing: Option<[Vec<Card>; 4]>,
    pub passed_cards: Option<(Vec<Card>, Vec<Card>)>,
    pub bidding_history: Vec<(ActionType, PlaceAtTable)>,
    /// PlaceAtTable for who got the trick
    pub tricks: Vec<FinishedTrick>,
    pub all_events: Vec<GameEvent>,
    pub team_points: [i32; 2],
}

impl From<Game> for GameFinishedInfo {
    fn from(game: Game) -> Self {
        if game.state.phase != GamePhase::Ended {
            panic!("Cannot convert unfinished game!");
        }
        let no_one_played = game.state.declarer().is_none();

        let points = team_points(&game.state.all_tricks, &game.all_events);
        let schwarz_game = points.tricks.contains(&0);

        let mut playing_party: Option<PlaceAtTable> = None;
        let mut won: Option<bool> = None;
        let mut passed_cards: Option<(Vec<Card>, Vec<Card>)> = None;
        let mut after_passing: Option<[Vec<Card>; 4]> = None;
        if !no_one_played {
            let playing_player = game.state.declarer().expect("a bid was made");
            playing_party = Some(playing_player.party());
            let mut passed_forth: Option<Vec<Card>> = None;
            let mut passed_back: Option<Vec<Card>> = None;
            for event in &game.all_events {
                if let ActionType::Pass(cards) = event.last_action.action_type {
                    if passed_forth.is_none() {
                        passed_forth = Some(cards.to_vec());
                    } else {
                        passed_back = Some(cards.to_vec());
                    }
                }
            }
            passed_cards = Some((passed_forth.clone().unwrap(), passed_back.clone().unwrap()));

            let mut cards_after_passing = game.info.player_start_cards.clone();
            //partner cards
            cards_after_passing[playing_player.partner().0 as usize] = cards_after_passing
                [playing_player.partner().0 as usize]
                .clone()
                .into_iter()
                .filter(|c| !passed_forth.clone().unwrap().contains(c))
                .collect();
            cards_after_passing[playing_player.partner().0 as usize].append(&mut passed_back.clone().unwrap());
            // cards of playing player
            cards_after_passing[playing_player.0 as usize].append(&mut passed_forth.unwrap());
            cards_after_passing[playing_player.0 as usize] = cards_after_passing[playing_player.0 as usize]
                .clone()
                .into_iter()
                .filter(|c| !passed_back.clone().unwrap().contains(c))
                .collect();

            after_passing = Some(cards_after_passing);

            won = Some(points.total()[playing_party.unwrap().0 as usize] >= game.state.value.0);
        }

        // Announced pairs count in every game, all-pass games included (as on the site).
        let team_points = points.total();

        GameFinishedInfo {
            info: (*game.info).clone(),
            game_value: game.state.value,
            won,
            no_one_played,
            schwarz_game,
            playing_party,
            declarer: game.state.declarer(),
            after_passing,
            passed_cards,
            bidding_history: game.state.bidding_history,
            tricks: game.state.all_tricks.to_vec(),
            all_events: game.all_events,
            team_points,
        }
    }
}

impl GameFinishedInfo {
    pub fn set_times(&mut self, created: String, started: String, ended: String) {
        self.info.create_time = created;
        self.info.start_time = Some(started);
        self.info.end_time = Some(ended);
    }

    /// What scoring needs: the playing team, the game value, points and tricks per team.
    pub fn outcome(&self) -> GameOutcome {
        let mut team_tricks = [0u8; 2];
        for t in &self.tricks {
            team_tricks[(t.winner.0 % 2) as usize] += 1;
        }
        GameOutcome {
            playing_team: self.playing_party.map(|p| p.0 % 2),
            game_value: self.game_value.0,
            team_points: self.team_points,
            team_tricks,
        }
    }

    /// Score of the game for each team (team 0 = seats 0 and 2), by the rules page; see
    /// `game::scoring`.
    pub fn scores(&self) -> [i32; 2] {
        self.scores_with(SCHWARZ_FACTOR_FIFTHS)
    }

    /// `scores` with a series' schwarz factor (in fifths of the game value).
    pub fn scores_with(&self, schwarz_factor_fifths: i32) -> [i32; 2] {
        game_scores(&self.outcome(), schwarz_factor_fifths)
    }
}

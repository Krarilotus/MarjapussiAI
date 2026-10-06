//! Shared helpers for the engine's guard tests: seeded deals, a stable digest, and a text
//! rendering of everything observable about a `Game` except wall-clock times.
#![allow(dead_code)]

use std::fmt::Write;

use marjapussi_engine::game::cards::{get_all_cards, Card};
use marjapussi_engine::game::gameevent::{ActionType, GameAction, GameEvent};
use marjapussi_engine::game::gameinfo::GameFinishedInfo;
use marjapussi_engine::game::Game;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::Rng;

/// FNV-1a 64 over strings, with a separator byte so that ("ab","c") != ("a","bc").
pub struct Digest(pub u64);

impl Digest {
    pub fn new() -> Digest {
        Digest(0xcbf2_9ce4_8422_2325)
    }

    pub fn add(&mut self, s: &str) {
        for b in s.bytes().chain(std::iter::once(0xff)) {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

pub fn deal(rng: &mut StdRng) -> [Vec<Card>; 4] {
    let mut deck = get_all_cards();
    deck.shuffle(rng);
    [
        deck[0..9].to_vec(),
        deck[9..18].to_vec(),
        deck[18..27].to_vec(),
        deck[27..36].to_vec(),
    ]
}

pub fn new_game(rng: &mut StdRng) -> Game {
    Game::new(
        "guard".into(),
        ["S0", "S1", "S2", "S3"].map(String::from),
        Some(deal(rng)),
    )
}

/// A random legal action; undo requests with probability 5 % when other actions exist.
pub fn pick_action(game: &Game, rng: &mut StdRng) -> GameAction {
    let legal = &game.legal_actions;
    let normal: Vec<&GameAction> = legal
        .iter()
        .filter(|a| !matches!(a.action_type, ActionType::UndoRequest))
        .collect();
    if normal.is_empty() || (normal.len() < legal.len() && rng.random_bool(0.05)) {
        return legal[rng.random_range(0..legal.len())].clone();
    }
    normal[rng.random_range(0..normal.len())].clone()
}

/// Everything observable in the game state, legal actions included, without times.
pub fn state_line(game: &Game) -> String {
    let s = &game.state;
    let mut out = String::new();
    write!(
        out,
        "phase {:?} started {} ps {:?} pau {:?} bp {} bh {:?} trump {:?} called {:?} turn {:?} value {:?} tricks {:?} cur {:?}",
        s.phase,
        s.started,
        s.players_started,
        s.players_accept_undo,
        s.bidding_players,
        s.bidding_history,
        s.trump,
        s.trump_called,
        s.player_at_turn,
        s.value,
        s.all_tricks,
        s.current_trick,
    )
    .unwrap();
    for p in &s.players {
        write!(
            out,
            " | {} {:?} {:?} {:?} {:?} {} {:?}",
            p.name, p.place_at_table, p.partner, p.next_player, p.cards, p.bidding, p.trump
        )
        .unwrap();
        assert!(p.tricks.is_empty());
    }
    write!(
        out,
        " | legal {:?} | undo {:?}",
        game.legal_actions,
        game.last_state.as_ref().map(|l| (l.phase.clone(), l.player_at_turn.clone()))
    )
    .unwrap();
    out
}

pub fn event_line(e: &GameEvent) -> String {
    format!("{:?} {:?} {:?}", e.last_action, e.callback, e.player_at_turn)
}

pub fn finished_line(f: &GameFinishedInfo) -> String {
    let mut out = format!(
        "{} {:?} {:?} | value {:?} won {:?} none {} schwarz {} party {:?} after {:?} passed {:?} bh {:?} tricks {:?} team {:?}",
        f.info.name,
        f.info.player_names,
        f.info.player_start_cards,
        f.game_value,
        f.won,
        f.no_one_played,
        f.schwarz_game,
        f.playing_party,
        f.after_passing,
        f.passed_cards,
        f.bidding_history,
        f.tricks,
        f.team_points,
    );
    for e in &f.all_events {
        out.push_str(&event_line(e));
    }
    out
}

/// `GameFinishedInfo::scores` as reviewed on feat/heuristic-a (264fa3d), verbatim logic.
pub fn scores_264fa3d(info: &GameFinishedInfo) -> [i32; 2] {
    let rounded = |points: i32| (points + 2) / 5 * 5;
    let Some(playing) = &info.playing_party else {
        return info.team_points.map(rounded);
    };
    let (s, n) = (playing.0 as usize % 2, 1 - playing.0 as usize % 2);
    let won = info.won.unwrap_or(false);
    let value = info.game_value.0;
    let n_tricks = info.tricks.iter().filter(|t| t.winner.0 as usize % 2 == n).count();
    let mut scores = [0; 2];
    scores[s] = if won { value } else { -value };
    scores[n] = if n_tricks == 0 && won { -2 * value } else { rounded(info.team_points[n]) };
    scores
}

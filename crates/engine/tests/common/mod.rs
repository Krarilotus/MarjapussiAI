//! Shared helpers for the engine's guard tests: seeded deals, a stable digest, and a text
//! rendering of everything observable about a `Game` except wall-clock times.
#![allow(dead_code)]

use std::fmt::Write;

use marjapussi_engine::game::cards::{get_all_cards, Card};
use marjapussi_engine::game::gameevent::{ActionType, GameAction, GameEvent};
use marjapussi_engine::game::gameinfo::GameFinishedInfo;
use marjapussi_engine::game::gamestate::FinishedTrick;
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
    let legal = &game.legal_actions();
    let normal: Vec<&GameAction> = legal
        .iter()
        .filter(|a| !matches!(a.action_type, ActionType::UndoRequest))
        .collect();
    if normal.is_empty() || (normal.len() < legal.len() && rng.random_bool(0.05)) {
        return legal[rng.random_range(0..legal.len())];
    }
    *normal[rng.random_range(0..normal.len())]
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
        Tricks(&s.all_tricks),
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
    }
    write!(
        out,
        " | legal {:?} | undo {:?}",
        game.legal_actions(),
        game.last_state.as_ref().map(|l| (l.phase.clone(), l.player_at_turn))
    )
    .unwrap();
    out
}

/// Tricks in the Debug format of main 68c98f0 (without the `leader` field added in E5), so the
/// pinned digests compare the same facts.
pub struct Tricks<'a>(pub &'a [FinishedTrick]);

impl std::fmt::Debug for Tricks<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let items: Vec<String> = self
            .0
            .iter()
            .map(|t| {
                format!(
                    "FinishedTrick {{ cards: {:?}, winner: {:?}, points: {:?} }}",
                    t.cards, t.winner, t.points
                )
            })
            .collect();
        write!(f, "[{}]", items.join(", "))
    }
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
        Tricks(&f.tricks),
        f.team_points,
    );
    for e in &f.all_events {
        out.push_str(&event_line(e));
    }
    out
}

/// The site's per-game scoring (`marjapussi-online` `SeriesTable.vue`), ported line by line,
/// on a `GameFinishedInfo`. The source of truth for scoring (owner decision D2).
pub fn site_scores(info: &GameFinishedInfo) -> [i32; 2] {
    let mut value = info.game_value.0;
    if value == 115 {
        value = 0;
    }
    let playing = info.playing_party.map(|p| (p.0 % 2) as usize);
    let points = info.team_points;
    let round = |p: i32| (p + 2).div_euclid(5) * 5;
    let mut gets = [round(points[0]), round(points[1])];
    for t in 0..2 {
        if playing == Some(t) {
            gets[t] = if points[t] >= value { value } else { -value };
        }
    }
    if points[0] == 0 && info.schwarz_game && value >= 120 && gets[1] > 0 {
        gets[0] = -10 * value / 5;
    }
    if points[1] == 0 && info.schwarz_game && value >= 120 && gets[0] > 0 {
        gets[1] = -10 * value / 5;
    }
    gets
}

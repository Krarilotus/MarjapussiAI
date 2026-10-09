//! A game runner that keeps each seat's observations and legal actions in step.
//! Shared by live/evaluation tables and hypothetical policy rollouts; the policy
//! receives `SeatView`, never the simulated deal.

use crate::{SeatKind, SeatView, SeriesScore};
use engine::{ActionType, Card, CardSet, Game, GameAction};
use knowledge::Tracker;
use observation::{start_hand, Seat, SeatEvent, SeatFilter};

struct SeatState {
    filter: SeatFilter,
    tracker: Tracker,
    events: Vec<SeatEvent>,
}

/// A game plus each seat's private view, kept in step event by event.
pub struct Table {
    pub game: Game,
    seats: Vec<SeatState>,
    seen: usize,
    pub series: SeriesScore,
    /// Who plays each seat (public metadata, P3).
    pub kinds: [SeatKind; 4],
    /// The engine's legal actions now, refilled once per event (borrowed by `to_act`).
    legal: Vec<GameAction>,
}

impl Table {
    pub fn new(hands: &[Vec<Card>; 4]) -> Table {
        let mut game = Game::new(
            "table".into(),
            ["S0", "S1", "S2", "S3"].map(String::from),
            Some(hands.clone()),
        );
        for _ in 0..4 {
            let start = game.legal_actions()[0];
            game.apply_action_mut(start);
        }
        let seats = (0..4u8)
            .map(|s| SeatState {
                filter: SeatFilter::new(s),
                tracker: Tracker::new(s, start_hand(&game, s)),
                events: vec![],
            })
            .collect();
        let mut table = Table {
            game,
            seats,
            seen: 0,
            series: SeriesScore::default(),
            kinds: [SeatKind::Unknown; 4],
            legal: Vec::with_capacity(64),
        };
        table.sync();
        table
    }

    fn sync(&mut self) {
        for event in &self.game.all_events[self.seen..] {
            for st in &mut self.seats {
                if let Some(e) = st.filter.filter(event) {
                    st.tracker
                        .apply(&e)
                        .unwrap_or_else(|c| panic!("knowledge contradiction {c:?}"));
                    st.events.push(e);
                }
            }
        }
        self.seen = self.game.all_events.len();
        self.game.legal_actions_into(&mut self.legal);
    }

    /// The seat to act and its legal actions, borrowed (no copy per call, Judge J13 P8). The
    /// engine lists only the acting seat's actions, plus at most one trailing undo request,
    /// which agents never use.
    pub fn to_act(&self) -> Option<(Seat, &[GameAction])> {
        if self.game.ended() {
            return None;
        }
        let seat = self.game.state.player_at_turn.0;
        let all = &self.legal[..];
        let legal = match all.last() {
            Some(a) if a.action_type == ActionType::UndoRequest => &all[..all.len() - 1],
            _ => all,
        };
        debug_assert!(legal.iter().all(|a| a.player.0 == seat));
        Some((seat, legal))
    }

    pub fn view(&self, seat: Seat) -> SeatView<'_> {
        let st = &self.seats[seat as usize];
        SeatView {
            seat,
            knowledge: st.tracker.knowledge(),
            events: &st.events,
            series: self.series,
            kinds: self.kinds,
        }
    }

    pub fn apply(&mut self, action: GameAction) {
        self.game.apply_action_mut(action);
        self.sync();
    }

    pub fn hand(&self, seat: Seat) -> CardSet {
        CardSet::from_cards(&self.game.state.players[seat as usize].cards)
    }
}

//! The interface of every AI player (Judge M1 item 8): what an agent sees, and what it returns.
//!
//! An agent decides from one seat's view only: its `Knowledge` (hard facts) and its
//! `SeatEvent`s, plus the engine's legal actions for that seat. It never sees the engine's full
//! game state. Implementations: the heuristic player, random play, and later the search agent.

mod table;
pub use table::Table;

use engine::GameAction;
use engine::SeriesTally;
use knowledge::Knowledge;
use observation::{Seat, SeatEvent};

/// Both teams' series totals (team 0 = seats 0 and 2) and the games left: public. A view of
/// the engine's `game::scoring::SeriesTally`, the one owner of series scoring (Judge J17 item 3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeriesScore {
    pub team_totals: [i32; 2],
    /// Games left in a tournament round; `None` when playing to a target total.
    pub games_left: Option<u32>,
}

impl From<&SeriesTally> for SeriesScore {
    fn from(tally: &SeriesTally) -> SeriesScore {
        SeriesScore {
            team_totals: tally.totals(),
            games_left: tally.games_left(),
        }
    }
}

/// Who plays a seat: public metadata of the table (M3 P3), like the names on the site. It says
/// how a seat's actions are read (another HP1 with HP1's own policy, a human with the
/// human-calibrated conventions), never anything about its cards.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SeatKind {
    #[default]
    Unknown,
    Human,
    /// The heuristic player HP1.
    Heuristic,
    /// Any other program (random play, search agents).
    Program,
}

/// What one seat knows: its knowledge and its event stream, plus the public table metadata.
#[derive(Clone, Copy, Debug)]
pub struct SeatView<'a> {
    pub seat: Seat,
    pub knowledge: &'a Knowledge,
    pub events: &'a [SeatEvent],
    pub series: SeriesScore,
    /// Who plays each seat (public).
    pub kinds: [SeatKind; 4],
}

/// One fact a decision used, for logs and explanations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fact {
    pub key: &'static str,
    pub value: f32,
}

/// Why an agent chose its action: a short label (e.g. a rule ID) and the facts it used.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Explanation {
    pub label: &'static str,
    pub facts: Vec<Fact>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub action: GameAction,
    pub explanation: Explanation,
}

/// One interface for "how an AI acts".
pub trait Agent {
    /// `legal`: the engine's legal actions for this seat (computed from its own hand and the
    /// public state only).
    fn act(&mut self, view: &SeatView, legal: &[GameAction]) -> Decision;

    /// What the table shows about this agent (P3).
    fn kind(&self) -> SeatKind {
        SeatKind::Program
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{SeriesLength, SeriesSettings};

    #[test]
    fn the_series_score_is_read_from_the_engine_tally() {
        let mut tally = SeriesTally::new(SeriesSettings::default());
        tally.add_scores([120, -35]);
        assert_eq!(
            SeriesScore::from(&tally),
            SeriesScore {
                team_totals: [120, -35],
                games_left: Some(3),
            }
        );
        let to_500 = SeriesTally::new(SeriesSettings {
            length: SeriesLength::UntilPoints(500),
            ..SeriesSettings::default()
        });
        assert_eq!(SeriesScore::from(&to_500).games_left, None);
    }
}

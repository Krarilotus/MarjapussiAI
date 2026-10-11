//! Qualification records are replayed against engine legal actions, never parsed as rules.

use agent::{SeatKind, Table};
use engine::{GameAction, GameFinishedInfo};
use eval::{
    matchstats::{MatchObserver, MatchStats},
    suite::DealSuite,
};
use serde_json::Value;

/// Verified original record plus canonical engine actions. True deals stay in this
/// measurement layer; policies receive only `Table::view`.
pub struct Fixture {
    pub index: u64,
    pub source: Value,
    pub actions: Vec<GameAction>,
    pub phases: Vec<&'static str>,
    pub original: MatchStats,
    suite: DealSuite,
}

pub(super) fn identity(index: u64, row: &Value, suite: &DealSuite) -> Result<(), String> {
    if index >= suite.deals.saturating_mul(4)
        || row["deal"].as_u64() != Some(index / 4)
        || row["deal_seed"].as_u64() != Some(suite.deal_seed(index / 4))
        || row["new_team"].as_u64() != Some(index % 2)
        || row["turned"].as_bool() != Some(index % 4 >= 2)
    {
        return Err(format!("game {index}: qualification identity mismatch"));
    }
    Ok(())
}

fn start(index: u64, suite: &DealSuite) -> Table {
    let mut hands = suite.hands(index / 4);
    if index % 4 >= 2 {
        hands.rotate_left(2);
    }
    let mut table = Table::new(&hands);
    table.kinds = [SeatKind::Heuristic; 4];
    table
}

impl Fixture {
    /// The qualification format has implicit ordered indices and Debug actions.
    /// Reject a wrong actor, side, step, illegal action or mismatching final outcome.
    pub fn from_qualification(index: u64, source: Value, suite: DealSuite) -> Result<Self, String> {
        identity(index, &source, &suite)?;
        let rows = source["decisions"].as_array().ok_or("missing decisions")?;
        if rows.is_empty() || rows.len() > 128 {
            return Err(format!("game {index}: expected 1..128 decisions"));
        }
        let mut table = start(index, &suite);
        let mut actions = Vec::with_capacity(rows.len());
        let mut phases = Vec::with_capacity(rows.len());
        let mut observer = MatchObserver::default();
        for (step, row) in rows.iter().enumerate() {
            let (seat, legal) = table
                .to_act()
                .ok_or_else(|| format!("game {index}: early end at {step}"))?;
            let side = if u64::from(seat % 2) == index % 2 { "new" } else { "old" };
            if row["step"].as_u64() != Some(step as u64)
                || row["seat"].as_u64() != Some(u64::from(seat))
                || row["side"].as_str() != Some(side)
            {
                return Err(format!("game {index}: wrong actor/side/step at {step}"));
            }
            let text = row["action"].as_str().ok_or("missing Debug action")?;
            let action = *legal
                .iter()
                .find(|a| format!("{a:?}") == text)
                .ok_or_else(|| format!("game {index}: illegal or unknown action at {step}: {text}"))?;
            observer.observe(&table);
            phases.push(crate::selfplay::phase(&table.game.state.phase));
            table.apply(action);
            actions.push(action);
        }
        if !table.game.ended() {
            return Err(format!("game {index}: incomplete original game"));
        }
        let info = GameFinishedInfo::from(table.game);
        for (key, expected) in [
            ("scores", serde_json::json!(info.scores())),
            ("team_points", serde_json::json!(info.team_points)),
            ("contract", serde_json::json!(info.game_value.0)),
            ("declarer", serde_json::json!(info.declarer.map(|s| s.0))),
            ("made", serde_json::json!(info.won)),
            ("all_passed", serde_json::json!(info.no_one_played)),
            ("schwarz", serde_json::json!(info.schwarz_game)),
            ("tricks", serde_json::json!(info.tricks.len())),
        ] {
            if source[key] != expected {
                return Err(format!("game {index}: original outcome mismatch for {key}"));
            }
        }
        Ok(Self {
            index,
            source,
            actions,
            phases,
            original: observer.finish(&info),
            suite,
        })
    }

    /// Exactly `step` original decisions have been applied (START events are excluded).
    pub fn prefix(&self, step: usize) -> Result<Table, String> {
        Ok(self.prefix_observed(step)?.0)
    }

    pub(super) fn prefix_observed(&self, step: usize) -> Result<(Table, MatchObserver), String> {
        if step > self.actions.len() {
            return Err(format!("game {}: prefix {step} outside record", self.index));
        }
        let mut table = start(self.index, &self.suite);
        let mut observer = MatchObserver::default();
        for &action in &self.actions[..step] {
            observer.observe(&table);
            table.apply(action);
        }
        Ok((table, observer))
    }
}

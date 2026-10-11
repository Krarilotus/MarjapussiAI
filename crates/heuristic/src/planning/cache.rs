//! Reuse decisions only when the acting seat has exactly the same information.
//! This cache belongs to one forecast, whose policy parameters never change.

use std::collections::HashMap;

use agent::{SeatKind, SeatView, SeriesScore};
use engine::GameAction;
use knowledge::Knowledge;
use observation::SeatEvent;

use crate::{Decision, HeuristicAgent};

struct Observation {
    knowledge: Knowledge,
    events: Vec<SeatEvent>,
    kinds: [SeatKind; 4],
    series: SeriesScore,
    legal: Vec<GameAction>,
    decision: Decision,
}

impl Observation {
    fn matches(&self, view: &SeatView, legal: &[GameAction]) -> bool {
        self.knowledge == *view.knowledge
            && self.events == view.events
            && self.kinds == view.kinds
            && self.series == view.series
            && self.legal == legal
    }
}

#[derive(Default)]
pub(super) struct Decisions {
    // A cheap bucket key; complete equality above resolves every collision.
    entries: HashMap<(u8, u64, usize), Vec<Observation>>,
    count: usize,
    pub hits: u32,
    pub evaluated: u32,
}

impl Decisions {
    pub fn decide(&mut self, player: &mut HeuristicAgent, view: &SeatView, legal: &[GameAction]) -> Decision {
        let _timing = beliefs::profile::scope("forecast.cached_decision");
        let key = (view.seat, view.knowledge.certain(view.seat).0, view.events.len());
        // The endgame uses a wall-clock limit: even a fallback may have depended
        // on an interrupted solve. Do not memoize decisions in that region.
        let reusable = player.intent.borrow().is_none()
            && view.knowledge.hand_len(view.seat) as u32 > player.params.endgame_tricks;
        if reusable {
            if let Some(cached) = self
                .entries
                .get(&key)
                .and_then(|xs| xs.iter().find(|x| x.matches(view, legal)))
            {
                self.hits += 1;
                return cached.decision.clone();
            }
        }
        self.evaluated += 1;
        let decision = player.decide_before(view, legal, None);
        if reusable && self.count < 4096 {
            self.entries.entry(key).or_default().push(Observation {
                knowledge: *view.knowledge,
                events: view.events.to_vec(),
                kinds: view.kinds,
                series: view.series,
                legal: legal.to_vec(),
                decision: decision.clone(),
            });
            self.count += 1;
        }
        decision
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Params;
    use agent::Table;

    fn table() -> Table {
        let hands = std::array::from_fn(|s| (0..9).map(|c| engine::Card::from_index((s * 9 + c) as u8)).collect());
        Table::new(&hands)
    }

    fn player() -> HeuristicAgent {
        HeuristicAgent::new(Params {
            return_worlds: 0,
            plan_worlds: 0,
            endgame_tricks: 0,
            ..Params::default()
        })
    }

    #[test]
    fn reuse_requires_identical_visible_history_metadata_and_legal_actions() {
        let table = table();
        let (seat, legal) = table.to_act().unwrap();
        let mut view = table.view(seat);
        let mut cache = Decisions::default();
        let mut agent = player();
        let first = cache.decide(&mut agent, &view, legal);
        assert_eq!(cache.decide(&mut agent, &view, legal), first);
        assert_eq!(cache.hits, 1);
        view.series.team_totals[0] = 490;
        cache.decide(&mut agent, &view, legal);
        view.kinds[1] = SeatKind::Human;
        cache.decide(&mut agent, &view, legal);
        cache.decide(&mut agent, &view, &legal[..1]);
        assert_eq!(cache.hits, 1);
        assert_eq!(cache.evaluated, 4);
    }

    #[test]
    fn private_cards_and_timed_search_regions_cannot_share_decisions() {
        let table = table();
        let (seat, legal) = table.to_act().unwrap();
        let mut cache = Decisions::default();
        let mut agent = player();
        cache.decide(&mut agent, &table.view(seat), legal);
        let mut hands = table.game.info.player_start_cards.clone();
        let a = hands[0][0];
        hands[0][0] = hands[1][0];
        hands[1][0] = a;
        let changed = Table::new(&hands);
        cache.decide(&mut agent, &changed.view(seat), changed.to_act().unwrap().1);
        assert_eq!(cache.hits, 0);
        let mut limited = Decisions::default();
        agent.params.endgame_tricks = 9;
        limited.decide(&mut agent, &table.view(seat), legal);
        limited.decide(&mut agent, &table.view(seat), legal);
        assert_eq!(limited.hits, 0);
    }

    #[test]
    fn a_fresh_actor_can_hit_the_cache_then_read_its_next_turn() {
        let mut table = table();
        let mut agent = player();
        let mut cache = Decisions::default();
        let (seat, legal) = table.to_act().unwrap();
        let first = cache.decide(&mut agent, &table.view(seat), legal);
        let mut fresh = player();
        assert_eq!(cache.decide(&mut fresh, &table.view(seat), legal), first);
        assert_eq!(cache.hits, 1);
        table.apply(first.action);
        for _ in 0..32 {
            let (next, legal) = table.to_act().expect("actor gets a later turn");
            if next == seat {
                let after = cache.decide(&mut fresh, &table.view(seat), legal);
                assert_eq!(after, player().decide(&table.view(seat), legal));
                return;
            }
            let action = player().decide(&table.view(next), legal).action;
            table.apply(action);
        }
        panic!("actor did not get a later turn");
    }
}

//! Scenario helpers: a seat's view built from its hand and hand-written `SeatEvent`s, with
//! legal actions computed by the engine's own rules (`bits::play_levels`).
#![allow(dead_code)]

use engine::is_first_trick;
use engine::Card;
use engine::PlaceAtTable;
use engine::{play_levels, CardSet};
use engine::{ActionType, GameAction};
use heuristic::{Decision, HeuristicAgent, Params, SeatKind, SeatView, SeriesScore};
use knowledge::Tracker;
use observation::{Seat, SeatEvent};

pub fn c(s: &str) -> u8 {
    s.parse::<Card>().unwrap().index()
}

pub fn set(cards: &[&str]) -> CardSet {
    cards.iter().fold(CardSet::EMPTY, |a, s| a | CardSet::single(c(s)))
}

pub fn card(i: u8) -> Card {
    engine::Card::from_index(i)
}

pub struct Scene {
    pub me: Seat,
    pub tracker: Tracker,
    pub events: Vec<SeatEvent>,
    /// Who plays each seat (public; `Unknown` reads every seat with the convention model).
    pub kinds: [SeatKind; 4],
}

impl Scene {
    pub fn new(me: Seat, hand: &[&str]) -> Scene {
        Scene {
            me,
            tracker: Tracker::new(me, set(hand)),
            events: vec![],
            kinds: [SeatKind::Unknown; 4],
        }
    }

    pub fn ev(&mut self, e: SeatEvent) -> &mut Scene {
        self.tracker.apply(&e).unwrap();
        self.events.push(e);
        self
    }

    /// Bidding actions in order: `(seat, Some(bid))` or `(seat, None)` for a pass.
    pub fn auction(&mut self, actions: &[(Seat, Option<i32>)]) -> &mut Scene {
        for &(seat, bid) in actions {
            match bid {
                Some(value) => self.ev(SeatEvent::Bid { seat, value }),
                None => self.ev(SeatEvent::StopBidding { seat }),
            };
        }
        self
    }

    pub fn play(&mut self, seat: Seat, card: &str) -> &mut Scene {
        self.ev(SeatEvent::CardPlayed { seat, card: c(card) })
    }

    pub fn pass(&mut self, from: Seat, to: Seat, cards: Option<&[&str]>) -> &mut Scene {
        self.ev(SeatEvent::Pass {
            from,
            to,
            cards: cards.map(set),
        })
    }

    pub fn view(&self) -> SeatView<'_> {
        SeatView {
            seat: self.me,
            knowledge: self.tracker.knowledge(),
            events: &self.events,
            series: SeriesScore::default(),
            kinds: self.kinds,
        }
    }

    pub fn hand(&self) -> CardSet {
        self.tracker.knowledge().certain(self.me)
    }

    fn action(&self, t: ActionType) -> GameAction {
        GameAction {
            action_type: t,
            player: PlaceAtTable(self.me),
        }
    }

    /// Pass and every bid above the current value.
    pub fn bid_legal(&self) -> Vec<GameAction> {
        let value = self
            .events
            .iter()
            .rev()
            .find_map(|e| match e {
                SeatEvent::Bid { value, .. } => Some(*value),
                _ => None,
            })
            .map_or(engine::NO_BID, engine::Points);
        let mut legal = vec![self.action(ActionType::StopBidding)];
        legal.extend(engine::bid_range(value).map(|v| self.action(ActionType::NewBid(v))));
        legal
    }

    pub fn pass_legal(&self) -> Vec<GameAction> {
        let cards: Vec<_> = self.hand().iter().collect();
        let mut legal = Vec::new();
        for a in 0..cards.len() {
            for b in a + 1..cards.len() {
                for c in b + 1..cards.len() {
                    for d in c + 1..cards.len() {
                        legal.push(self.action(ActionType::Pass([cards[d], cards[c], cards[b], cards[a]].map(card))));
                    }
                }
            }
        }
        legal
    }

    /// The cards the engine allows now.
    pub fn card_legal(&self) -> Vec<GameAction> {
        let k = self.tracker.knowledge();
        let hand = self.hand();
        let levels = play_levels(k.trick(), k.trump(), is_first_trick(k.hand_len(self.me) as usize));
        levels
            .allowed(hand)
            .iter()
            .map(|i| self.action(ActionType::CardPlayed(Card::from_index(i))))
            .collect()
    }

    pub fn with(&self, extra: Vec<ActionType>, mut legal: Vec<GameAction>) -> Vec<GameAction> {
        legal.extend(extra.into_iter().map(|t| self.action(t)));
        legal
    }

    pub fn act(&self, legal: &[GameAction]) -> Decision {
        // These isolated rule fixtures exercise the fallback policy. Full lawful
        // continuations and their hidden-card guards are in contract_planning.
        self.act_with(
            Params {
                ..Params::default()
            },
            legal,
        )
    }

    pub fn act_with(&self, params: Params, legal: &[GameAction]) -> Decision {
        HeuristicAgent::new(params).decide(&self.view(), legal)
    }
}

pub fn bid_of(d: &Decision) -> Option<i32> {
    match d.action.action_type {
        ActionType::NewBid(v) => Some(v),
        _ => None,
    }
}

pub fn played(d: &Decision) -> Option<u8> {
    match &d.action.action_type {
        ActionType::CardPlayed(card) => Some(card.index()),
        _ => None,
    }
}

pub fn passed(d: &Decision) -> CardSet {
    match &d.action.action_type {
        ActionType::Pass(cards) => CardSet::from_cards(cards),
        _ => CardSet::EMPTY,
    }
}

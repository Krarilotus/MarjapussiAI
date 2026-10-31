//! Posthoc match diagnostics from true finished games. Never inputs to a policy.
//! Convention support describes a common-default reading, not receiver understanding.

use beliefs::conventions::{
    follow_target, info_to_tell, walk_auction, HandClass, Message, Params, MESSAGES,
};
use engine::{ActionType, AnswerType, CardSet, GameCallback, GameFinishedInfo, GamePhase};

use crate::Table;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SeatCounts {
    pub auction_bids: u32,
    pub auction_passes: u32,
    /// Decisions in the raising phase with at least one legal higher bid.
    pub raise_opportunities: u32,
    pub raises: u32,
    pub raise_amount: i32,
    pub questions: u32,
    /// First announcements, including pairs shown in an answer (NewTrump).
    pub melds: u32,
    pub negative_answers: u32,
}

/// The only diagnostic needing pre-decision legality; use in play_game_with's hook.
#[derive(Clone, Debug, Default)]
pub struct MatchObserver {
    raise_opportunities: [u32; 4],
}

impl MatchObserver {
    pub fn observe(&mut self, table: &Table) {
        if table.game.state.phase != GamePhase::Raising {
            return;
        }
        let Some((seat, legal)) = table.to_act() else { return };
        if legal.iter().any(|a| matches!(a.action_type, ActionType::NewBid(_))) {
            self.raise_opportunities[seat as usize] += 1;
        }
    }

    pub fn finish(self, info: &GameFinishedInfo) -> MatchStats {
        MatchStats::from_finished(info, self.raise_opportunities)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractStats {
    pub declarer: u8,
    pub team: u8,
    pub auction_value: i32,
    pub final_value: i32,
    pub raise_amount: i32,
    pub made: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BidReading {
    pub seat: u8,
    /// None denotes an auction pass.
    pub target: Option<i32>,
    /// walk_auction's nominal context reading, even when alternatives exist.
    pub message: Option<Message>,
    /// None for passes, unrecognized bids, and ambiguous readings.
    pub supported: Option<bool>,
    pub ambiguous: bool,
    /// All live shifted/literal readings, including alternatives to nominal.
    pub candidates: Vec<Message>,
    /// Live content held at a pass; descriptive, never a mistake/error count.
    pub available_at_pass: Vec<Message>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommunicationStats {
    /// Unique recognizable readings only; ambiguous readings are separate.
    pub recognizable: u32,
    pub supported: u32,
    pub unsupported: u32,
    pub unrecognized_bids: u32,
    pub ambiguous_bids: u32,
    pub passes_with_available_message: u32,
    pub readings: Vec<BidReading>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchStats {
    pub scores: [i32; 2],
    pub team_points: [i32; 2],
    /// Engine trick points, including the last-trick bonus.
    pub card_points: [i32; 2],
    pub pair_points: [i32; 2],
    pub card_point_ceiling: i32,
    pub pair_point_ceiling: i32,
    pub tricks: [u8; 2],
    pub contract: Option<ContractStats>,
    /// A contract Schwarz penalty applies, not merely a zero-trick all-pass team.
    pub schwarz: bool,
    pub schwarz_teams: [bool; 2],
    pub seats: [SeatCounts; 4],
    pub communication: CommunicationStats,
}

impl MatchStats {
    pub fn from_finished(info: &GameFinishedInfo, raise_opportunities: [u32; 4]) -> Self {
        let points = engine::team_points(&info.tricks, &info.all_events);
        let mut seats: [SeatCounts; 4] = std::array::from_fn(|s| SeatCounts {
            raise_opportunities: raise_opportunities[s],
            ..SeatCounts::default()
        });
        let mut auction = Vec::new();
        let mut auction_value = engine::NO_BID.0;
        let mut exchanged = false;
        for event in &info.all_events {
            let seat = event.last_action.player.0;
            let counts = &mut seats[seat as usize];
            match event.last_action.action_type {
                ActionType::NewBid(value) if !exchanged => {
                    counts.auction_bids += 1;
                    auction_value = value;
                    auction.push((seat, Some(value)));
                }
                ActionType::StopBidding if !exchanged => {
                    counts.auction_passes += 1;
                    auction.push((seat, None));
                }
                ActionType::NewBid(value) => {
                    counts.raises += 1;
                    counts.raise_amount += value - auction_value;
                }
                ActionType::Pass(_) => exchanged = true,
                ActionType::Question(_) => counts.questions += 1,
                ActionType::Answer(AnswerType::NoPair | AnswerType::NoHalf(_)) => counts.negative_answers += 1,
                _ => {}
            }
            if matches!(event.callback, Some(GameCallback::NewTrump(_))) {
                counts.melds += 1;
            }
        }
        let contract = info.declarer.map(|seat| ContractStats {
            declarer: seat.0,
            team: seat.0 % 2,
            auction_value,
            final_value: info.game_value.0,
            raise_amount: info.game_value.0 - auction_value,
            made: info.won.unwrap_or(false),
        });
        // Ask the scoring owner whether its Schwarz factor changes either team's score.
        // Unlike GameFinishedInfo::schwarz_game, this excludes zero-trick all-pass games.
        let scores = info.scores();
        let without_schwarz_penalty = info.scores_with(0);
        let schwarz_teams = std::array::from_fn(|team| scores[team] != without_schwarz_penalty[team]);
        Self {
            scores,
            team_points: points.total(),
            card_points: points.cards,
            pair_points: points.pairs,
            card_point_ceiling: engine::CARD_POINTS_TOTAL,
            pair_point_ceiling: engine::PAIR_POINTS.iter().sum(),
            tricks: points.tricks,
            contract,
            schwarz: schwarz_teams.contains(&true),
            schwarz_teams,
            seats,
            communication: communication(info, &auction),
        }
    }
}

fn communication(info: &GameFinishedInfo, auction: &[(u8, Option<i32>)]) -> CommunicationStats {
    let params = Params::default();
    let classes = info.info.player_start_cards.each_ref().map(|hand| HandClass::of(CardSet::from_cards(hand)));
    let mut summary = CommunicationStats::default();
    for (&(seat, target), obs) in auction.iter().zip(walk_auction(auction, &params)) {
        let class = classes[seat as usize];
        let candidates: Vec<_> = MESSAGES.into_iter().filter(|&m| {
            obs.ctx.live(&params) & m.bit() != 0
                && target.is_some_and(|bid| follow_target(m, &obs.ctx, &params) == bid || obs.ctx.value + m.step() == bid)
        }).collect();
        let message = obs.nominal;
        let in_window = params.tell_window == 0 || obs.ctx.n_before < params.tell_window;
        let ambiguous = candidates.len() > 1;
        let supported = message.filter(|_| !ambiguous).map(|m| m.holds(class, params.strict_two_halves, in_window));
        let available_at_pass: Vec<_> = if target.is_none() {
            info_to_tell(class, &obs.ctx, &params).collect()
        } else {
            vec![]
        };
        if target.is_some() {
            if ambiguous {
                summary.ambiguous_bids += 1;
            } else if let Some(supported) = supported {
                summary.recognizable += 1;
                summary.supported += u32::from(supported);
                summary.unsupported += u32::from(!supported);
            } else {
                summary.unrecognized_bids += 1;
            }
        } else if !available_at_pass.is_empty() {
            summary.passes_with_available_message += 1;
        }
        summary.readings.push(BidReading {
            seat, target, message, supported, ambiguous, candidates, available_at_pass,
        });
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent::{Agent, Decision, Explanation, SeatView};
    use engine::{Card, GameAction};

    struct Script {
        contract: bool,
        announce: bool,
    }

    impl Agent for Script {
        fn act(&mut self, view: &SeatView, legal: &[GameAction]) -> Decision {
            let bid = if self.contract && view.seat == 0 {
                // First bid 120, then the lowest legal raise after the exchange.
                legal.iter().filter(|a| matches!(a.action_type, ActionType::NewBid(_)))
                    .min_by_key(|a| match a.action_type { ActionType::NewBid(value) => value, _ => unreachable!() })
            } else {
                None
            };
            let action = if let Some(bid) = bid {
                *bid
            } else {
                *legal.iter().find(|a| self.announce && matches!(a.action_type, ActionType::AnnounceTrump(_)))
                    .or_else(|| legal.iter().find(|a| matches!(a.action_type, ActionType::StopBidding | ActionType::CardPlayed(_))))
                    .unwrap_or(&legal[0])
            };
            Decision { action, explanation: Explanation::default() }
        }
    }

    fn play(contract: bool) -> (GameFinishedInfo, MatchStats) {
        play_with(contract, false)
    }

    fn play_with(contract: bool, announce: bool) -> (GameFinishedInfo, MatchStats) {
        // Entire suits per seat: legal all-pass play takes every trick for seat 0.
        let hands: [Vec<Card>; 4] = std::array::from_fn(|s| (0..9).map(|v| Card::from_index((s * 9 + v) as u8)).collect());
        let mut players: Vec<Box<dyn Agent>> = (0..4).map(|_| {
            Box::new(Script { contract, announce }) as Box<dyn Agent>
        }).collect();
        let mut observer = MatchObserver::default();
        let (info, _) = crate::play_game_with(&hands, &mut players, |table, _, _| observer.observe(table));
        let metrics = observer.finish(&info);
        (info, metrics)
    }

    #[test]
    fn all_pass_scores_use_engine_and_no_tricks_is_not_contract_schwarz() {
        let (info, m) = play(false);
        assert_eq!(m.contract, None);
        assert_eq!(m.tricks, [9, 0]);
        assert!(info.schwarz_game);
        assert!(!m.schwarz);
        assert_eq!(m.schwarz_teams, [false, false]);
        assert_eq!(m.scores, info.scores());
        assert_eq!(m.scores, [140, 0]);
        assert_eq!(m.card_points.iter().sum::<i32>(), m.card_point_ceiling);
        assert_eq!((m.card_point_ceiling, m.pair_point_ceiling), (140, 280));
    }

    #[test]
    fn raise_is_separate_from_auction_and_has_one_legal_opportunity() {
        let (info, m) = play(true);
        let c = m.contract.unwrap();
        assert_eq!((c.auction_value, c.final_value, c.raise_amount), (120, 125, 5));
        assert_eq!(m.seats[0].auction_bids, 1);
        assert_eq!(m.seats[0].raises, 1);
        assert_eq!(m.seats[0].raise_amount, 5);
        assert_eq!(m.seats[0].raise_opportunities, 1);
        assert_eq!(m.seats.iter().map(|s| s.raise_opportunities).sum::<u32>(), 1);
        assert_eq!(m.scores, info.scores());
        assert_eq!(m.communication.readings.len(), 4);
    }

    #[test]
    fn all_pass_announced_pairs_count_in_engine_points_and_scores() {
        let (info, m) = play_with(false, true);
        assert_eq!(m.pair_points, [engine::PAIR_POINTS[engine::Suit::Green as usize], 0]);
        assert_eq!(m.seats[0].melds, 1);
        assert_eq!(m.team_points, engine::team_points(&info.tricks, &info.all_events).total());
        assert_eq!(m.scores, [180, 0]);
        assert!(!m.schwarz);
    }

    #[test]
    fn convention_reading_support_is_content_not_an_error_at_pass() {
        let (info, _) = play(false);
        // Seat 0's Green ace supports +5; its unannounced pair remains available at pass.
        let s = communication(&info, &[(0, Some(120)), (0, None), (1, Some(420))]);
        assert_eq!(s.readings[0].message, Some(Message::Ace));
        assert_eq!(s.readings[0].supported, Some(true));
        assert!(!s.readings[1].available_at_pass.is_empty());
        assert_eq!(s.readings[1].supported, None);
        assert_eq!(s.unrecognized_bids, 1);
        assert_eq!(s.readings[2].message, None);
        let s = communication(&info, &[(0, Some(130))]);
        assert_eq!(s.readings[0].message, Some(Message::BigPair));
        assert_eq!(s.readings[0].supported, Some(false));
        assert_eq!(s.unsupported, 1);
        let s = communication(&info, &[(0, Some(120)), (0, Some(125))]);
        assert_eq!(s.readings[1].message, Some(Message::TwoHalves));
        // The Green pair alone does not meet strict_two_halves' common-default 140 sum.
        assert_eq!(s.readings[1].supported, Some(false));
        // The common convention shifts +10 from 130 to 145, also a literal +15.
        let s = communication(&info, &[(0, Some(130)), (1, Some(145))]);
        assert!(s.readings[1].ambiguous);
        assert_eq!(s.readings[1].supported, None);
        assert_eq!(s.ambiguous_bids, 1);
    }
}

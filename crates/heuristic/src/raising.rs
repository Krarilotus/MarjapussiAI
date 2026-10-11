//! Raising after the exchange.
//! - HZ-2: proven pre-play floors and their follow-through from seat-visible information.
//!   Reuse the complete claim proof where possible; otherwise certify opening Aces, side runs
//!   and one own trump run.
//! - HZ-1: `contract_estimate`, the team's expected points from my 9 cards and the pairs I
//!   passed back, with its observed spread. The raise is the value with the best expected
//!   contract score, (2 P(make) - 1) x value. HZ-3 caps it at 140 without a certain team pair.

use engine::{partner, points_set, ActionType, CardSet, GameAction, Suit, SUITS};
use observation::SeatEvent;

use crate::rules::RuleId;
use crate::{decision, fact, leads, play, Ctx, Decision};

struct Certificate {
    suit: Suit,
    side: CardSet,
    trumps: CardSet,
    floor: i32,
    card_points: i32,
    tricks: u32,
}

impl Certificate {
    fn of(hand: CardSet) -> Option<Self> {
        if hand.len() != engine::HAND_SIZE as u32 || (hand & CardSet::ACES).is_empty() {
            return None;
        }
        // The same single highest own pair that live announcement policy chooses.
        let suit = hand.pair_suits().max_by_key(|&s| beliefs::conventions::pair_value(s))?;
        let standing = knowledge::own_standing_of(hand);
        let trumps = standing & CardSet::suit(suit);
        // Preserve the policy's reservation of other held-half suits. We neither
        // add their pairs nor assume those halves will survive a separate route.
        let side = SUITS
            .into_iter()
            .filter(|&s| s != suit && (hand & CardSet::halves(s)).is_empty())
            .fold(CardSet::EMPTY, |cards, s| cards | (standing & CardSet::suit(s)));
        let (card_points, tricks) = engine::play_levels(&[], None, true)
            .allowed(hand)
            .iter()
            .map(|opening| {
                let won = side | trumps | CardSet::single(opening);
                let points = if won.len() == engine::HAND_SIZE as u32 {
                    // Nine own wins capture EVERY card and the last-trick bonus.
                    engine::CARD_POINTS.iter().map(|&p| p as i32).sum::<i32>() + engine::LAST_TRICK_BONUS
                } else {
                    points_set(won)
                };
                (points, won.len())
            })
            .min()?;
        Some(Self {
            suit,
            side,
            trumps,
            card_points,
            tricks,
            floor: card_points + beliefs::conventions::pair_value(suit),
        })
    }
}

/// HZ-1 `contract_estimate`: the declaring team's points (cards + announced pairs) from the
/// declarer's hand after the exchange. One named table from the human-game analysis
/// (owner-requested; `tools/kbstudy/teampoints.py`, 1,577 games, least squares).
struct TeamPoints {
    intercept: f32,
    per_standing: f32,
    per_ace: f32,
    per_trump_card: f32,
    per_void: f32,
    per_potential: f32,
    own_pairs_share: f32,
    given_pairs_share: f32,
    /// Residuals (actual - estimate) at the 0th, 5th, ..., 100th percentile.
    residuals: [f32; 21],
}

const TEAM_POINTS: TeamPoints = TeamPoints {
    intercept: 118.77,
    per_standing: 4.32,
    per_ace: 0.12,
    per_trump_card: 0.96,
    per_void: -0.46,
    per_potential: 2.95,
    own_pairs_share: 0.7117,
    given_pairs_share: 0.503,
    residuals: [
        -278.7, -86.2, -65.9, -52.1, -42.9, -33.1, -24.7, -16.9, -11.9, -6.7, -2.2, 4.0, 10.4, 16.6, 22.9, 32.6,
        43.4, 54.2, 75.3, 98.1, 179.3,
    ],
};

impl TeamPoints {
    /// P(actual >= target): the share of the residual distribution at or above `target - estimate`.
    fn p_reach(&self, estimate: f32, target: i32) -> f32 {
        let need = target as f32 - estimate;
        let r = &self.residuals;
        if need <= r[0] {
            return 1.0;
        }
        if need >= r[20] {
            return 0.0;
        }
        let i = r.iter().rposition(|&x| x <= need).unwrap_or(0).min(19);
        let below = (i as f32 + (need - r[i]) / (r[i + 1] - r[i]).max(1e-6)) * 0.05;
        1.0 - below
    }
}

/// The cards I passed back to my partner: certain to the partner and to me.
fn given_back(ctx: &Ctx) -> CardSet {
    let mut given = CardSet::EMPTY;
    for event in ctx.mind.events() {
        if let SeatEvent::Pass {
            from,
            to,
            cards: Some(cards),
        } = *event
        {
            if from == ctx.me && to == partner(ctx.me) {
                given |= cards;
            }
        }
    }
    given
}

/// HZ-1 with HZ-3: the best expected-score raise, if it beats keeping the current value.
fn estimated_raise(ctx: &Ctx, legal: &[GameAction]) -> Option<(GameAction, i32, Vec<crate::Fact>)> {
    if !ctx.params.estimated_raise || ctx.k.declarer() != Some(ctx.me) || !ctx.k.played().is_empty() {
        return None;
    }
    let hand = ctx.k.certain(ctx.me);
    let given = given_back(ctx);
    let own_pairs: i32 = hand.pair_suits().map(beliefs::conventions::pair_value).sum();
    let given_pairs: i32 = given.pair_suits().map(beliefs::conventions::pair_value).sum();
    let trump_len = hand.pair_suits().map(|s| (hand & CardSet::suit(s)).len()).max().unwrap_or(0);
    let voids = SUITS.into_iter().filter(|&s| (hand & CardSet::suit(s)).is_empty()).count();
    let t = &TEAM_POINTS;
    let estimate = t.intercept
        + t.per_standing * knowledge::own_standing_of(hand).len() as f32
        + t.per_ace * (hand & CardSet::ACES).len() as f32
        + t.per_trump_card * trump_len as f32
        + t.per_void * voids as f32
        + t.per_potential * knowledge::standing_potential_of(hand) as f32
        + t.own_pairs_share * own_pairs as f32
        + t.given_pairs_share * given_pairs as f32;
    let value = ctx.k.value();
    // HZ-3 [O]: never raise across 140 without a certain team pair.
    let cap = if value < 140 && own_pairs + given_pairs == 0 { 140 } else { i32::MAX };
    let score = |v: i32| (2.0 * t.p_reach(estimate, v) - 1.0) * v as f32;
    let keep = score(value);
    let (action, target) = legal
        .iter()
        .filter_map(|a| match a.action_type {
            ActionType::NewBid(v) if v <= cap => Some((*a, v)),
            _ => None,
        })
        .max_by(|a, b| score(a.1).total_cmp(&score(b.1)).then(b.1.cmp(&a.1)))?;
    (score(target) > keep).then(|| {
        (
            action,
            target,
            vec![
                fact("contract_estimate", estimate),
                fact("p_make", t.p_reach(estimate, target)),
                fact("expected_score", score(target)),
                fact("expected_score_keep", keep),
                fact("raise_target", target),
            ],
        )
    })
}

/// The same seat-visible proof used by the optional human claim. Recompute its
/// remaining route; a consumed card or lost declaration right cannot persist.
fn claim(ctx: &Ctx, legal: &[GameAction]) -> Option<(Vec<GameAction>, i32)> {
    let plan = knowledge::remaining_claim(ctx.k, legal)?;
    let floor = ctx.k.team_points()[ctx.me as usize % 2]
        + engine::points_set(CardSet::ALL - ctx.k.played())
        + engine::LAST_TRICK_BONUS
        + plan
            .iter()
            .filter_map(|a| match a.action_type {
                ActionType::AnnounceTrump(s) => Some(beliefs::conventions::pair_value(s)),
                _ => None,
            })
            .sum::<i32>();
    Some((plan, floor))
}

pub(crate) fn decide(ctx: &Ctx, legal: &[GameAction], cards: &[GameAction]) -> Decision {
    let estimated = estimated_raise(ctx, legal);
    let above = |target: i32| estimated.as_ref().filter(|e| e.1 > target).cloned();
    let estimated_decision = |(a, _, why): (GameAction, i32, Vec<crate::Fact>)| decision(&a, RuleId::HZ1, why);
    // Reuse the same seat-visible proof as "Zum Hinlegen". It tracks suit
    // exhaustion and every legal own announcement, so lower trumps promoted by
    // the run and multiple reachable pairs are counted through a real sequence.
    if ctx.k.declarer() == Some(ctx.me) && ctx.k.played().is_empty() {
        if let Some((plan, floor)) = claim(ctx, cards) {
            if let Some(a) = legal
                .iter()
                .filter(|a| matches!(a.action_type, ActionType::NewBid(v) if v <= floor))
                .max_by_key(|a| match a.action_type {
                    ActionType::NewBid(v) => v,
                    _ => unreachable!(),
                })
            {
                let ActionType::NewBid(target) = a.action_type else {
                    unreachable!()
                };
                if let Some(e) = above(target) {
                    return estimated_decision(e);
                }
                return decision(
                    a,
                    RuleId::HZ2,
                    vec![
                        fact("raise_floor", floor),
                        fact("raise_target", target),
                        fact("certified_claim", 1),
                    ],
                );
            }
            if floor >= ctx.k.value() {
                return decision(
                    plan.first().expect("nonempty claim"),
                    RuleId::HZ2,
                    vec![fact("raise_floor", floor), fact("certified_claim", 1)],
                );
            }
        }
    }
    let certificate = (ctx.k.declarer() == Some(ctx.me)
        && ctx.k.played().is_empty()
        && ctx.k.trump().is_none()
        && ctx.k.rights(ctx.me).may_announce()
        && SUITS.into_iter().all(|s| !ctx.k.is_called(s))
        && cards
            .iter()
            .all(|a| matches!(a.action_type, ActionType::CardPlayed(c) if CardSet::ACES.contains(c.index()))))
    .then(|| Certificate::of(ctx.k.certain(ctx.me)))
    .flatten();
    if let Some(c) = &certificate {
        if let Some(a) = legal
            .iter()
            .filter(|a| matches!(a.action_type, ActionType::NewBid(v) if v <= c.floor))
            .max_by_key(|a| match a.action_type {
                ActionType::NewBid(v) => v,
                _ => unreachable!(),
            })
        {
            let ActionType::NewBid(target) = a.action_type else {
                unreachable!()
            };
            if let Some(e) = above(target) {
                return estimated_decision(e);
            }
            let team = (ctx.me % 2) as usize;
            let mut outcome = engine::GameOutcome {
                playing_team: Some(team as u8),
                game_value: target,
                team_points: [0; 2],
                team_tricks: [0; 2],
            };
            outcome.team_points[team] = c.floor;
            outcome.team_tricks[team] = c.tricks as u8;
            // Only our guaranteed contract score is queried here. This is not
            // an estimate or a bound on the defenders' final score.
            return decision(
                a,
                RuleId::HZ2,
                vec![
                    fact("raise_floor", c.floor),
                    fact("raise_target", target),
                    fact("certified_card_points", c.card_points),
                    fact("certified_tricks", c.tricks),
                    fact("certified_pair", c.suit as u8),
                    fact(
                        "own_score_floor",
                        engine::game_scores(&outcome, engine::SCHWARZ_FACTOR_FIFTHS)[team],
                    ),
                ],
            );
        }
    }
    if let Some(e) = estimated {
        return estimated_decision(e);
    }
    let mut d = play::decide(ctx, cards);
    d.why.extend([
        fact("raise_floor", certificate.map_or(0, |c| c.floor)),
        fact("raise_target", ctx.k.value()),
    ]);
    d
}

/// The public pre-play Bid after our return identifies raising, not an auction
/// message. Reconstruct only our dealt cards and transfers visible to this seat.
fn raised_hand(ctx: &Ctx) -> Option<(CardSet, i32, usize)> {
    let mut hand = ctx.k.dealt_certain(ctx.me);
    let mut returned = false;
    let mut value = engine::NO_BID.0;
    for (i, event) in ctx.mind.events().iter().enumerate() {
        match *event {
            SeatEvent::Pass { from, to, cards } if from == ctx.me => {
                hand -= cards?;
                returned = to == partner(ctx.me);
            }
            SeatEvent::Pass { to, cards, .. } if to == ctx.me => hand |= cards?,
            SeatEvent::Bid { seat, value: target } => {
                if returned && seat == ctx.me && target > value {
                    return Some((hand, target, i + 1));
                }
                value = target;
            }
            SeatEvent::CardPlayed { .. } => return None,
            _ => {}
        }
    }
    None
}

/// Before the sampled endgame: realize the certified floor rather than replace
/// its guaranteed route with an attractive continuation in a few sampled deals.
pub(crate) fn follow(ctx: &Ctx, legal: &[GameAction]) -> Option<Decision> {
    let _timing = beliefs::profile::scope("claim.follow");
    if ctx.k.declarer() != Some(ctx.me)
        || !ctx.k.trick().is_empty()
        || ctx.k.trick_winners().iter().any(|&seat| seat != ctx.me)
    {
        return None;
    }
    let raised = raised_hand(ctx);
    if let Some((plan, floor)) = claim(ctx, legal) {
        if let Some(action) = plan.first().filter(|_| floor >= ctx.k.value()) {
            let mut why = vec![fact("certified_claim", 1), fact("raise_floor", floor)];
            if raised.is_some() {
                why.push(fact("certified_raise_route", 1));
            }
            return Some(decision(action, RuleId::HZ2, why));
        }
    }
    let (initial, target, after_raise) = raised?;
    let c = Certificate::of(initial)?;
    if c.floor < target || ctx.k.value() != target {
        return None;
    }
    let mut own_played = CardSet::EMPTY;
    let mut opening = None;
    let mut announced = false;
    for event in &ctx.mind.events()[after_raise..] {
        match *event {
            SeatEvent::CardPlayed { seat, card } if seat == ctx.me => {
                if opening.is_none() {
                    if !CardSet::ACES.contains(card) {
                        return None;
                    }
                    opening = Some(card);
                } else if !(if announced { c.trumps } else { c.side }).contains(card) {
                    return None;
                }
                own_played |= CardSet::single(card);
            }
            SeatEvent::AnnounceTrump { seat, suit } if seat == ctx.me && suit == c.suit && !announced => {
                if !(c.side - own_played).is_empty() {
                    return None;
                }
                announced = true;
            }
            SeatEvent::AnnounceTrump { .. }
            | SeatEvent::YesPair { .. }
            | SeatEvent::YesHalf { .. }
            | SeatEvent::AskPair { .. }
            | SeatEvent::AskHalf { .. } => return None,
            _ => {}
        }
    }
    // Preserve the ordinary opening choice: the floor covers every legal Ace.
    opening?;
    if ctx.k.certain(ctx.me) | own_played != initial {
        return None;
    }
    let desired = if !announced {
        if ctx.k.trump().is_some()
            || !ctx.k.rights(ctx.me).may_announce()
            || !CardSet::halves(c.suit).is_subset(ctx.k.certain(ctx.me))
        {
            return None;
        }
        let side = c.side - own_played;
        if side.is_empty() {
            ActionType::AnnounceTrump(c.suit)
        } else {
            ActionType::CardPlayed(engine::Card::from_index(leads::cash_card(
                side,
                side & ctx.k.retaining_lead(ctx.me),
            )?))
        }
    } else {
        if ctx.k.trump() != Some(c.suit) {
            return None;
        }
        let trumps = c.trumps - own_played;
        ActionType::CardPlayed(engine::Card::from_index(leads::cash_card(
            trumps,
            trumps & ctx.k.retaining_lead(ctx.me),
        )?))
    };
    let a = legal.iter().find(|a| a.action_type == desired)?;
    let rule = match desired {
        ActionType::AnnounceTrump(_) => RuleId::HC2,
        _ if announced => RuleId::HL2,
        _ => RuleId::HC4,
    };
    Some(decision(
        a,
        rule,
        vec![fact("certified_raise_route", 1), fact("raise_floor", c.floor)],
    ))
}

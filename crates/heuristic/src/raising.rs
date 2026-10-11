//! Proven pre-play floors and their follow-through from seat-visible information.
//! Reuse the complete claim proof where possible; otherwise certify opening Aces,
//! side runs and one own trump run. Uncertain raises belong to shared planning.

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

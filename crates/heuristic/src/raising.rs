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
use crate::series::SeriesStakes;
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
                    engine::CARD_POINTS_TOTAL
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
        -278.7, -86.2, -65.9, -52.1, -42.9, -33.1, -24.7, -16.9, -11.9, -6.7, -2.2, 4.0, 10.4, 16.6, 22.9, 32.6, 43.4,
        54.2, 75.3, 98.1, 179.3,
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

/// HZ-1 `contract_estimate` for a hand: my standing, aces, trump length, voids and potential,
/// my own complete pairs, and the partner's pair points (certain when passed back; in the
/// auction, the believed `derived_pair_value`).
pub(crate) fn contract_estimate(hand: CardSet, own_pairs: i32, partner_pairs: f32) -> f32 {
    let t = &TEAM_POINTS;
    let trump_len = hand
        .pair_suits()
        .map(|s| (hand & CardSet::suit(s)).len())
        .max()
        .unwrap_or(0);
    let voids = SUITS
        .into_iter()
        .filter(|&s| (hand & CardSet::suit(s)).is_empty())
        .count();
    t.intercept
        + t.per_standing * knowledge::own_standing_of(hand).len() as f32
        + t.per_ace * (hand & CardSet::ACES).len() as f32
        + t.per_trump_card * trump_len as f32
        + t.per_void * voids as f32
        + t.per_potential * knowledge::standing_potential_of(hand) as f32
        + t.own_pairs_share * own_pairs as f32
        + t.given_pairs_share * partner_pairs
}

/// P(the team makes `target`) given `contract_estimate`, from the observed spread.
pub(crate) fn p_make(estimate: f32, target: i32) -> f32 {
    TEAM_POINTS.p_reach(estimate, target)
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

/// HZ-1 by route (Astra, the owner on 2026-10-12: a raise counts the pairs and the points of
/// clear lines, not a flat add-on): the team's final points and tricks over the believed
/// worlds (`outlook::final_outcomes`), and the legal target with the best expected engine score
/// difference (failure, the defenders' points and schwarz included), if it beats keeping the
/// current value. HZ-3 caps it.
fn route_raise(ctx: &Ctx, legal: &[GameAction], cap: i32) -> Option<(GameAction, i32, Vec<crate::Fact>)> {
    let outcomes = crate::outlook::final_outcomes(ctx)?;
    let us = ctx.me % 2;
    let value = ctx.k.value();
    let keep = outcomes.score_difference(us, value);
    let (action, target, best) = legal
        .iter()
        .filter_map(|a| match a.action_type {
            ActionType::NewBid(v) if v <= cap => Some((*a, v, outcomes.score_difference(us, v))),
            _ => None,
        })
        .max_by(|a, b| a.2.total_cmp(&b.2).then(b.1.cmp(&a.1)))?;
    (best > keep).then(|| {
        (
            action,
            target,
            vec![
                fact("route_points", outcomes.mean_points(us)),
                fact("expected_score", best),
                fact("expected_score_keep", keep),
                fact("raise_target", target),
            ],
        )
    })
}

/// HZ-1 with HZ-3: the best expected-score raise, if it beats keeping the current value.
/// HZ-1 with HZ-3 for a declarer holding `hand` after passing back `given`, at the contract
/// `value`: the legal target with the best expected contract score, or `value` to keep it. The
/// one owner of this rule: the raise decision and the reading of a raise both use it.
pub(crate) struct Hz1 {
    pub target: i32,
    pub estimate: f32,
    pub score: f32,
    pub keep: f32,
    /// HZ-3 [O]: never raise across 140 without a certain team pair.
    pub cap: i32,
}

/// What HZ-1 weighs besides the hand: the team's series stakes (HS-1) and how many points the
/// policy's own play adds to the human-fitted `contract_estimate`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RaiseView {
    pub stakes: SeriesStakes,
    pub estimate_shift: f32,
}

impl RaiseView {
    pub fn of(ctx: &Ctx) -> Self {
        RaiseView {
            stakes: ctx.series,
            estimate_shift: ctx.params.raise_estimate_shift,
        }
    }
}

pub(crate) fn hz1(hand: CardSet, given: CardSet, value: i32, view: &RaiseView) -> Hz1 {
    let stakes = &view.stakes;
    let own_pairs: i32 = hand.pair_suits().map(beliefs::conventions::pair_value).sum();
    let given_pairs: i32 = given.pair_suits().map(beliefs::conventions::pair_value).sum();
    let cap = if value <= 140 && own_pairs + given_pairs == 0 {
        140
    } else {
        i32::MAX
    };
    let t = &TEAM_POINTS;
    let estimate = contract_estimate(hand, own_pairs, given_pairs as f32) + view.estimate_shift;
    // The contract's series worth relative to scoring nothing, with the defenders' points at
    // the rest of the estimate (HS-1); outside a round, (2 P(make) - 1) x value.
    let total = engine::CARD_POINTS_TOTAL;
    let theirs = engine::round5((total - estimate.round() as i32).clamp(0, total));
    let worth = |ours: i32| (stakes.worth_of(ours, theirs) - stakes.worth_of(0, theirs)) as f32;
    let score = |v: i32| {
        let p = t.p_reach(estimate, v);
        p * worth(v) + (1.0 - p) * worth(-v)
    };
    let keep = score(value);
    let best = engine::bid_range(engine::Points(value))
        .filter(|&v| v <= cap)
        .max_by(|&a, &b| score(a).total_cmp(&score(b)).then(b.cmp(&a)))
        .filter(|&v| score(v) > keep);
    Hz1 {
        target: best.unwrap_or(value),
        estimate,
        score: best.map_or(keep, score),
        keep,
        cap,
    }
}

fn estimated_raise(ctx: &Ctx, legal: &[GameAction]) -> Option<(GameAction, i32, Vec<crate::Fact>)> {
    if !ctx.params.estimated_raise || ctx.k.declarer() != Some(ctx.me) || !ctx.k.played().is_empty() {
        return None;
    }
    let value = ctx.k.value();
    let h = hz1(ctx.k.certain(ctx.me), given_back(ctx), value, &RaiseView::of(ctx));
    if ctx.params.route_raise {
        return route_raise(ctx, legal, h.cap);
    }
    if h.target == value {
        return None;
    }
    let action = legal
        .iter()
        .find(|a| matches!(a.action_type, ActionType::NewBid(v) if v == h.target))?;
    Some((
        *action,
        h.target,
        vec![
            fact("contract_estimate", h.estimate),
            fact("p_make", TEAM_POINTS.p_reach(h.estimate, h.target)),
            fact("expected_score", h.score),
            fact("expected_score_keep", h.keep),
            fact("raise_target", h.target),
        ],
    ))
}

/// The partner's raise decision as I observed it (D1, Astra's review §16): the contract before
/// it, the target it chose (the same value when it kept the contract and led), and the cards
/// it passed back to me. `None` while it hasn't decided or isn't the declarer.
pub(crate) struct RaiseObservation {
    pub raiser: observation::Seat,
    pub before: i32,
    pub target: i32,
    pub given_to_me: CardSet,
    /// Its seat kind: an agent's raise is read by its actual rule, a human's by the measured
    /// human choices.
    pub agent: bool,
    /// What an agent's rule weighs besides the hand: the team's public series stakes (HS-1) and
    /// the shared policy's estimate shift.
    pub view: RaiseView,
}

pub(crate) fn partner_raise(ctx: &Ctx) -> Option<RaiseObservation> {
    let raiser = partner(ctx.me);
    if ctx.k.declarer() != Some(raiser) {
        return None;
    }
    let (mut value, mut returned, mut given) = (engine::NO_BID.0, false, CardSet::EMPTY);
    let mut before = None;
    for event in ctx.mind.events() {
        match *event {
            SeatEvent::Pass { from, to, cards } if from == raiser && to == ctx.me => {
                given = cards?;
                returned = true;
            }
            SeatEvent::Bid { seat, value: target } if returned && seat == raiser => {
                return Some(RaiseObservation {
                    raiser,
                    before: before.unwrap_or(value),
                    target,
                    given_to_me: given,
                    agent: ctx.mind.kinds()[raiser as usize] == crate::SeatKind::Heuristic,
                    view: RaiseView::of(ctx),
                });
            }
            SeatEvent::Bid { value: v, .. } => {
                value = v;
                before = Some(v);
            }
            SeatEvent::CardPlayed { .. } if returned => {
                return Some(RaiseObservation {
                    raiser,
                    before: value,
                    target: value,
                    given_to_me: given,
                    agent: ctx.mind.kinds()[raiser as usize] == crate::SeatKind::Heuristic,
                    view: RaiseView::of(ctx),
                });
            }
            _ => {}
        }
    }
    None
}

/// Human contracts after the exchange relative to the declarer's countable team pairs P (its own
/// complete pairs plus those it passed back), by band of P: (lowest P of the band, the first
/// V - P of the counts, counts of V - P in steps of 5). From the 1,577 human games
/// (`tools/kbstudy/raisepairs.py`; n = 477, 374, 498, 110, 118). The owner reads a raise the
/// same way: 245 with the Green pair passed back says "two more pairs" (game 1187).
const HUMAN_TARGET_OVER_PAIRS: [(i32, i32, &[u16]); 5] = [
    (
        0,
        120,
        &[
            7, 13, 25, 54, 219, 61, 24, 16, 13, 7, 8, 1, 8, 10, 1, 2, 0, 1, 0, 1, 1, 0, 1, 0, 2, 0, 0, 0, 1, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 1,
        ],
    ),
    (
        40,
        60,
        &[
            3, 2, 2, 9, 30, 57, 28, 26, 55, 43, 15, 9, 8, 3, 4, 2, 40, 7, 2, 0, 4, 4, 1, 2, 5, 3, 1, 1, 3, 0, 0, 1, 1,
            0, 0, 0, 3,
        ],
    ),
    (
        80,
        25,
        &[
            2, 1, 3, 19, 54, 28, 28, 30, 52, 36, 36, 32, 30, 20, 14, 14, 18, 6, 7, 5, 6, 2, 6, 31, 3, 2, 0, 1, 2, 1, 0,
            2, 0, 3, 1, 0, 0, 0, 0, 2, 0, 0, 0, 1,
        ],
    ),
    (
        120,
        -10,
        &[
            1, 1, 2, 2, 5, 2, 4, 5, 8, 6, 3, 12, 4, 3, 4, 9, 8, 2, 4, 4, 5, 0, 1, 3, 2, 0, 1, 0, 0, 1, 4, 0, 1, 0, 0,
            0, 0, 1, 1, 0, 0, 1,
        ],
    ),
    (
        160,
        -80,
        &[
            1, 0, 0, 1, 0, 0, 0, 0, 2, 1, 1, 2, 2, 2, 4, 2, 3, 9, 0, 2, 4, 5, 2, 3, 5, 20, 4, 1, 2, 5, 9, 1, 2, 6, 2,
            0, 2, 0, 1, 2, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
        ],
    ),
];

impl RaiseObservation {
    /// P(this target | the raiser holding `hand` at its decision), normalized over its actual
    /// choices (keeping the contract or a legal raise): an agent's by its HZ-1 rule, with the
    /// convention reader's floor `floor` for its other branches (HZ-2 certificates and anything
    /// else), read by the broad human model; a human's by the human model alone.
    pub fn likelihood(&self, hand: CardSet, floor: f32) -> f64 {
        let human = self.human_likelihood(hand);
        if !self.agent {
            return human;
        }
        let hit = (hz1(hand, self.given_to_me, self.before, &self.view).target == self.target) as u8 as f64;
        (1.0 - floor as f64) * hit + floor as f64 * human
    }

    /// `HUMAN_TARGET_OVER_PAIRS` for the countable team pairs of `hand`, add-one smoothed and
    /// normalized over the available choices: an unavoidable choice (at 420 only keeping) carries
    /// no evidence (Astra's review 17.7 A).
    fn human_likelihood(&self, hand: CardSet) -> f64 {
        let pairs: i32 = (hand.pair_suits().chain(self.given_to_me.pair_suits()))
            .map(beliefs::conventions::pair_value)
            .sum();
        let (_, start, counts) = HUMAN_TARGET_OVER_PAIRS
            .iter()
            .rev()
            .find(|b| pairs >= b.0)
            .copied()
            .unwrap_or(HUMAN_TARGET_OVER_PAIRS[0]);
        let weight = |target: i32| {
            let i = (target - pairs - start).div_euclid(5);
            usize::try_from(i)
                .ok()
                .and_then(|i| counts.get(i))
                .copied()
                .unwrap_or(0) as f64
                + 1.0
        };
        let choices = std::iter::once(self.before).chain(engine::bid_range(engine::Points(self.before)));
        let total: f64 = choices.map(weight).sum();
        weight(self.target) / total
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

#[cfg(test)]
mod raise_reading_tests {
    use super::*;

    /// An unavoidable choice carries no evidence (Astra's review 17.7 A): at 420 only keeping is
    /// possible, so every hand explains it equally.
    #[test]
    fn keeping_at_420_reads_the_same_for_every_hand() {
        let hand = |cards: [&str; 9]| CardSet::from_cards(&cards.map(|c| c.parse().unwrap()));
        let obs = |agent| RaiseObservation {
            raiser: 0,
            before: 420,
            target: 420,
            given_to_me: CardSet::EMPTY,
            agent,
            view: Default::default(),
        };
        for agent in [false, true] {
            let a = obs(agent).likelihood(
                hand(["r-K", "r-O", "s-K", "s-O", "e-A", "g-6", "g-7", "e-6", "e-7"]),
                0.02,
            );
            let b = obs(agent).likelihood(
                hand(["r-6", "r-7", "s-6", "s-7", "e-8", "g-8", "g-9", "e-9", "s-8"]),
                0.02,
            );
            assert!(
                (a - b).abs() < 1e-9 && (a - 1.0).abs() < 1e-9,
                "agent {agent}: {a} vs {b}"
            );
        }
    }
}

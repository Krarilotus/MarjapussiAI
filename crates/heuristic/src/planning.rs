//! Conditional whole-game outcomes under the existing seat-local policy.
//! Each forecast follows one coherent legal continuation per weighted deal.
//! Its realized entries, promotions, announcements and last trick are not
//! independent bonuses: the engine's final score still selects the plan.

use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

mod cache;

use agent::{SeatView, Table};
use engine::{ActionType, CardSet, GameAction, GameFinishedInfo, GameOutcome, QuestionType};
use knowledge::World;

use crate::{fact, Ctx, Decision, Fact, HeuristicAgent, Rng, RuleId};

pub(crate) const BUDGET: Duration = Duration::from_millis(2_000);

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Outcome {
    pub score: f64,
    pub points: f64,
    pub made: f64,
    declaring: f64,
    pub shortfall: f64,
    pub tricks: f64,
    pub pairs: f64,
    pub last: f64,
    pub opening: u8,
    terminal: Option<GameOutcome>,
    winners: [f64; 36],
    developed: [f64; 36],
}

impl Default for Outcome {
    fn default() -> Self {
        Self {
            score: 0.0,
            points: 0.0,
            made: 0.0,
            declaring: 0.0,
            shortfall: 0.0,
            tricks: 0.0,
            pairs: 0.0,
            last: 0.0,
            opening: 0,
            terminal: None,
            winners: [0.0; 36],
            developed: [0.0; 36],
        }
    }
}

impl Outcome {
    fn made_given_declaring(&self) -> Option<f64> {
        (self.declaring > 0.0).then(|| self.made / self.declaring)
    }

    /// Expected tricks won by these allocated cards, before normalizing sample mass.
    pub fn wins(&self, cards: CardSet) -> f64 {
        cards.iter().map(|c| self.winners[c as usize]).sum()
    }

    /// Wins after all initially external higher ranks fell in earlier tricks.
    /// This is realized promotion in a policy continuation, not a guarantee that
    /// a blocked card will win or that its suit is safe from a later trump change.
    pub fn developed_wins(&self, cards: CardSet) -> f64 {
        cards.iter().map(|c| self.developed[c as usize]).sum()
    }

    pub(crate) fn add(&mut self, other: &Self, weight: f64) {
        self.score += weight * other.score;
        self.points += weight * other.points;
        self.made += weight * other.made;
        self.declaring += weight * other.declaring;
        self.shortfall += weight * other.shortfall;
        self.tricks += weight * other.tricks;
        self.pairs += weight * other.pairs;
        self.last += weight * other.last;
        self.opening = other.opening;
        for (mine, theirs) in self.winners.iter_mut().zip(other.winners) {
            *mine += weight * theirs;
        }
        for (mine, theirs) in self.developed.iter_mut().zip(other.developed) {
            *mine += weight * theirs;
        }
    }
}

/// Finish a candidate using only each simulated actor's own view.
/// Callers disable recursive forecasting and supply the comparison deadline.
fn rollout(
    mut table: Table,
    action: GameAction,
    players: &mut [HeuristicAgent; 4],
    deadline: Instant,
    decisions: &mut cache::Decisions,
) -> Option<Outcome> {
    let _timing = beliefs::profile::scope("forecast.rollout");
    let team = action.player.0 as usize % 2;
    table.apply(action);
    let mut opening = match action.action_type {
        ActionType::CardPlayed(c) => Some(c.index()),
        _ => None,
    };
    // Negative questions are suppressed by the existing policy. The cap is a
    // defensive bound, not a terminal heuristic: unfinished games do not count.
    for _ in 0..128 {
        let Some((seat, legal)) = table.to_act() else {
            break;
        };
        if Instant::now() >= deadline {
            return None;
        }
        let decision = decisions.decide(&mut players[seat as usize], &table.view(seat), legal);
        if !legal.contains(&decision.action) {
            return None;
        }
        if let ActionType::CardPlayed(c) = decision.action.action_type {
            opening.get_or_insert(c.index());
        }
        table.apply(decision.action);
    }
    if !table.game.ended() {
        return None;
    }
    let info = GameFinishedInfo::from(table.game);
    let scores = info.scores();
    let outcome = info.outcome();
    let card_points: i32 = info
        .tricks
        .iter()
        .filter(|t| t.winner.0 as usize % 2 == team)
        .map(|t| t.points.0)
        .sum();
    let initial = info.after_passing.as_ref().unwrap_or(&info.info.player_start_cards);
    let mut played = CardSet::EMPTY;
    let mut winners = [0.0; 36];
    let mut developed = [0.0; 36];
    for trick in &info.tricks {
        if trick.winner.0 as usize % 2 == team {
            let i = (trick.winner.0 + 4 - trick.leader.0) as usize % 4;
            let c = trick.cards[i].index();
            let blockers = crate::hand::higher(c) - CardSet::from_cards(&initial[trick.winner.0 as usize]);
            winners[c as usize] = 1.0;
            developed[c as usize] = f64::from(!blockers.is_empty() && blockers.is_subset(played));
        }
        played |= CardSet::from_cards(&trick.cards);
    }
    Some(Outcome {
        score: (scores[team] - scores[1 - team]) as f64,
        points: info.team_points[team] as f64,
        declaring: f64::from(outcome.playing_team == Some(team as u8)),
        made: f64::from(outcome.playing_team == Some(team as u8) && info.team_points[team] >= info.game_value.0),
        shortfall: if outcome.playing_team == Some(team as u8) {
            (info.game_value.0 - info.team_points[team]).max(0) as f64
        } else {
            0.0
        },
        tricks: outcome.team_tricks[team] as f64,
        pairs: (info.team_points[team] - card_points) as f64,
        last: f64::from(info.tricks.last()?.winner.0 as usize % 2 == team),
        opening: opening?,
        winners,
        developed,
        terminal: Some(outcome),
    })
}

/// One cohort for every alternative. The retained tables are hypotheses, never
/// observations passed to a policy. The proposal pool keeps correlated bidding
/// evidence represented before spending the much larger rollout budget.
pub(crate) struct Forecast {
    tables: Vec<(Table, f64)>,
    // Actor views are resynchronized on every call. Reuse the bounded solver
    // allocations and exact transposition entries across hypothesized branches.
    players: RefCell<[HeuristicAgent; 4]>,
    deadline: Instant,
    pub ess: f64,
    decisions: RefCell<cache::Decisions>,
    outcomes: RefCell<Vec<Vec<(GameAction, Outcome)>>>,
}

impl Forecast {
    pub fn new(ctx: &Ctx, n: usize, deadline: Instant) -> Option<Self> {
        let _timing = beliefs::profile::scope("forecast.build");
        if n == 0
            || Instant::now() >= deadline
            || ctx
                .mind
                .events()
                .iter()
                .any(|e| matches!(e, observation::SeatEvent::Pass { cards: None, .. }))
        {
            return None;
        }
        let mut rng = Rng::new(ctx.seed);
        let mut pool = {
            let model = ctx.mind.deal_model();
            let (space, weights) = (model.space(), model.weights());
            if space.count() <= n as u64 {
                use knowledge::Belief;
                let mut pool = Vec::new();
                space.for_each_deal(|hands| {
                    pool.push(World {
                        hands: *hands,
                        weight: weights.weight(hands),
                    })
                });
                pool
            } else {
                space.sample_worlds(&mut rng, n * 256, weights)
            }
        };
        pool.retain(|w| w.weight.is_finite() && w.weight > 0.0);
        let mass: f64 = pool.iter().map(|w| w.weight).sum();
        if mass <= 0.0 {
            return None;
        }
        let ess = mass * mass / pool.iter().map(|w| w.weight * w.weight).sum::<f64>();
        let worlds = if pool.len() <= n {
            for w in &mut pool {
                w.weight /= mass;
            }
            pool
        } else {
            let step = mass / n as f64;
            let start = (rng.next() >> 11) as f64 / (1u64 << 53) as f64 * step;
            let mut worlds: Vec<World> = Vec::new();
            let (mut index, mut sum) = (0, pool[0].weight);
            for i in 0..n {
                let target = start + i as f64 * step;
                while sum <= target && index + 1 < pool.len() {
                    index += 1;
                    sum += pool[index].weight;
                }
                let hands = pool[index].hands;
                if let Some(w) = worlds.iter_mut().find(|w| w.hands == hands) {
                    w.weight += 1.0 / n as f64;
                } else {
                    worlds.push(World {
                        hands,
                        weight: 1.0 / n as f64,
                    });
                }
            }
            worlds
        };
        let view = SeatView {
            seat: ctx.me,
            knowledge: ctx.k,
            events: ctx.mind.events(),
            series: ctx.series,
            kinds: ctx.mind.kinds(),
        };
        let tables: Option<Vec<_>> = worlds
            .into_iter()
            .map(|w| Some((Table::from_view(&view, w.hands)?, w.weight)))
            .collect();
        let mut params = ctx.params.clone();
        params.return_worlds = 0;
        params.plan_worlds = 0;
        params.endgame_tricks = params.endgame_tricks.min(2);
        params.endgame_worlds = params.endgame_worlds.min(8);
        let tables = tables?;
        let outcomes = (0..tables.len()).map(|_| Vec::new()).collect();
        Some(Self {
            tables,
            players: RefCell::new(std::array::from_fn(|_| HeuristicAgent::new(params.clone()))),
            deadline,
            ess,
            decisions: RefCell::new(cache::Decisions::default()),
            outcomes: RefCell::new(outcomes),
        })
    }

    pub fn compare(&self, actions: &[GameAction]) -> Option<Comparison> {
        let _timing = beliefs::profile::scope("forecast.compare");
        let mut means: Vec<Outcome> = actions.iter().map(|_| Outcome::default()).collect();
        let mut terminals: Vec<Vec<_>> = actions.iter().map(|_| Vec::new()).collect();
        let mut outcomes = self.outcomes.borrow_mut();
        let mut decisions = self.decisions.borrow_mut();
        let mut players = self.players.borrow_mut();
        for (world, (table, weight)) in self.tables.iter().enumerate() {
            for (i, &action) in actions.iter().enumerate() {
                if Instant::now() >= self.deadline || !table.to_act()?.1.contains(&action) {
                    return None;
                }
                if !outcomes[world].iter().any(|(a, _)| *a == action) {
                    let result = rollout(table.clone(), action, &mut players, self.deadline, &mut decisions)?;
                    outcomes[world].push((action, result));
                }
                let result = &outcomes[world].iter().find(|(a, _)| *a == action)?.1;
                terminals[i].push((result.terminal?, *weight));
                means[i].add(result, *weight);
            }
        }
        Some(Comparison { means, terminals })
    }

    pub fn work_facts(&self) -> [Fact; 2] {
        let decisions = self.decisions.borrow();
        [
            fact("forecast_decisions", decisions.evaluated),
            fact("forecast_reused_decisions", decisions.hits),
        ]
    }

    pub fn facts(&self, comparison: &Comparison, best: usize) -> Vec<Fact> {
        let chosen = &comparison.means[best];
        let mut facts = vec![
            fact("forecast_complete", 1),
            fact("forecast_worlds", self.tables.len() as u32),
            fact("forecast_candidates", comparison.means.len() as u32),
            fact("forecast_proposal_ess", self.ess),
            fact("forecast_score", chosen.score),
            fact("forecast_points", chosen.points),
            fact("forecast_declaring_share", chosen.declaring),
            fact("forecast_shortfall", chosen.shortfall),
            fact("forecast_pair_points", chosen.pairs),
            fact("forecast_tricks", chosen.tricks),
            fact("forecast_last", chosen.last),
            fact("forecast_gain", chosen.score - comparison.means[0].score),
        ];
        facts.extend(self.work_facts());
        if let Some(made) = chosen.made_given_declaring() {
            facts.push(fact("forecast_made", made));
        }
        facts
    }
}

pub(crate) struct Comparison {
    pub means: Vec<Outcome>,
    terminals: Vec<Vec<(GameOutcome, f64)>>,
}

impl Comparison {
    pub fn best(&self) -> usize {
        (0..self.means.len())
            .max_by(|&a, &b| self.means[a].score.total_cmp(&self.means[b].score).then(b.cmp(&a)))
            .unwrap_or(0)
    }

    fn alternative(&self, selected: usize) -> Option<usize> {
        (0..self.means.len())
            .filter(|&i| i != selected)
            .max_by(|&a, &b| self.means[a].score.total_cmp(&self.means[b].score).then(b.cmp(&a)))
    }
}

fn useful(ctx: &Ctx, action: &GameAction) -> bool {
    let mate = engine::partner(ctx.me);
    match action.action_type {
        ActionType::Question(QuestionType::Yours) => ctx.k.pairs(mate).possible != 0,
        ActionType::Question(QuestionType::YourHalf(suit)) => {
            !ctx.k.is_called(suit)
                && match ctx.k.known_half(mate, suit) {
                    Some(false) => false,
                    Some(true) => !(ctx.k.certain(ctx.me) & CardSet::halves(suit)).is_empty(),
                    None => true,
                }
        }
        ActionType::CardPlayed(_) | ActionType::AnnounceTrump(_) | ActionType::Answer(_) => true,
        _ => false,
    }
}

/// Compare executable continuations at a contract decision or an open lead.
/// Existing early-auction signals, certified follow-through and small-endgame
/// search remain their respective owners. A partial batch never changes a move.
pub(crate) fn choose(ctx: &Ctx, legal: &[GameAction], mut baseline: Decision) -> Decision {
    if ctx.params.plan_worlds == 0 || baseline.rule == RuleId::HE1 {
        return baseline;
    }
    let auction = legal.iter().any(|a| a.action_type == ActionType::StopBidding);
    let raising = !auction && legal.iter().any(|a| matches!(a.action_type, ActionType::NewBid(_)));
    if auction && baseline.action.action_type != ActionType::StopBidding {
        return baseline;
    }
    if !auction && !raising && (!ctx.k.trick().is_empty() || ctx.k.hand_len(ctx.me) as u32 <= ctx.params.endgame_tricks)
    {
        return baseline;
    }
    let deadline = Instant::now() + BUDGET;
    let Some(forecast) = Forecast::new(ctx, ctx.params.plan_worlds as usize, deadline) else {
        return baseline;
    };
    let mut options = vec![baseline.action];
    if auction {
        if let Some(a) = legal
            .iter()
            .filter(|a| matches!(a.action_type, ActionType::NewBid(_)))
            .min_by_key(|a| match a.action_type {
                ActionType::NewBid(v) => v,
                _ => unreachable!(),
            })
        {
            options.push(*a);
        }
    } else if !raising {
        options.extend(legal.iter().filter(|a| **a != baseline.action && useful(ctx, a)));
    }
    let Some(mut compared) = forecast.compare(&options) else {
        baseline.why.push(fact("forecast_complete", 0));
        baseline.why.extend(forecast.work_facts());
        return baseline;
    };
    if raising {
        let team = ctx.me as usize % 2;
        // Repricing proposes one target. It is then replayed at its actual new
        // contract: a changed target may alter passing, play and announcements.
        let proposal = legal
            .iter()
            .filter(|a| matches!(a.action_type, ActionType::NewBid(_)) && **a != baseline.action)
            .map(|a| {
                let ActionType::NewBid(value) = a.action_type else {
                    unreachable!()
                };
                let score = compared.terminals[0]
                    .iter()
                    .map(|(outcome, w)| {
                        let mut outcome = *outcome;
                        outcome.game_value = value;
                        let scores = engine::game_scores(&outcome, engine::SCHWARZ_FACTOR_FIFTHS);
                        w * (scores[team] - scores[1 - team]) as f64
                    })
                    .sum::<f64>();
                (a, score)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((a, score)) = proposal.filter(|(_, score)| *score > compared.means[0].score) {
            options.push(*a);
            let Some(verified) = forecast.compare(&options) else {
                baseline.why.push(fact("forecast_complete", 0));
                baseline.why.extend(forecast.work_facts());
                return baseline;
            };
            compared = verified;
            baseline.why.push(fact("forecast_proposed_score", score));
        }
    }
    let best = compared.best();
    if best != 0 {
        baseline.action = options[best];
        baseline.rule = RuleId::HP1;
        baseline.why.clear();
    }
    baseline.why.extend(forecast.facts(&compared, best));
    if let Some(index) = compared.alternative(best) {
        let other = &compared.means[index];
        baseline.why.extend([
            fact("forecast_selected_index", best as u32),
            fact("forecast_alternative_index", index as u32),
            fact("forecast_alternative_score", other.score),
            fact("forecast_alternative_points", other.points),
            fact("forecast_alternative_declaring_share", other.declaring),
            fact("forecast_alternative_joint_made", other.made),
            fact("forecast_alternative_pairs", other.pairs),
        ]);
        if let Some(made) = other.made_given_declaring() {
            baseline.why.push(fact("forecast_alternative_made", made));
        }
        let identity = match options[index].action_type {
            ActionType::CardPlayed(c) => Some(fact("forecast_alternative_card", c.index())),
            ActionType::AnnounceTrump(s) => Some(fact("forecast_alternative_announce", s as u8)),
            ActionType::Question(QuestionType::Yours) => Some(fact("forecast_alternative_ask_pair", 1)),
            ActionType::Question(QuestionType::YourHalf(s)) => Some(fact("forecast_alternative_ask_half", s as u8)),
            ActionType::NewBid(value) => Some(fact("forecast_alternative_bid", value)),
            ActionType::StopBidding => Some(fact("forecast_alternative_pass", 1)),
            _ => None,
        };
        baseline.why.extend(identity);
    }
    baseline
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Params;

    #[test]
    fn repricing_reuses_completed_baseline_without_replaying_it() {
        let hands = std::array::from_fn(|s| (0..9).map(|c| engine::Card::from_index((9 * s + c) as u8)).collect());
        let mut table = Table::new(&hands);
        for kind in [
            ActionType::NewBid(120),
            ActionType::StopBidding,
            ActionType::StopBidding,
            ActionType::StopBidding,
        ] {
            let action = *table
                .to_act()
                .unwrap()
                .1
                .iter()
                .find(|a| a.action_type == kind)
                .unwrap();
            table.apply(action);
        }
        for _ in 0..2 {
            let action = *table
                .to_act()
                .unwrap()
                .1
                .iter()
                .find(|a| matches!(a.action_type, ActionType::Pass(_)))
                .unwrap();
            table.apply(action);
        }
        assert_eq!(table.game.state.phase, engine::GamePhase::Raising);
        assert_eq!(table.game.state.value.0, 120);
        let legal = table.to_act().unwrap().1;
        let actions = [
            *legal
                .iter()
                .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
                .unwrap(),
            *legal.iter().find(|a| a.action_type == ActionType::NewBid(125)).unwrap(),
        ];
        let forecast = Forecast {
            tables: vec![(table, 1.0)],
            players: RefCell::new(std::array::from_fn(|_| HeuristicAgent::new(Params {
                plan_worlds: 0,
                return_worlds: 0,
                endgame_tricks: 0,
                ..Params::default()
            }))),
            deadline: Instant::now() + Duration::from_secs(30),
            ess: 1.0,
            decisions: RefCell::new(cache::Decisions::default()),
            outcomes: RefCell::new(vec![vec![]]),
        };
        let first = forecast.compare(&actions[..1]).unwrap();
        let work = (forecast.decisions.borrow().evaluated, forecast.decisions.borrow().hits);
        let repeated = forecast.compare(&actions[..1]).unwrap();
        assert_eq!(repeated.means[0].score, first.means[0].score);
        assert_eq!(
            (forecast.decisions.borrow().evaluated, forecast.decisions.borrow().hits),
            work
        );
        let expanded = forecast.compare(&actions).unwrap();
        assert_eq!(expanded.means[0].score, first.means[0].score);
        assert_eq!(expanded.terminals[0][0].0.game_value, 120);
        assert_eq!(expanded.terminals[1][0].0.game_value, 125);
        assert_eq!(forecast.outcomes.borrow()[0].len(), 2);
    }
}

#[cfg(test)]
#[path = "../tests/internal/forecast_reuse.rs"]
mod reuse_tests;

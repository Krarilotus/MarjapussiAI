//! A seat's moves judged over worlds from its knowledge: perfect-information Monte Carlo with
//! exact leaves, or every consistent deal when there are few. Only the seat's `Knowledge` and
//! the engine's `PublicState` go in, so the true hidden hands cannot reach the decision.

use engine::{game_scores, FastState, GameOutcome, Move, PublicState, SCHWARZ_FACTOR_FIFTHS, TRICKS};
use knowledge::{Belief, Knowledge, World};
use rand::RngCore;

use crate::search::Budget;
use crate::{Limits, Solver};

/// A quantity of the end of play in one world, under best play by both teams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    /// Final points (cards and pairs) of `team`; `team` maximises them, the other team
    /// minimises them.
    Points { team: u8 },
    /// 1 if `team` can end with at least `target` points against best defence, else 0.
    ReachPoints { team: u8, target: i32 },
    /// 1 if `team` can end with at least `target` tricks against best defence, else 0.
    ReachTricks { team: u8, target: u8 },
}

/// A measure and the direction the deciding seat wants it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Criterion {
    pub measure: Measure,
    pub maximize: bool,
}

/// What the deciding seat plays for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Objective {
    /// Its team's final points.
    Points,
    /// `team` ending with at least `target` points; maximised by `team`, minimised by the
    /// other team.
    Reach { team: u8, target: i32 },
    /// The contract rule HC-9 for either side; criteria in order of importance:
    /// - declarers: P(contract made); while the defenders have no trick and the scoring punishes
    ///   that (schwarz), P(they stay without one), to minimise (the owner's
    ///   `late_question_insurance`: with schwarz certain, no question or trump change that can
    ///   hand them a trick); then own points. Once the contract is made (or cannot be made in
    ///   any world), the defenders' points, to minimise;
    /// - defenders: own points. While they have no trick and the scoring punishes that
    ///   (schwarz, read from `game::scoring`), P(at least one trick) comes first.
    ///
    /// A side that the scoring punishes for taking no trick gets the trick criterion first,
    /// so a change of the declarers' schwarz rule in `game::scoring` applies here unchanged.
    Contract { playing_team: u8, game_value: i32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct MoveValue {
    pub mv: Move,
    /// Weighted mean over the worlds per criterion of the decision.
    pub means: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    /// Most important first.
    pub criteria: Vec<Criterion>,
    /// Every legal move, in `FastState::moves` order.
    pub moves: Vec<MoveValue>,
    pub worlds: usize,
    /// Every requested world was completed for every compared move/criterion.
    pub complete: bool,
    /// All consistent deals were considered, and every positive-weight continuation
    /// completed. This is not a proof of optimal imperfect-information play.
    pub all_deals: bool,
    /// Effective sample size of the weights, (Σw)² / Σw².
    pub ess: f64,
}

/// Means closer than this count as equal, so the next criterion decides.
const TIE: f64 = 1e-9;

impl Decision {
    /// The best move by the criteria in order (the first in move order on a full tie).
    pub fn best(&self) -> Option<Move> {
        let better = |a: &MoveValue, b: &MoveValue| {
            for (c, (x, y)) in self.criteria.iter().zip(a.means.iter().zip(&b.means)) {
                if (x - y).abs() > TIE {
                    return (x > y) == c.maximize;
                }
            }
            false
        };
        let mut best: Option<&MoveValue> = None;
        for m in &self.moves {
            if best.is_none_or(|b| better(m, b)) {
                best = Some(m);
            }
        }
        best.map(|m| m.mv)
    }
}

/// The solver crate's error type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolverError {
    /// The knowledge is not of the seat to move.
    NotToMove,
    /// The knowledge and the public state disagree (trick, trump or hand sizes).
    Inconsistent,
    /// No deal is consistent with the knowledge.
    NoConsistentDeal,
    /// The belief gave every world zero weight.
    ZeroWeight,
    /// The budget ended before one world was completed for all candidate moves.
    Interrupted,
}

impl std::fmt::Display for SolverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for SolverError {}

/// Whether the scoring (`game::scoring`) gives `team` less for taking no trick than for one,
/// all else equal, in a made or a lost contract.
fn trickless_costs(team: u8, playing_team: u8, game_value: i32) -> bool {
    [false, true].iter().any(|&made| {
        let score = |tricks: u8| {
            let mut o = GameOutcome {
                playing_team: Some(playing_team),
                game_value,
                team_points: [0; 2],
                team_tricks: [0; 2],
            };
            o.team_points[playing_team as usize] = if made { game_value } else { 0 };
            o.team_tricks[team as usize] = tricks;
            o.team_tricks[1 - team as usize] = TRICKS - tricks;
            game_scores(&o, SCHWARZ_FACTOR_FIFTHS)[team as usize]
        };
        score(0) != score(1)
    })
}

fn criteria(objective: Objective, table: &PublicState, me: u8) -> Vec<Criterion> {
    let mine = me & 1;
    let max = |measure| Criterion {
        measure,
        maximize: true,
    };
    let min = |measure| Criterion {
        measure,
        maximize: false,
    };
    match objective {
        Objective::Points => vec![max(Measure::Points { team: mine })],
        Objective::Reach { team, target } => vec![Criterion {
            measure: Measure::ReachPoints { team: team & 1, target },
            maximize: team & 1 == mine,
        }],
        Objective::Contract {
            playing_team,
            game_value,
        } => {
            let (s, n) = (playing_team & 1, 1 - (playing_team & 1));
            let mut out = vec![];
            if table.tricks[mine as usize] == 0 && trickless_costs(mine, s, game_value) {
                out.push(max(Measure::ReachTricks { team: mine, target: 1 }));
            }
            if mine == s {
                let points = (table.card_points[s as usize] + table.pair_points[s as usize]) as i32;
                let schwarz = table.tricks[n as usize] == 0 && trickless_costs(n, s, game_value);
                let keep_schwarz = min(Measure::ReachTricks { team: n, target: 1 });
                if points >= game_value {
                    if schwarz {
                        out.push(keep_schwarz);
                    }
                    out.push(min(Measure::Points { team: n }));
                } else {
                    out.push(max(Measure::ReachPoints {
                        team: s,
                        target: game_value,
                    }));
                    if schwarz {
                        out.push(keep_schwarz);
                    }
                    out.push(max(Measure::Points { team: s }));
                }
            } else {
                out.push(max(Measure::Points { team: n }));
            }
            out
        }
    }
}

/// The measure after move `m` in world `s`.
fn measure(solver: &mut Solver, s: &FastState, m: Move, measure: Measure, budget: &Budget) -> Option<f64> {
    let child = s.after(m);
    match measure {
        Measure::Points { team } => {
            Some((child.points(team) + solver.solve(&child, team, false, None, budget)?) as f64)
        }
        Measure::ReachPoints { team, target } => {
            let needed = target - child.points(team);
            Some((solver.solve(&child, team, false, Some(needed), budget)? >= needed) as u8 as f64)
        }
        Measure::ReachTricks { team, target } => {
            let needed = target as i32 - child.tricks_won(team) as i32;
            Some((solver.solve(&child, team, true, Some(needed), budget)? >= needed) as u8 as f64)
        }
    }
}

/// The deciding seat's moves judged over worlds from `knowledge`, by `objective`.
///
/// If at most `worlds` deals are consistent, each is solved once. Otherwise `worlds`
/// deals are sampled uniformly. Each world is weighted by `belief`, every legal move is solved
/// exactly in it, and the weighted means per criterion are returned.
pub fn evaluate<R: RngCore + ?Sized, B: Belief + ?Sized>(
    solver: &mut Solver,
    knowledge: &Knowledge,
    table: &PublicState,
    objective: Objective,
    worlds: usize,
    belief: &B,
    rng: &mut R,
) -> Result<Decision, SolverError> {
    evaluate_limited(
        solver,
        knowledge,
        table,
        objective,
        worlds,
        belief,
        rng,
        Limits::default(),
    )
}

/// Bounded evaluation. Only worlds completed for every root move enter the means;
/// interrupted branches never count as losses or exact results.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_limited<R: RngCore + ?Sized, B: Belief + ?Sized>(
    solver: &mut Solver,
    knowledge: &Knowledge,
    table: &PublicState,
    objective: Objective,
    worlds: usize,
    belief: &B,
    rng: &mut R,
    limits: Limits,
) -> Result<Decision, SolverError> {
    let budget = Budget::new(limits, solver);
    if budget.stopped(solver) {
        return Err(SolverError::Interrupted);
    }
    let me = knowledge.me();
    if table.to_move() != me {
        return Err(SolverError::NotToMove);
    }
    if knowledge.trick() != &table.trick[..table.trick_len as usize] || knowledge.trump() != table.trump {
        return Err(SolverError::Inconsistent);
    }
    let space = knowledge.deals();
    if space.count() == 0 {
        return Err(SolverError::NoConsistentDeal);
    }
    let exact = space.count() <= worlds as u64;
    let samples = if exact {
        let mut all = vec![];
        space.for_each_deal(|hands| {
            all.push(World {
                hands: *hands,
                weight: belief.weight(hands),
            })
        });
        all
    } else {
        space.sample_worlds(rng, worlds, belief)
    };
    let states = samples
        .iter()
        .filter(|w| w.weight != 0.0)
        .map(|w| {
            Ok((
                FastState::from_public(*table, w.hands).map_err(|_| SolverError::Inconsistent)?,
                w.weight,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if states.is_empty() {
        return Err(SolverError::ZeroWeight);
    }
    // The seat to move knows its hand, so its moves are the same in every world.
    let moves: Vec<Move> = states[0].0.moves().collect();
    debug_assert!(states.iter().all(|(s, _)| s.moves().eq(moves.iter().copied())));

    let mut criteria = criteria(objective, table, me);
    let mut values: Vec<_> = moves
        .iter()
        .map(|&mv| MoveValue {
            mv,
            means: vec![0.0; criteria.len()],
        })
        .collect();
    let (mut completed, mut weight, mut weight_sq) = (0usize, 0.0, 0.0);
    let world_values = |solver: &mut Solver, s: &FastState, criteria: &[Criterion]| -> Option<Vec<Vec<f64>>> {
        moves
            .iter()
            .map(|&mv| {
                criteria
                    .iter()
                    .map(|c| measure(solver, s, mv, c.measure, &budget))
                    .collect()
            })
            .collect()
    };
    for (s, w) in &states {
        let Some(row) = world_values(solver, s, &criteria) else {
            break;
        };
        for (value, means) in values.iter_mut().zip(row) {
            for (sum, mean) in value.means.iter_mut().zip(means) {
                *sum += w * mean;
            }
        }
        completed += 1;
        weight += w;
        weight_sq += w * w;
    }
    if completed == 0 {
        return Err(SolverError::Interrupted);
    }
    for value in &mut values {
        for mean in &mut value.means {
            *mean /= weight;
        }
    }
    let mut complete = completed == states.len();
    // Declarers who cannot make the contract in any world: the loss is fixed, so they keep
    // the defenders' points down (HC-9).
    if let (Objective::Contract { playing_team, .. }, Some(i)) = (
        objective,
        criteria
            .iter()
            .position(|c| matches!(c.measure, Measure::ReachPoints { .. })),
    ) {
        if complete && values.iter().all(|v| v.means[i] == 0.0) {
            let mut fallback = criteria[..i].to_vec();
            fallback.push(Criterion {
                measure: Measure::Points {
                    team: 1 - (playing_team & 1),
                },
                maximize: false,
            });
            let mut sums = vec![vec![0.0; fallback.len()]; moves.len()];
            let mut finished = true;
            for (s, w) in &states {
                let Some(row) = world_values(solver, s, &fallback) else {
                    finished = false;
                    break;
                };
                for (sum, means) in sums.iter_mut().zip(row) {
                    for (sum, mean) in sum.iter_mut().zip(means) {
                        *sum += w * mean;
                    }
                }
            }
            if finished {
                criteria = fallback;
                for (value, means) in values.iter_mut().zip(sums) {
                    value.means = means.into_iter().map(|sum| sum / weight).collect();
                }
            } else {
                complete = false;
            }
        }
    }
    Ok(Decision {
        criteria,
        moves: values,
        worlds: completed,
        complete,
        all_deals: exact && complete,
        ess: weight * weight / weight_sq,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schwarz_rule_is_read_from_the_scoring() {
        // Site scoring (engine E4, owner decision D2): a party played schwarz loses 2A, the
        // declarers included; both sides now get the trick criterion while trickless.
        for (playing, value) in [(0, 120), (1, 200)] {
            assert!(trickless_costs(1 - playing, playing, value));
            assert!(trickless_costs(playing, playing, value));
        }
    }

    #[test]
    fn best_is_lexicographic() {
        let c = |maximize| Criterion {
            measure: Measure::Points { team: 0 },
            maximize,
        };
        let v = |c: u8, means: Vec<f64>| MoveValue {
            mv: Move::Card(c),
            means,
        };
        let d = Decision {
            criteria: vec![c(true), c(false)],
            moves: vec![
                v(1, vec![0.5, 30.0]),
                v(2, vec![0.5 + 1e-12, 20.0]),
                v(3, vec![0.4, 0.0]),
            ],
            worlds: 1,
            complete: true,
            all_deals: true,
            ess: 1.0,
        };
        assert_eq!(d.best(), Some(Move::Card(2)));
        let d = Decision {
            criteria: vec![c(false)],
            ..d
        };
        assert_eq!(d.best(), Some(Move::Card(3)));
    }
}

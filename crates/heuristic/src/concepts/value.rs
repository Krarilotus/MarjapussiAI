//! Executable value definitions over one immutable, completed comparison cohort.
//! Engine scores are exact conditional on supplied terminals. Integrated values
//! are model estimates, never hard proof or calibrated success probabilities.
//! Refs are session-local; creating a new store discards every cached result.

use engine::{GameOutcome, SCHWARZ_FACTOR_FIFTHS};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScoresRef(pub NodeId);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutcomesRef(pub NodeId);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeightsRef(pub NodeId);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BranchValuesRef(pub NodeId);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumberRef(pub NodeId);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RankingRef(pub NodeId);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CohortMethod {
    Enumerated,
    ImportanceSampled {
        draws: usize,
        proposal_ess: f64,
    },
    Resampled {
        draws: usize,
        proposal_ess: f64,
    },
    /// Current allocation preparation followed by one conditional private-history
    /// sample per emitted slot. Even current_draws == 0 is not an enumerated
    /// JOINT history cohort. Current sampling metadata remains separately named.
    HistorySampled {
        current_draws: usize,
        current_proposal_ess: Option<f64>,
        current_resampled: bool,
        model: &'static str,
        proposals: usize,
        slots: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Provenance {
    pub belief_model: &'static str,
    pub response_model: &'static str,
    pub cohort: CohortMethod,
}

/// An authored scalar dependency, not a callback that chooses a tactic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Definition {
    pub opponent_weight: f64,
}
impl Default for Definition {
    fn default() -> Self {
        Self { opponent_weight: 1.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Declaring,
    MadeAndDeclaring,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Operator {
    CohortWeights,
    ConditionalOutcomes {
        candidate: usize,
    },
    TotalWeight {
        weights: WeightsRef,
    },
    Constant(f64),
    EngineScores {
        outcomes: OutcomesRef,
    },
    ExpectedScore {
        scores: ScoresRef,
        weights: WeightsRef,
        team: usize,
        mass: NumberRef,
    },
    EventWeight {
        outcomes: OutcomesRef,
        weights: WeightsRef,
        event: Event,
    },
    Ratio {
        numerator: NumberRef,
        denominator: NumberRef,
    },
    BranchDifference {
        scores: ScoresRef,
        team: usize,
        opponent_weight: NumberRef,
    },
    ExpectedDifference {
        values: BranchValuesRef,
        weights: WeightsRef,
        mass: NumberRef,
    },
    Best {
        values: Vec<NumberRef>,
    },
    Assessment {
        ranking: RankingRef,
        diagnostics: Vec<NumberRef>,
    },
}
impl Operator {
    fn dependencies(&self) -> Vec<NodeId> {
        match self {
            Self::TotalWeight { weights } => vec![weights.0],
            Self::EngineScores { outcomes } => vec![outcomes.0],
            Self::ExpectedScore {
                scores, weights, mass, ..
            } => vec![scores.0, weights.0, mass.0],
            Self::EventWeight { outcomes, weights, .. } => vec![outcomes.0, weights.0],
            Self::Ratio { numerator, denominator } => vec![numerator.0, denominator.0],
            Self::BranchDifference {
                scores,
                opponent_weight,
                ..
            } => vec![scores.0, opponent_weight.0],
            Self::ExpectedDifference { values, weights, mass } => vec![values.0, weights.0, mass.0],
            Self::Best { values } => values.iter().map(|r| r.0).collect(),
            Self::Assessment { ranking, diagnostics } => std::iter::once(ranking.0)
                .chain(diagnostics.iter().map(|r| r.0))
                .collect(),
            _ => vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeValue {
    Assessment,
    ConditionalOutcomes(Vec<GameOutcome>),
    Weights(Vec<f64>),
    ConditionalScores(Vec<[i32; 2]>),
    BranchValues(Vec<f64>),
    Number(Option<f64>),
    Ranking {
        selected: usize,
        alternative: Option<usize>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResultMeaning {
    /// A declared objective coefficient, not evidence about play.
    DefinitionConstant,
    /// Exact arithmetic under the supplied terminal-outcome assumptions.
    ConditionalExact,
    /// Numerical estimate under the declared hand and response models.
    ModelEstimate,
    /// Ordering of estimates within the declared candidate set.
    ModelSelection,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EvaluatedNode {
    pub reference: NodeId,
    pub name: &'static str,
    pub meaning: ResultMeaning,
    pub operator: Operator,
    pub dependencies: Vec<NodeId>,
    pub value: NodeValue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateRefs {
    pub scores: ScoresRef,
    pub our_score: NumberRef,
    pub opponent_score: NumberRef,
    pub difference: NumberRef,
    pub declaring: NumberRef,
    pub joint_made: NumberRef,
    pub made_given_declaring: NumberRef,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CandidateEstimate {
    pub refs: CandidateRefs,
    pub our_score: f64,
    pub opponent_score: f64,
    pub difference: f64,
    pub declaring: f64,
    pub joint_made: f64,
    pub made_given_declaring: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Assessment {
    pub root: NodeId,
    pub candidates: Vec<CandidateEstimate>,
    pub selected: usize,
    pub alternative: Option<usize>,
    pub ranking: RankingRef,
    pub provenance: Provenance,
    pub definition: Definition,
    pub root_team: usize,
    pub cohort_size: usize,
    pub cohort_weight: f64,
    /// Every supplied candidate/cohort outcome was available and integrated.
    /// This says nothing about exhaustive coverage of the full belief support.
    pub cohort_complete: bool,
    pub trace: Vec<EvaluatedNode>,
    pub evaluated_nodes: usize,
    pub memo_hits: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueError {
    InvalidContext,
    InvalidWeight,
    MismatchedCohort,
    ZeroMass,
    NonfiniteValue,
    InvalidReference,
}

/// O(candidates * cohort size) retained score data. Input order aligns worlds;
/// equal weights cannot establish world identity, which remains caller-owned.
pub struct ValueStore<'a> {
    terminals: &'a [&'a [(GameOutcome, f64)]],
    root_team: usize,
    provenance: Provenance,
    definition: Definition,
    mass: f64,
    nodes: Vec<(&'static str, Operator)>,
    memo: Vec<Option<NodeValue>>,
    consumed: Vec<NodeId>,
    hits: usize,
    candidates: Vec<CandidateRefs>,
    ranking: RankingRef,
    assessment: NodeId,
}

impl<'a> ValueStore<'a> {
    pub fn new(
        terminals: &'a [&'a [(GameOutcome, f64)]],
        root_team: usize,
        provenance: Provenance,
    ) -> Result<Self, ValueError> {
        Self::with_definition(terminals, root_team, provenance, Definition::default())
    }

    pub fn with_definition(
        terminals: &'a [&'a [(GameOutcome, f64)]],
        root_team: usize,
        provenance: Provenance,
        definition: Definition,
    ) -> Result<Self, ValueError> {
        if root_team > 1
            || terminals.is_empty()
            || terminals[0].is_empty()
            || provenance.belief_model.is_empty()
            || provenance.response_model.is_empty()
            || !definition.opponent_weight.is_finite()
        {
            return Err(ValueError::InvalidContext);
        }
        if let CohortMethod::Resampled { draws, proposal_ess }
        | CohortMethod::ImportanceSampled { draws, proposal_ess } = provenance.cohort
        {
            if draws < terminals[0].len() || !proposal_ess.is_finite() || proposal_ess <= 0.0 {
                return Err(ValueError::InvalidContext);
            }
        }
        if let CohortMethod::HistorySampled {
            current_draws,
            current_proposal_ess,
            current_resampled,
            model,
            proposals,
            slots,
        } = provenance.cohort
        {
            let current_valid = if current_draws == 0 {
                current_proposal_ess.is_none() && !current_resampled
            } else {
                current_draws >= slots && current_proposal_ess.is_some_and(|ess| ess.is_finite() && ess > 0.0)
            };
            if !current_valid || model.is_empty() || slots == 0 || proposals < slots {
                return Err(ValueError::InvalidContext);
            }
        }
        let mut mass = 0.0;
        for (_, weight) in terminals[0] {
            if !weight.is_finite() || *weight < 0.0 {
                return Err(ValueError::InvalidWeight);
            }
            mass += weight;
        }
        if !mass.is_finite() {
            return Err(ValueError::InvalidWeight);
        }
        if mass == 0.0 {
            return Err(ValueError::ZeroMass);
        }
        for candidate in terminals {
            if candidate.len() != terminals[0].len() {
                return Err(ValueError::MismatchedCohort);
            }
            for ((outcome, weight), (_, expected)) in candidate.iter().zip(terminals[0]) {
                if !weight.is_finite() || *weight < 0.0 {
                    return Err(ValueError::InvalidWeight);
                }
                if weight != expected {
                    return Err(ValueError::MismatchedCohort);
                }
                if outcome.playing_team.is_some_and(|team| team > 1) {
                    return Err(ValueError::InvalidContext);
                }
            }
        }
        let mut store = Self {
            terminals,
            root_team,
            provenance,
            definition,
            mass,
            nodes: vec![],
            memo: vec![],
            consumed: vec![],
            hits: 0,
            candidates: vec![],
            ranking: RankingRef(NodeId(0)),
            assessment: NodeId(0),
        };
        let weights = WeightsRef(store.add("joint_branch_weights", Operator::CohortWeights));
        let mass = NumberRef(store.add("cohort_weight", Operator::TotalWeight { weights }));
        let opponent_weight = NumberRef(store.add("opponent_weight", Operator::Constant(definition.opponent_weight)));
        for candidate in 0..terminals.len() {
            let outcomes =
                OutcomesRef(store.add("conditional_game_outcomes", Operator::ConditionalOutcomes { candidate }));
            let scores = ScoresRef(store.add("conditional_engine_scores", Operator::EngineScores { outcomes }));
            let our_score = NumberRef(store.add(
                "expected_our_score",
                Operator::ExpectedScore {
                    scores,
                    weights,
                    team: root_team,
                    mass,
                },
            ));
            let opponent_score = NumberRef(store.add(
                "expected_opponent_score",
                Operator::ExpectedScore {
                    scores,
                    weights,
                    team: 1 - root_team,
                    mass,
                },
            ));
            let differences = BranchValuesRef(store.add(
                "conditional_score_differences",
                Operator::BranchDifference {
                    scores,
                    team: root_team,
                    opponent_weight,
                },
            ));
            let difference = NumberRef(store.add(
                "expected_score_difference",
                Operator::ExpectedDifference {
                    values: differences,
                    weights,
                    mass,
                },
            ));
            let declaring_mass = NumberRef(store.add(
                "declaring_weight",
                Operator::EventWeight {
                    outcomes,
                    weights,
                    event: Event::Declaring,
                },
            ));
            let made_mass = NumberRef(store.add(
                "made_and_declaring_weight",
                Operator::EventWeight {
                    outcomes,
                    weights,
                    event: Event::MadeAndDeclaring,
                },
            ));
            let declaring = NumberRef(store.add(
                "declaring_probability",
                Operator::Ratio {
                    numerator: declaring_mass,
                    denominator: mass,
                },
            ));
            let joint_made = NumberRef(store.add(
                "made_and_declaring_probability",
                Operator::Ratio {
                    numerator: made_mass,
                    denominator: mass,
                },
            ));
            let made_given_declaring = NumberRef(store.add(
                "make_given_declaring",
                Operator::Ratio {
                    numerator: made_mass,
                    denominator: declaring_mass,
                },
            ));
            store.candidates.push(CandidateRefs {
                scores,
                our_score,
                opponent_score,
                difference,
                declaring,
                joint_made,
                made_given_declaring,
            });
        }
        store.ranking = RankingRef(store.add(
            "best_and_runner_up",
            Operator::Best {
                values: store.candidates.iter().map(|candidate| candidate.difference).collect(),
            },
        ));
        store.assessment = store.add(
            "decision_assessment",
            Operator::Assessment {
                ranking: store.ranking,
                diagnostics: store
                    .candidates
                    .iter()
                    .flat_map(|refs| {
                        [
                            refs.our_score,
                            refs.opponent_score,
                            refs.declaring,
                            refs.joint_made,
                            refs.made_given_declaring,
                        ]
                    })
                    .collect(),
            },
        );
        Ok(store)
    }

    fn add(&mut self, name: &'static str, operator: Operator) -> NodeId {
        let reference = NodeId(self.nodes.len());
        self.nodes.push((name, operator));
        self.memo.push(None);
        reference
    }

    fn number(&self, reference: NumberRef) -> Result<Option<f64>, ValueError> {
        match self.memo.get(reference.0 .0).and_then(Option::as_ref) {
            Some(NodeValue::Number(value)) => Ok(*value),
            _ => Err(ValueError::InvalidReference),
        }
    }

    fn weights(&self, reference: WeightsRef) -> Result<&[f64], ValueError> {
        match self.memo.get(reference.0 .0).and_then(Option::as_ref) {
            Some(NodeValue::Weights(values)) => Ok(values),
            _ => Err(ValueError::InvalidReference),
        }
    }

    fn outcomes(&self, reference: OutcomesRef) -> Result<&[GameOutcome], ValueError> {
        match self.memo.get(reference.0 .0).and_then(Option::as_ref) {
            Some(NodeValue::ConditionalOutcomes(values)) => Ok(values),
            _ => Err(ValueError::InvalidReference),
        }
    }

    fn required(&self, reference: NumberRef) -> Result<f64, ValueError> {
        self.number(reference)?.ok_or(ValueError::InvalidReference)
    }

    fn evaluate(&mut self, reference: NodeId) -> Result<(), ValueError> {
        let Some(cached) = self.memo.get(reference.0) else {
            return Err(ValueError::InvalidReference);
        };
        if cached.is_some() {
            self.hits += 1;
            return Ok(());
        }
        let operator = self.nodes[reference.0].1.clone();
        for dependency in operator.dependencies() {
            self.evaluate(dependency)?;
        }
        let result = match operator {
            Operator::CohortWeights => {
                NodeValue::Weights(self.terminals[0].iter().map(|(_, weight)| *weight).collect())
            }
            Operator::ConditionalOutcomes { candidate } => {
                NodeValue::ConditionalOutcomes(self.terminals[candidate].iter().map(|(outcome, _)| *outcome).collect())
            }
            Operator::TotalWeight { weights } => NodeValue::Number(Some(self.weights(weights)?.iter().sum())),
            Operator::Constant(value) => NodeValue::Number(Some(value)),
            Operator::EngineScores { outcomes } => NodeValue::ConditionalScores(
                self.outcomes(outcomes)?
                    .iter()
                    .map(|outcome| engine::game_scores(outcome, SCHWARZ_FACTOR_FIFTHS))
                    .collect(),
            ),
            Operator::ExpectedScore {
                scores,
                weights,
                team,
                mass,
            } => {
                let Some(NodeValue::ConditionalScores(scores)) = &self.memo[scores.0 .0] else {
                    return Err(ValueError::InvalidReference);
                };
                let mean = scores
                    .iter()
                    .zip(self.weights(weights)?)
                    .map(|(scores, weight)| scores[team] as f64 * weight)
                    .sum::<f64>()
                    / self.required(mass)?;
                NodeValue::Number(Some(mean))
            }
            Operator::EventWeight {
                outcomes,
                weights,
                event,
            } => NodeValue::Number(Some(
                self.outcomes(outcomes)?
                    .iter()
                    .zip(self.weights(weights)?)
                    .filter(|(outcome, _)| {
                        outcome.playing_team == Some(self.root_team as u8)
                            && (event == Event::Declaring || outcome.team_points[self.root_team] >= outcome.game_value)
                    })
                    .map(|(_, weight)| weight)
                    .sum(),
            )),
            Operator::Ratio { numerator, denominator } => {
                let denominator = self.required(denominator)?;
                NodeValue::Number(
                    (denominator > 0.0)
                        .then(|| self.required(numerator).map(|n| n / denominator))
                        .transpose()?,
                )
            }
            Operator::BranchDifference {
                scores,
                team,
                opponent_weight,
            } => {
                let Some(NodeValue::ConditionalScores(scores)) = &self.memo[scores.0 .0] else {
                    return Err(ValueError::InvalidReference);
                };
                let coefficient = self.required(opponent_weight)?;
                NodeValue::BranchValues(
                    scores
                        .iter()
                        .map(|scores| scores[team] as f64 - coefficient * scores[1 - team] as f64)
                        .collect(),
                )
            }
            Operator::ExpectedDifference { values, weights, mass } => {
                let Some(NodeValue::BranchValues(values)) = &self.memo[values.0 .0] else {
                    return Err(ValueError::InvalidReference);
                };
                // Integrate each outcome's difference before averaging. Separate
                // team expectations would introduce different cancellation error.
                NodeValue::Number(Some(
                    values
                        .iter()
                        .zip(self.weights(weights)?)
                        .map(|(value, weight)| value * weight)
                        .sum::<f64>()
                        / self.required(mass)?,
                ))
            }
            Operator::Assessment { .. } => NodeValue::Assessment,
            Operator::Best { values } => {
                let mut ranked: Vec<_> = values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| Ok((index, self.required(*value)?)))
                    .collect::<Result<_, ValueError>>()?;
                ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
                NodeValue::Ranking {
                    selected: ranked[0].0,
                    alternative: ranked.get(1).map(|v| v.0),
                }
            }
        };
        if matches!(&result, NodeValue::Number(Some(value)) if !value.is_finite()) {
            return Err(ValueError::NonfiniteValue);
        }
        self.memo[reference.0] = Some(result);
        self.consumed.push(reference);
        Ok(())
    }

    pub fn assess(&mut self) -> Result<Assessment, ValueError> {
        self.evaluate(self.assessment)?;
        let mut candidates = Vec::with_capacity(self.candidates.len());
        for refs in self.candidates.clone() {
            candidates.push(CandidateEstimate {
                refs,
                our_score: self.required(refs.our_score)?,
                opponent_score: self.required(refs.opponent_score)?,
                difference: self.required(refs.difference)?,
                declaring: self.required(refs.declaring)?,
                joint_made: self.required(refs.joint_made)?,
                made_given_declaring: self.number(refs.made_given_declaring)?,
            });
        }
        let Some(NodeValue::Ranking { selected, alternative }) = self.memo[self.ranking.0 .0].as_ref() else {
            return Err(ValueError::InvalidReference);
        };
        Ok(Assessment {
            root: self.assessment,
            candidates,
            selected: *selected,
            alternative: *alternative,
            ranking: self.ranking,
            provenance: self.provenance,
            definition: self.definition,
            root_team: self.root_team,
            cohort_size: self.terminals[0].len(),
            cohort_weight: self.mass,
            cohort_complete: true,
            evaluated_nodes: self.consumed.len(),
            memo_hits: self.hits,
            trace: self
                .consumed
                .iter()
                .map(|id| EvaluatedNode {
                    reference: *id,
                    name: self.nodes[id.0].0,
                    meaning: match self.nodes[id.0].1 {
                        Operator::Constant(_) => ResultMeaning::DefinitionConstant,
                        Operator::EngineScores { .. }
                        | Operator::ConditionalOutcomes { .. }
                        | Operator::BranchDifference { .. } => ResultMeaning::ConditionalExact,
                        Operator::Best { .. } | Operator::Assessment { .. } => ResultMeaning::ModelSelection,
                        _ => ResultMeaning::ModelEstimate,
                    },
                    operator: self.nodes[id.0].1.clone(),
                    dependencies: self.nodes[id.0].1.dependencies(),
                    value: self.memo[id.0].as_ref().expect("consumed nodes are evaluated").clone(),
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn provenance() -> Provenance {
        Provenance {
            belief_model: "fixture-joint",
            response_model: "fixture-completed",
            cohort: CohortMethod::Enumerated,
        }
    }
    fn outcome(team: Option<u8>, points: [i32; 2]) -> GameOutcome {
        GameOutcome {
            playing_team: team,
            game_value: 120,
            team_points: points,
            team_tricks: [5, 4],
        }
    }

    #[test]
    fn integrates_joint_events_and_engine_scores_without_marginal_products() {
        let terminals = [(outcome(Some(0), [130, 10]), 0.25), (outcome(Some(1), [100, 40]), 0.75)];
        let slices = [terminals.as_slice()];
        let a = ValueStore::new(&slices, 0, provenance()).unwrap().assess().unwrap();
        let value = &a.candidates[0];
        assert_eq!(
            (value.our_score, value.opponent_score, value.difference),
            (105.0, -87.5, 192.5)
        );
        assert_eq!(
            (value.declaring, value.joint_made, value.made_given_declaring),
            (0.25, 0.25, Some(1.0))
        );
        assert_ne!(value.joint_made, value.declaring * 0.25);
        assert!(a.cohort_complete);
        assert_eq!((a.selected, a.alternative), (0, None));
    }

    #[test]
    fn authored_dependency_changes_ranking_and_shared_nodes_are_evaluated_once() {
        let left = [(outcome(None, [160, 80]), 1.0)];
        let right = [(outcome(None, [115, 25]), 1.0)];
        let slices = [left.as_slice(), right.as_slice(), right.as_slice()];
        let mut store = ValueStore::new(&slices, 0, provenance()).unwrap();
        assert!(store.consumed.is_empty(), "definitions are lazy");
        store.evaluate(store.candidates[0].difference.0).unwrap();
        assert_eq!(
            store
                .consumed
                .iter()
                .filter(|id| matches!(store.nodes[id.0].1, Operator::EngineScores { .. }))
                .count(),
            1
        );
        let a = store.assess().unwrap();
        assert_eq!((a.selected, a.alternative), (1, Some(2)));
        assert_eq!(a.candidates[0].made_given_declaring, None);
        assert_eq!(a.trace.iter().filter(|n| n.name == "cohort_weight").count(), 1);
        assert_eq!(
            a.trace.iter().filter(|n| n.name == "conditional_engine_scores").count(),
            3
        );
        assert_eq!(
            a.trace.len(),
            a.trace
                .iter()
                .map(|n| n.reference)
                .collect::<std::collections::HashSet<_>>()
                .len()
        );
        for (position, node) in a.trace.iter().enumerate() {
            assert!(node
                .dependencies
                .iter()
                .all(|id| a.trace[..position].iter().any(|n| n.reference == *id)));
        }
        let difference = a
            .trace
            .iter()
            .find(|n| n.reference == a.candidates[0].refs.difference.0)
            .unwrap();
        assert!(matches!(difference.operator, Operator::ExpectedDifference { .. }));
        assert!(a.trace.iter().any(|node| matches!(node.operator,
            Operator::BranchDifference { scores, .. } if scores == a.candidates[0].refs.scores)));
        let repeated = store.assess().unwrap();
        assert_eq!(a.trace, repeated.trace);
        assert_eq!(a.evaluated_nodes, repeated.evaluated_nodes);
        assert!(repeated.memo_hits > a.memo_hits);
        let changed = ValueStore::with_definition(&slices, 0, provenance(), Definition { opponent_weight: 0.0 })
            .unwrap()
            .assess()
            .unwrap();
        assert_eq!(changed.selected, 0);
        let other_team = ValueStore::new(&slices, 1, provenance()).unwrap().assess().unwrap();
        assert_eq!(other_team.selected, 0, "a new context has its own results");
    }

    #[test]
    fn validates_weights_alignment_context_and_preserves_first_tie() {
        let a = [(outcome(None, [70, 70]), 2.0)];
        let slices = [a.as_slice(), a.as_slice()];
        let ranked = ValueStore::new(&slices, 1, provenance()).unwrap().assess().unwrap();
        assert_eq!((ranked.selected, ranked.alternative), (0, Some(1)));
        assert_eq!(ranked.candidates[0].our_score, 70.0);
        assert!(matches!(
            ValueStore::new(&slices, 2, provenance()),
            Err(ValueError::InvalidContext)
        ));
        for weight in [-1.0, f64::NAN, f64::INFINITY] {
            let invalid = [(a[0].0, weight)];
            assert!(matches!(
                ValueStore::new(&[&invalid], 0, provenance()),
                Err(ValueError::InvalidWeight)
            ));
        }
        let zero = [(a[0].0, 0.0)];
        assert!(matches!(
            ValueStore::new(&[&zero], 0, provenance()),
            Err(ValueError::ZeroMass)
        ));
        let mismatch = [(a[0].0, 1.0)];
        assert!(matches!(
            ValueStore::new(&[&a, &mismatch], 0, provenance()),
            Err(ValueError::MismatchedCohort)
        ));
        assert!(matches!(
            ValueStore::new(&[&a, &[]], 0, provenance()),
            Err(ValueError::MismatchedCohort)
        ));
    }

    #[test]
    fn equal_branch_differences_keep_the_tie_with_nondyadic_weights() {
        let a = [
            (outcome(None, [40, 100]), 1.0 / 3.0),
            (outcome(None, [80, 60]), 2.0 / 3.0),
        ];
        let b = [
            (outcome(None, [40, 100]), 1.0 / 3.0),
            (outcome(None, [120, 100]), 2.0 / 3.0),
        ];
        let slices = [a.as_slice(), b.as_slice()];
        let assessment = ValueStore::new(&slices, 0, provenance()).unwrap().assess().unwrap();
        let left = &assessment.candidates[0];
        let right = &assessment.candidates[1];
        assert_ne!(
            left.our_score - left.opponent_score,
            right.our_score - right.opponent_score
        );
        assert_eq!(left.difference, right.difference);
        assert_eq!(left.difference, (-60.0 * (1.0 / 3.0)) + (20.0 * (2.0 / 3.0)));
        assert_eq!(assessment.selected, 0);
    }

    #[test]
    fn resampled_completion_does_not_claim_enumerated_belief_support() {
        let terminals = [(outcome(None, [100, 40]), 1.0)];
        let slices = [terminals.as_slice()];
        let provenance = Provenance {
            cohort: CohortMethod::Resampled {
                draws: 8,
                proposal_ess: 3.0,
            },
            ..provenance()
        };
        let a = ValueStore::new(&slices, 0, provenance).unwrap().assess().unwrap();
        assert!(a.cohort_complete);
        assert_eq!(a.provenance.cohort, provenance.cohort);
        let invalid = Provenance {
            cohort: CohortMethod::Resampled {
                draws: 0,
                proposal_ess: 3.0,
            },
            ..provenance
        };
        assert!(matches!(
            ValueStore::new(&slices, 0, invalid),
            Err(ValueError::InvalidContext)
        ));
    }
    #[test]
    fn sampled_histories_remain_explicit_when_current_allocations_are_enumerated() {
        let terminals = [(outcome(None, [100, 40]), 1.0)];
        let slices = [terminals.as_slice()];
        let cohort = CohortMethod::HistorySampled {
            current_draws: 0,
            current_proposal_ess: None,
            current_resampled: false,
            model: "conditional-history-fixture",
            proposals: 3,
            slots: 1,
        };
        let completed = ValueStore::new(&slices, 0, Provenance { cohort, ..provenance() })
            .unwrap()
            .assess()
            .unwrap();
        assert_eq!(completed.provenance.cohort, cohort);
        assert!(completed.cohort_complete);
        let invalid = CohortMethod::HistorySampled {
            current_draws: 0,
            current_proposal_ess: None,
            current_resampled: false,
            model: "conditional-history-fixture",
            proposals: 0,
            slots: 1,
        };
        assert!(matches!(
            ValueStore::new(
                &slices,
                0,
                Provenance {
                    cohort: invalid,
                    ..provenance()
                }
            ),
            Err(ValueError::InvalidContext)
        ));
    }
}

//! Computed-IEEE enclosures in the same immutable ValueStore.
//! Numerical ordering is not completion/domain/receipt authorization.

use super::*;
use knowledge::ScoreIntervals;
use std::cmp::Ordering;

#[cfg(test)]
#[path = "../../../tests/internal/value_bounds.rs"]
mod tests;

/// Construction-local public input summary. Origin labels/weights are not
/// private cohort identity or a cache/rebind key; the executor retains BranchKey.
#[derive(Clone, Debug, PartialEq)]
pub struct ForwardSnapshot {
    pub knowledge: knowledge::Knowledge,
    pub phase: engine::GamePhase,
    pub legal: Vec<engine::GameAction>,
    pub events: Vec<observation::SeatEvent>,
    pub support: Vec<(usize, u64)>,
    pub root_team: usize,
    pub opponent_weight_bits: u64,
    pub provenance: Provenance,
}

/// A completed incumbent plus numerical bounds, never a complete assessment of
/// the full alternative domain. Only sequence's private constructor authorizes it.
#[derive(Clone, Debug, PartialEq)]
pub struct CompletionBounds {
    pub root: CertificateRef,
    pub selected: usize,
    pub selected_value: CandidateEstimate,
    pub competitors: Vec<(usize, IntervalRef, FloatInterval)>,
    pub trace: Vec<BoundTraceNode>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScoreBoundsRef(pub NodeId);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BranchBoundsRef(pub NodeId);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntervalRef(pub NodeId);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CertificateRef(pub NodeId);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundFailure {
    InvalidIntervals,
    InvalidMass,
    NonfiniteArithmetic,
    NoSeparation,
}

/// Endpoints enclose the computed finite value under total_cmp, including ±0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloatInterval {
    pub lower: f64,
    pub upper: f64,
}
impl FloatInterval {
    fn new(lower: f64, upper: f64) -> Result<Self, BoundFailure> {
        if !lower.is_finite() || !upper.is_finite() {
            return Err(BoundFailure::NonfiniteArithmetic);
        }
        if lower.total_cmp(&upper) == Ordering::Greater {
            return Err(BoundFailure::InvalidIntervals);
        }
        Ok(Self { lower, upper })
    }
    fn product(self, coefficient: f64) -> Result<Self, BoundFailure> {
        let (lo, hi) = if coefficient.is_sign_negative() {
            (self.upper, self.lower)
        } else {
            (self.lower, self.upper)
        };
        Self::new(lo * coefficient, hi * coefficient)
    }
    fn subtract(self, other: Self) -> Result<Self, BoundFailure> {
        Self::new(self.lower - other.upper, self.upper - other.lower)
    }
    fn add(self, other: Self) -> Result<Self, BoundFailure> {
        Self::new(self.lower + other.lower, self.upper + other.upper)
    }
    fn divide(self, mass: f64) -> Result<Self, BoundFailure> {
        if !mass.is_finite() || mass <= 0.0 {
            return Err(BoundFailure::InvalidMass);
        }
        Self::new(self.lower / mass, self.upper / mass)
    }
    pub fn contains(self, value: f64) -> bool {
        value.is_finite()
            && self.lower.total_cmp(&value) != Ordering::Greater
            && value.total_cmp(&self.upper) != Ordering::Greater
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BoundSource {
    /// The original honest snapshot is reconsumed by the knowledge rule owner.
    PublicForwardEnvelope {
        snapshot: std::sync::Arc<ForwardSnapshot>,
        remaining_card_budget: i32,
        future_pair_budget: i32,
    },
    /// Numerical tests assume these inputs; NOT a knowledge/fact certificate.
    #[cfg(test)]
    NumericalFixture,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BoundOperator {
    ScoreIntervals {
        source: BoundSource,
        slots: Vec<ScoreIntervals>,
        weights: WeightsRef,
    },
    BranchDifference {
        scores: ScoreBoundsRef,
        team: usize,
        opponent_weight: NumberRef,
    },
    ExpectedDifference {
        values: BranchBoundsRef,
        weights: WeightsRef,
        mass: NumberRef,
    },
    CertifiedBest {
        selected: usize,
        incumbent: NumberRef,
        competitors: Vec<(usize, IntervalRef)>,
    },
}
impl BoundOperator {
    pub(super) fn dependencies(&self) -> Vec<NodeId> {
        match self {
            Self::ScoreIntervals { weights, .. } => vec![weights.0],
            Self::BranchDifference {
                scores,
                opponent_weight,
                ..
            } => vec![scores.0, opponent_weight.0],
            Self::ExpectedDifference { values, weights, mass } => vec![values.0, weights.0, mass.0],
            Self::CertifiedBest {
                incumbent, competitors, ..
            } => std::iter::once(incumbent.0)
                .chain(competitors.iter().map(|(_, reference)| reference.0))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BoundValue {
    Scores(Vec<ScoreIntervals>),
    Branches(Vec<FloatInterval>),
    Interval(FloatInterval),
    /// Numerical ordering of submitted references ONLY. Full original-domain
    /// coverage/applicability belongs to the sequence evidence constructor.
    Certified {
        selected: usize,
        excluded: Vec<(usize, f64)>,
    },
    Unresolved(BoundFailure),
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoundTraceNode {
    pub reference: NodeId,
    pub name: &'static str,
    pub operator: RuntimeOperator,
    pub dependencies: Vec<NodeId>,
    pub value: RuntimeValue,
    pub meaning: BoundMeaning,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BoundMeaning {
    CompleteValue(ResultMeaning),
    ConditionalPublicEnvelope,
    AssumedNumericalInputs,
    ComputedModelEnclosure,
    CompletionOrdering,
    Unresolved,
}

impl ValueStore<'_> {
    pub(crate) fn completion_bounds(
        &mut self,
        snapshot: std::sync::Arc<ForwardSnapshot>,
        original_count: usize,
        selected: usize,
    ) -> Result<Option<CompletionBounds>, ValueError> {
        if self.candidates.len() != 1 || original_count < 2 || selected >= original_count {
            return Err(ValueError::InvalidContext);
        }
        // This single completed assessment describes ONLY the incumbent.
        let selected_value = self.assess()?.candidates.remove(0);
        let Ok(envelope) = knowledge::PublicOutcomeEnvelope::from_forward_play(
            &snapshot.knowledge,
            &snapshot.phase,
            &snapshot.legal,
            &snapshot.events,
        ) else {
            return Ok(None);
        };
        let scores = envelope.score_intervals();
        let source = BoundSource::PublicForwardEnvelope {
            remaining_card_budget: envelope.remaining_card_budget(),
            future_pair_budget: envelope.future_pair_budget(),
            snapshot: snapshot.clone(),
        };
        let reference = self.interval_nodes(vec![scores; snapshot.support.len()], source);
        let competitors: Vec<_> = (0..original_count)
            .filter(|&i| i != selected)
            .map(|i| (i, reference))
            .collect();
        let root = CertificateRef(self.add_bound(
            "completion_dominance",
            BoundOperator::CertifiedBest {
                selected,
                incumbent: selected_value.refs.difference,
                competitors: competitors.clone(),
            },
        ));
        self.evaluate(root.0)?;
        if !matches!(self.bound(root.0)?, BoundValue::Certified { selected: i, excluded }
            if *i == selected && excluded.len() + 1 == original_count)
        {
            return Ok(None);
        }
        let BoundValue::Interval(interval) = self.bound(reference.0)? else {
            return Ok(None);
        };
        Ok(Some(CompletionBounds {
            root,
            selected,
            selected_value,
            competitors: competitors.into_iter().map(|(i, r)| (i, r, *interval)).collect(),
            trace: self.bound_trace(),
        }))
    }

    fn add_bound(&mut self, name: &'static str, operator: BoundOperator) -> NodeId {
        let reference = NodeId(self.nodes.len());
        self.nodes.push((name, RuntimeOperator::Bound(operator)));
        self.memo.push(None);
        reference
    }
    fn bound(&self, reference: NodeId) -> Result<&BoundValue, ValueError> {
        match self.memo.get(reference.0).and_then(Option::as_ref) {
            Some(RuntimeValue::Bound(value)) => Ok(value),
            _ => Err(ValueError::InvalidReference),
        }
    }
    pub(super) fn evaluate_bound(&self, operator: BoundOperator) -> Result<BoundValue, ValueError> {
        let result = match operator {
            BoundOperator::ScoreIntervals { slots, source, weights } => {
                let weights = self.weights(weights)?;
                // The hard-evidence leaf is executable, not an unchecked label on
                // numeric inputs. Reconsume knowledge's SAME checked rule mapping.
                match source {
                    BoundSource::PublicForwardEnvelope {
                        snapshot,
                        remaining_card_budget,
                        future_pair_budget,
                    } => {
                        let envelope = knowledge::PublicOutcomeEnvelope::from_forward_play(
                            &snapshot.knowledge,
                            &snapshot.phase,
                            &snapshot.legal,
                            &snapshot.events,
                        );
                        let Ok(envelope) = envelope else {
                            return Ok(BoundValue::Unresolved(BoundFailure::InvalidIntervals));
                        };
                        let checked_scores = envelope.score_intervals();
                        if envelope.remaining_card_budget() != remaining_card_budget
                            || envelope.future_pair_budget() != future_pair_budget
                            || slots.len() != snapshot.support.len()
                            || snapshot.support.len() != weights.len()
                            || snapshot
                                .support
                                .iter()
                                .zip(weights)
                                .any(|((_, bits), weight)| *bits != weight.to_bits())
                            || snapshot.root_team != self.root_team
                            || snapshot.opponent_weight_bits != self.definition.opponent_weight.to_bits()
                            || snapshot.provenance != self.provenance
                            || slots.iter().any(|scores| *scores != checked_scores)
                        {
                            return Ok(BoundValue::Unresolved(BoundFailure::InvalidIntervals));
                        }
                    }
                    #[cfg(test)]
                    BoundSource::NumericalFixture => {}
                }
                // engine game_scores outputs i32; conversion to f64 is exact.
                if slots.is_empty()
                    || slots.len() != weights.len()
                    || slots.iter().flat_map(|s| s.teams).any(|s| {
                        s.lower > s.upper || i32::try_from(s.lower).is_err() || i32::try_from(s.upper).is_err()
                    })
                {
                    BoundValue::Unresolved(BoundFailure::InvalidIntervals)
                } else {
                    BoundValue::Scores(slots)
                }
            }
            BoundOperator::BranchDifference {
                scores,
                team,
                opponent_weight,
            } => {
                let coefficient = self.required(opponent_weight)?;
                match self.bound(scores.0)? {
                    BoundValue::Scores(scores) if team < 2 => {
                        let intervals = scores
                            .iter()
                            .map(|scores| {
                                let ours = scores.teams[team];
                                let other = scores.teams[1 - team];
                                FloatInterval::new(ours.lower as f64, ours.upper as f64)?.subtract(
                                    FloatInterval::new(other.lower as f64, other.upper as f64)?.product(coefficient)?,
                                )
                            })
                            .collect::<Result<Vec<_>, _>>();
                        match intervals {
                            Ok(values) => BoundValue::Branches(values),
                            Err(reason) => BoundValue::Unresolved(reason),
                        }
                    }
                    BoundValue::Unresolved(reason) => BoundValue::Unresolved(*reason),
                    _ => BoundValue::Unresolved(BoundFailure::InvalidIntervals),
                }
            }
            BoundOperator::ExpectedDifference { values, weights, mass } => {
                match self.bound(values.0)? {
                    BoundValue::Branches(values) => {
                        let weights = self.weights(weights)?;
                        let result = (|| {
                            if values.len() != weights.len() || values.is_empty() {
                                return Err(BoundFailure::InvalidIntervals);
                            }
                            // EXACT sum::<f64>() fold seed/order, including zero terms.
                            let mut sum = FloatInterval::new(-0.0, -0.0)?;
                            for (value, weight) in values.iter().zip(weights) {
                                // is_sign_negative handles canonical -0 weights too.
                                sum = sum.add(value.product(*weight)?)?;
                            }
                            sum.divide(self.required(mass).map_err(|_| BoundFailure::InvalidMass)?)
                        })();
                        match result {
                            Ok(value) => BoundValue::Interval(value),
                            Err(reason) => BoundValue::Unresolved(reason),
                        }
                    }
                    BoundValue::Unresolved(reason) => BoundValue::Unresolved(*reason),
                    _ => BoundValue::Unresolved(BoundFailure::InvalidIntervals),
                }
            }
            BoundOperator::CertifiedBest {
                selected,
                incumbent,
                competitors,
            } => {
                let value = self.required(incumbent)?;
                let mut excluded = Vec::new();
                for (index, reference) in competitors {
                    match self.bound(reference.0)? {
                        BoundValue::Interval(interval) => {
                            let order = interval.upper.total_cmp(&value);
                            if index == selected
                                || order == Ordering::Greater
                                || index < selected && order == Ordering::Equal
                            {
                                return Ok(BoundValue::Unresolved(BoundFailure::NoSeparation));
                            }
                            excluded.push((index, interval.upper));
                        }
                        BoundValue::Unresolved(reason) => return Ok(BoundValue::Unresolved(*reason)),
                        _ => return Ok(BoundValue::Unresolved(BoundFailure::InvalidIntervals)),
                    }
                }
                BoundValue::Certified { selected, excluded }
            }
        };
        Ok(result)
    }
    fn interval_nodes(&mut self, scores: Vec<ScoreIntervals>, source: BoundSource) -> IntervalRef {
        // Reuse the authored public scalar and canonical weights/mass refs.
        let Operator::ExpectedDifference { values, weights, mass } =
            self.nodes[self.candidates[0].difference.0 .0].1.value().unwrap()
        else {
            unreachable!()
        };
        let (weights, mass) = (*weights, *mass);
        let Operator::BranchDifference { opponent_weight, .. } = self.nodes[values.0 .0].1.value().unwrap() else {
            unreachable!()
        };
        let opponent_weight = *opponent_weight;
        let scores = ScoreBoundsRef(self.add_bound(
            "conditional_engine_score_bounds",
            BoundOperator::ScoreIntervals {
                source,
                slots: scores,
                weights,
            },
        ));
        let branches = BranchBoundsRef(self.add_bound(
            "conditional_score_difference_bounds",
            BoundOperator::BranchDifference {
                scores,
                team: self.root_team,
                opponent_weight,
            },
        ));
        IntervalRef(self.add_bound(
            "computed_expected_difference_bounds",
            BoundOperator::ExpectedDifference {
                values: branches,
                weights,
                mass,
            },
        ))
    }
    pub(crate) fn bound_trace(&self) -> Vec<BoundTraceNode> {
        self.consumed
            .iter()
            .map(|id| BoundTraceNode {
                reference: *id,
                name: self.nodes[id.0].0,
                operator: self.nodes[id.0].1.clone(),
                dependencies: self.nodes[id.0].1.dependencies(),
                value: self.memo[id.0].as_ref().unwrap().clone(),
                meaning: match (&self.nodes[id.0].1, self.memo[id.0].as_ref().unwrap()) {
                    (_, RuntimeValue::Bound(BoundValue::Unresolved(_))) => BoundMeaning::Unresolved,
                    (RuntimeOperator::Value(operator), _) => BoundMeaning::CompleteValue(operator.meaning()),
                    (
                        RuntimeOperator::Bound(BoundOperator::ScoreIntervals {
                            source: BoundSource::PublicForwardEnvelope { .. },
                            ..
                        }),
                        _,
                    ) => BoundMeaning::ConditionalPublicEnvelope,
                    #[cfg(test)]
                    (
                        RuntimeOperator::Bound(BoundOperator::ScoreIntervals {
                            source: BoundSource::NumericalFixture,
                            ..
                        }),
                        _,
                    ) => BoundMeaning::AssumedNumericalInputs,
                    (RuntimeOperator::Bound(BoundOperator::CertifiedBest { .. }), _) => {
                        BoundMeaning::CompletionOrdering
                    }
                    _ => BoundMeaning::ComputedModelEnclosure,
                },
            })
            .collect()
    }
}

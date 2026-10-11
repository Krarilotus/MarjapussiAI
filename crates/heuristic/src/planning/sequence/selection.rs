//! Completed parent selection and its reachable conditional execution records.
//! Only the evaluator can mint this evidence, after every parent candidate has
//! completed. Availability membership alone never creates authorization.

use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct SelectedPlan {
    root: std::sync::Arc<PreparedChoice>,
    records: Vec<PreparedChoice>,
}

/// Compact runtime projection. `records` aligns with SequenceTrace::choices;
/// each index names a certificate in this parent's immutable selected tree.
/// Exporters can intern the parent once by its semantic input/domain/context,
/// then link subsequent steps without serializing the future tree.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionAuthorization {
    pub parent: std::sync::Arc<PreparedChoice>,
    pub records: Vec<usize>,
}

impl SelectedPlan {
    pub fn root(&self) -> &PreparedChoice {
        &self.root
    }

    pub fn records(&self) -> &[PreparedChoice] {
        &self.records
    }

    pub fn authorization(&self, information: &Information) -> SelectionAuthorization {
        SelectionAuthorization {
            parent: self.root.clone(),
            records: self
                .records
                .iter()
                .enumerate()
                .filter_map(|(index, record)| (record.information == *information).then_some(index))
                .collect(),
        }
    }

    pub(super) fn completed(
        information: Information,
        context: &Context,
        membership: Membership,
        values: &Assessment,
        support: Vec<(usize, f64)>,
        selected: &Execution,
    ) -> Result<Self, SequenceError> {
        if !values.cohort_complete || values.candidates.len() != membership.programs.len() {
            return Err(SequenceError::InvalidValue);
        }
        let selected_program = selected.candidate.program.suffix(selected.candidate.stage)?;
        if membership.programs.get(values.selected) != Some(&selected_program) {
            return Err(SequenceError::WrongContext);
        }
        let root = PreparedChoice {
            scope: ChoiceScope::Parent,
            information,
            semantic_suffix: selected_program,
            context: context.clone(),
            alternatives: membership.programs.iter().cloned().map(Selection::Stage).collect(),
            membership,
            selected: values.selected,
            proof: Some(SelectionProof::Complete(values.clone())),
            consumed: Vec::new(),
            support,
        };
        // choose_bound appends ONLY records from its selected child. The outer
        // evaluator likewise supplies ONLY the selected execution here. This
        // private ownership is the parent->selected-branch certificate: callers
        // cannot attach an unrelated local assessment to a retained plan.
        let mut records: Vec<PreparedChoice> = Vec::new();
        for record in &selected.prepared {
            if record.context != root.context
                || record.support.is_empty()
                || record.information.seat != root.information.seat
                || !record.information.events.starts_with(&root.information.events)
            {
                return Err(SequenceError::WrongContext);
            }
            match (&record.scope, &record.proof) {
                (ChoiceScope::Global | ChoiceScope::Local, Some(SelectionProof::Complete(assessment)))
                    if assessment.cohort_complete
                        && assessment.candidates.len() == record.alternatives.len()
                        && assessment.selected == record.selected => {}
                (ChoiceScope::Local, Some(SelectionProof::CompletionDominance(proof))) if proof.validates(record) => {}
                (ChoiceScope::FixedAction | ChoiceScope::Response, None)
                    if record.alternatives.len() == 1 && record.selected == 0 => {}
                _ => return Err(SequenceError::InvalidValue),
            }
            if !dominance::support_subsequence(&record.support, &root.support) {
                return Err(SequenceError::InvalidCohort);
            }
            // Fixed actions are emitted per branch by drive. Join their exact
            // equivalent certificates, retaining original ids/mass, rather than
            // allowing a first-world singleton to stand for a joint comparison.
            let matching = records.iter_mut().find(|old| {
                old.proof.is_none()
                    && record.proof.is_none()
                    && old.scope == record.scope
                    && old.information == record.information
                    && old.semantic_suffix == record.semantic_suffix
                    && old.membership == record.membership
                    && old.alternatives == record.alternatives
                    && old.selected == record.selected
                    && old.consumed == record.consumed
            });
            if let Some(old) = matching {
                for pair in &record.support {
                    if !old.support.contains(pair) {
                        old.support.push(*pair);
                    }
                }
                old.support.sort_by_key(|pair| pair.0);
            } else {
                records.push(record.clone());
            }
        }
        Ok(Self {
            root: std::sync::Arc::new(root),
            records,
        })
    }
}

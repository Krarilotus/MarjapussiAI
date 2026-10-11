//! Test-only access to an actual descendant evaluation. No enclosing selection
//! or authorization is constructed, and no Program/domain is substituted.
use super::*;

pub(crate) struct DescendantRun {
    pub terminals: Vec<Terminal>,
    pub choices: Vec<PreparedChoice>,
    pub work: Work,
}

pub(crate) fn finish<F>(
    cohort: &[(Table, f64)],
    receipts: &[Receipt],
    context: &Context,
    limits: Limits,
    leaf: &mut F,
) -> Result<DescendantRun, SequenceError>
where
    F: FnMut(&SeatView<'_>, &[GameAction], LeafBudget) -> Result<LeafResponse, LeafFailure>,
{
    if cohort.is_empty() || cohort.len() != receipts.len() {
        return Err(SequenceError::InvalidCohort);
    }
    let first = cohort[0].0.view(0);
    let information = Information::of(&first);
    let legal = crate::legal_actions::eligible(cohort[0].0.to_act().ok_or(SequenceError::WrongContext)?.1);
    // Reuse the existing test constructor for the real Session. All work starts
    // at this diagnostic boundary; the scripted prefix is not claimed evaluated.
    let mut session = super::cache_tests::session(context, leaf, limits);
    let mut branches = Vec::new();
    for (origin, ((table, weight), receipt)) in cohort.iter().zip(receipts).enumerate() {
        if !weight.is_finite()
            || *weight <= 0.0
            || !information.matches(&table.view(0))
            || !receipt.accepted.matches(&table.view(0))
            || receipt.pending.is_some()
            || receipt.program != receipts[0].program
            || receipt.stage != receipts[0].stage
            || !table
                .to_act()
                .is_some_and(|(seat, actions)| seat == 0 && crate::legal_actions::eligible(actions) == legal)
        {
            return Err(SequenceError::InvalidCohort);
        }
        session.suffix(&receipt.program, receipt.stage)?;
        branches.push(Branch {
            table: table.clone(),
            origin,
            weight: *weight,
            receipt: receipt.clone(),
            first: None,
            first_program: None,
            tricks: Vec::new(),
            consumed: Vec::new(),
            interrupted: false,
        });
    }
    let mut choices = Vec::new();
    let terminals = session.finish(branches, &mut choices)?;
    Ok(DescendantRun {
        terminals,
        choices,
        work: session.work,
    })
}

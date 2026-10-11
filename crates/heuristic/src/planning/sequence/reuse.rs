//! Private completed leaves/dependencies; no weighted ranking survives a call.

use super::*;

#[derive(PartialEq)]
struct Scope {
    root: u8,
    params: crate::Params,
    belief_model: &'static str,
    response_model: &'static str,
    entries: usize,
    bytes: usize,
}

/// Bounded private scratch for one actor's modeled continuations. The callback
/// MUST return a semantically completed, deterministic response for its exact
/// information/legal inputs and configured model. Eligible but incomplete,
/// timed-partial or timed-fallback responses must return an error: they cannot
/// populate this cache. A bare GameAction result cannot enforce that contract;
/// the owner supplies the strict completed-response wrapper. Never serialize
/// or share this cache as another real player's knowledge or intent.
///
/// Posterior weights, cohort ids, sampling seed, prepared choices and weighted
/// optimums are NOT persistent. `Context.params` must include the actual leaf
/// configuration/RNG seed; version `response_model` when the callback changes.
/// Current Forecast's `Context.seed` generates the cohort, not actor responses.
/// Fork/model reset belongs to the caller via `clear`; equality is also checked
/// here. Bounds changes reset the completed-response scope between comparisons.
/// Solver effort is not in the present GameAction API. Represented engine
/// transitions/comparisons are debited on hits; canonical leaf/solver effort
/// needs the next additive action-plus-effort interface before exact logical
/// solver-budget parity can be qualified.
#[derive(Default)]
pub struct CompletedCache {
    scope: Option<Scope>,
    inputs: Inputs,
}

#[derive(Default)]
pub(super) struct Inputs {
    pub responses: Vec<(ResponseKey, GameAction)>,
    pub hard_reads: Vec<(Information, beliefs::ConceptQuery, Option<bool>)>,
    pub tails: Vec<TailRun>,
    pub canonical: Vec<CanonicalProgram>,
    pub memberships: Vec<AuthoredMembership>,
    pub bytes: usize,
    pub response_bytes: usize,
}

impl CompletedCache {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn retained_bytes(&self) -> usize {
        self.inputs.bytes
    }
    pub(super) fn begin(&mut self, root: u8, context: &Context, limits: Limits) -> Inputs {
        let scope = Scope {
            root,
            params: context.params.clone(),
            belief_model: context.provenance.belief_model,
            response_model: context.provenance.response_model,
            entries: limits.memo_entries,
            bytes: limits.memo_bytes,
        };
        if self.scope.as_ref() != Some(&scope) {
            self.clear();
            self.scope = Some(scope);
        }
        std::mem::take(&mut self.inputs)
    }
    pub(super) fn finish(&mut self, inputs: Inputs) {
        self.inputs = inputs;
    }
}

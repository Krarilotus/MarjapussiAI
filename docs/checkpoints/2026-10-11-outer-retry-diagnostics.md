# Outer pending-decision retry diagnostics

Status: **incomplete**, source-only isolated handoff. Root owns integration,
validation and focused commit/tag. No builds, tests or policy runs performed.

Baseline: executable-concept-contract `4bff913`, current integrated selected-plan
API. Scope: heuristic/lib.rs outer PendingDecision fast path and one focused
sequence_policy regression. No evaluator/strategy/recorder/parent API changes.

A cache hit preserves the Decision and semantic evidence, marks SequenceTrace
Retry and reports zero new Work. Original parent/child scope, support, receipts,
assessments and authorization remain unchanged. The first hit lazily changes the
cached diagnostic projection using Arc COW when shared; later hits reuse it.
Externally held original trace metadata remains untouched. Policy synchronization
still runs normally; zero Work means no new sequence evaluation, not zero CPU.

The test uses a real completed evaluator comparison containing Local evidence,
then seeds the transaction cache and invokes public decide twice and on a clone.
It checks exact action/receipt/assessment/semantic trace preservation, original
parent Arc identity, zero Work, preserved original snapshot, and projection reuse.
It does not qualify the production forecast's completion/performance.

Static Rust parsing and patch apply/whitespace checks only; test unrun. Retained
original total_candidates describes evidence scope, not newly attempted work.
No jobs/build outputs created; baselines and patch retained, 0 bytes reclaimed.

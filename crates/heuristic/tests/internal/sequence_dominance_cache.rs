//! Accounting companion: real completed Local proof, synthetic deterministic
//! nonzero leaf effort. No solver/runtime-performance claim.
use super::*;
use std::cell::Cell;

const NODES: u64 = 7;

fn response(_: &SeatView<'_>, legal: &[GameAction], budget: LeafBudget) -> Result<LeafResponse, LeafFailure> {
    if let Some(remaining) = budget.remaining_solver_nodes.filter(|n| *n < NODES) {
        return Err(LeafFailure {
            reason: SequenceError::Budget,
            canonical_solver_nodes: remaining,
        });
    }
    Ok(LeafResponse {
        // Same card choice as the parent's fixture leaf; Start is used only to
        // populate distinct response keys in the eviction test.
        action: *legal
            .iter()
            .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
            .unwrap_or(&legal[0]),
        canonical_solver_nodes: NODES,
    })
}

fn branches(table: &Table, candidate: &Candidate) -> Vec<Branch> {
    [(2, 0.25), (7, 0.75)]
        .into_iter()
        .map(|(origin, weight)| Branch {
            table: table.clone(),
            origin,
            weight,
            receipt: Receipt::new(candidate.program.clone(), &table.view(0)).unwrap(),
            first: None,
            first_program: None,
            tricks: vec![],
            consumed: vec![],
            interrupted: false,
        })
        .collect()
}

fn key(table: &Table, candidate: &Candidate) -> Vec<BranchKey> {
    branches(table, candidate)
        .iter()
        .map(|b| BranchKey::of(b, candidate.program.suffix(candidate.stage).unwrap(), None))
        .collect()
}

fn represented(work: Work) -> (usize, usize, u64) {
    (
        work.represented_transitions,
        work.represented_comparisons,
        work.represented_solver_nodes,
    )
}

fn assert_proof(records: &[PreparedChoice]) {
    let record = records
        .iter()
        .find(|r| matches!(r.proof, Some(SelectionProof::CompletionDominance(_))))
        .expect("a genuinely completed parent must contain the new retained payload");
    let certificate = proof(record);
    assert!(certificate.validates(record));
    assert_eq!(
        certificate.scope().support,
        vec![(2, 0.25_f64.to_bits()), (7, 0.75_f64.to_bits())]
    );
    assert_eq!(certificate.values().competitors.len() + 1, record.alternatives.len());
    assert!(record.assessment().is_none(), "no fabricated loser Assessment");
}

#[test]
fn completed_parent_hit_preserves_proof_and_nonzero_work_and_rejects_unaffordable_hit() {
    let table = fixture();
    let candidate = local(&cards(&table));
    let context = context();
    let mut uncached_leaf = response;
    let mut uncached = session(&context, &mut uncached_leaf);
    let mut expected_records = vec![];
    let expected = uncached
        .finish(branches(&table, &candidate), &mut expected_records)
        .unwrap();
    assert_proof(&expected_records);
    assert!(uncached.memo.is_empty());
    assert!(uncached.work.actual_solver_nodes > 0);
    assert_eq!(
        uncached.work.actual_solver_nodes,
        NODES * uncached.work.response_calls as u64
    );

    let calls = Cell::new(0usize);
    let mut counted = |view: &SeatView<'_>, legal: &[GameAction], budget: LeafBudget| {
        calls.set(calls.get() + 1);
        response(view, legal, budget)
    };
    let mut cached = session(&context, &mut counted);
    cached.limits = limits(true);
    let mut records = vec![];
    assert_eq!(
        cached.finish(branches(&table, &candidate), &mut records).unwrap(),
        expected
    );
    assert_eq!(records, expected_records);
    assert_eq!(represented(cached.work), represented(uncached.work));
    let parent_key = key(&table, &candidate);
    let run = cached.memo.iter().find(|r| r.key == parent_key).unwrap();
    assert_proof(&run.prepared);
    let cost = (run.transitions, run.comparisons, run.solver_nodes);
    assert_eq!(cost, represented(uncached.work));
    assert!(cost.1 > 0 && cost.2 > 0);
    let before = cached.work;
    let calls_before = calls.get();
    cached.limits.comparisons = before.represented_comparisons + cost.1 - 1;
    let mut rejected = vec![];
    assert_eq!(
        cached.finish(branches(&table, &candidate), &mut rejected),
        Err(SequenceError::Budget)
    );
    assert!(
        matches!(cached.limit, Some(DeferredLimit::RepresentedComparisons { required, remaining })
        if required == cost.1 && remaining == cost.1 - 1)
    );
    assert!(rejected.is_empty());
    assert_eq!(represented(cached.work), represented(before));
    assert_eq!(calls.get(), calls_before);
    assert_eq!(cached.work.suffix_hits, before.suffix_hits);

    cached.limits.comparisons = limits(true).comparisons;
    cached.limit = None;
    let mut reused = vec![];
    assert_eq!(
        cached.finish(branches(&table, &candidate), &mut reused).unwrap(),
        expected
    );
    assert_eq!(reused, expected_records);
    assert_eq!(
        cached.work.represented_transitions - before.represented_transitions,
        cost.0
    );
    assert_eq!(
        cached.work.represented_comparisons - before.represented_comparisons,
        cost.1
    );
    assert_eq!(
        cached.work.represented_solver_nodes - before.represented_solver_nodes,
        cost.2
    );
    assert_eq!(cached.work.suffix_hits, before.suffix_hits + 1);
    assert_eq!(cached.work.comparisons, before.comparisons, "proof is not reevaluated");
    assert_eq!(cached.work.actual_solver_nodes, before.actual_solver_nodes);
    assert_eq!(calls.get(), calls_before);
}

#[test]
fn response_pressure_preserves_or_clears_real_dominance_parent_with_exact_ledger() {
    let table = fixture();
    let candidate = local(&cards(&table));
    let context = context();
    let parent_key = key(&table, &candidate);
    for pressure in ["entries", "bytes", "both"] {
        let mut leaf = response;
        let mut state = session(&context, &mut leaf);
        state.limits = limits(true);
        let mut expected_records = vec![];
        let expected = state
            .finish(branches(&table, &candidate), &mut expected_records)
            .unwrap();
        assert_proof(&state.memo.iter().find(|r| r.key == parent_key).unwrap().prepared);
        // Build the genuine parent first, then exercise the existing 32-response
        // batch boundary without reducing the candidate/cohort/continuation domain.
        state.limits.memo_entries = 1;
        assert!(state.responses.len() < 32);
        let fresh = |shift: usize| {
            let hands = std::array::from_fn(|seat| {
                (0..9)
                    .map(|i| Card::from_index(((seat * 9 + i + shift) % 36) as u8))
                    .collect::<Vec<_>>()
            });
            Table::new(&hands)
        };
        let mut shift = 0;
        while state.responses.len() < 32 {
            state.response(&fresh(shift)).unwrap();
            shift += 1;
            assert!(shift <= 32);
        }
        let next = fresh(shift);
        let (actor, legal) = next.to_act().unwrap();
        let legal = crate::legal_actions::eligible(legal);
        assert!(!state
            .responses
            .iter()
            .any(|(k, _)| k.information.matches(&next.view(actor)) && k.legal == legal));
        let total = state.work.retained_bytes;
        let reusable = state.reusable_bytes;
        let old_responses = state.response_bytes;
        let memo_count = state.memo.len();
        assert!(total > reusable && reusable >= old_responses && old_responses > 0);
        if pressure == "bytes" {
            state.limits.memo_entries = 2;
        }
        if pressure != "entries" {
            state.limits.memo_bytes = total;
        }
        state.response(&next).unwrap();
        assert!(state.work.retained_bytes <= state.limits.memo_bytes);
        assert_eq!(state.responses.len(), if pressure == "bytes" { 33 } else { 1 });
        if pressure == "entries" {
            assert_eq!(state.memo.len(), memo_count);
            assert_eq!(state.work.retained_bytes, total - old_responses + state.response_bytes);
            assert_eq!(state.reusable_bytes, reusable - old_responses + state.response_bytes);
            let run = state.memo.iter().find(|r| r.key == parent_key).unwrap();
            assert_proof(&run.prepared);
            let cost = (run.transitions, run.comparisons, run.solver_nodes);
            let before = state.work;
            let mut reused = vec![];
            assert_eq!(
                state.finish(branches(&table, &candidate), &mut reused).unwrap(),
                expected
            );
            assert_eq!(reused, expected_records);
            assert_eq!(
                state.work.represented_transitions - before.represented_transitions,
                cost.0
            );
            assert_eq!(
                state.work.represented_comparisons - before.represented_comparisons,
                cost.1
            );
            assert_eq!(
                state.work.represented_solver_nodes - before.represented_solver_nodes,
                cost.2
            );
            assert_eq!(state.work.response_calls, before.response_calls);
            assert_eq!(state.work.actual_solver_nodes, before.actual_solver_nodes);
            assert_eq!(state.work.suffix_hits, before.suffix_hits + 1);
        } else {
            assert!(
                state.memo.is_empty(),
                "byte pressure releases proof-bearing cache entries"
            );
            assert!(state.tails.is_empty() && state.hard_reads.is_empty());
            assert!(state.canonical.is_empty() && state.memberships.is_empty());
            assert_eq!(state.work.retained_bytes, state.response_bytes);
            assert_eq!(state.reusable_bytes, state.response_bytes);
            assert_proof(&expected_records); // caller-owned result is not a cache entry
        }
    }
}

#[test]
fn genuine_parent_payload_has_exact_and_one_less_byte_admission_boundaries() {
    let table = fixture();
    let candidate = local(&cards(&table));
    let context = context();
    let parent_key = key(&table, &candidate);
    let mut leaf = response;
    let mut baseline = session(&context, &mut leaf);
    baseline.limits = limits(true);
    let mut expected_records = vec![];
    let expected = baseline
        .finish(branches(&table, &candidate), &mut expected_records)
        .unwrap();
    assert_proof(&expected_records);
    assert!(baseline.memo.last().is_some_and(|run| run.key == parent_key));
    let required = baseline.work.retained_bytes;
    let run = baseline.memo.last().unwrap();
    // Exact ledger boundary for the real borrowed payload, without inventing
    // loser outcomes. Clone capacities need not equal pre-clone admission sizes.
    let payload = run_bytes(&run.key, &run.terminals, &run.prepared);
    let proof_bytes = run
        .prepared
        .iter()
        .filter_map(|r| match &r.proof {
            Some(SelectionProof::CompletionDominance(p)) => Some(p.owned_bytes()),
            _ => None,
        })
        .sum::<usize>();
    assert!(proof_bytes > 0 && payload >= 2 * proof_bytes);
    for cap in [payload - 1, payload] {
        let mut leaf = response;
        let mut ledger = session(&context, &mut leaf);
        ledger.limits.memo_bytes = cap;
        assert_eq!(ledger.retain(payload), cap == payload);
        assert_eq!(ledger.work.retained_bytes, if cap == payload { payload } else { 0 });
    }
    // Full finish path: the parent is the final admission after all identical
    // children. One less byte rejects only that optional cache entry.
    for cap in [required - 1, required] {
        let mut leaf = response;
        let mut state = session(&context, &mut leaf);
        state.limits = Limits {
            memo_bytes: cap,
            ..limits(true)
        };
        let mut records = vec![];
        assert_eq!(
            state.finish(branches(&table, &candidate), &mut records).unwrap(),
            expected
        );
        assert_eq!(records, expected_records);
        assert_eq!(represented(state.work), represented(baseline.work));
        assert_eq!(state.work.actual_solver_nodes, baseline.work.actual_solver_nodes);
        assert_eq!(state.memo.iter().any(|r| r.key == parent_key), cap == required);
        assert_eq!(state.memo.len(), baseline.memo.len() - usize::from(cap < required));
        assert!(state.work.retained_bytes <= cap);
        if cap == required {
            assert_eq!(state.work.retained_bytes, required);
        }
    }
}

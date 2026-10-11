//! Bounded, exact authored-program lookup. Fingerprints are accelerators only;
//! every hit checks full equality, including labels, version and unused nodes.
//! Sorted storage avoids a second allocation/index and shares batch eviction
//! with the existing cache. No fingerprint is a model identity or a receipt.

use super::{Program, Suffix};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub(super) struct CanonicalProgram {
    pub fingerprint: u64,
    pub program: Program,
    pub suffixes: Vec<Option<Suffix>>,
}

/// Return the exact entry, or its insertion point after the collision bucket.
/// Bucket order never orders authored alternatives or breaks value ties.
pub(super) fn locate(entries: &[CanonicalProgram], program: &Program, fingerprint: u64) -> Result<usize, usize> {
    let first = entries.partition_point(|entry| entry.fingerprint < fingerprint);
    let mut end = first;
    while let Some(entry) = entries.get(end).filter(|entry| entry.fingerprint == fingerprint) {
        if entry.program == *program {
            return Ok(end);
        }
        end += 1;
    }
    Err(end)
}

// Derived Hash follows the model's fields without a second traversal owner.
// Fingerprints are private lookup hints, not stable semantic/model identities;
// full Program equality remains authoritative for every collision candidate.
pub(super) fn fingerprint(program: &Program) -> u64 {
    let mut hash = DefaultHasher::new();
    program.hash(&mut hash);
    hash.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::concepts::sequence::{Node, Stage};

    #[test]
    fn exact_lookup_resolves_collisions_and_preserves_sorted_buckets() {
        let original = Program { name: "collision", version: 1, start: Stage(0), nodes: vec![Node::Tail] };
        let mut changed = original.clone();
        changed.nodes.push(Node::Deferred); // unused authored data is still exact input
        let mut renamed = original.clone();
        renamed.name = "renamed";
        let mut versioned = original.clone();
        versioned.version = 2;
        let mut entries = Vec::new();
        // Force the same fingerprint for distinct inputs, with adjacent buckets.
        for (program, key) in [(&original, 9), (&changed, 9), (&renamed, 9), (&original, 1), (&original, 20)] {
            let index = locate(&entries, program, key).unwrap_err();
            entries.insert(index, CanonicalProgram { fingerprint: key, program: program.clone(), suffixes: vec![] });
        }
        assert_eq!(locate(&entries, &original, 9), Ok(1));
        assert_eq!(locate(&entries, &changed, 9), Ok(2));
        assert_eq!(locate(&entries, &renamed, 9), Ok(3));
        assert_eq!(locate(&entries, &versioned, 9), Err(4));
        assert_eq!(locate(&entries, &original, 10), Err(4));
        assert_eq!(fingerprint(&original), fingerprint(&original.clone()));
    }
}

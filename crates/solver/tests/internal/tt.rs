use super::*;
use rand::{rngs::StdRng, Rng, SeedableRng};

fn key(n: u64) -> Key {
    Key([n, n.rotate_left(7), n.rotate_left(19), n.rotate_left(31)])
}

fn value(table: &TransTable, key: &Key) -> Option<(i16, i16, u8)> {
    table.probe(key).map(|e| (e.lo, e.hi, e.best))
}

fn collision(table: &TransTable, first: Key) -> Key {
    (1..1024)
        .map(key)
        .find(|candidate| *candidate != first && table.slot(candidate) == table.slot(&first))
        .expect("small table must have a collision in this fixture")
}

#[test]
fn epoch_fits_previous_entry_size_and_alignment() {
    // The old field layout is the memory baseline, not a packed substitute.
    #[allow(dead_code)]
    struct PreviousEntry {
        key: Key,
        lo: i16,
        hi: i16,
        best: u8,
    }
    assert_eq!(std::mem::size_of::<Entry>(), std::mem::size_of::<PreviousEntry>());
    assert_eq!(std::mem::align_of::<Entry>(), std::mem::align_of::<PreviousEntry>());
    #[cfg(target_pointer_width = "64")]
    assert_eq!(std::mem::size_of::<Entry>(), 40);
}

#[test]
fn colliding_key_always_replaces_bounds_and_best_move() {
    let mut table = TransTable::new(2);
    let a = key(0);
    let b = collision(&table, a);
    table.store(a, 40, 50, 3);
    table.store(b, 10, 90, 7);
    assert_eq!(value(&table, &a), None);
    assert_eq!(value(&table, &b), Some((10, 90, 7)));
    table.store(a, 0, 100, NO_MOVE);
    assert_eq!(value(&table, &b), None);
    assert_eq!(value(&table, &a), Some((0, 100, NO_MOVE)));
}

#[test]
fn same_key_tightens_only_in_current_epoch() {
    let mut table = TransTable::new(2);
    let a = key(3);
    assert_eq!(value(&table, &a), None);
    table.store(a, i32::MIN, i32::MAX, 1);
    assert_eq!(value(&table, &a), Some((i16::MIN, i16::MAX, 1)));
    table.store(a, 10, 90, 2);
    table.store(a, 20, 100, 3);
    table.store(a, 0, 80, 4);
    assert_eq!(value(&table, &a), Some((20, 80, 4)));
    // A weaker bound still replaces best, including the no-move sentinel.
    table.store(a, 0, 100, NO_MOVE);
    assert_eq!(value(&table, &a), Some((20, 80, NO_MOVE)));
    table.clear();
    assert_eq!(value(&table, &a), None);
    table.store(a, 0, 100, 5);
    assert_eq!(value(&table, &a), Some((0, 100, 5)));
    table.store(a, 30, 70, 6);
    assert_eq!(value(&table, &a), Some((30, 70, 6)));
}

#[test]
fn wrap_erases_untouched_first_epoch_and_last_epoch_entries() {
    let mut table = TransTable::new(2);
    let old = key(0);
    let recent = (1..1024)
        .map(key)
        .find(|candidate| table.slot(candidate) != table.slot(&old))
        .unwrap();
    table.store(old, 30, 40, 2);
    // Exercise the real clear path through a full epoch cycle. The old slot
    // stays untouched, so simply resetting the counter would resurrect it.
    for _ in 1..u16::MAX {
        table.clear();
    }
    assert_eq!(table.epoch, u16::MAX);
    assert_eq!(value(&table, &old), None);
    table.store(recent, 50, 60, 3);
    assert_eq!(value(&table, &recent), Some((50, 60, 3)));
    table.clear();
    assert_eq!(table.epoch, 1);
    assert_eq!(value(&table, &old), None);
    assert_eq!(value(&table, &recent), None);
    assert!(table.entries.iter().all(|entry| entry.epoch == 0));
    table.store(old, 0, 100, 4);
    assert_eq!(value(&table, &old), Some((0, 100, 4)));
}

#[test]
fn operation_stream_matches_physical_clear_reference() {
    // Independent validity representation: physically empty Option slots.
    // Deliberately tiny capacity makes replacement and repeated-key tightening
    // common; compare every candidate after each store or clear.
    let mut table = TransTable::new(2);
    let mut reference: Vec<Option<(Key, i16, i16, u8)>> = vec![None; 4];
    let keys: Vec<_> = (0..24).map(key).collect();
    let mut rng = StdRng::seed_from_u64(0xE90C_2026);
    for step in 0..1024 {
        if step % 17 == 0 {
            table.clear();
            reference.fill(None);
        } else {
            let k = keys[rng.random_range(0..keys.len())];
            let lo = rng.random_range(0..40i16);
            let hi = rng.random_range(60..100i16);
            let best = rng.random::<u8>();
            table.store(k, lo as i32, hi as i32, best);
            let slot = (k.hash() >> 62) as usize;
            reference[slot] = Some(match reference[slot] {
                Some((old, lower, upper, _)) if old == k => (k, lower.max(lo), upper.min(hi), best),
                _ => (k, lo, hi, best),
            });
        }
        for k in &keys {
            let expected = reference[(k.hash() >> 62) as usize]
                .filter(|(stored, ..)| stored == k)
                .map(|(_, lo, hi, best)| (lo, hi, best));
            assert_eq!(value(&table, k), expected, "step {step}, key {k:?}");
        }
    }
}

#[test]
fn packed_ranks_are_no_pair_halves() {
    // The key packs the lowest ZERO_POINT_RANKS of each suit; none of them may be a King or
    // Ober (a pair half), whose identity the play depends on beyond its order.
    for suit in SUITS {
        for rank in 0..ZERO_POINT_RANKS {
            let card = suit as u8 * SUIT_CARDS + rank;
            assert!(!engine::CardSet::halves(suit).contains(card), "{suit:?} rank {rank}");
            assert_eq!(CARD_POINTS[card as usize], 0);
        }
    }
}

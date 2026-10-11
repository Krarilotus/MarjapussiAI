//! Independent finite oracle for the declared Q extension, not the real posterior.
//! No engine replay is needed for the 90,090-path count/density comparison.

use super::*;
use std::collections::BTreeMap;

fn choose(n: usize, k: usize) -> usize {
    if k > n {
        return 0;
    }
    (0..k.min(n - k)).fold(1, |value, i| value * (n - i) / (i + 1))
}

fn subsets(cards: CardSet, count: u32) -> Vec<CardSet> {
    let cards: Vec<_> = cards.iter().collect();
    assert!(cards.len() <= 18); // Test-only finite universe, not a general enumerator.
    (0u32..1u32 << cards.len())
        .filter(|mask| mask.count_ones() == count)
        .map(|mask| {
            cards.iter().enumerate().fold(CardSet::EMPTY, |set, (i, &card)| {
                if mask & (1 << i) != 0 {
                    set | CardSet::single(card)
                } else {
                    set
                }
            })
        })
        .collect()
}

fn reference_density(final_first: CardSet, original_first: CardSet) -> f64 {
    // BaseOnly has flat bid likelihoods. Under the declared rejection envelope,
    // each feature changing owner contributes (1-stay)/stay, in either direction.
    let moved = ((final_first - original_first) | (original_first - final_first)) & (CardSet::ACES | CardSet::HALVES);
    let stay = beliefs::UNIFORM_STAY as f64;
    ((1.0 - stay) / stay).powi(moved.len() as i32)
}

struct Oracle {
    multiplicities: BTreeMap<u64, usize>,
    by_swaps: [f64; 5],
    feature: u8,
    retained_feature: f64,
    normalizer: f64,
}

fn original_split_oracle(hands: &Hands, first: usize, second: usize) -> Oracle {
    let final_first = hands[first];
    let team = final_first | hands[second];
    assert_eq!(final_first.len(), 9);
    assert_eq!(hands[second].len(), 9);
    assert_eq!(team.len(), 18);
    let feature = (final_first & (CardSet::ACES | CardSet::HALVES)).iter().next().unwrap();
    let mut result = Oracle {
        multiplicities: BTreeMap::new(),
        by_swaps: [0.0; 5],
        feature,
        retained_feature: 0.0,
        normalizer: 0.0,
    };
    // This enumerates original nine-card splits, NOT the reverse-transfer tree.
    // If k cards cross each way, the return contains those k cards plus 4-k
    // cards returned to their original holder. Choose those from the 9-k stays.
    for original_first in subsets(team, 9) {
        let k = (original_first - final_first).len() as usize;
        if k > 4 {
            continue; // Four-card transfers cannot realize this original split.
        }
        let multiplicity = choose(9 - k, 4 - k);
        let mass = multiplicity as f64 * reference_density(final_first, original_first);
        result.multiplicities.insert(original_first.0, multiplicity);
        result.by_swaps[k] += mass;
        result.normalizer += mass;
        if original_first.contains(feature) {
            result.retained_feature += mass;
        }
    }
    result
}

#[test]
fn two_pass_reverse_paths_match_independent_original_split_multiplicity_and_density() {
    let table = fixture(true);
    let view = table.view(1);
    let first = view.knowledge.first_passer().unwrap() as usize;
    let second = engine::partner(first as u8) as usize;
    let hands = std::array::from_fn(|seat| table.hand(seat as u8));
    let mind = reader(&view);
    let model = mind.deal_model();
    let weight_before = model.weights().weight(&hands);
    let oracle = original_split_oracle(&hands, first, second);
    let mut seen = BTreeMap::<u64, usize>::new();
    let mut counts = [0usize; 5];
    let mut mass_by_swaps = [0.0; 5];
    // Separate reference traversal of every B subset M, then every F subset V+B.
    // Returned cards F intersect B are deliberately allowed.
    for back in subsets(hands[first], 4) {
        for forth in subsets(hands[second] | back, 4) {
            let mut dealt = hands;
            dealt[first] = (hands[first] - back) | forth;
            dealt[second] = (hands[second] | back) - forth;
            assert_eq!(dealt[first].len(), 9);
            assert_eq!(dealt[second].len(), 9);
            assert!((dealt[first] & dealt[second]).is_empty());
            assert_eq!(dealt[first] | dealt[second], hands[first] | hands[second]);
            let k = (dealt[first] - hands[first]).len() as usize;
            assert_eq!((forth & back).len() as usize, 4 - k);
            *seen.entry(dealt[first].0).or_default() += 1;
            counts[k] += 1;
            let expected = reference_density(hands[first], dealt[first]);
            let actual = model.weights().history_acceptance(&hands, &dealt).unwrap();
            assert!((actual - expected).abs() < 1e-12);
            mass_by_swaps[k] += actual;
        }
    }
    assert_eq!(counts, [126, 4536, 27216, 42336, 15876]);
    assert_eq!(counts.iter().sum::<usize>(), 90_090);
    assert_eq!(seen.len(), 24_310);
    assert_eq!(seen, oracle.multiplicities);
    for (actual, expected) in mass_by_swaps.into_iter().zip(oracle.by_swaps) {
        assert!((actual - expected).abs() <= 1e-9 * expected.max(1.0));
    }
    assert_eq!(model.weights().weight(&hands), weight_before);
}

#[test]
fn seeded_two_pass_samples_match_the_independently_weighted_history_oracle() {
    const SAMPLES: usize = 1024;
    let table = fixture(true);
    let view = table.view(1);
    let first = view.knowledge.first_passer().unwrap() as usize;
    let second = engine::partner(first as u8) as usize;
    let hands = std::array::from_fn(|seat| table.hand(seat as u8));
    let oracle = original_split_oracle(&hands, first, second);
    let mind = reader(&view);
    let model = mind.deal_model();
    let weight = 1.0 / SAMPLES as f64;
    let slots = vec![World { hands, weight }; SAMPLES];
    // Exercise the actual completion/rejection loop, not a test-copy sampler.
    let completed = complete_slots(
        &view,
        &slots,
        model.weights(),
        0x5457_4F50_4153_5351,
        HistoryLimits {
            max_proposals_per_slot: 4096,
        },
        &mut || true,
    )
    .unwrap();
    assert_eq!(completed.tables.len(), SAMPLES);
    let mut counts = [0usize; 5];
    let mut retained_feature = 0;
    for (sample, sample_weight) in completed.tables {
        assert_eq!(sample_weight, weight);
        assert_eq!(sample.view(1).events, view.events);
        let original_first = sample.view(first as u8).knowledge.dealt_certain(first as u8);
        assert_eq!(original_first.len(), 9);
        assert!(oracle.multiplicities.contains_key(&original_first.0));
        counts[(original_first - hands[first]).len() as usize] += 1;
        retained_feature += usize::from(original_first.contains(oracle.feature));
    }
    // Fixed seed, broad six-sigma + six-count sanity bounds. This is a sampler
    // regression, not a statistical calibration claim or a proof of Q adequacy.
    let check_frequency = |observed: usize, mass: f64| {
        let p = mass / oracle.normalizer;
        let expected = SAMPLES as f64 * p;
        let tolerance = 6.0 * (SAMPLES as f64 * p * (1.0 - p)).sqrt() + 6.0;
        assert!(
            (observed as f64 - expected).abs() <= tolerance,
            "observed={observed}, expected={expected}, tolerance={tolerance}"
        );
    };
    for (observed, mass) in counts.into_iter().zip(oracle.by_swaps) {
        check_frequency(observed, mass);
    }
    check_frequency(retained_feature, oracle.retained_feature);
}

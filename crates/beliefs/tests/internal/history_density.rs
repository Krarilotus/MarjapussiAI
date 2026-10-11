use super::*;
use agent::SeatKind;
use observation::SeatEvent;

fn weights() -> (DealWeights, Hands, Hands) {
    let initial: Hands =
        std::array::from_fn(|seat| (0..9).fold(CardSet::EMPTY, |set, i| set | CardSet::single((4 * i + seat) as u8)));
    let given = initial[2].cards()[..4]
        .iter()
        .fold(CardSet::EMPTY, |s, c| s | CardSet::of(c));
    let mut current = initial;
    current[2] -= given;
    current[0] |= given;
    let mut mind = Mind::new(1, initial[1], [SeatKind::Unknown; 4], crate::BeliefParams::base_only());
    for event in [
        SeatEvent::Bid { seat: 0, value: 120 },
        SeatEvent::StopBidding { seat: 1 },
        SeatEvent::StopBidding { seat: 2 },
        SeatEvent::StopBidding { seat: 3 },
        SeatEvent::Pass {
            from: 2,
            to: 0,
            cards: None,
        },
    ] {
        mind.apply(&event).unwrap();
    }
    (mind.deal_weights(), current, initial)
}

#[test]
fn completion_density_uses_existing_dealt_likelihoods_and_kernel_without_reweighting() {
    let (mut weights, current, dealt) = weights();
    let original_weight = weights.weight(&current);
    let base = weights.history_acceptance(&current, &dealt).unwrap();
    assert!(base > 0.0 && base <= 1.0);
    assert_eq!(weights.weight(&current), original_weight);
    // These are the same stored class likelihoods consumed by DealWeights::weight,
    // not a new convention encoder or another posterior product.
    let mut likelihood = Box::new([1.0f32; crate::conventions::N_CLASSES]);
    likelihood[HandClass::of(dealt[0]).index()] = 0.25;
    weights.likelihoods[0] = Some(likelihood);
    assert!((weights.history_acceptance(&current, &dealt).unwrap() - base * 0.25).abs() < 1e-12);
    weights.kernel = Kernel { stay: [[0.5; 2]; 2] };
    assert_eq!(weights.history_acceptance(&current, &dealt), Some(0.25));
    // Zero density is retained as zero; no epsilon or fallback distribution is invented.
    weights.likelihoods[0].as_mut().unwrap()[HandClass::of(dealt[0]).index()] = 0.0;
    assert_eq!(weights.history_acceptance(&current, &dealt), Some(0.0));
}

#[test]
fn invalid_completion_or_model_cannot_publish_a_density() {
    let (mut weights, current, dealt) = weights();
    let mut bad = dealt;
    bad[0] = bad[2];
    assert_eq!(weights.history_acceptance(&current, &bad), None);
    weights.kernel.stay[0][0] = f32::NAN;
    assert_eq!(weights.history_acceptance(&current, &dealt), None);
}

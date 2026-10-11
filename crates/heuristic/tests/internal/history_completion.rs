use super::*;
use beliefs::{BeliefParams, Mind};
use engine::{ActionType, Card};
use knowledge::Belief;

fn fixture(both: bool) -> Table {
    let hands = std::array::from_fn(|seat| (0..9).map(|i| Card::from_index((4 * i + seat) as u8)).collect());
    let mut table = Table::new(&hands);
    for kind in [
        ActionType::NewBid(120),
        ActionType::StopBidding,
        ActionType::StopBidding,
        ActionType::StopBidding,
    ] {
        let action = *table
            .to_act()
            .unwrap()
            .1
            .iter()
            .find(|a| a.action_type == kind)
            .unwrap();
        table.apply(action);
    }
    for _ in 0..if both { 2 } else { 1 } {
        let action = *table
            .to_act()
            .unwrap()
            .1
            .iter()
            .find(|a| matches!(a.action_type, ActionType::Pass(_)))
            .unwrap();
        table.apply(action);
    }
    table
}

fn reader(view: &SeatView<'_>) -> Mind {
    let mut mind = Mind::new(
        view.seat,
        view.knowledge.dealt_certain(view.seat),
        view.kinds,
        BeliefParams::base_only(),
    );
    for event in view.events {
        mind.apply(event).unwrap();
    }
    assert_eq!(mind.knowledge(), view.knowledge);
    mind
}

fn signature(result: &CompletedHistories) -> Vec<(Vec<Vec<SeatEvent>>, f64)> {
    result
        .tables
        .iter()
        .map(|(table, weight)| ((0..4).map(|seat| table.view(seat).events.to_vec()).collect(), *weight))
        .collect()
}

#[test]
fn completion_preserves_each_duplicate_slot_mass_and_deterministic_filtered_history() {
    for both in [false, true] {
        let table = fixture(both);
        let view = table.view(1);
        let mind = reader(&view);
        let model = mind.deal_model();
        let hands = std::array::from_fn(|seat| table.hand(seat as u8));
        let slots: Vec<_> = [0.125, 0.125, 0.25, 0.5]
            .into_iter()
            .map(|weight| World { hands, weight })
            .collect();
        let limits = HistoryLimits {
            max_proposals_per_slot: 4096,
        };
        let first = complete_slots(&view, &slots, model.weights(), 42, limits, &mut || true).unwrap();
        let second = complete_slots(&view, &slots, model.weights(), 42, limits, &mut || true).unwrap();
        assert_eq!(signature(&first), signature(&second));
        assert_eq!(first.proposals, second.proposals);
        assert_eq!(first.model, MODEL);
        assert_eq!(first.tables.len(), slots.len());
        assert_eq!(
            first.tables.iter().map(|x| x.1).collect::<Vec<_>>(),
            slots.iter().map(|x| x.weight).collect::<Vec<_>>()
        );
        for (rebuilt, _) in &first.tables {
            assert_eq!(rebuilt.view(1).events, view.events);
            assert_eq!(rebuilt.view(1).knowledge, view.knowledge);
            for seat in 0..4 {
                assert_eq!(rebuilt.hand(seat), hands[seat as usize]);
            }
            assert!(rebuilt
                .view(2)
                .events
                .iter()
                .any(|e| matches!(e, SeatEvent::Pass { cards: Some(_), .. })));
        }
        assert!(
            first
                .tables
                .iter()
                .skip(1)
                .any(|(t, _)| t.view(2).events != first.tables[0].0.view(2).events),
            "duplicate current deals need separate private histories"
        );
        // No actual backend history enters the API. Any reconstructed history
        // with the same observer view must yield exactly the same seeded result.
        let alternate_view = first.tables[0].0.view(1);
        let alternate_mind = reader(&alternate_view);
        let alternate_model = alternate_mind.deal_model();
        let alternate = complete_slots(
            &alternate_view,
            &slots,
            alternate_model.weights(),
            42,
            limits,
            &mut || true,
        )
        .unwrap();
        assert_eq!(signature(&first), signature(&alternate));
    }
}

#[test]
fn bounded_or_interrupted_completion_never_returns_a_partial_cohort() {
    let table = fixture(true);
    let view = table.view(1);
    let mind = reader(&view);
    let model = mind.deal_model();
    let hands = std::array::from_fn(|seat| table.hand(seat as u8));
    let slots = [World { hands, weight: 0.4 }, World { hands, weight: 0.6 }];
    let limits = HistoryLimits {
        max_proposals_per_slot: 4096,
    };
    assert!(matches!(
        complete_slots(
            &view,
            &slots,
            model.weights(),
            7,
            HistoryLimits {
                max_proposals_per_slot: 0
            },
            &mut || true
        ),
        Err(HistoryError::ProposalLimit { slot: 0, proposals: 0 })
    ));
    let mut calls = 0;
    let full = complete_slots(&view, &slots, model.weights(), 7, limits, &mut || {
        calls += 1;
        true
    })
    .unwrap();
    for allowed in [0, calls / 2, calls - 1] {
        let mut count = 0;
        assert!(matches!(
            complete_slots(&view, &slots, model.weights(), 7, limits, &mut || {
                count += 1;
                count <= allowed
            }),
            Err(HistoryError::Interrupted)
        ));
    }
    let retried = complete_slots(&view, &slots, model.weights(), 7, limits, &mut || true).unwrap();
    assert_eq!(signature(&retried), signature(&full));
    let invalid = [
        slots[0],
        World {
            hands,
            weight: f64::NAN,
        },
    ];
    assert!(matches!(
        complete_slots(&view, &invalid, model.weights(), 7, limits, &mut || true),
        Err(HistoryError::InvalidWeight)
    ));
}

#[test]
fn exact_forth_history_oracle_normalizes_q_without_changing_current_weight() {
    let table = fixture(false);
    let view = table.view(1);
    let mind = reader(&view);
    let model = mind.deal_model();
    let hands = std::array::from_fn(|seat| table.hand(seat as u8));
    let weight_before = model.weights().weight(&hands);
    let recipient = hands[0].cards();
    assert_eq!(recipient.len(), 13);
    let index = view
        .events
        .iter()
        .position(|e| matches!(e, SeatEvent::Pass { cards: None, .. }))
        .unwrap();
    let mut densities = Vec::new();
    for a in 0..10 {
        for b in a + 1..11 {
            for c in b + 1..12 {
                for d in c + 1..13 {
                    let given = CardSet::from_cards(&[recipient[a], recipient[b], recipient[c], recipient[d]]);
                    let mut dealt = hands;
                    dealt[0] -= given;
                    dealt[2] |= given;
                    let density = model.weights().history_acceptance(&hands, &dealt).unwrap();
                    // Independent BaseOnly kernel reference: moved feature cards contribute
                    // (1-stay)/stay; all retained features contribute one under the envelope.
                    let moved = (given & (CardSet::ACES | CardSet::HALVES)).len();
                    let stay = beliefs::UNIFORM_STAY as f64;
                    let expected = ((1.0 - stay) / stay).powi(moved as i32);
                    assert!((density - expected).abs() < 1e-12);
                    let restored = Table::from_view_with_passes(
                        &view,
                        hands,
                        &[HypothesizedPass {
                            event_index: index,
                            cards: given,
                        }],
                    )
                    .unwrap();
                    assert_eq!(restored.view(1).events, view.events);
                    densities.push(density);
                }
            }
        }
    }
    assert_eq!(densities.len(), 715);
    let normalization: f64 = densities.iter().sum();
    assert!(normalization > 0.0);
    let marginal: f64 = densities.iter().map(|g| 0.37 * g / normalization).sum();
    assert!((marginal - 0.37).abs() < 1e-12);
    assert_eq!(
        model.weights().weight(&hands),
        weight_before,
        "completion does not change W(C)"
    );
}

#[test]
fn defender_forecast_completes_hidden_history_and_labels_conditional_sampling() {
    use super::super::{Forecast, PreparationError};
    use crate::concepts::value::CohortMethod;
    use crate::{Ctx, Params};
    use std::{
        cell::{Cell, RefCell},
        time::{Duration, Instant},
    };

    let mut table = fixture(true);
    let opening = *table
        .to_act()
        .unwrap()
        .1
        .iter()
        .find(|a| matches!(a.action_type, ActionType::CardPlayed(_)))
        .unwrap();
    table.apply(opening);
    assert_eq!(table.to_act().unwrap().0, 1);
    let view = table.view(1);
    let mind = reader(&view);
    let params = Params {
        beliefs: BeliefParams::base_only(),
        ..Params::default()
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    let ctx = Ctx {
        me: view.seat,
        k: mind.knowledge(),
        mind: &mind,
        params: &params,
        solver: &RefCell::new(None),
        seed: 42,
        deadline: Some(deadline),
        raw_events: view.events,
        routes: RefCell::new(None),
        concepts: RefCell::new(crate::concepts::Reads::default()),
        intent: &RefCell::new(None),
        completed: &RefCell::default(),
        formation_attempted: &Cell::new(false),
        response_completion: None,
        series: view.series,
    };
    let forecast = Forecast::new_checked(&ctx, 2, deadline, &mut || true).unwrap();
    assert_eq!(forecast.tables.len(), 2);
    assert_eq!(forecast.context.proposal_draws, 2 * 256);
    assert!(matches!(
        forecast.provenance.cohort,
        CohortMethod::HistorySampled {
            current_draws: 512,
            model: MODEL,
            slots: 2,
            ..
        }
    ));
    for (table, _) in &forecast.tables {
        assert_eq!(table.view(1).events, view.events);
        assert_eq!(table.view(1).knowledge, view.knowledge);
    }
    assert!(matches!(
        Forecast::new_checked(&ctx, 2, deadline, &mut || false),
        Err(PreparationError::Deferred)
    ));
}

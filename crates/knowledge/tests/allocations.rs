use engine::{CardSet, CARDS};
use knowledge::{DealSpace, Hands, Knowledge, Tracker};
use observation::{SeatEvent, SeatFilter};
use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};

fn hand() -> CardSet {
    (0..9).fold(CardSet::EMPTY, |h, c| h | CardSet::single(c))
}

#[test]
fn identical_roles_count_allocations_without_enumerating_deals() {
    let space = DealSpace::new(&Knowledge::new(0, hand()));
    let mut classes = 0;
    let result = space.visit_allocations(&[0; CARDS], 100, |hands, ways| {
        classes += 1;
        assert_eq!(ways, space.count());
        assert!(hands.iter().all(|h| h.len() == 9));
        false
    });
    assert!(result.complete());
    assert_eq!(classes, 1);
    assert!(result.covered_deals > 1_000_000);
    assert!(result.nodes < 20);
}

#[test]
fn private_hand_partition_keeps_opponent_permutations_compact() {
    let k = Knowledge::new(0, hand());
    let space = k.deals();
    let mut private = Vec::new();
    let visit = space.visit_private_allocations(&k, 1, &[0; CARDS], 1_000, 2, |hands, ways| {
        private.push(hands[1]);
        assert_eq!(ways, 48_620, "18 choose 9 opponent allocations per private hand");
        false
    });
    assert_eq!(private.len(), 2);
    assert_ne!(private[0], private[1]);
    assert_eq!(visit.classes, 2);
    assert_eq!(visit.covered_deals, 2 * 48_620);
    assert!(!visit.complete());
    assert!(visit.nodes < 100, "no repeated charging for complete opponent worlds");
}

#[test]
fn refinement_preserves_exact_pair_correlation_and_total_multiplicity() {
    let space = DealSpace::new(&Knowledge::new(0, hand()));
    let a = 9;
    let b = 10;
    let labels = std::array::from_fn(|c| u64::from(c == a || c == b));
    let (mut pair, mut one, mut total) = (0, 0, 0);
    let result = space.visit_allocations(&labels, 1_000, |hands, ways| {
        let n = (hands[1] & (CardSet::single(a as u8) | CardSet::single(b as u8))).len();
        pair += if n == 2 { ways } else { 0 };
        one += if n == 1 { ways } else { 0 };
        total += ways;
        false
    });
    assert!(result.complete());
    assert_eq!(total, space.count());
    assert!((pair as f64 / total as f64 - (9.0 / 27.0) * (8.0 / 26.0)).abs() < 1e-14);
    assert!((one as f64 / total as f64 - 2.0 * (9.0 / 27.0) * (18.0 / 26.0)).abs() < 1e-14);
}

#[test]
fn tied_constraint_groups_are_refined_without_losing_or_repeating_deals() {
    let mut k = Knowledge::new(0, hand());
    let suit = engine::Suit::Red;
    k.apply(&SeatEvent::NoPair { seat: 1 }).unwrap();
    let space = DealSpace::new(&k);
    let halves = CardSet::halves(suit);
    let labels = std::array::from_fn(|c| u64::from(halves.contains(c as u8)));
    let mut total = 0;
    let result = space.visit_allocations(&labels, 100_000, |hands, ways| {
        assert!(!halves.is_subset(hands[1]));
        total += ways;
        false
    });
    assert!(result.complete());
    assert_eq!(total, space.count());
}

#[test]
fn work_and_callback_stops_report_the_unexplored_deals() {
    let space = Knowledge::new(0, hand()).deals();
    let labels = std::array::from_fn(|c| c as u64);
    let stopped = space.visit_allocations(&labels, 8, |_, _| panic!("no leaf within eight nodes"));
    assert!(!stopped.complete());
    assert_eq!(stopped.nodes, 8);
    assert_eq!(stopped.unresolved_deals(), space.count());
    let first = space.visit_allocations(&labels, 1_000, |_, _| true);
    assert_eq!(first.classes, 1);
    assert_eq!(first.covered_deals, 1);
    assert_eq!(first.unresolved_deals() + first.covered_deals, space.count());
}

#[test]
fn returned_cards_use_the_existing_first_and_second_passer_ownership() {
    let start = hand();
    let sent = (0..4).fold(CardSet::EMPTY, |h, c| h | CardSet::single(c));
    let returned = CardSet::single(0) | CardSet::single(9) | CardSet::single(10) | CardSet::single(11);
    let mut k = Knowledge::new(0, start);
    k.apply(&SeatEvent::Pass {
        from: 0,
        to: 2,
        cards: Some(sent),
    })
    .unwrap();
    k.apply(&SeatEvent::Pass {
        from: 2,
        to: 0,
        cards: Some(returned),
    })
    .unwrap();
    assert_eq!(k.dealt_from_current(0, (start - sent) | returned), Some(start));
    assert_eq!(k.dealt_from_current(0, CardSet::EMPTY), None);
    let mut hidden = Knowledge::new(1, (18..27).fold(CardSet::EMPTY, |h, c| h | CardSet::single(c)));
    hidden
        .apply(&SeatEvent::Pass {
            from: 0,
            to: 2,
            cards: None,
        })
        .unwrap();
    let candidate = (4..9).fold(CardSet::EMPTY, |h, c| h | CardSet::single(c));
    assert_eq!(hidden.dealt_from_current(0, candidate), None);
}

fn small_state() -> Knowledge {
    let mut rng = StdRng::seed_from_u64(113);
    let mut deck = engine::get_all_cards();
    deck.shuffle(&mut rng);
    let mut game = engine::Game::new(
        "allocations".into(),
        ["0", "1", "2", "3"].map(String::from),
        Some(engine::hands_from_deck(&deck)),
    );
    let mut tracker = Tracker::new(0, observation::start_hand(&game, 0));
    let mut filter = SeatFilter::new(0);
    let mut bid = false;
    for _ in 0..160 {
        let legal = game.legal_actions();
        let action = if let Some(a) = legal
            .iter()
            .find(|a| !bid && a.action_type == engine::ActionType::NewBid(130))
        {
            bid = true;
            *a
        } else if let Some(a) = legal.iter().find(|a| a.action_type == engine::ActionType::StopBidding) {
            *a
        } else {
            let plays: Vec<_> = legal
                .iter()
                .filter(|a| matches!(a.action_type, engine::ActionType::CardPlayed(_)))
                .collect();
            if plays.is_empty() {
                legal[0]
            } else {
                *plays[rng.random_range(0..plays.len())]
            }
        };
        game.apply_action_mut(action);
        if let Some(event) = filter.filter(game.all_events.last().unwrap()) {
            tracker.apply(&event).unwrap();
        }
        let k = tracker.knowledge();
        if (!k.played()).len() <= 8 && (!k.played()).len() >= 4 {
            return *k;
        }
    }
    panic!("small-state fixture did not finish");
}

// Independent card-by-card oracle: no DealSpace splits or multiplicities.
fn brute(k: &Knowledge, cards: &[u8], hands: &mut Hands, f: &mut dyn FnMut(&Hands)) {
    let Some((&card, rest)) = cards.split_first() else {
        if (0..4).all(|s| hands[s].len() == k.hand_len(s as u8) as u32)
            && k.constraints()
                .iter()
                .all(|c| (c.lo as u32..=c.hi as u32).contains(&(hands[c.seat as usize] & c.mask).len()))
        {
            f(hands);
        }
        return;
    };
    for seat in 0..4u8 {
        let i = seat as usize;
        if k.possible(seat).contains(card) && hands[i].len() < k.hand_len(seat) as u32 {
            hands[i] |= CardSet::single(card);
            brute(k, rest, hands, f);
            hands[i] -= CardSet::single(card);
        }
    }
}

#[test]
fn allocation_multiplicities_match_independent_small_state_enumeration() {
    let k = small_state();
    let masks = [
        CardSet::suit(engine::Suit::Red),
        CardSet::ACES,
        CardSet::halves(engine::Suit::Green),
    ];
    let labels = std::array::from_fn(|c| {
        masks
            .iter()
            .enumerate()
            .fold(0, |bits, (i, mask)| bits | (u64::from(mask.contains(c as u8)) << i))
    });
    let event = |hands: &Hands| (hands[1] & masks[0]).len() == 1 && !(hands[2] & masks[1]).is_empty();
    let mut known: Hands = std::array::from_fn(|s| k.certain(s as u8));
    let unknown: Vec<_> = (!(known.iter().fold(k.played(), |all, h| all | *h))).iter().collect();
    let (mut count, mut positive) = (0, 0);
    brute(&k, &unknown, &mut known, &mut |hands| {
        count += 1;
        positive += u64::from(event(hands));
    });
    let (mut grouped_count, mut grouped_positive) = (0, 0);
    let visit = k.deals().visit_allocations(&labels, 100_000, |hands, ways| {
        grouped_count += ways;
        grouped_positive += if event(hands) { ways } else { 0 };
        false
    });
    assert!(count > 0 && visit.complete());
    assert_eq!((grouped_count, grouped_positive), (count, positive));
    let (mut private_count, mut private_positive) = (0, 0);
    let private = k
        .deals()
        .visit_private_allocations(&k, 1, &labels, 100_000, 32, |hands, ways| {
            private_count += ways;
            private_positive += if event(hands) { ways } else { 0 };
            false
        });
    assert!(private.complete());
    assert_eq!((private_count, private_positive), (count, positive));
}

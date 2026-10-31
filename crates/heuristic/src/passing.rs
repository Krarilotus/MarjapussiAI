//! C42 pass choice: extract the giver's information, score legal options, then select.
//! The active belief model does not yet infer intent from observed pass choices.

mod features;
pub use features::{
    back_features, back_plan, forth_features, score, Partner, PassingWeights, BACK_FEATURES, FORTH_FEATURES,
    PASSING_WEIGHTS,
};

use beliefs::conventions::{BidObs, Message};
use beliefs::Mind;
use engine::{partner, ActionType, CardSet, GameAction, Suit, HAND_SIZE, SUITS};

use crate::rules::RuleId;
use crate::{decision, fact, Ctx, Decision};

impl Partner {
    /// Snapshot only the acting giver's estimates. Features never receive the table's hands.
    pub fn of(mind: &Mind) -> Self {
        let seat = partner(mind.seat());
        let features = mind.features(seat);
        Self {
            cards: mind.holds_row(seat),
            ace: features.ace,
            void: SUITS.map(|s| mind.p_void(seat, s)),
            half: features.half,
        }
    }
}

impl Partner {
    /// What a giver holding `reader` reads about its partner from the auction alone: the
    /// partner's class likelihood (`Mind::class_likelihood`) over the uniform deal of the
    /// cards the giver doesn't see (`beliefs::conventions::read_class`); voids by counting.
    /// The level-0 exchange of the outlook's take-or-defend routes uses it.
    pub fn read(reader: CardSet, likelihood: Option<&[f32; beliefs::conventions::N_CLASSES]>) -> Self {
        let (features, cards) = beliefs::conventions::read_class(reader, CardSet::EMPTY, HAND_SIZE, likelihood);
        Self {
            cards,
            ace: features.ace,
            void: Self::uniform(reader).void,
            half: features.half,
        }
    }

    /// The giver's remaining hand after it passed me `received`, as I read it from its dealt-hand
    /// class likelihood and my own dealt cards `mine`: the received cards are certain parts of its
    /// dealt hand and now mine; its halves and voids follow from its remaining cards.
    pub fn read_after_pass(
        mine: CardSet,
        received: CardSet,
        likelihood: Option<&[f32; beliefs::conventions::N_CLASSES]>,
    ) -> Self {
        let (features, mut cards) = beliefs::conventions::read_class(mine, received, HAND_SIZE, likelihood);
        received.iter().for_each(|c| cards[c as usize] = 0.0);
        let none = |set: CardSet| set.iter().map(|c| 1.0 - cards[c as usize]).product::<f32>();
        Self {
            cards,
            ace: features.ace,
            void: SUITS.map(|s| none(CardSet::suit(s))),
            half: SUITS.map(|s| 1.0 - none(CardSet::halves(s))),
        }
    }

    /// What a giver can estimate about its partner from its own hand alone: the partner holds 9
    /// of the 27 cards it doesn't see, uniformly (P(card) = 1/3; an ace, a void, a half by
    /// counting).
    pub fn uniform(hand: CardSet) -> Self {
        let unseen = 36 - hand.len() as usize;
        let held = HAND_SIZE;
        // P(the partner holds none of `n` given unseen cards) = C(unseen - n, 9) / C(unseen, 9).
        let none = |n: u32| -> f32 {
            (0..held)
                .map(|i| (unseen - n as usize).saturating_sub(i) as f32 / (unseen - i) as f32)
                .product()
        };
        let p = held as f32 / unseen as f32;
        let mut cards = [0.0; engine::CARDS];
        for c in (CardSet::ALL - hand).iter() {
            cards[c as usize] = p;
        }
        Self {
            cards,
            ace: 1.0 - none((CardSet::ACES - hand).len()),
            void: SUITS.map(|s| none((CardSet::suit(s) - hand).len())),
            half: SUITS.map(|s| 1.0 - none((CardSet::halves(s) - hand).len())),
        }
    }
}

/// The best option of `hand` by the passing store: every 4-card set, scored by `features`.
fn best_pass<const N: usize>(hand: CardSet, weights: &[f64; N], features: impl Fn(CardSet) -> [f64; N]) -> CardSet {
    let cards: Vec<u8> = hand.iter().collect();
    let mut best = (f64::NEG_INFINITY, CardSet::EMPTY);
    for a in 0..cards.len() {
        for b in a + 1..cards.len() {
            for c in b + 1..cards.len() {
                for d in c + 1..cards.len() {
                    let x = [cards[a], cards[b], cards[c], cards[d]]
                        .iter()
                        .fold(CardSet::EMPTY, |s, &k| s | CardSet::single(k));
                    let v = score(weights, &features(x));
                    if v > best.0 {
                        best = (v, x);
                    }
                }
            }
        }
    }
    best.1
}

/// The level-0 exchange of a deal in which `declarer` plays: its partner passes forth and it
/// passes back by the passing store, each reading the other only from the auction
/// (`Partner::read` with `likelihood[seat]`, the seat's class likelihood as its partner reads
/// it) and knowing its own pair messages (`known[seat]`). Returns the hands after the exchange.
pub fn level0_exchange(
    hands: [CardSet; 4],
    declarer: u8,
    partner_half_conf: f32,
    likelihood: &[Option<Box<[f32; beliefs::conventions::N_CLASSES]>>; 4],
    known: [[bool; 4]; 4],
) -> [CardSet; 4] {
    let (d, p) = (declarer as usize, partner(declarer) as usize);
    let giver = hands[p];
    let reads_declarer = Partner::read(giver, likelihood[d].as_deref());
    let forth = best_pass(giver, &PASSING_WEIGHTS.forth, |x| {
        forth_features(giver, &reads_declarer, known[p], x)
    });
    level0_back(hands, declarer, forth, partner_half_conf, likelihood[p].as_deref())
}

/// The exchange after the declarer's partner passed `forth`: the declarer passes back by the
/// passing store, reading its partner from `giver_likelihood` (its class likelihood as the
/// declarer reads it) and the received cards. Returns the hands after the exchange.
pub fn level0_back(
    mut hands: [CardSet; 4],
    declarer: u8,
    forth: CardSet,
    partner_half_conf: f32,
    giver_likelihood: Option<&[f32; beliefs::conventions::N_CLASSES]>,
) -> [CardSet; 4] {
    let (d, p) = (declarer as usize, partner(declarer) as usize);
    hands[p] -= forth;
    hands[d] |= forth;
    let taker = hands[d];
    let estimate = Partner::read_after_pass(taker - forth, forth, giver_likelihood);
    let trump = back_plan(taker, estimate.half, partner_half_conf);
    // The same paired-run prior as the real return policy. Without a pair, the
    // level-0 actor keeps its longest top-controlled run; the real return compares
    // that candidate with the unfiltered store choice by terminal engine score.
    let run = paired_return_run(taker, trump).or_else(|| {
        trump
            .is_none()
            .then(|| {
                SUITS
                    .into_iter()
                    .filter(|&s| knowledge::control_run(taker, s))
                    .max_by_key(|&s| (taker & CardSet::suit(s)).len())
            })
            .flatten()
    });
    let available = run.map_or(taker, |s| taker - CardSet::suit(s));
    let back = best_pass(available, &PASSING_WEIGHTS.back, |y| {
        back_features(taker, &estimate, trump, y)
    });
    hands[d] -= back;
    hands[p] |= back;
    hands
}

fn paired_return_run(hand: CardSet, trump: Option<Suit>) -> Option<Suit> {
    trump.filter(|&s| knowledge::control_run(hand, s))
}

/// Suits covered by my own nominal pair messages. This is the existing convention's
/// "partner knows" feature, not a claim that the partner is certain I hold that pair.
pub fn known_pairs(bids: &[BidObs]) -> [bool; 4] {
    let sent = bids.iter().fold(0u8, |a, o| a | o.nominal.map_or(0, |m| m.bit()));
    SUITS.map(|s| match s {
        Suit::Green | Suit::Acorns => sent & Message::SmallOrHalves.bit() != 0,
        Suit::Bells => sent & Message::BigPair.bit() != 0,
        Suit::Red => sent & (Message::BigPair.bit() | Message::RedPair.bit()) != 0,
    })
}

pub(crate) fn decide(ctx: &Ctx, legal: &[GameAction]) -> Decision {
    let hand = ctx.k.certain(ctx.me);
    let partner = Partner::of(ctx.mind);
    if hand.len() as usize == HAND_SIZE {
        let known = known_pairs(&ctx.mind.record().seats[ctx.me as usize].bids);
        let (store, options) = choose(
            ctx,
            legal,
            &PASSING_WEIGHTS.forth,
            &FORTH_FEATURES,
            RuleId::PForth,
            |x| forth_features(hand, &partner, known, x),
        );
        if !ctx.params.pass_route {
            return store;
        }
        route_forth(ctx, hand, &options, store)
    } else {
        let trump = back_plan(hand, partner.half, ctx.params.partner_half_conf);
        // A plan suit that runs from the top stays whole: the run is the route that owning the
        // game gives me (`knowledge::control_run`; Astra's review 2026-10-13 of deal 573).
        let run = paired_return_run(hand, trump);
        let keeps_run: Vec<GameAction> = legal
            .iter()
            .filter(|a| match (&a.action_type, run) {
                (ActionType::Pass(cards), Some(t)) => (CardSet::from_cards(cards) & CardSet::suit(t)).is_empty(),
                _ => true,
            })
            .copied()
            .collect();
        let legal = if keeps_run.is_empty() { legal } else { &keeps_run[..] };
        let (store, options) = choose(ctx, legal, &PASSING_WEIGHTS.back, &BACK_FEATURES, RuleId::PBack, |y| {
            back_features(hand, &partner, trump, y)
        });
        if !ctx.params.return_run_route {
            return store;
        }
        route_back(ctx, hand, &options, store)
    }
}

/// A top-controlled long suit need not contain a pair. Preserve it as an explicit
/// candidate, not a blanket filter: current winners, future trump changes and
/// the return recipient's entries are valued together by the engine-scored route.
fn route_back(ctx: &Ctx, hand: CardSet, options: &[(GameAction, CardSet, f64)], store: Decision) -> Decision {
    let ActionType::Pass(cards) = &store.action.action_type else {
        return store;
    };
    let chosen = CardSet::from_cards(cards);
    let candidates = return_candidates(hand, options, chosen);
    if candidates.len() < 2 {
        return store;
    }
    let Some(values) = crate::outlook::return_values(ctx, &candidates) else {
        return store;
    };
    let best = values.best();
    let Some(base) = values.index(chosen) else {
        return store;
    };
    if best == base || !values.clearly_better(best, base) {
        return store;
    }
    let selected = values.options[best];
    let Some(action) = options.iter().find(|o| o.1 == selected).map(|o| o.0) else {
        return store;
    };
    decision(
        &action,
        RuleId::PRoute,
        vec![
            fact("expected_score_difference", values.mean(best)),
            fact("over_store", values.mean(best) - values.mean(base)),
            fact(
                "kept_run_cards",
                SUITS
                    .into_iter()
                    .filter(|&s| knowledge::control_run(hand, s) && (selected & CardSet::suit(s)).is_empty())
                    .map(|s| (hand & CardSet::suit(s)).len())
                    .max()
                    .unwrap_or(0),
            ),
        ],
    )
}

fn return_candidates(hand: CardSet, options: &[(GameAction, CardSet, f64)], chosen: CardSet) -> Vec<CardSet> {
    let mut candidates = vec![chosen];
    for suit in SUITS.into_iter().filter(|&s| knowledge::control_run(hand, s)) {
        if let Some(option) = options
            .iter()
            .filter(|o| (o.1 & CardSet::suit(suit)).is_empty())
            .max_by(|a, b| a.2.total_cmp(&b.2))
        {
            if !candidates.contains(&option.1) {
                candidates.push(option.1);
            }
        }
    }
    candidates
}

/// P-route: the store's pass forth stands unless another candidate is clearly better by route
/// (`outlook::pass_values`). The candidates cover the passes' purposes (Astra's reviews
/// 2026-10-13, S12/S13; the owner's game 3), with the store's choice as the baseline: the
/// store's best pass per set of suits it makes blank, every whole suit of four, per ace the best
/// pass delivering it, and per pair of mine the best pass keeping it.
fn route_forth(ctx: &Ctx, hand: CardSet, options: &[(GameAction, CardSet, f64)], store: Decision) -> Decision {
    let ActionType::Pass(cards) = &store.action.action_type else {
        return store;
    };
    let chosen = CardSet::from_cards(cards);
    let blanked = |x: CardSet| {
        SUITS
            .into_iter()
            .filter(|&s| {
                let held = hand & CardSet::suit(s);
                !held.is_empty() && held.is_subset(x)
            })
            .fold(0u8, |bits, s| bits | 1 << s as u8)
    };
    let best_where = |keep: &dyn Fn(CardSet) -> bool| {
        options
            .iter()
            .filter(|o| keep(o.1))
            .max_by(|a, b| a.2.total_cmp(&b.2))
            .map(|o| o.1)
    };
    let mut patterns: Vec<u8> = options.iter().map(|o| blanked(o.1)).collect();
    patterns.sort_unstable();
    patterns.dedup();
    let blanks = patterns.into_iter().filter_map(|b| best_where(&|x| blanked(x) == b));
    let whole_suits = SUITS
        .map(|s| hand & CardSet::suit(s))
        .into_iter()
        .filter(|x| x.len() == 4);
    let aces = (hand & CardSet::ACES)
        .iter()
        .filter_map(|a| best_where(&|x| x.contains(a)));
    let pairs_kept = hand
        .pair_suits()
        .filter_map(|s| best_where(&|x| (x & CardSet::halves(s)).is_empty()));
    let mut candidates = vec![chosen];
    for x in blanks.chain(whole_suits).chain(aces).chain(pairs_kept) {
        if !candidates.contains(&x) && options.iter().any(|o| o.1 == x) {
            candidates.push(x);
        }
    }
    let Some(values) = crate::outlook::pass_values(ctx, &candidates) else {
        return store;
    };
    let top = values.best();
    let Some(base) = values.index(chosen) else {
        return store;
    };
    if top == base || !values.clearly_better(top, base) {
        return store;
    }
    let x = values.options[top];
    let Some(action) = options.iter().find(|o| o.1 == x).map(|o| o.0) else {
        return store;
    };
    decision(
        &action,
        RuleId::PRoute,
        vec![
            fact("expected_score_difference", values.mean(top)),
            fact("over_store", values.mean(top) - values.mean(base)),
            fact("blank_signs", blanked(x)),
        ],
    )
}

/// Keep scores for selection, and calculate an explanation only for the chosen option.
fn choose<const N: usize>(
    ctx: &Ctx,
    legal: &[GameAction],
    weights: &[f64; N],
    names: &[&'static str; N],
    rule: RuleId,
    features: impl Fn(CardSet) -> [f64; N],
) -> (Decision, Vec<(GameAction, CardSet, f64)>) {
    let opts: Vec<_> = legal
        .iter()
        .filter_map(|a| match &a.action_type {
            ActionType::Pass(cards) => {
                let given = CardSet::from_cards(cards);
                Some((*a, given, score(weights, &features(given))))
            }
            _ => None,
        })
        .collect();
    let i = if ctx.params.selection() == crate::Selection::Argmax {
        // Preserve f64 precision and the first legal option on ties.
        (0..opts.len())
            .max_by(|&a, &b| opts[a].2.total_cmp(&opts[b].2).then(b.cmp(&a)))
            .unwrap_or(0)
    } else {
        ctx.select(&opts.iter().map(|o| o.2 as f32).collect::<Vec<_>>())
    };
    let chosen = decision(&opts[i].0, rule, explain(weights, names, features(opts[i].1)));
    (chosen, opts)
}

fn explain<const N: usize>(weights: &[f64; N], names: &[&'static str; N], selected: [f64; N]) -> Vec<crate::Fact> {
    let mut why = vec![fact("score", score(weights, &selected))];
    why.extend(
        names
            .iter()
            .zip(weights.iter().zip(selected))
            .filter(|(_, (_, f))| *f != 0.0)
            .map(|(&name, (&weight, value))| fact(name, weight * value)),
    );
    why
}

#[cfg(test)]
mod control_tests {
    use super::*;

    fn cards(names: &[&str]) -> CardSet {
        CardSet::from_cards(&names.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>())
    }

    #[test]
    fn return_run_comparison_is_enabled_independently_of_pass_forth() {
        let params = crate::Params::default();
        assert!(params.return_run_route);
        assert!(!params.pass_route);
        let disabled = crate::Params::from_toml("return_run_route = false").unwrap();
        assert!(!disabled.return_run_route);
        assert!(!disabled.pass_route);
    }

    #[test]
    fn default_return_evaluates_the_pairless_run_from_a_thirteen_card_seat_view() {
        use observation::SeatEvent;
        use std::cell::RefCell;
        let params = crate::Params {
            outlook_worlds: 8,
            beliefs: beliefs::BeliefParams::base_only(),
            ..crate::Params::default()
        };
        let own = cards(&["e-A", "e-Z", "e-9", "e-8", "e-7", "g-O", "g-8", "g-6", "s-6"]);
        let forth = cards(&["e-O", "e-6", "g-A", "g-Z"]);
        let mut mind = Mind::new(0, own, [crate::SeatKind::Unknown; 4], params.beliefs.clone());
        for event in [
            SeatEvent::Bid { seat: 0, value: 120 },
            SeatEvent::StopBidding { seat: 1 },
            SeatEvent::StopBidding { seat: 2 },
            SeatEvent::StopBidding { seat: 3 },
            SeatEvent::Pass {
                from: 2,
                to: 0,
                cards: Some(forth),
            },
        ] {
            mind.apply(&event).unwrap();
        }
        let legal = RefCell::new(Vec::new());
        best_pass(own | forth, &PASSING_WEIGHTS.back, |set| {
            let list: Vec<_> = set.iter().map(engine::Card::from_index).collect();
            legal.borrow_mut().push(GameAction {
                player: engine::PlaceAtTable(0),
                action_type: ActionType::Pass(list.try_into().unwrap()),
            });
            [0.0; 17]
        });
        let legal = legal.into_inner();
        let ctx = Ctx {
            me: 0,
            k: mind.knowledge(),
            mind: &mind,
            params: &params,
            solver: &RefCell::new(None),
            seed: 7,
            deadline: None,
            deadline_hit: Default::default(),
            routes: RefCell::new(None),
            concepts: RefCell::new(crate::concepts::Reads::default()),
        };
        let selected = decide(&ctx, &legal);
        assert!(
            ctx.routes.borrow().is_some(),
            "default return must actually evaluate its route"
        );
        assert!(legal.contains(&selected.action));
        assert!(
            !params.pass_route,
            "the unrelated pass-forth comparison remains disabled"
        );
    }

    #[test]
    fn a_pairless_run_is_a_return_candidate_without_removing_the_store_choice() {
        let hand = cards(&[
            "e-A", "e-Z", "e-O", "e-9", "e-8", "e-7", "e-6", "s-A", "s-Z", "g-6", "g-7", "r-6", "r-7",
        ]);
        let store = cards(&["e-6", "e-7", "e-8", "g-6"]);
        let keep = cards(&["g-6", "g-7", "r-6", "r-7"]);
        let option = |set: CardSet, score| {
            let list: Vec<_> = set.iter().map(engine::Card::from_index).collect();
            (
                GameAction {
                    player: engine::PlaceAtTable(0),
                    action_type: ActionType::Pass(list.try_into().unwrap()),
                },
                set,
                score,
            )
        };
        let options = [option(store, 10.0), option(keep, 0.0)];
        assert_eq!(return_candidates(hand, &options, store), vec![store, keep]);
        // Length alone does not justify a run-preserving alternative.
        assert_eq!(return_candidates(hand - cards(&["e-A"]), &options, store), vec![store]);
    }

    #[test]
    fn level0_return_keeps_pairless_control_without_reading_partner_hidden_cards() {
        let own = cards(&["e-A", "e-Z", "e-O", "e-9", "e-8", "e-7", "e-6", "s-A", "s-Z"]);
        let forth = cards(&["g-6", "g-7", "r-6", "r-7"]);
        let rest: Vec<_> = (CardSet::ALL - own - forth).iter().collect();
        let set = |slice: &[u8]| slice.iter().fold(CardSet::EMPTY, |s, &c| s | CardSet::single(c));
        let hands = [own, set(&rest[..9]), forth | set(&rest[18..]), set(&rest[9..18])];
        let kept = level0_back(hands, 0, forth, 1.0, None);
        assert_eq!((kept[0] & CardSet::suit(Suit::Acorns)).len(), 7);
        assert!(kept.iter().all(|h| h.len() == 9));
        let mut swapped = hands;
        let from_partner = (swapped[2] - forth).iter().next().unwrap();
        let from_enemy = swapped[1].iter().next().unwrap();
        swapped[2] = (swapped[2] - CardSet::single(from_partner)) | CardSet::single(from_enemy);
        swapped[1] = (swapped[1] - CardSet::single(from_enemy)) | CardSet::single(from_partner);
        assert_eq!(level0_back(swapped, 0, forth, 1.0, None)[0], kept[0]);
    }
}

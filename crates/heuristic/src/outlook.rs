//! The plan layer's chain at a lead (KB Batch 3/4 and the owner's game reviews, 2026-10-12):
//! for each candidate card, over the believed worlds (the shared `DealModel` pool of `routes`):
//!
//! 1. **Trick outcome:** the engine's legal replies (follow, overtake, trump) are applied exactly;
//!    genuine choices use a level-0 response (`respond`): with the partner winning, play cheap
//!    (the last player feeds points that are not a live pair's half); otherwise win with the
//!    cheapest winning card, else discard the cheapest card, keeping pair halves.
//! 2. **Next position:** the trick's winner leads; the cards are spent.
//! 3. **`trick_ownership` of the rest:** suit by suit, each round goes to the highest card of
//!    the suit, lower cards are promoted as the higher ones fall (exhaustion), and a player void
//!    in a side suit who still holds trump ruffs it. The team with the top trump (or the
//!    leader's team without trump) is credited the last trick.
//!
//! The value of a candidate is our expected points minus theirs over those worlds: an estimate
//! under the level-0 responses and the per-suit remainder, not a proof. Exposed-world estimates
//! can use cards the players cannot see (strategy fusion); this is the stated approximation.

use engine::{suit_of, CardSet, FastState, PlayPhase, Suit, CARD_POINTS, LAST_TRICK_BONUS, SUITS};
use knowledge::World;

use crate::Ctx;

fn points(cards: CardSet) -> i32 {
    cards.iter().map(|c| CARD_POINTS[c as usize] as i32).sum()
}

/// The strongest card of `set` in suit `s` (no other card of `set` in `s` beats it).
fn top_of(set: CardSet, s: Suit) -> Option<u8> {
    (set & CardSet::suit(s)).iter().max_by_key(|&c| beats_key(c))
}

fn low_of(set: CardSet, s: Suit) -> Option<u8> {
    (set & CardSet::suit(s)).iter().min_by_key(|&c| beats_key(c))
}

/// Order of strength within a suit, as the engine's `takes` defines it.
fn beats_key(c: u8) -> u8 {
    let mut key = 0;
    for other in CardSet::suit(suit_of(c)).iter() {
        if other != c && engine::takes(c, other, None) {
            key += 1;
        }
    }
    key
}

/// Halves of the pairs a seat still holds complete (cards it should not throw away).
fn live_halves(hand: CardSet) -> CardSet {
    hand.pair_suits().fold(CardSet::EMPTY, |set, s| set | CardSet::halves(s))
}

/// Level-0 response of the seat to move in `state` (its legal cards under the engine's rules).
fn respond(state: &FastState) -> u8 {
    let seat = state.to_move();
    let legal = state.legal_cards();
    if legal.len() == 1 {
        return legal.iter().next().expect("one legal card");
    }
    let trick = state.trick();
    let keep = live_halves(state.hand(seat));
    let cheapest = |set: CardSet| {
        let open = match set - keep {
            o if o.is_empty() => set,
            o => o,
        };
        open.iter()
            .min_by_key(|&c| (CARD_POINTS[c as usize], beats_key(c)))
            .expect("nonempty")
    };
    if trick.is_empty() {
        return cheapest(legal);
    }
    let leader = state.leader();
    let winner_seat = engine::winning_seat(leader, trick, state.trump());
    let winning_card = trick[((winner_seat + 4 - leader) % 4) as usize];
    let last = trick.len() == 3;
    if winner_seat % 2 == seat % 2 {
        if last {
            // The trick is ours: feed points that are not a live pair's half.
            let feed = legal - keep;
            if let Some(c) = feed.iter().max_by_key(|&c| (CARD_POINTS[c as usize], beats_key(c))) {
                return c;
            }
        }
        return cheapest(legal);
    }
    let winners = legal
        .iter()
        .filter(|&c| engine::takes(c, winning_card, state.trump()))
        .fold(CardSet::EMPTY, |s, c| s | CardSet::single(c));
    if !winners.is_empty() {
        return winners
            .iter()
            .min_by_key(|&c| (beats_key(c) as i32 + if keep.contains(c) { 100 } else { 0 }, CARD_POINTS[c as usize]))
            .expect("nonempty");
    }
    cheapest(legal)
}

/// `trick_ownership` of the remaining cards: card points (incl. the last trick) per team.
fn remainder(mut hands: [CardSet; 4], trump: Option<Suit>, leader: u8) -> [i32; 2] {
    let mut pts = [0i32; 2];
    if hands.iter().all(|h| h.is_empty()) {
        return pts;
    }
    // The last trick: the top trump's team, or the leader's team without trump.
    let last_team = trump
        .and_then(|t| {
            let all = hands.iter().fold(CardSet::EMPTY, |s, h| s | *h);
            let top = top_of(all, t)?;
            (0..4u8).find(|&s| hands[s as usize].contains(top)).map(|s| s % 2)
        })
        .unwrap_or(leader % 2);
    pts[last_team as usize] += LAST_TRICK_BONUS;
    let side: Vec<Suit> = SUITS.into_iter().filter(|&s| Some(s) != trump).collect();
    for s in side.into_iter().chain(trump) {
        while (0..4).any(|p| !(hands[p] & CardSet::suit(s)).is_empty()) {
            let holders: Vec<u8> = (0..4u8).filter(|&p| !(hands[p as usize] & CardSet::suit(s)).is_empty()).collect();
            let top = holders
                .iter()
                .map(|&p| (p, top_of(hands[p as usize], s).expect("holder")))
                .max_by_key(|&(_, c)| beats_key(c))
                .expect("a holder");
            let mut round = CardSet::single(top.1);
            let mut winner = top.0;
            // A void player of the other team with trump ruffs a side suit.
            if Some(s) != trump {
                if let Some(t) = trump {
                    let ruff = (0..4u8)
                        .filter(|&p| p % 2 != top.0 % 2 && (hands[p as usize] & CardSet::suit(s)).is_empty())
                        .filter_map(|p| low_of(hands[p as usize], t).map(|c| (p, c)))
                        .next();
                    if let Some((p, c)) = ruff {
                        hands[p as usize] -= CardSet::single(c);
                        round |= CardSet::single(c);
                        winner = p;
                    }
                }
            }
            hands[top.0 as usize] -= CardSet::single(top.1);
            for &p in &holders {
                if p != top.0 {
                    let low = low_of(hands[p as usize], s).expect("holder");
                    hands[p as usize] -= CardSet::single(low);
                    round |= CardSet::single(low);
                }
            }
            pts[(winner % 2) as usize] += points(round);
        }
    }
    pts
}

/// The expected value (our points minus theirs) of leading each card in `candidates`, over the
/// believed worlds. `None` when there are no worlds.
pub(crate) fn lead_values(ctx: &Ctx, candidates: CardSet) -> Option<Vec<(u8, f32)>> {
    let _timing = beliefs::profile::scope("outlook");
    let public = ctx.k.public_state(ctx.me, PlayPhase::Cards);
    let us = (ctx.me % 2) as usize;
    ctx.with_worlds(|worlds: &[World]| {
        let total: f64 = worlds.iter().map(|w| w.weight).sum();
        if worlds.is_empty() || total <= 0.0 {
            return None;
        }
        let mut out = Vec::new();
        for c in candidates.iter() {
            let mut value = 0.0f64;
            for w in worlds {
                let Ok(mut state) = FastState::from_public(public, w.hands) else { continue };
                if !state.legal_cards().contains(c) {
                    continue;
                }
                let before = [state.card_points(0), state.card_points(1)];
                state.play(c);
                while !state.trick().is_empty() && state.trick().len() < 4 && !state.is_over() {
                    let r = respond(&state);
                    state.play(r);
                }
                let trick = [state.card_points(0) - before[0], state.card_points(1) - before[1]];
                let rest = remainder(state.hands(), state.trump(), state.leader());
                let ours = (trick[us] + rest[us]) as f64;
                let theirs = (trick[1 - us] + rest[1 - us]) as f64;
                value += w.weight / total * (ours - theirs);
            }
            out.push((c, value as f32));
        }
        Some(out)
    })
}

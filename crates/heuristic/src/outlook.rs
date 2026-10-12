//! The plan layer's chain at a lead (KB Batch 3/4/7 and the owner's game reviews, 2026-10-12):
//! for each candidate card, over the believed worlds (the shared `DealModel` pool of `routes`):
//!
//! 1. **Trick outcome:** the engine's legal replies (follow, overtake, trump) are applied exactly;
//!    genuine choices use a level-0 response (`respond`): with the partner winning, play cheap
//!    (the last player feeds points that are not a live pair's half); otherwise win with the
//!    cheapest winning card, else discard the cheapest card, keeping pair halves.
//! 2. **Next leader and what it can do (`activate`):** at every trick start the leader's team
//!    activates what the rules let it: an own pair (the shorter suit first, so the longest stays
//!    trump), else the partner's pair by "Yours", else a half it holds whose other half the
//!    partner holds. Rights, the trump change and the pair points are the engine's.
//! 3. **The rest of the game (`playout`):** the leader cashes a card that stands (the top card of
//!    its suit that no void opponent can ruff), else leads its cheapest card outside its pairs;
//!    the others respond as in 1. So pairs live or die by their guards and by the discard
//!    material in the suits their holder is void in, and one won lead can activate both pairs
//!    (announce, keep the lead with a standing card, then ask) as in the owner's game 938.
//!
//! The value of a candidate is our expected points (cards, last trick, pairs) minus theirs over
//! those worlds: an estimate under level-0 play, not a proof. Exposed-world estimates can use
//! cards the players cannot see (strategy fusion: a question is asked only where it is answered
//! yes); this is the stated approximation.

use engine::{partner, suit_of, AnswerType, CardSet, FastState, Move, PlayPhase, QuestionType, Suit, CARD_POINTS, SUITS};
use knowledge::World;

use crate::Ctx;

/// The strongest card of `set` in suit `s` (no other card of `set` in `s` beats it).
fn top_of(set: CardSet, s: Suit) -> Option<u8> {
    (set & CardSet::suit(s)).iter().max_by_key(|&c| beats_key(c))
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

/// The activation the leader's team makes at a trick start (see the module doc, step 2).
fn activate(state: &mut FastState) {
    if state.phase() != PlayPhase::Start {
        return;
    }
    let leader = state.leader();
    let mine = state.hand(leader);
    let mate = state.hand(partner(leader));
    let team_len = |s: Suit| ((mine | mate) & CardSet::suit(s)).len();
    let open = |s: Suit| state.called() >> s as u8 & 1 == 0;
    let own = state.announcements();
    if let Some(s) = SUITS.into_iter().filter(|&s| own >> s as u8 & 1 == 1).min_by_key(|&s| team_len(s)) {
        state.announce(s);
        return;
    }
    let legal = |m: Move, st: &FastState| st.moves().any(|x| x == m);
    if let Some(s) = mate.pair_suits().filter(|&s| open(s)).min_by_key(|&s| team_len(s)) {
        if legal(Move::Question(QuestionType::Yours), state) {
            state.apply(Move::Question(QuestionType::Yours));
            state.apply(Move::Answer(AnswerType::YesPair(s)));
            return;
        }
    }
    // A half question spends the remaining rights: ask for the most valuable split pair.
    let split = SUITS.into_iter().rev().find(|&s| {
        open(s) && !(mine & CardSet::halves(s)).is_empty() && (mine | mate) & CardSet::halves(s) == CardSet::halves(s)
    });
    if let Some(s) = split {
        if legal(Move::Question(QuestionType::YourHalf(s)), state) {
            state.apply(Move::Question(QuestionType::YourHalf(s)));
            state.apply(Move::Answer(AnswerType::YesHalf(s)));
        }
    }
}

/// The level-0 lead (see the module doc, step 3).
fn lead0(state: &FastState) -> u8 {
    let seat = state.leader();
    let hands = state.hands();
    let all = state.in_hands();
    let trump = state.trump();
    let ruffable = |s: Suit| {
        trump.is_some_and(|t| {
            t != s
                && [1u8, 3].iter().any(|&o| {
                    let h = hands[((seat + o) % 4) as usize];
                    (h & CardSet::suit(s)).is_empty() && !(h & CardSet::suit(t)).is_empty()
                })
        })
    };
    state
        .legal_cards()
        .iter()
        .filter(|&c| top_of(all, suit_of(c)) == Some(c) && !ruffable(suit_of(c)))
        .max_by_key(|&c| (CARD_POINTS[c as usize], Some(suit_of(c)) != trump))
        .unwrap_or_else(|| respond(state))
}

/// Plays the game to its end at level 0 (see the module doc, steps 2 and 3).
fn playout(state: &mut FastState) {
    while !state.is_over() {
        let c = if state.trick().is_empty() {
            activate(state);
            lead0(state)
        } else {
            respond(state)
        };
        state.play(c);
    }
}

/// Per card: P(our team takes the trick) and the expected points in it.
pub(crate) type TrickOutcomes = [Option<(f32, f32)>; 36];

/// The trick outcome of playing each of my cards now, over the believed worlds: P(our team
/// takes the trick) and the expected points in the trick (cards and a last-trick bonus).
/// The engine's legal replies are exact; genuine choices use the level-0 response.
pub(crate) fn trick_outcomes(ctx: &Ctx) -> Option<TrickOutcomes> {
    let _timing = beliefs::profile::scope("outlook.trick");
    let public = ctx.k.public_state(ctx.me, PlayPhase::Cards);
    let us = ctx.me % 2;
    let mine = ctx.k.certain(ctx.me);
    ctx.with_worlds(|worlds: &[World]| {
        let total: f64 = worlds.iter().map(|w| w.weight).sum();
        if worlds.is_empty() || total <= 0.0 {
            return None;
        }
        let mut out = [None; 36];
        for c in mine.iter() {
            let (mut p_ours, mut pts, mut mass) = (0.0f64, 0.0f64, 0.0f64);
            for w in worlds {
                let Ok(mut state) = FastState::from_public(public, w.hands) else { continue };
                if !state.legal_cards().contains(c) {
                    continue;
                }
                let before = state.card_points(0) + state.card_points(1);
                let tricks = state.tricks_done();
                state.play(c);
                while state.tricks_done() == tricks && !state.is_over() {
                    let r = respond(&state);
                    state.play(r);
                }
                let p = w.weight / total;
                mass += p;
                pts += p * (state.card_points(0) + state.card_points(1) - before) as f64;
                if state.leader() % 2 == us {
                    p_ours += p;
                }
            }
            if mass > 0.0 {
                out[c as usize] = Some(((p_ours / mass) as f32, (pts / mass) as f32));
            }
        }
        Some(out)
    })
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
                let before = [state.points(0), state.points(1)];
                state.play(c);
                playout(&mut state);
                let ours = (state.points(us as u8) - before[us]) as f64;
                let theirs = (state.points(1 - us as u8) - before[1 - us]) as f64;
                value += w.weight / total * (ours - theirs);
            }
            out.push((c, value as f32));
        }
        Some(out)
    })
}

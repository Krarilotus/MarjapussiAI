//! A sufficient seat-visible certificate, not a hidden-deal search. Each lead
//! wins for the claimant itself; all announceable own pairs must be collected.

use std::collections::HashSet;

use engine::{is_first_trick, play_levels, ActionType, CardSet, GameAction, Move, Suit, HAND_SIZE, SUITS};

use crate::{own_standing_of, Knowledge};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct State {
    remaining: CardSet,
    called: u8,
    trump: Option<Suit>,
    start: bool,
}

struct Proof<'a> {
    k: &'a Knowledge,
    legal: &'a [GameAction],
    hand: CardSet,
    required: u8,
    bounds: [[(u8, u8); 4]; 4],
    failed: HashSet<State>,
}

/// Certify a sequence of claimant-only card plays and own announcements.
///
/// The caller supplies the engine's current legal actions and gates live human
/// ownership. This function requires an empty logical trick and an exact own
/// hand, and accepts only card-play/start-trick actions (ignoring undo requests).
/// It preserves every own uncalled pair announceable with the current rights,
/// and refuses possibly reachable partner/split pairs rather than forfeiting
/// their points. Other seats never get a declaration window in the certificate.
///
/// No opponent cards are invented. Suit lower bounds lose at most one card per
/// completed trick; each lead of a suit exhausts one card of that suit if held.
/// Failure is inconclusive. With at most nine cards, the memo has at most
/// 512 * 16 * 5 * 2 states (hand subsets, called mask, trump, phase).
pub fn remaining_claim(k: &Knowledge, legal: &[GameAction]) -> Option<Vec<GameAction>> {
    let me = k.me();
    if me >= 4 || !k.trick().is_empty() {
        return None;
    }
    let hand = k.certain(me);
    let n = hand.len();
    if n == 0
        || n > HAND_SIZE as u32
        || !hand.is_subset(CardSet::ALL)
        || !(hand & k.played()).is_empty()
        || !k.possible(me).is_empty()
        || (0..4).any(|seat| k.hand_len(seat) as u32 != n)
    {
        return None;
    }
    let mut start = false;
    let mut cards = CardSet::EMPTY;
    for action in legal {
        if action.action_type == ActionType::UndoRequest {
            continue;
        }
        if action.player.0 != me {
            return None;
        }
        match action.action_type {
            ActionType::CardPlayed(card) => cards |= CardSet::of(&card),
            ActionType::AnnounceTrump(_) | ActionType::Question(_) => start = true,
            _ => return None,
        }
    }
    if cards.is_empty() || !cards.is_subset(hand) || !(hand - own_standing_of(hand | k.played())).is_empty() {
        return None;
    }
    let pairs = k.team_pairs(me);
    if pairs.ours.possible != 0 || k.rights(me).may_ask_pair() && pairs.yours.possible != 0 {
        return None;
    }
    let called = SUITS
        .into_iter()
        .filter(|&s| k.is_called(s))
        .fold(0, |mask, s| mask | 1 << s as u8);
    let required = if k.rights(me).may_announce() {
        pairs.mine.certain
    } else {
        0
    };
    let mut proof = Proof {
        k,
        legal,
        hand,
        required,
        bounds: std::array::from_fn(|seat| std::array::from_fn(|s| k.count_bounds(seat as u8, SUITS[s]))),
        failed: HashSet::new(),
    };
    let mut plan = Vec::with_capacity(n as usize + required.count_ones() as usize);
    proof
        .search(
            State {
                remaining: hand,
                called,
                trump: k.trump(),
                start,
            },
            &mut plan,
        )
        .then_some(plan)
}

impl Proof<'_> {
    fn search(&mut self, state: State, plan: &mut Vec<GameAction>) -> bool {
        if state.remaining.is_empty() {
            return self.required & !state.called == 0;
        }
        if self.failed.contains(&state) {
            return false;
        }
        let removed = self.hand - state.remaining;
        let i = removed.len();
        let pending = self.required & !state.called;
        if state.start && self.k.rights(self.k.me()).may_announce() {
            for suit in state.remaining.pair_suits() {
                let bit = 1 << suit as u8;
                if state.called & bit != 0 {
                    continue;
                }
                let action = Move::Announce(suit).to_action(self.k.me());
                if i == 0 && !self.legal.contains(&action) {
                    continue;
                }
                plan.push(action);
                if self.search(
                    State {
                        called: state.called | bit,
                        trump: Some(suit),
                        start: false,
                        ..state
                    },
                    plan,
                ) {
                    return true;
                }
                plan.pop();
            }
        }
        let allowed =
            play_levels(&[], state.trump, is_first_trick(state.remaining.len() as usize)).allowed(state.remaining);
        for suit in SUITS {
            let Some(card) = (state.remaining & CardSet::suit(suit)).iter().last() else {
                continue;
            };
            let action = Move::Card(card).to_action(self.k.me());
            if !allowed.contains(card)
                || i == 0 && !self.legal.contains(&action)
                || pending & (1 << suit as u8) != 0 && CardSet::HALVES.contains(card)
            {
                continue;
            }
            if let Some(trump) = state.trump.filter(|&t| t != suit) {
                let drawn = (removed & CardSet::suit(trump)).len();
                if (0..4).filter(|&seat| seat != self.k.me()).any(|seat| {
                    self.bounds[seat as usize][suit as usize].0 as u32 <= i
                        && self.bounds[seat as usize][trump as usize].1 as u32 > drawn
                }) {
                    continue;
                }
            }
            plan.push(action);
            if self.search(
                State {
                    remaining: state.remaining - CardSet::single(card),
                    start: true,
                    ..state
                },
                plan,
            ) {
                return true;
            }
            plan.pop();
        }
        self.failed.insert(state);
        false
    }
}

#[cfg(test)]
#[path = "claim_tests.rs"]
mod tests;

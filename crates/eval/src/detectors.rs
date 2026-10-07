//! Intention detectors: owner intentions checked on the true hands at every decision, for any
//! agent (C22, C24, S7b, P10a, HC-1). Each looks at the table before the decision is applied.

use engine::team_points;
use engine::{partner, takes, trick_winner, winning_seat, CardSet, SUITS};
use engine::{ActionType, GameAction};
use engine::{CARD_POINTS, LAST_TRICK_BONUS};
use observation::Seat;

use crate::Table;

/// What the detectors found at one decision.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    /// HC-1 (C4): a K or O of an own uncalled pair played while another card was legal.
    pub voluntary_pair_half: bool,
    /// P10a: suits (bit = suit) of uncalled pairs the seat holds at a lead where it may ask
    /// but not announce them, because its `Own` right is spent.
    pub blocked_pairs: u8,
    /// C22: an A or 10 put onto the partner's winning trick while it was standing (every
    /// higher live card of its suit with its team) and another card was legal.
    pub schmiered_standing: bool,
    /// C24 / S7b: a half of an uncalled pair the team holds, led while questions were legal and
    /// while the team's contract could not be made without meld points.
    pub needed_half_played: bool,
}

fn card_of(action: &GameAction) -> Option<u8> {
    match &action.action_type {
        ActionType::CardPlayed(c) => Some(c.index()),
        _ => None,
    }
}

fn other_card_legal(legal: &[GameAction], c: u8) -> bool {
    legal.iter().filter_map(card_of).any(|x| x != c)
}

/// All detectors for `seat` choosing `action` from `legal`.
pub fn flags(table: &Table, seat: Seat, legal: &[GameAction], action: &GameAction) -> Flags {
    Flags {
        voluntary_pair_half: voluntary_pair_half(table, seat, legal, action),
        blocked_pairs: blocked_pairs(table, seat, legal),
        schmiered_standing: schmiered_standing(table, seat, legal, action),
        needed_half_played: needed_half_played(table, seat, legal, action),
    }
}

fn uncalled_pair_halves(table: &Table, cards: CardSet) -> CardSet {
    let called = &table.game.state.trump_called;
    SUITS
        .into_iter()
        .filter(|s| !called.contains(s) && CardSet::halves(*s).is_subset(cards))
        .fold(CardSet::EMPTY, |a, s| a | CardSet::halves(s))
}

pub fn voluntary_pair_half(table: &Table, seat: Seat, legal: &[GameAction], action: &GameAction) -> bool {
    let Some(c) = card_of(action) else { return false };
    let halves = uncalled_pair_halves(table, table.hand(seat));
    let alternative = legal.iter().filter_map(card_of).any(|x| !halves.contains(x));
    halves.contains(c) && alternative
}

pub fn blocked_pairs(table: &Table, seat: Seat, legal: &[GameAction]) -> u8 {
    let can_ask = legal.iter().any(|a| matches!(a.action_type, ActionType::Question(_)));
    if !can_ask {
        return 0;
    }
    let called = &table.game.state.trump_called;
    table
        .hand(seat)
        .pair_suits()
        .filter(|s| !called.contains(s))
        .filter(|s| !legal.iter().any(|a| a.action_type == ActionType::AnnounceTrump(*s)))
        .fold(0, |m, s| m | 1 << s as u8)
}

fn played(table: &Table) -> CardSet {
    let state = &table.game.state;
    let done = state.all_tricks.iter().flat_map(|t| t.cards.iter());
    let current = if state.current_trick.len().is_multiple_of(4) {
        [].iter()
    } else {
        state.current_trick.iter()
    };
    done.chain(current).fold(CardSet::EMPTY, |a, c| a | CardSet::of(c))
}

pub fn schmiered_standing(table: &Table, seat: Seat, legal: &[GameAction], action: &GameAction) -> bool {
    let state = &table.game.state;
    let Some(c) = card_of(action) else { return false };
    if CARD_POINTS[c as usize] < 10 || state.current_trick.len().is_multiple_of(4) || !other_card_legal(legal, c) {
        return false;
    }
    let trick: Vec<u8> = state.current_trick.iter().map(|c| c.index()).collect();
    let n = trick.len() as u8;
    let w = trick_winner(&trick, state.trump);
    if winning_seat((seat + 4 - n) % 4, &trick, state.trump) != partner(seat) || takes(c, trick[w], state.trump) {
        return false;
    }
    let team = table.hand(seat) | table.hand(partner(seat));
    let suit = engine::Card::from_index(c).suit;
    let higher = (CardSet::suit(suit) - played(table))
        .iter()
        .filter(|&b| takes(b, c, None))
        .fold(CardSet::EMPTY, |a, b| a | CardSet::single(b));
    higher.is_subset(team)
}

pub fn needed_half_played(table: &Table, seat: Seat, legal: &[GameAction], action: &GameAction) -> bool {
    let state = &table.game.state;
    let Some(c) = card_of(action) else { return false };
    let can_ask = legal.iter().any(|a| matches!(a.action_type, ActionType::Question(_)));
    let team_cards = table.hand(seat) | table.hand(partner(seat));
    if !can_ask || !uncalled_pair_halves(table, team_cards).contains(c) {
        return false;
    }
    let Some(declarer) = state
        .bidding_history
        .iter()
        .rev()
        .find(|(t, _)| matches!(t, ActionType::NewBid(_)))
        .map(|(_, p)| p.0)
    else {
        return false;
    };
    if declarer % 2 != seat % 2 {
        return false;
    }
    let team = (seat % 2) as usize;
    let have = team_points(&state.all_tricks, &table.game.all_events).total()[team];
    let left: i32 = (!played(table))
        .iter()
        .map(|x| CARD_POINTS[x as usize] as i32)
        .sum::<i32>()
        + if state.all_tricks.len() < engine::TRICKS as usize {
            LAST_TRICK_BONUS
        } else {
            0
        };
    have + left < state.value.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::Card;

    fn cards(s: &[&str]) -> Vec<Card> {
        s.iter().map(|c| c.parse::<Card>().expect("card")).collect()
    }

    /// Applies the legal action of the seat to act that `pick` accepts.
    fn act(t: &mut Table, pick: impl Fn(&ActionType) -> bool) -> GameAction {
        let (_, legal) = t.to_act().expect("game on");
        let a = *legal.iter().find(|a| pick(&a.action_type)).expect("legal");
        t.apply(a);
        a
    }

    #[test]
    fn demo_123_trick_1_the_thrown_green_ace_was_standing() {
        // Owner report C22: VH (0) leads r-A, LH plays r-Z, MH (2) throws the g-A VH gave it.
        let hands = [
            cards(&["e-O", "s-6", "s-8", "g-A", "g-U", "r-O", "r-9", "e-8", "s-7"]),
            cards(&["e-9", "s-U", "e-A", "g-Z", "s-9", "e-U", "g-6", "r-Z", "e-K"]),
            cards(&["s-O", "g-9", "e-7", "r-6", "g-O", "r-A", "s-Z", "g-K", "r-U"]),
            cards(&["s-K", "s-A", "e-Z", "g-8", "g-7", "r-8", "e-6", "r-7", "r-K"]),
        ];
        let mut t = Table::new(&hands);
        for bid in [Some(120), Some(125), Some(135), None, Some(140), None, None] {
            act(&mut t, |a| match bid {
                Some(v) => *a == ActionType::NewBid(v),
                None => *a == ActionType::StopBidding,
            });
        }
        let set = |s: &[&str]| CardSet::from_cards(&cards(s));
        let pass = |x: CardSet| move |a: &ActionType| matches!(a, ActionType::Pass(c) if CardSet::from_cards(c) == x);
        act(&mut t, pass(set(&["r-A", "r-U", "r-6", "e-7"])));
        act(&mut t, pass(set(&["e-O", "e-8", "e-7", "g-A"])));
        let play = |c: &'static str| move |a: &ActionType| matches!(a, ActionType::CardPlayed(x) if x.to_string() == c);
        act(&mut t, play("r-A"));
        act(&mut t, play("r-Z"));
        let (seat, legal) = t.to_act().expect("MH to play");
        let find = |c: &str| {
            *legal
                .iter()
                .find(|a| matches!(&a.action_type, ActionType::CardPlayed(x) if x.to_string() == c))
                .expect("legal")
        };
        assert!(flags(&t, seat, legal, &find("g-A")).schmiered_standing);
        assert!(!flags(&t, seat, legal, &find("e-8")).schmiered_standing);
    }
}

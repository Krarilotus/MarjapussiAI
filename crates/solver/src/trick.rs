//! Known-deal control of the current trick and the next announcement opportunity.
//! Each goal has its own optimal continuation. Their conjunction is NOT guaranteed.

use engine::{FastState, Move, Suit};

/// Independently forceable goals: a seat's lead, a team's trick, or a team's
/// surviving uncalled pair activatable at the next opening.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrickControl(u16);

impl TrickControl {
    pub fn lead(self, seat: u8) -> bool {
        self.0 & (1 << seat) != 0
    }
    pub fn team(self, team: u8) -> bool {
        self.0 & (1 << (4 + team)) != 0
    }
    pub fn activation(self, team: u8, suit: Suit) -> bool {
        self.0 & (1 << (6 + 4 * team + suit as u8)) != 0
    }
}

const ALL: u16 = (1 << 14) - 1;
const EVEN: u16 = (1 << 0) | (1 << 2) | (1 << 4) | (0b1111 << 6);

/// `mv` must be legal. Questions/answers and their trump changes are traversed
/// using the engine. This is exposed-deal capability, not a hidden-information policy.
pub fn trick_control(state: FastState, mv: Move) -> TrickControl {
    debug_assert!(state.moves().any(|m| m == mv));
    TrickControl(control(state.after(mv), state.tricks_done() + 1))
}

fn control(state: FastState, end: u8) -> u16 {
    if state.tricks_done() == end {
        let seat = state.leader();
        let team = seat % 2;
        let mut goals = (1 << seat) | (1 << (4 + team));
        if !state.is_over() {
            for mv in state.moves() {
                match mv {
                    Move::Announce(s) => goals |= 1 << (6 + 4 * team + s as u8),
                    Move::Question(_) => {
                        let asked = state.after(mv);
                        for answer in asked.moves() {
                            let after = asked.after(answer);
                            let newly_called = after.called() & !state.called();
                            goals |= (newly_called as u16) << (6 + 4 * team);
                        }
                    }
                    _ => {}
                }
            }
        }
        return goals;
    }
    let ours = if state.to_move().is_multiple_of(2) {
        EVEN
    } else {
        ALL ^ EVEN
    };
    let (mut any, mut every) = (0, ALL);
    for mv in state.moves() {
        let child = control(state.after(mv), end);
        any |= child;
        every &= child;
    }
    (any & ours) | (every & !ours)
}

//! Replay a certificate using only actual engine transitions. Small ends explore
//! every legal follower branch; larger ends use deterministic representatives.

use engine::{points_set, trump_points, FastState, GameAction, Move, LAST_TRICK_BONUS};

pub fn check(state: FastState, plan: &[GameAction], exhaustive: bool) {
    let me = state.to_move();
    let required = if state.rights(me).may_announce() {
        state.hand(me).pair_suits().fold(0, |mask, s| mask | 1 << s as u8) & !state.called()
    } else {
        0
    };
    let pair_points: i32 = state
        .hand(me)
        .pair_suits()
        .filter(|&s| required & (1 << s as u8) != 0)
        .map(|s| trump_points(s, false))
        .sum();
    let expected = state.points(me % 2) + points_set(state.in_hands()) + LAST_TRICK_BONUS + pair_points;
    let mut leaves = 0;
    if exhaustive {
        assert!(state.hand(me).len() <= 3, "bound the exhaustive oracle");
        replay(state, plan, 0, me, required, expected, None, &mut leaves);
    } else {
        for representative in 0..4 {
            replay(
                state,
                plan,
                0,
                me,
                required,
                expected,
                Some(representative),
                &mut leaves,
            );
        }
    }
    assert!(leaves > 0);
}

#[allow(clippy::too_many_arguments)]
fn replay(
    state: FastState,
    plan: &[GameAction],
    next: usize,
    me: u8,
    required: u8,
    expected: i32,
    representative: Option<usize>,
    leaves: &mut usize,
) {
    if state.is_over() {
        assert_eq!(next, plan.len());
        assert_eq!(required & !state.called(), 0, "uncalled own pair points forfeited");
        assert_eq!(state.points(me % 2), expected, "normal engine scoring");
        *leaves += 1;
        return;
    }
    if state.to_move() == me {
        assert!(state.trick().is_empty(), "claimant must always lead");
        let action = plan.get(next).expect("unfinished certificate");
        assert_eq!(action.player.0, me);
        let mv = Move::from_action(&action.action_type).unwrap();
        assert!(matches!(mv, Move::Card(_) | Move::Announce(_)));
        assert!(
            state.moves().any(|legal| legal == mv),
            "illegal claim action {action:?}"
        );
        replay(
            state.after(mv),
            plan,
            next + 1,
            me,
            required,
            expected,
            representative,
            leaves,
        );
    } else {
        let legal: Vec<_> = state.moves().collect();
        assert!(
            legal.iter().all(|mv| matches!(mv, Move::Card(_))),
            "outside declaration window"
        );
        for (index, mv) in legal.iter().enumerate() {
            if representative.is_some_and(|r| index != (r + next + state.to_move() as usize) % legal.len()) {
                continue;
            }
            let after = state.after(*mv);
            if after.tricks_done() > state.tricks_done() {
                assert_eq!(after.leader(), me, "claimant lost a trick");
            }
            replay(after, plan, next, me, required, expected, representative, leaves);
        }
    }
}

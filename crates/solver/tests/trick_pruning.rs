//! Check the vector search against scalar minimax, one goal at a time.
use engine::{CardSet, FastState, Move, PlayPhase, PlayerTrumpPossibilities as Rights, Suit, SUITS};

#[derive(Clone, Copy)]
enum Goal {
    Lead(u8),
    Team(u8),
    Activation(u8, Suit),
}

impl Goal {
    fn team(self) -> u8 {
        match self {
            Self::Lead(s) => s % 2,
            Self::Team(t) | Self::Activation(t, _) => t,
        }
    }

    fn terminal(self, s: FastState) -> bool {
        match self {
            Self::Lead(seat) => s.leader() == seat,
            Self::Team(team) => s.leader() % 2 == team,
            Self::Activation(team, suit) => {
                if s.leader() % 2 != team || s.is_over() {
                    return false;
                }
                s.moves().any(|mv| match mv {
                    Move::Announce(named) => named == suit,
                    Move::Question(_) => {
                        let asked = s.after(mv);
                        asked
                            .moves()
                            .any(|answer| (asked.after(answer).called() & !s.called()) & (1 << suit as u8) != 0)
                    }
                    _ => false,
                })
            }
        }
    }
}

fn scalar(state: FastState, end: u8, goal: Goal) -> bool {
    if state.tricks_done() == end {
        return goal.terminal(state);
    }
    if state.to_move() % 2 == goal.team() {
        state.moves().any(|m| scalar(state.after(m), end, goal))
    } else {
        state.moves().all(|m| scalar(state.after(m), end, goal))
    }
}

#[test]
fn every_goal_matches_independent_scalar_search() {
    let mut rng = 4107u64;
    for example in 0..64 {
        let mut cards: Vec<u8> = (0..36).collect();
        for i in (1..36).rev() {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            cards.swap(i, (rng >> 32) as usize % (i + 1));
        }
        let n = 1 + example % 3;
        let hands = std::array::from_fn(|seat| {
            cards[seat * n..(seat + 1) * n]
                .iter()
                .fold(CardSet::EMPTY, |h, &c| h | CardSet::single(c))
        });
        let mut public = FastState::new(hands, (example % 4) as u8).public();
        public.tricks_done = 9 - n as u8;
        public.tricks = [public.tricks_done, 0];
        public.phase = PlayPhase::Start;
        public.trump = (example % 5 < 4).then(|| SUITS[example % 5]);
        public.called = public.trump.map_or(0, |s| 1 << s as u8);
        public.rights = std::array::from_fn(|seat| [Rights::Own, Rights::Yours, Rights::Ours][(example + seat) % 3]);
        let state = FastState::from_public(public, hands).unwrap();
        for mv in state.moves() {
            let actual = solver::trick_control(state, mv);
            let after = state.after(mv);
            let end = state.tricks_done() + 1;
            for seat in 0..4 {
                assert_eq!(
                    actual.lead(seat),
                    scalar(after, end, Goal::Lead(seat)),
                    "example {example}, {mv:?}, seat {seat}"
                );
            }
            for team in 0..2 {
                assert_eq!(actual.team(team), scalar(after, end, Goal::Team(team)));
                for suit in SUITS {
                    assert_eq!(
                        actual.activation(team, suit),
                        scalar(after, end, Goal::Activation(team, suit)),
                        "example {example}, {mv:?}, {suit:?}"
                    );
                }
            }
        }
    }
}

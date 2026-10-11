//! Relaxed public-fact bounds, not selection or feasibility.
//! These bound forward legal completions of one trusted post-exchange snapshot;
//! separate sequence evidence owns authorization, never this box or probabilities.

use engine::{
    game_scores, ActionType, AnswerType, GameAction, GameOutcome, GamePhase, PlayPhase, CARD_POINTS, LAST_TRICK_BONUS,
    MAX_BID, NO_BID, PAIR_POINTS, SCHWARZ_FACTOR_FIFTHS, SUITS, TRICKS,
};
use observation::SeatEvent;

use crate::Knowledge;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvelopeError {
    UnsupportedPhase,
    InvalidDomain,
    UndoHistory,
    InvalidPublicState,
}

/// Inclusive integer interval. It is not an expected value or ranking proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntegerInterval {
    pub lower: i64,
    pub upper: i64,
}
impl IntegerInterval {
    pub fn contains(self, value: i64) -> bool {
        self.lower <= value && value <= self.upper
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScoreIntervals {
    pub teams: [IntegerInterval; 2],
}
impl ScoreIntervals {
    /// Conservative signed integer objective; no f64/weighted certificate.
    pub fn difference(self, root_team: usize, opponent_weight: i32) -> Option<IntegerInterval> {
        let ours = *self.teams.get(root_team)?;
        let theirs = self.teams[1 - root_team];
        let alpha = i64::from(opponent_weight);
        let (lo, hi) = if alpha >= 0 {
            (theirs.lower, theirs.upper)
        } else {
            (theirs.upper, theirs.lower)
        };
        Some(IntegerInterval {
            lower: ours.lower.checked_sub(alpha.checked_mul(hi)?)?,
            upper: ours.upper.checked_sub(alpha.checked_mul(lo)?)?,
        })
    }
}

/// Independent team boxes are deliberate supersets, not feasible allocations.
/// Snapshot/history borrows prevent mutation through these references while used.
/// Caller must supply mutually consistent trusted Knowledge, actual public phase,
/// the FULL nonempty eligible engine domain and this observer's honest history.
/// We check shape/public budgets, not replay or domain completeness. Histories
/// containing Undone are unsupported, including canonical post-Undo recomputation.
/// The absence of a supplied Undone is not a global epoch/absence-of-Undo proof.
pub struct PublicOutcomeEnvelope<'a> {
    _snapshot: &'a Knowledge,
    _history: &'a [SeatEvent],
    playing_team: Option<u8>,
    game_value: i32,
    points: [[i32; 2]; 2],
    tricks: [[u8; 2]; 2],
    remaining_cards: i32,
    future_pairs: i32,
}

impl<'a> PublicOutcomeEnvelope<'a> {
    pub fn from_forward_play(
        knowledge: &'a Knowledge,
        phase: &GamePhase,
        legal: &[GameAction],
        history: &'a [SeatEvent],
    ) -> Result<Self, EnvelopeError> {
        if !matches!(
            phase,
            GamePhase::Trick | GamePhase::StartTrick | GamePhase::AnsweringPair | GamePhase::AnsweringHalf(_)
        ) {
            return Err(EnvelopeError::UnsupportedPhase);
        }
        if history.contains(&SeatEvent::Undone) {
            return Err(EnvelopeError::UndoHistory);
        }
        let actor = legal.first().ok_or(EnvelopeError::InvalidDomain)?.player;
        if actor.0 >= 4
            || legal
                .iter()
                .any(|action| action.player != actor || !matches_phase(&action.action_type, phase))
            || (!matches!(phase, GamePhase::Trick) && !knowledge.trick().is_empty())
        {
            return Err(EnvelopeError::InvalidDomain);
        }
        let public = knowledge.public_state(actor.0, PlayPhase::from_actions(legal));
        let total_cards = CARD_POINTS.iter().map(|&p| i32::from(p)).sum::<i32>() + LAST_TRICK_BONUS;
        let banked_cards = public.card_points.map(i32::from);
        let banked_pairs = public.pair_points.map(i32::from);
        let remaining_cards = total_cards - banked_cards.iter().sum::<i32>();
        let called_points = SUITS
            .into_iter()
            .filter(|&s| knowledge.is_called(s))
            .map(|s| PAIR_POINTS[s as usize])
            .sum::<i32>();
        let playing_team = knowledge.declarer().map(|seat| seat % 2);
        let game_value = knowledge.value();
        if public.tricks_done >= TRICKS
            || public.tricks.iter().sum::<u8>() != public.tricks_done
            || remaining_cards < 0
            || banked_cards.iter().chain(&banked_pairs).any(|&p| p < 0)
            || banked_pairs.iter().sum::<i32>() != called_points
            || playing_team.is_none() && game_value != NO_BID.0
            || playing_team.is_some() && !(NO_BID.0 < game_value && game_value <= MAX_BID)
        {
            return Err(EnvelopeError::InvalidPublicState);
        }
        // A spent half can never return under this forward/no-transfer contract.
        // Called pair credit is already banked and survives spending its halves.
        // Rights/opportunity are deliberately relaxed; no own right is NOT proof
        // that all whole/split partner-question activation routes are impossible.
        let future_pairs = SUITS
            .into_iter()
            .filter(|&s| !knowledge.is_called(s) && (knowledge.played() & engine::CardSet::halves(s)).is_empty())
            .map(|s| PAIR_POINTS[s as usize])
            .sum::<i32>();
        let left = TRICKS - public.tricks_done;
        Ok(Self {
            _snapshot: knowledge,
            _history: history,
            playing_team,
            game_value,
            points: std::array::from_fn(|t| {
                let banked = banked_cards[t] + banked_pairs[t];
                [banked, banked + remaining_cards + future_pairs]
            }),
            tricks: public.tricks.map(|won| [won, won + left]),
            remaining_cards,
            future_pairs,
        })
    }

    pub fn remaining_card_budget(&self) -> i32 {
        self.remaining_cards
    }
    pub fn future_pair_budget(&self) -> i32 {
        self.future_pairs
    }

    /// True means box containment, NEVER proof that this tuple is reachable.
    /// Context is fixed only within the constructor's forward no-Undo scope.
    pub fn contains(&self, outcome: &GameOutcome) -> bool {
        outcome.playing_team == self.playing_team
            && outcome.game_value == self.game_value
            && (0..2).all(|t| {
                self.points[t][0] <= outcome.team_points[t]
                    && outcome.team_points[t] <= self.points[t][1]
                    && self.tricks[t][0] <= outcome.team_tricks[t]
                    && outcome.team_tricks[t] <= self.tricks[t][1]
            })
    }

    pub fn score_intervals(&self) -> ScoreIntervals {
        score_box(self.points, self.tricks, self.playing_team, self.game_value)
    }
}

fn matches_phase(action: &ActionType, phase: &GamePhase) -> bool {
    match (phase, action) {
        (GamePhase::Trick, ActionType::CardPlayed(_)) => true,
        (GamePhase::StartTrick, ActionType::CardPlayed(_) | ActionType::AnnounceTrump(_) | ActionType::Question(_)) => {
            true
        }
        (GamePhase::AnsweringPair, ActionType::Answer(AnswerType::NoPair | AnswerType::YesPair(_))) => true,
        (GamePhase::AnsweringHalf(s), ActionType::Answer(AnswerType::NoHalf(t) | AnswerType::YesHalf(t))) => s == t,
        _ => false,
    }
}

/// At most 4*4*2*2 score calls. On each threshold/zero-trick region the engine's
/// integer scores are constant or monotone (round5 included). Endpoints plus
/// threshold-1/threshold cover every region; trick endpoints cover zero/nonzero.
/// This uses a larger independent box even when its combinations are infeasible.
fn score_box(points: [[i32; 2]; 2], tricks: [[u8; 2]; 2], playing_team: Option<u8>, game_value: i32) -> ScoreIntervals {
    let probes = points.map(|[lo, hi]| [lo, hi, (game_value - 1).clamp(lo, hi), game_value.clamp(lo, hi)]);
    let mut teams = [IntegerInterval {
        lower: i64::MAX,
        upper: i64::MIN,
    }; 2];
    for a in probes[0] {
        for b in probes[1] {
            for x in tricks[0] {
                for y in tricks[1] {
                    let scores = game_scores(
                        &GameOutcome {
                            playing_team,
                            game_value,
                            team_points: [a, b],
                            team_tricks: [x, y],
                        },
                        SCHWARZ_FACTOR_FIFTHS,
                    );
                    for t in 0..2 {
                        teams[t].lower = teams[t].lower.min(i64::from(scores[t]));
                        teams[t].upper = teams[t].upper.max(i64::from(scores[t]));
                    }
                }
            }
        }
    }
    ScoreIntervals { teams }
}

#[cfg(test)]
#[path = "outcome_envelope_tests.rs"]
mod tests;

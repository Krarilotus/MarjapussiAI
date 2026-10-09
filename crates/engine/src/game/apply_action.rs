//! Applying a legal action, one function per kind of action. The functions change the state,
//! the undo snapshot and (at start and end) the meta info; legality is checked by the caller.
//!
//! The undo snapshot (`last_state`) is the state before a bid or a card; undo requests and
//! accepts keep it; every other action clears it.

use std::sync::Arc;

use crate::bits::{self, CardSet};
use crate::game::bidding::NO_BID;
use crate::game::cards::{Card, Suit};
use crate::game::clock::current_time_string;
use crate::game::gameevent::{ActionType, AnswerType, GameAction, GameCallback, QuestionType};
use crate::game::gamestate::{FinishedTrick, GamePhase, GameState};
use crate::game::meta::GameMetaInfo;
use crate::game::player::PlaceAtTable;
use crate::game::points::Points;
use crate::game::rules::trick_points;

/// What one action may change.
pub(crate) struct Mutable<'a> {
    pub state: &'a mut GameState,
    pub last_state: &'a mut Option<GameState>,
    pub info: &'a mut Arc<GameMetaInfo>,
}

/// Applies a legal action and returns its callback.
pub(crate) fn apply(t: Mutable, action: &GameAction) -> Option<GameCallback> {
    let player = action.player;
    match action.action_type {
        ActionType::Start => start(t, player),
        ActionType::NewBid(value) => bid(t, action, value),
        ActionType::StopBidding => stop_bidding(t, action),
        ActionType::Pass(cards) => pass(t, player, cards),
        ActionType::CardPlayed(card) => play(t, player, card),
        ActionType::AnnounceTrump(suit) => {
            *t.last_state = None;
            Some(new_trump(t.state, suit))
        }
        ActionType::Question(q) => question(t, player, q),
        ActionType::Answer(a) => answer(t, a),
        ActionType::UndoRequest | ActionType::UndoAccept | ActionType::UndoDecline => undo(t, action),
    }
}

fn start(t: Mutable, player: PlaceAtTable) -> Option<GameCallback> {
    let state = t.state;
    if !state.players_started.contains(&player) {
        state.players_started.push(player);
    }
    if state.players_started.len() == 4 {
        state.started = true;
        Arc::make_mut(t.info).start_time = Some(current_time_string());
        state.phase = GamePhase::Bidding;
    }
    *t.last_state = None;
    None
}

fn bid(t: Mutable, action: &GameAction, value: i32) -> Option<GameCallback> {
    snapshot(t.last_state, t.state);
    let state = t.state;
    state.bidding_history.push((action.action_type, action.player));
    state.value = Points(value);
    if state.phase == GamePhase::Raising {
        state.phase = GamePhase::Trick;
        return None;
    }
    let next = next_bidder(state);
    if next == state.player_at_turn {
        //same player can't bid against himself
        state.phase = GamePhase::PassingForth;
        state.player_at_turn = state.player_at_turn.partner();
    } else {
        state.player_at_turn = next;
    }
    None
}

/// The next seat after the player at turn that is still bidding.
fn next_bidder(state: &GameState) -> PlaceAtTable {
    let mut next = state.player_at_turn.next();
    while !state.player_at_place(next).bidding {
        next = next.next();
    }
    next
}

fn stop_bidding(t: Mutable, action: &GameAction) -> Option<GameCallback> {
    snapshot(t.last_state, t.state);
    let state = t.state;
    state.bidding_history.push((action.action_type, action.player));
    state.player_at_turn_mut().bidding = false;
    state.bidding_players -= 1;
    state.player_at_turn = if state.bidding_players >= 1 {
        next_bidder(state)
    } else {
        state.player_at_turn.next()
    };
    //bidding ends
    if state.bidding_players == 1 && state.value != NO_BID {
        state.phase = GamePhase::PassingForth;
        for player in &state.players {
            if player.bidding {
                state.player_at_turn = player.place_at_table.partner();
            }
        }
    }
    if state.bidding_players == 0 {
        // nobody takes game
        state.phase = GamePhase::Trick;
    }
    None
}

fn pass(t: Mutable, player: PlaceAtTable, cards: [Card; 4]) -> Option<GameCallback> {
    let state = t.state;
    state.player_at_place_mut(player).cards.retain(|x| !cards.contains(x));
    state
        .player_at_place_mut(player.partner())
        .cards
        .extend_from_slice(&cards);
    if state.phase == GamePhase::PassingBack {
        state.phase = GamePhase::Raising;
    } else {
        state.phase = GamePhase::PassingBack;
        state.player_at_turn = player.partner();
    }
    *t.last_state = None;
    None
}

fn play(t: Mutable, player: PlaceAtTable, card: Card) -> Option<GameCallback> {
    snapshot(t.last_state, t.state);
    let state = t.state;
    act_card(card, state);
    state.player_at_place_mut(player).play_card(card);
    if state.player_at_turn().cards.is_empty() {
        state.phase = GamePhase::Ended;
        Arc::make_mut(t.info).end_time = Some(current_time_string());
    }
    None
}

/// `suit` becomes trump (an announcement or a pair shown in an answer); once per suit.
fn new_trump(state: &mut GameState, suit: Suit) -> GameCallback {
    state.trump_called.push(suit);
    state.trump = Some(suit);
    state.phase = GamePhase::Trick;
    GameCallback::NewTrump(suit)
}

fn question(t: Mutable, player: PlaceAtTable, q: QuestionType) -> Option<GameCallback> {
    let state = t.state;
    let asker = state.player_at_place_mut(player);
    asker.trump = asker.trump.after_question(q);
    state.phase = match q {
        QuestionType::Yours => GamePhase::AnsweringPair,
        QuestionType::YourHalf(suit) => GamePhase::AnsweringHalf(suit),
    };
    state.player_at_turn = state.player_at_turn.partner();
    *t.last_state = None;
    None
}

fn answer(t: Mutable, a: AnswerType) -> Option<GameCallback> {
    let state = t.state;
    *t.last_state = None;
    let callback = match a {
        AnswerType::NoPair => None,
        AnswerType::YesPair(suit) => Some(new_trump(state, suit)),
        AnswerType::NoHalf(suit) => Some(GameCallback::NoHalf(suit)),
        AnswerType::YesHalf(suit) => {
            let asker_hand = CardSet::from_cards(&state.partner().cards);
            if let Some(trump) = super::rules::answer_trump(asker_hand, a, state.trump_called.contains(&suit)) {
                Some(new_trump(state, trump))
            } else if (asker_hand & CardSet::halves(suit)).is_empty() {
                Some(GameCallback::OnlyHalf(suit))
            } else {
                // Both halves remain public evidence, but a called suit cannot become trump again.
                Some(GameCallback::StillTrump(suit))
            }
        }
    };
    state.player_at_turn = state.player_at_turn.partner();
    state.phase = GamePhase::Trick;
    callback
}

fn undo(t: Mutable, action: &GameAction) -> Option<GameCallback> {
    let state = t.state;
    match action.action_type {
        ActionType::UndoAccept => {
            if !state.players_accept_undo.contains(&action.player) {
                state.players_accept_undo.push(action.player);
            }
            if state.players_accept_undo.len() == 2 {
                if let Some(mut previous) = t.last_state.take() {
                    previous.players_accept_undo.clear();
                    *state = previous;
                }
            }
        }
        ActionType::UndoDecline => {
            if let GamePhase::PendingUndo(previous_phase) = &mut state.phase {
                let previous = std::mem::replace(&mut **previous_phase, GamePhase::Ended);
                state.phase = previous;
                state.players_accept_undo.clear();
            }
            *t.last_state = None;
        }
        _ => {
            //last_state is always Some() because otherwise action not legal
            if t.last_state.is_some() {
                let phase = std::mem::replace(&mut state.phase, GamePhase::Ended);
                state.phase = GamePhase::PendingUndo(Box::new(phase));
            }
        }
    }
    None
}

/// Stores `state` as the undo snapshot, reusing the previous snapshot's buffers.
fn snapshot(last_state: &mut Option<GameState>, state: &GameState) {
    match last_state {
        Some(last) => last.clone_from(state),
        None => *last_state = Some(state.clone()),
    }
}

/// Puts `card` into the current trick and, when the trick is full, scores it and gives the
/// turn to its winner (rule owners: `bits::winning_seat`, `rules::trick_points`).
pub(crate) fn act_card(card: Card, state: &mut GameState) {
    if state.current_trick.len() >= 4 {
        state.current_trick.clear();
    }
    state.current_trick.push(card);
    state.phase = GamePhase::Trick;
    state.player_at_turn = state.player_at_turn.next();
    if state.current_trick.len() == 4 {
        let cards: [Card; 4] = [
            state.current_trick[0],
            state.current_trick[1],
            state.current_trick[2],
            state.current_trick[3],
        ];
        let idx = cards.map(|c| c.index());
        // After four plays the turn is back at the leader.
        let leader = state.player_at_turn;
        state.player_at_turn = PlaceAtTable(bits::winning_seat(leader.0, &idx, state.trump));
        state.phase = GamePhase::StartTrick;
        let points = trick_points(idx, state.all_tricks.len() as u8);
        state.all_tricks.push(FinishedTrick {
            cards,
            leader,
            winner: state.player_at_turn,
            points: Points(points),
        });
    }
}

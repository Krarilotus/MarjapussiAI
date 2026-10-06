//! Legal actions of a state, as a list (`push_legal_actions`) or for one action
//! (`is_legal_in_phase`). Undo requests are added by `Game`. Works on the state and the undo
//! snapshot only; the card-play rule comes from `bits::play_levels`.

use crate::bits::{self, CardSet};
use crate::game::bidding::bid_range;
use crate::game::cards::{Card, Suit, SUITS};
use crate::game::gameevent::{ActionType, AnswerType, GameAction, QuestionType};
use crate::game::gamestate::{GamePhase, GameState};
use crate::game::player::{PlayerTrumpPossibilities, HAND_SIZE};

/// Appends the legal actions of the state's phase to `out` (without undo requests).
/// `last` is the undo snapshot (needed while an undo is pending).
pub(crate) fn push_legal_actions(state: &GameState, last: Option<&GameState>, out: &mut Vec<GameAction>) {
    match &state.phase {
        GamePhase::WaitingForStart => out.extend(
            state
                .players
                .iter()
                .filter(|p| !state.players_started.contains(&p.place_at_table))
                .map(|p| GameAction {
                    action_type: ActionType::Start,
                    player: p.place_at_table,
                }),
        ),
        GamePhase::Bidding => push_bidding(state, out),
        GamePhase::PassingForth | GamePhase::PassingBack => push_passing(state, out),
        GamePhase::Raising => {
            // The bids from the highest down, without stopping (the bidding list reversed,
            // its StopBidding removed).
            let start = out.len();
            push_bidding(state, out);
            out[start..].reverse();
            out.pop();
            push_cards(state, out);
        }
        GamePhase::StartTrick => {
            push_question(state, out);
            push_cards(state, out);
        }
        GamePhase::Trick => push_cards(state, out),
        GamePhase::AnsweringPair => push_answer(state, QuestionType::Yours, out),
        GamePhase::AnsweringHalf(suit) => push_answer(state, QuestionType::YourHalf(*suit), out),
        GamePhase::Ended => {}
        GamePhase::PendingUndo(_previous_phase) => {
            let next_player = last.expect("an undo is pending").player_at_turn.next();
            for player in [next_player, next_player.partner()] {
                if !state.players_accept_undo.contains(&player) {
                    out.push(GameAction {
                        action_type: ActionType::UndoAccept,
                        player,
                    });
                    out.push(GameAction {
                        action_type: ActionType::UndoDecline,
                        player,
                    });
                }
            }
        }
    }
}

/// Whether `action` (not an undo request) is in the phase's legal actions. Cards, passes, bids
/// and starts are checked directly; the rest against the phase's short list
/// (tests/legality.rs checks the equivalence).
pub(crate) fn is_legal_in_phase(state: &GameState, last: Option<&GameState>, action: &GameAction) -> bool {
    let at_turn = action.player == state.player_at_turn;
    let bid_ok = |v: i32| at_turn && bid_range(state.value).any(|b| b == v);
    match (&state.phase, &action.action_type) {
        (GamePhase::Raising | GamePhase::StartTrick | GamePhase::Trick, ActionType::CardPlayed(card)) => {
            at_turn && allowed_cards(state).contains(card.index())
        }
        (_, ActionType::CardPlayed(_)) => false,
        (GamePhase::PassingForth | GamePhase::PassingBack, ActionType::Pass(cards)) => {
            at_turn && is_legal_pass(state, cards)
        }
        (_, ActionType::Pass(_)) => false,
        (GamePhase::Bidding | GamePhase::Raising, ActionType::NewBid(v)) => bid_ok(*v),
        (_, ActionType::NewBid(_)) => false,
        (GamePhase::Bidding, ActionType::StopBidding) => at_turn,
        (_, ActionType::StopBidding) => false,
        (GamePhase::WaitingForStart, ActionType::Start) => {
            action.player.0 < 4 && !state.players_started.contains(&action.player)
        }
        (_, ActionType::Start) => false,
        _ => {
            let mut list = Vec::with_capacity(16);
            push_legal_actions(state, last, &mut list);
            list.contains(action)
        }
    }
}

fn push_bidding(state: &GameState, out: &mut Vec<GameAction>) {
    let player = state.player_at_turn;
    out.push(GameAction {
        action_type: ActionType::StopBidding,
        player,
    });
    for value in bid_range(state.value) {
        out.push(GameAction {
            action_type: ActionType::NewBid(value),
            player,
        })
    }
}

/// Every 4-card subset of the hand, in lexicographic order of hand positions, each sorted from
/// high to low.
fn push_passing(state: &GameState, out: &mut Vec<GameAction>) {
    let cards = &state.player_at_turn().cards;
    let player = state.player_at_turn;
    let n = cards.len();
    if n < 4 {
        return;
    }
    out.reserve(n * (n - 1) * (n - 2) * (n - 3) / 24);
    // `Card::index` orders cards like `Card`'s `Ord` (suit, then value).
    let idx: Vec<u8> = cards.iter().map(|c| c.index()).collect();
    for a in 0..n {
        for b in a + 1..n {
            for c in b + 1..n {
                for d in c + 1..n {
                    let pass = sorted_desc([idx[a], idx[b], idx[c], idx[d]]).map(Card::from_index);
                    out.push(GameAction {
                        action_type: ActionType::Pass(pass),
                        player,
                    })
                }
            }
        }
    }
}

/// Four card indices from high to low (a sorting network).
fn sorted_desc(mut x: [u8; 4]) -> [u8; 4] {
    for (i, j) in [(0, 1), (2, 3), (0, 2), (1, 3), (1, 2)] {
        if x[i] < x[j] {
            x.swap(i, j);
        }
    }
    x
}

/// Whether `cards` is a pass the player at turn may make: four cards of their hand from high
/// to low, i.e. exactly the members of the passing list.
fn is_legal_pass(state: &GameState, cards: &[Card]) -> bool {
    let hand = &state.player_at_turn().cards;
    cards.len() == 4 && cards.windows(2).all(|w| w[0] > w[1]) && cards.iter().all(|c| hand.contains(c))
}

/// The first trick is played while the player still holds a full hand.
pub fn is_first_trick(hand_len: usize) -> bool {
    hand_len == HAND_SIZE
}

/// The cards the player at turn may play, as a set (rule owner: `bits::play_levels`).
fn allowed_cards(state: &GameState) -> CardSet {
    let cards = &state.player_at_turn().cards;
    let trick = if state.current_trick.len() == 4 {
        &[][..]
    } else {
        &state.current_trick[..]
    };
    let mut idx = [0u8; 4];
    for (slot, c) in idx.iter_mut().zip(trick) {
        *slot = c.index();
    }
    let hand = CardSet::from_cards(cards);
    bits::play_levels(&idx[..trick.len()], state.trump, is_first_trick(cards.len())).allowed(hand)
}

/// The cards the player at turn may play, in hand order.
fn push_cards(state: &GameState, out: &mut Vec<GameAction>) {
    let player = state.player_at_turn;
    let allowed = allowed_cards(state);
    for &card in &state.player_at_turn().cards {
        if allowed.contains(card.index()) {
            out.push(GameAction {
                action_type: ActionType::CardPlayed(card),
                player,
            });
        }
    }
}

/// Suits of which `hand` holds both King and Ober, in enum order.
fn pair_suits(hand: CardSet) -> impl Iterator<Item = Suit> {
    SUITS.into_iter().filter(move |&s| CardSet::halves(s).is_subset(hand))
}

fn push_question(state: &GameState, actions: &mut Vec<GameAction>) {
    let player = state.player_at_turn();
    let action = |action_type| GameAction {
        action_type,
        player: player.place_at_table,
    };
    if player.trump == PlayerTrumpPossibilities::Own {
        for suit in pair_suits(CardSet::from_cards(&player.cards)) {
            if !state.trump_called.contains(&suit) {
                actions.push(action(ActionType::AnnounceTrump(suit)));
            }
        }
    }
    if player.trump != PlayerTrumpPossibilities::Ours {
        actions.push(action(ActionType::Question(QuestionType::Yours)));
    }
    for suit in [Suit::Red, Suit::Bells, Suit::Acorns, Suit::Green] {
        actions.push(action(ActionType::Question(QuestionType::YourHalf(suit))));
    }
}

/// The answers to `question`, which the phase records: `AnsweringPair` follows "yours?",
/// `AnsweringHalf(suit)` follows "your half of `suit`?".
fn push_answer(state: &GameState, question: QuestionType, actions: &mut Vec<GameAction>) {
    let hand = CardSet::from_cards(&state.player_at_turn().cards);
    let player = state.player_at_turn;
    let answer = |a| GameAction {
        action_type: ActionType::Answer(a),
        player,
    };
    match question {
        QuestionType::Yours => {
            let start = actions.len();
            for suit in pair_suits(hand) {
                //don't allow double calling
                if !state.trump_called.contains(&suit) {
                    actions.push(answer(AnswerType::YesPair(suit)));
                }
            }
            if actions.len() == start {
                actions.push(answer(AnswerType::NoPair));
            }
        }
        QuestionType::YourHalf(suit) => actions.push(answer(if (hand & CardSet::halves(suit)).is_empty() {
            AnswerType::NoHalf(suit)
        } else {
            AnswerType::YesHalf(suit)
        })),
    }
}

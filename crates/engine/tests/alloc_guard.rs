//! Guard for the measured speed wins (R3), without timing noise: heap allocations counted
//! by a global allocator, per thread.
//! - `fast`: no allocation at all in card play.
//! - `Game` card play: the undo snapshot and the legal-action list reuse their buffers.
//! - `Game` passing: a pass is a `[Card; 4]`, so the 126 + 715 pass actions allocate nothing.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use marjapussi_engine::bits::CardSet;
use marjapussi_engine::fast::FastState;
use marjapussi_engine::game::cards::{get_all_cards, Card};
use marjapussi_engine::game::gameevent::ActionType;
use marjapussi_engine::game::Game;

struct Counting;

thread_local! {
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOCS.try_with(|a| a.set(a.get() + 1));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _ = ALLOCS.try_with(|a| a.set(a.get() + 1));
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn allocs() -> u64 {
    ALLOCS.with(|a| a.get())
}

fn deal(seed: usize) -> [Vec<Card>; 4] {
    let mut deck = get_all_cards();
    let mut x = seed as u64 * 2_654_435_761 + 1;
    for i in (1..36).rev() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        deck.swap(i, (x % (i as u64 + 1)) as usize);
    }
    [0, 1, 2, 3].map(|s| deck[s * 9..s * 9 + 9].to_vec())
}

/// Plays one game with the first legal action (no undo, no stop while bidding is open);
/// returns (allocations, actions) for the card plays and for the two passes.
fn play(seed: usize) -> ((u64, u64), (u64, u64)) {
    let mut game = Game::new(
        "alloc".into(),
        ["S0", "S1", "S2", "S3"].map(String::from),
        Some(deal(seed)),
    );
    let (mut card, mut pass) = ((0, 0), (0, 0));
    while !game.ended() {
        // The legal list is built outside the measured window (it is computed on demand).
        let legal = game.legal_actions();
        let action = *legal
            .iter()
            .find(|a| a.action_type != ActionType::UndoRequest && a.action_type != ActionType::StopBidding)
            .unwrap_or(&legal[0]);
        let card_play = matches!(action.action_type, ActionType::CardPlayed(_));
        let passing = matches!(action.action_type, ActionType::Pass(_));
        let before = allocs();
        game.apply_action_mut(action);
        let used = allocs() - before;
        if card_play {
            card = (card.0 + used, card.1 + 1);
        }
        if passing {
            pass = (pass.0 + used, pass.1 + 1);
        }
    }
    (card, pass)
}

#[test]
fn speed_wins_hold_in_allocations() {
    // fast: a whole game of card play without allocation.
    let hands = deal(1).map(|h| CardSet::from_cards(&h));
    let before = allocs();
    let mut s = FastState::new(hands, 0);
    while !s.is_over() {
        let m = s.moves().next().unwrap();
        s.apply(m);
    }
    assert_eq!(allocs() - before, 0, "fast card play allocated");

    play(2); // warms buffers that are reused across games
    let ((card_allocs, cards), (pass_allocs, passes)) = play(3);
    eprintln!("Game: {card_allocs} allocations in {cards} card plays, {pass_allocs} in {passes} passes");
    assert!(cards == 36 && passes == 2);
    // Per card: the event's time string and amortized list growth; after a question the undo
    // snapshot is rebuilt (this policy asks at every trick start). Measured 2026-10-06: 148 on
    // this branch, 2,696 on main (68c98f0).
    assert!(
        card_allocs <= 5 * cards,
        "card play allocates again: {card_allocs} in {cards}"
    );
    // Passes are `[Card; 4]` (no allocation each). Measured: 5 for both passes; 2,224 on main
    // 68c98f0.
    assert!(pass_allocs <= 20, "pass lists allocate again: {pass_allocs}");
}

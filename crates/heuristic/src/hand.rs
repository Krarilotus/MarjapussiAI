//! Card order within a suit as bit operations on `CardSet`, from the engine's trick order
//! (`engine::takes`). Hand classes, pair values and the EGP model are the conventions'
//! (`beliefs::conventions`).

use engine::{suit_of, takes, CardSet};

/// Per card, the cards of its suit that take it without trump: computed once from the engine's
/// trick order (`bits::takes`).
fn higher_table() -> &'static [CardSet; 36] {
    static HIGHER: std::sync::OnceLock<[CardSet; 36]> = std::sync::OnceLock::new();
    HIGHER.get_or_init(|| {
        std::array::from_fn(|i| {
            let i = i as u8;
            CardSet(
                CardSet::suit(suit_of(i))
                    .iter()
                    .filter(|&b| takes(b, i, None))
                    .fold(0, |a, b| a | 1 << b),
            )
        })
    })
}

/// Cards of `index`'s suit that take it when no trump is involved.
pub fn higher(index: u8) -> CardSet {
    higher_table()[index as usize]
}

/// The highest card of `set` (by the trick order within one suit), if any.
pub fn top(set: CardSet) -> Option<u8> {
    set.iter().find(|&c| (higher(c) & set).is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Suit, Value};

    #[test]
    fn top_follows_the_trick_order() {
        let s = CardSet::suit(Suit::Red);
        assert_eq!(top(s), Some(Suit::Red as u8 * 9 + Value::Ace as u8));
        assert_eq!(
            top(s - CardSet::value(Value::Ace)),
            Some(Suit::Red as u8 * 9 + Value::Ten as u8)
        );
    }
}

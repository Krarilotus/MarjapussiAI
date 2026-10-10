//! Fact queries on top of the deduction (knowledge audit K1–K5, C26): the one owner of the facts
//! both agents need. Each is a sound, tier-1 consequence of `Knowledge` (checked against the
//! true hands in `tests/soundness.rs`), computed lazily from the current state; `Knowledge`
//! changes only per event, so a caller may keep a result for the rest of a decision.

use engine::Suit;
use engine::{opponents, partner, suit_of, takes, winning_seat, CardSet, PlayerTrumpPossibilities, PublicState, SUITS};
use observation::Seat;

use crate::Knowledge;

/// KB `standing_potential` of a hand (C43b): Σ over the suits headed by A Z (my top run from
/// the ace ≥ 2 cards) with ≥ 4 cards of (length − run), the lower cards that become standing
/// once the opponents' cards of the suit are gone. The count form is the orchestrator's, of the
/// owner's "the other U and 9 can become standing pretty easy he just needs two more of that
/// suit given to him. Even more if there would be the king".
pub fn standing_potential_of(hand: CardSet) -> u32 {
    let run = own_standing_of(hand);
    SUITS
        .into_iter()
        .map(|s| {
            let (len, top) = ((hand & CardSet::suit(s)).len(), (run & CardSet::suit(s)).len());
            if top >= 2 && len >= 4 {
                len - top
            } else {
                0
            }
        })
        .sum()
}

/// KB `own_standing` of a set of cards (e.g. the 9 VH keeps, `vh_standing`): the cards that
/// stand from the set alone, the top run of each suit: every higher card of the suit is in it.
pub fn own_standing_of(hand: CardSet) -> CardSet {
    hand.iter()
        .filter(|&c| {
            CardSet::suit(suit_of(c))
                .iter()
                .filter(|&b| takes(b, c, None))
                .all(|b| hand.contains(b))
        })
        .fold(CardSet::EMPTY, |a, c| a | CardSet::single(c))
}

/// `stopper`: the lowest card that can stop a suit has 3 higher cards (the ober); the unter
/// (4 higher) and lower never stop it ([O] 2026-10-07).
pub const STOPPER_MAX_HIGHER: u32 = 3;

/// `trick_confidence` per stopper, by its number of higher cards (A, Z with 1 more, K with 2
/// more, O with 3 more). Owner-confirmed 2026-10-07 (`findings/kb/concepts.md`, batch 1b
/// addendum 4).
pub const STOPPER_CONFIDENCE: [f32; 4] = [1.0, 0.8, 0.5, 0.2];

/// `trick_confidence`: the chance to miss a trick grows by this factor per blank ten (a ten
/// alone in its suit, never a stopper). Owner-confirmed 2026-10-07 (batch 1b addendum 5).
pub const BLANK_TEN_FACTOR: f32 = 1.1;

/// Suits (bit = suit) of pairs: certainly held, and still possibly held (both halves live and
/// holdable, the suit not called: meldable).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pairs {
    pub certain: u8,
    pub possible: u8,
}

/// A team's pairs from one seat's side (K3): MY pair (the seat holds both halves), YOUR pair
/// (its partner does), OUR pair (one half each).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TeamPairs {
    pub mine: Pairs,
    pub yours: Pairs,
    pub ours: Pairs,
}

impl TeamPairs {
    /// Suits in which the team may still hold the pair, in any of the three ways.
    pub fn possible(&self) -> u8 {
        self.mine.possible | self.yours.possible | self.ours.possible
    }

    pub fn certain(&self) -> u8 {
        self.mine.certain | self.yours.certain | self.ours.certain
    }
}

/// How a seat's passes went, from my view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Passed {
    #[default]
    Nothing,
    /// I saw the cards.
    Seen(CardSet),
    /// The seat passed or received four cards I did not see.
    Hidden,
}

impl Knowledge {
    /// Known presence of a live half, from card sets and cardinality constraints.
    /// `None` means these facts do not decide the answer; no belief threshold is used.
    pub fn known_half(&self, seat: Seat, suit: Suit) -> Option<bool> {
        let halves = CardSet::halves(suit);
        if (self.may_hold(seat) & halves).is_empty() {
            Some(false)
        } else if !(self.certain(seat) & halves).is_empty()
            || self
                .constraints()
                .iter()
                .any(|c| c.seat == seat && c.lo > 0 && c.mask.is_subset(halves))
        {
            Some(true)
        } else {
            None
        }
    }

    /// Whether `seat` may hold all of `set`: every card holdable, and no constraint of the seat
    /// caps its count of them below that.
    fn may_hold_all(&self, seat: Seat, set: CardSet) -> bool {
        set.is_subset(self.may_hold(seat))
            && self
                .constraints()
                .iter()
                .filter(|c| c.seat == seat)
                .all(|c| (set & c.mask).len() <= c.hi as u32)
    }

    /// K4: sound bounds on how many cards of `suit` `seat` holds now.
    pub fn count_bounds(&self, seat: Seat, suit: Suit) -> (u8, u8) {
        let s = CardSet::suit(suit);
        let n = self.hand_len(seat) as u32;
        let lo_cons = self
            .constraints()
            .iter()
            .filter(|c| c.seat == seat && c.mask.is_subset(s))
            .map(|c| c.lo as u32)
            .max()
            .unwrap_or(0);
        let lo = (self.certain(seat) & s)
            .len()
            .max(n.saturating_sub((self.may_hold(seat) - s).len()))
            .max(lo_cons);
        let hi = (self.may_hold(seat) & s)
            .len()
            .min(n - (self.certain(seat) - s).len().min(n));
        (lo as u8, hi as u8)
    }

    /// K1: the cards whose lead certainly wins for `seat`'s team now (partner may overtake).
    /// Every higher live card
    /// of the suit is certainly with the seat's team, and (for a side suit under trump) each
    /// opponent certainly must follow the suit or certainly holds no trump. Top trumps count.
    pub fn standing(&self, seat: Seat) -> CardSet {
        let team = self.certain(seat) | self.certain(partner(seat));
        let opponents = opponents(seat);
        let mut out = CardSet::EMPTY;
        for c in self.certain(seat).iter() {
            let suit = suit_of(c);
            let higher = (CardSet::suit(suit) - self.played)
                .iter()
                .filter(|&b| takes(b, c, None))
                .fold(CardSet::EMPTY, |a, b| a | CardSet::single(b));
            if !higher.is_subset(team) {
                continue;
            }
            let safe = match self.trump {
                Some(t) if t != suit => opponents
                    .iter()
                    .all(|&o| self.count_bounds(o, suit).0 > 0 || (self.may_hold(o) & CardSet::suit(t)).is_empty()),
                _ => true,
            };
            if safe {
                out |= CardSet::single(c);
            }
        }
        out
    }

    /// Leads that certainly retain this particular seat's lead. Unlike `standing`,
    /// partner must not be able to overtake or ruff the card either.
    pub fn retaining_lead(&self, seat: Seat) -> CardSet {
        let mate = partner(seat);
        self.standing(seat)
            .iter()
            .filter(|&c| {
                let suit = suit_of(c);
                let over = (self.may_hold(mate) & CardSet::suit(suit))
                    .iter()
                    .any(|b| takes(b, c, None));
                let ruff = self.trump.is_some_and(|t| {
                    t != suit
                        && self.count_bounds(mate, suit).0 == 0
                        && !(self.may_hold(mate) & CardSet::suit(t)).is_empty()
                });
                !over && !ruff
            })
            .fold(CardSet::EMPTY, |set, c| set | CardSet::single(c))
    }

    /// Known partner cards that can legally win after `seat` leads `card`,
    /// whatever either opponent plays. The same winner works in every possible
    /// hand; this is a sufficient fact, not a sampled trick-win probability.
    /// The caller supplies a legal lead into an empty trick.
    pub fn partner_entry_winners(&self, seat: Seat, card: u8) -> CardSet {
        if !self.trick().is_empty() {
            return CardSet::EMPTY;
        }
        let mate = partner(seat);
        let [left, right] = opponents(seat);
        let first = engine::is_first_trick(self.hand_len(seat) as usize);
        let possible = self.may_hold(mate);
        let right_known = self.certain(right);
        self.standing(mate)
            .iter()
            .filter(|&winner| {
                self.may_hold(left).iter().all(|l| {
                    let trick = [card, l];
                    let high = trick[engine::trick_winner(&trick, self.trump)];
                    if !takes(winner, high, self.trump)
                        || !(engine::play_levels(&trick, self.trump, first).excluded_by(winner) & possible).is_empty()
                    {
                        return false;
                    }
                    let levels = engine::play_levels(&[card, l, winner], self.trump, first);
                    self.may_hold(right)
                        .iter()
                        .all(|r| !takes(r, winner, self.trump) || !(levels.excluded_by(r) & right_known).is_empty())
                })
            })
            .fold(CardSet::EMPTY, |set, c| set | CardSet::single(c))
    }

    /// K2: the seat's own pairs, certain and still meldable (uncalled, both halves live).
    pub fn pairs(&self, seat: Seat) -> Pairs {
        let mut p = Pairs::default();
        for s in SUITS.into_iter().filter(|&s| !self.is_called(s)) {
            let halves = CardSet::halves(s);
            if !(halves & self.played).is_empty() {
                continue;
            }
            if halves.is_subset(self.certain(seat)) {
                p.certain |= 1 << s as u8;
            }
            if self.may_hold_all(seat, halves) {
                p.possible |= 1 << s as u8;
            }
        }
        p
    }

    /// K3: the pairs of `seat`'s team, by who holds them.
    pub fn team_pairs(&self, seat: Seat) -> TeamPairs {
        let mate = partner(seat);
        let mut ours = Pairs::default();
        for s in SUITS.into_iter().filter(|&s| !self.is_called(s)) {
            let halves = CardSet::halves(s);
            if !(halves & self.played).is_empty() {
                continue;
            }
            let mut it = halves.iter().map(CardSet::single);
            let (Some(a), Some(b)) = (it.next(), it.next()) else {
                continue;
            };
            let certain = |x: CardSet, y: CardSet| x.is_subset(self.certain(seat)) && y.is_subset(self.certain(mate));
            let possible =
                |x: CardSet, y: CardSet| x.is_subset(self.may_hold(seat)) && y.is_subset(self.may_hold(mate));
            if certain(a, b) || certain(b, a) {
                ours.certain |= 1 << s as u8;
            }
            if possible(a, b) || possible(b, a) {
                ours.possible |= 1 << s as u8;
            }
        }
        TeamPairs {
            mine: self.pairs(seat),
            yours: self.pairs(mate),
            ours,
        }
    }

    /// The cards `seat` has played.
    pub fn played_by(&self, seat: Seat) -> CardSet {
        self.played_by[seat as usize]
    }

    /// KB `own_standing`: the cards of `seat` that stand from its own certain cards alone, the
    /// top run of each suit (A; A, Z; A, Z, K; …): every higher card of the suit is its own.
    pub fn own_standing(&self, seat: Seat) -> CardSet {
        own_standing_of(self.certain(seat))
    }

    /// KB `stopper(s)`: the highest card of `suit` in `seat`'s certain hand that stops the
    /// opponents from running the suit: the ace; the ten with ≥ 1 more card of the suit; the
    /// king with ≥ 2 more; the ober with ≥ 3 more ("weak sauce"); the unter and lower never
    /// ([O] 2026-10-07: "the opponent can't play that suit for us to regain control"). A card
    /// with r higher cards in the suit needs r more cards of mine to follow until it wins.
    pub fn stopper(&self, seat: Seat, suit: Suit) -> Option<u8> {
        self.stopper_rank(seat, suit).map(|(c, _)| c)
    }

    /// `stopper`, with its number of higher cards in the suit (0 = the ace … 3 = the ober).
    pub fn stopper_rank(&self, seat: Seat, suit: Suit) -> Option<(u8, u32)> {
        let mine = self.certain(seat) & CardSet::suit(suit);
        mine.iter()
            .map(|c| {
                let higher = CardSet::suit(suit).iter().filter(|&b| takes(b, c, None)).count() as u32;
                (c, higher)
            })
            .filter(|&(_, higher)| higher <= STOPPER_MAX_HIGHER && mine.len() > higher)
            .min_by_key(|&(_, higher)| higher)
    }

    /// KB `trick_confidence`: how sure `seat` is of a trick of its own. The chance to miss is
    /// the product over its stoppers of (1 − c), c by the stopper's rank
    /// (`STOPPER_CONFIDENCE`), times `BLANK_TEN_FACTOR` per blank ten ("the multiplications are
    /// the norm"); the confidence is 1 − that, floored at 0. The more and the stronger the
    /// stoppers, the less schwarz danger "per default"; blank tens weaken the hand on top ([O]
    /// 2026-10-07).
    pub fn trick_confidence(&self, seat: Seat) -> f32 {
        let hand = self.certain(seat);
        let blank_tens = SUITS
            .into_iter()
            .filter(|&s| (hand & CardSet::suit(s)).len() == 1 && !(hand & CardSet::suit(s) & CardSet::TENS).is_empty())
            .count() as i32;
        let miss = SUITS
            .into_iter()
            .filter_map(|s| self.stopper_rank(seat, s))
            .map(|(_, higher)| 1.0 - STOPPER_CONFIDENCE[higher as usize])
            .product::<f32>()
            * BLANK_TEN_FACTOR.powi(blank_tens);
        (1.0 - miss).max(0.0)
    }

    /// KB `standing_against_us`: how many cards of the opponents stand against `seat`'s
    /// certain hand, suit by suit, played from the top (owner-confirmed 2026-10-07, batch 1b
    /// addendum 7). Every live card the seat doesn't hold counts as the opponents' (the worst
    /// case after the partner's pass). While their highest card beats the seat's highest
    /// remaining card, it counts and the seat's lowest card of the suit falls; once the seat's
    /// card is the highest remaining, their lower cards don't count. If the seat's cards of the
    /// suit run out without winning (a void suit included), all their remaining cards of the
    /// suit count. Range 0–27.
    pub fn standing_against_us(&self, seat: Seat) -> u32 {
        let hand = self.certain(seat);
        // Highest first by the trick order.
        let order = |a: &u8, b: &u8| {
            if takes(*a, *b, None) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        };
        SUITS
            .into_iter()
            .map(|s| {
                let mut mine: Vec<u8> = (hand & CardSet::suit(s)).iter().collect();
                let mut theirs: Vec<u8> = ((CardSet::suit(s) - self.played()) - hand).iter().collect();
                mine.sort_by(order);
                theirs.sort_by(order);
                let mut standing = 0;
                let mut t = 0;
                loop {
                    match (mine.first(), theirs.get(t)) {
                        // My cards ran out without winning: all their remaining cards count.
                        (None, _) => return standing + (theirs.len() - t) as u32,
                        (Some(_), None) => return standing,
                        (Some(&m), Some(&x)) if takes(x, m, None) => {
                            standing += 1;
                            t += 1;
                            mine.pop();
                        }
                        // My card is the highest remaining: it wins.
                        _ => return standing,
                    }
                }
            })
            .sum()
    }

    /// KB `ace_ten(s)`: `seat` certainly holds the ace and the ten of `suit`.
    pub fn ace_ten(&self, seat: Seat, suit: Suit) -> bool {
        (self.certain(seat) & CardSet::suit(suit) & (CardSet::ACES | CardSet::TENS)).len() == 2
    }

    /// K6: the cards `seat` discarded: played neither in the led suit nor in trump.
    pub fn discarded(&self, seat: Seat) -> CardSet {
        self.discarded[seat as usize]
    }

    /// What `seat` gave and received in the exchange, from my view.
    pub fn given(&self, seat: Seat) -> Passed {
        self.given[seat as usize]
    }

    pub fn received(&self, seat: Seat) -> Passed {
        self.received[seat as usize]
    }

    /// The seat that passed first (the declarer's partner, MH), once the exchange began.
    pub fn first_passer(&self) -> Option<Seat> {
        self.first_passer
    }

    /// The cards of a pass I saw (else none).
    fn seen(p: Passed) -> CardSet {
        match p {
            Passed::Seen(cards) => cards,
            _ => CardSet::EMPTY,
        }
    }

    /// Cards that may have reached `seat` in the exchange. A hidden pass went between my two
    /// opponents, so nothing my team was dealt, holds or played can be in it.
    fn may_have_received(&self, seat: Seat) -> CardSet {
        match self.received(seat) {
            Passed::Nothing => CardSet::EMPTY,
            Passed::Seen(cards) => cards,
            Passed::Hidden if self.me == crate::PUBLIC => !self.played,
            Passed::Hidden => {
                let mate = partner(self.me);
                !(self.dealt_certain(self.me) | self.certain(mate) | self.played_by(mate) | self.played_by(self.me))
            }
        }
    }

    /// K5: the dealt hand from `now ∪ played`, what `seat` gave and what it got. The first
    /// passer gave before it got (cards it gave may come back); the second passer got before
    /// it gave (it may pass on cards it got). See `docs/design/knowledge.md` K5.
    fn dealt_from(&self, seat: Seat, now: CardSet, gave: CardSet, got: CardSet) -> CardSet {
        let held = now | self.played_by(seat);
        if self.first_passer == Some(seat) {
            (held - got) | gave
        } else {
            (held | gave) - got
        }
    }

    /// K5: cards certainly in `seat`'s **dealt** hand (the hand it bid with).
    pub fn dealt_certain(&self, seat: Seat) -> CardSet {
        let gave = Self::seen(self.given(seat));
        if seat == self.me {
            return self.dealt_from(seat, self.certain(seat), gave, Self::seen(self.received(seat)));
        }
        self.dealt_from(seat, self.certain(seat), gave, self.may_have_received(seat))
    }

    /// K5: cards that may have been in `seat`'s dealt hand.
    pub fn dealt_possible(&self, seat: Seat) -> CardSet {
        if seat == self.me {
            return self.dealt_certain(seat);
        }
        let gave = match self.given(seat) {
            Passed::Nothing => CardSet::EMPTY,
            Passed::Seen(cards) => cards,
            // The cards it gave are with its partner now, played by it, or given back.
            Passed::Hidden => {
                let to = partner(seat);
                self.may_hold(to) | self.played_by(to)
            }
        };
        let elsewhere = (0..4u8)
            .filter(|&y| y != seat)
            .fold(CardSet::EMPTY, |a, y| a | self.dealt_certain(y));
        self.dealt_from(seat, self.may_hold(seat), gave, Self::seen(self.received(seat))) - elsewhere
    }

    /// K6: the seat's question rights now (as the engine keeps them).
    pub fn rights(&self, seat: Seat) -> PlayerTrumpPossibilities {
        self.rights[seat as usize]
    }

    /// K6: the declarer (the seat whose bid set the game value); `None` before a bid and when
    /// all passed.
    pub fn declarer(&self) -> Option<Seat> {
        self.last_bidder
    }

    /// K6: the game value (115 before the first bid).
    pub fn value(&self) -> i32 {
        self.value
    }

    /// K6: the winners of the finished tricks, in order.
    pub fn trick_winners(&self) -> &[Seat] {
        &self.trick_winners[..self.tricks_done as usize]
    }

    /// K6: the seat that led the trick in progress (or the last finished trick).
    pub fn trick_leader(&self) -> Seat {
        self.trick_leader
    }

    /// K6: the seat and card winning the trick in progress (the engine's `winning_seat`).
    pub fn winning(&self) -> Option<(Seat, u8)> {
        let trick = self.trick();
        if trick.is_empty() {
            return None;
        }
        let w = winning_seat(self.trick_leader, trick, self.trump());
        Some((w, trick[(w + 4 - self.trick_leader) as usize % 4]))
    }

    /// K6: points per team (team 0 = seats 0 and 2): card points of the finished tricks with the
    /// last-trick bonus, plus announced pairs, as the engine's `team_points` counts them.
    pub fn team_points(&self) -> [i32; 2] {
        [0, 1].map(|t| self.card_points[t] + self.pair_points[t])
    }

    /// K6: the public card-play state (`engine::PublicState`) with `to_move` the seat to move
    /// now, for the solver's worlds. The public play phase comes from the legal actions.
    pub fn public_state(&self, to_move: Seat, phase: engine::PlayPhase) -> PublicState {
        let trick = self.trick();
        let mut cards = [0; 4];
        cards[..trick.len()].copy_from_slice(trick);
        let won = |team: u8| self.trick_winners().iter().filter(|&&w| w % 2 == team).count() as u8;
        PublicState {
            trick: cards,
            trick_len: trick.len() as u8,
            leader: if matches!(phase, engine::PlayPhase::Answer(_)) {
                engine::partner(to_move)
            } else {
                (to_move + 4 - trick.len() as u8) % 4
            },
            trump: self.trump(),
            called: self.called,
            rights: [0, 1, 2, 3].map(|seat| self.rights(seat)),
            phase,
            tricks_done: self.tricks_done,
            tricks: [won(0), won(1)],
            card_points: self.card_points.map(|p| p as i16),
            pair_points: self.pair_points.map(|p| p as i16),
        }
    }
}

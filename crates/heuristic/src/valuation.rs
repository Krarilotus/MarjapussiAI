//! What a card is worth to keep and to give: the one owner (C25; netspec C23 "a smart
//! baseline from ideas"). Every card choice (lead, follow, schmieren, give) is an argmax of
//! `value_of_play` over the legal cards; there are no separate filters or tie-breaks.
//!
//! Every term is in one unit, the expected final score difference (ours − theirs), from the
//! engine's scoring (`stakes.rs`): what a point is worth depends on who plays the contract,
//! how close it is, and whether a team still lacks a trick.
//!
//! A card's **keep value** is the sum of three ideas:
//! - **future-trick value (P5):** P(it wins a trick before the end) × that trick's worth. The
//!   probability is the believed form `Mind::future_win` (F10: 1 if it stands; else the higher
//!   cards are with the partner or fall first, and no opponent ruffs). A standing ace is worth
//!   much; a blank ten under an outstanding ace is not.
//! - **pair activation value (HC-1, HC-9, C24):** for a half of a reachable pair, meld × P(the
//!   other half is in our team) × P(activation) from `trump_plan::Reach` (rights, leads,
//!   attempts). The meld is worth what it changes in the expected score: for declarers only
//!   through P(made) × 2 × value (nothing once it is made); for defenders its points.
//! - **plan value (P7):** leading a suit in which the partner holds an unannounced half of our
//!   team's pair (passed, received or believed) can force that half out; the lead pays for it.
//!
//! Playing a card now is worth P(we take the trick) × the score if we take it plus the rest ×
//! the score if they do, plus the value of the lead if we take it (the **trick term**), minus
//! the card's keep value and, when
//! leading, its plan value. P(we take it) comes from belief-weighted beaters among the seats
//! still to play. A choice is labelled by the term that separates it most from the runner-up.

use engine::Suit;
use engine::{next_seat, opponents, partner, suit_of, takes, CardSet, SUITS};
use engine::{CARD_POINTS, LAST_TRICK_BONUS};
use observation::Seat;

use beliefs::conventions::pair_value;
use beliefs::{Concept, ConceptQuery};

use crate::hand::higher;
use crate::stakes::Stakes;
use crate::trump_plan::{Control, Reach};
use crate::Ctx;

/// The ideas a choice can turn on (for labels: the term that separates the choice from the
/// runner-up names the rule).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Term {
    /// This trick: its points for whoever takes it, and the lead it brings.
    Trick,
    Future,
    Pair,
    Plan,
    /// A certain loser is not led (HL-7).
    Win,
}

/// One card choice: the card, its value, the deciding term, and the runner-up.
pub(crate) struct Best {
    pub card: u8,
    pub value: f32,
    pub term: Term,
    pub runner_up: Option<u8>,
}

pub(crate) struct Valuation {
    /// Per card of mine: P(it wins a later trick).
    p_win: [f32; 36],
    future: [f32; 36],
    pair: [f32; 36],
    plan: [f32; 36],
    /// My cards that carry pair activation value.
    pub halves: CardSet,
    /// My cards in the suits of those pairs (P7): the suit becomes trump once the pair is
    /// announced, so they are not cashed or counted as sure leads before.
    pub plan_cards: CardSet,
    /// My cards in the suits of the uncalled pairs the partner certainly holds (e.g. one I
    /// passed it, K5): plan suits for me too, not led before the announcement, since the
    /// partner would have to follow with a half (F10, owner #25 item 7).
    pub protect: CardSet,
    /// Per suit: value gained by activating it at the next opening, conditional
    /// on the query establishing that the pair survives and can be activated.
    lead_gain: [f32; 4],
    /// Per suit of my own uncalled pair: the meld value lost if it is not named now (HC-6):
    /// its worth × P(it is not activated later).
    pub at_risk: [f32; 4],
    /// Expected points of a card from a hand I don't know.
    pub avg_card: f32,
    /// Our meld counts through the contract (HC-9): we declare.
    pub meld_needed: bool,
    /// What points are worth now (the score difference model).
    pub stakes: Stakes,
}

fn pts(c: u8) -> f32 {
    CARD_POINTS[c as usize] as f32
}

/// P(seat `o` holds a card of `set`), from the card beliefs.
fn p_any(ctx: &Ctx, o: Seat, set: CardSet) -> f32 {
    1.0 - (set & ctx.k.may_hold(o))
        .iter()
        .map(|b| 1.0 - ctx.mind.p_holds(o, b))
        .product::<f32>()
}

impl Valuation {
    pub fn new(ctx: &Ctx, reach: &Reach) -> Valuation {
        let k = ctx.k;
        let me = ctx.me;
        let hand = k.certain(me);
        let unknown = !(hand | k.played());
        let avg_card = unknown.iter().map(pts).sum::<f32>() / unknown.len().max(1) as f32;
        let mut v = Valuation {
            p_win: [0.0; 36],
            future: [0.0; 36],
            pair: [0.0; 36],
            plan: [0.0; 36],
            halves: CardSet::EMPTY,
            plan_cards: CardSet::EMPTY,
            protect: CardSet::EMPTY,
            lead_gain: [0.0; 4],
            at_risk: [0.0; 4],
            avg_card,
            meld_needed: false,
            stakes: Stakes::new(k, me),
        };
        // Future tricks: the believed P(it wins a trick before the end) (F10).
        for c in hand.iter() {
            v.p_win[c as usize] = ctx.mind.future_win(c);
        }
        // Pair activation (from Reach) and what a meld is worth in the score (HC-9).
        let stakes = v.stakes;
        let meld = |s: Suit| stakes.meld(pair_value(s) as f64) as f32;
        v.meld_needed = stakes.declaring();
        // Leads that can still activate a pair: wins before the last trick (a lead in trick 9
        // comes from winning trick 8, which is this trick's value, not a future one).
        let n = k.hand_len(me) as f32;
        let before_last = ((n - 2.0) / (n - 1.0).max(1.0)).max(0.0);
        let control = Control {
            own: hand.iter().map(|c| v.p_win[c as usize]).sum::<f32>() * before_last,
            partner: reach.p_partner_lead * before_last,
        };
        let mate = ctx.mind.features(partner(me));
        let mut future_lead = 0.0f32;
        for s in SUITS.into_iter().filter(|&s| !k.is_called(s)) {
            let mine = hand & CardSet::halves(s);
            let p_team = match mine.len() {
                2 => 1.0,
                1 => reach.halves.iter().find(|h| h.0 == s).map_or(0.0, |h| h.1),
                _ => mate.pair[s as usize],
            };
            if p_team <= 0.0 {
                continue;
            }
            let worth = p_team * meld(s);
            let p_act = reach.p_activate(ctx, s, control);
            for c in mine.iter() {
                v.pair[c as usize] = worth * p_act;
            }
            if worth * p_act > 0.0 {
                v.halves |= mine;
                if !mine.is_empty() || mate.pair[s as usize] >= 1.0 {
                    v.plan_cards |= hand & CardSet::suit(s);
                }
                if mate.pair[s as usize] >= 1.0 {
                    v.protect |= hand & CardSet::suit(s);
                }
            }
            // The joint query includes counterpart ownership exactly once.
            v.lead_gain[s as usize] = meld(s) * (1.0 - p_act);
            future_lead = future_lead.max(worth * (reach.p_activate(ctx, s, control.with_own_lead()) - p_act));
            if mine.len() == 2 {
                v.at_risk[s as usize] = worth * (1.0 - p_act);
            }
        }
        // In the last trick no lead follows.
        if n <= 1.0 {
            v.lead_gain = [0.0; 4];
        }
        // A trick won is worth its other three cards and, unless it is the last one, the lead
        // it brings (`before_last`: the share of my later tricks that are not the last).
        let lead = before_last * future_lead;
        for c in hand.iter() {
            let t = (pts(c) + 3.0 * avg_card) as f64;
            let (take, give) = stakes.trick(t, t);
            v.future[c as usize] = v.p_win[c as usize] * ((take - give) as f32 + lead);
        }
        // Plan (P7): leading suit s may force the partner's unannounced half of our pair out.
        let mate = partner(me);
        for s in SUITS.into_iter().filter(|&s| !k.is_called(s)) {
            let theirs = CardSet::halves(s) & k.may_hold(mate);
            if theirs.is_empty() {
                continue;
            }
            // Forced if the partner has no other card of the suit.
            let others = (k.may_hold(mate) & CardSet::suit(s)) - CardSet::halves(s);
            // Ownership and being forced to break that pair are dependent: the
            // same suit capacity constrains both. Query their conjunction.
            let required = if !(hand & CardSet::halves(s)).is_empty() {
                Concept::Count {
                    seat: mate,
                    cards: theirs,
                    min: 1,
                    max: 2,
                }
            } else {
                Concept::Pair { seat: mate, suit: s }
            };
            let forced_pair = ConceptQuery {
                event: vec![
                    required.into(),
                    Concept::Count {
                        seat: mate,
                        cards: others,
                        min: 0,
                        max: 0,
                    }
                    .into(),
                ],
                given: vec![],
            };
            let probability = ctx.joint_probability(&forced_pair).unwrap_or_else(|| {
                let pair = if !(hand & CardSet::halves(s)).is_empty() {
                    p_any(ctx, mate, theirs)
                } else {
                    ctx.mind.features(mate).pair[s as usize]
                };
                pair * (1.0 - p_any(ctx, mate, others))
            });
            let cost = probability * meld(s) * reach.p_activate(ctx, s, control);
            for c in (hand & CardSet::suit(s)).iter() {
                v.plan[c as usize] = cost;
            }
        }
        v
    }

    /// One next opening can activate at most one pair. Each suit has its own
    /// continuation; choosing the best expected route does not add incompatible plans.
    pub fn lead_value(&self, ctx: &Ctx, c: u8) -> f32 {
        if self.lead_gain.iter().all(|&v| v == 0.0) {
            return 0.0;
        }
        let entry = ctx.entry(c);
        self.lead_gain
            .iter()
            .zip(entry.activation)
            .map(|(&v, p)| v * p)
            .fold(0.0, f32::max)
    }
}

/// HL-7 (tier 0): an opponent certainly holds a higher card of the suit that my partner cannot
/// top, or an opponent certainly void in it certainly holds trump.
fn certainly_loses(ctx: &Ctx, c: u8) -> bool {
    let k = ctx.k;
    let mate = partner(ctx.me);
    let suit = suit_of(c);
    opponents(ctx.me).iter().any(|&o| {
        let over = k.certain(o) & higher(c);
        let beaten = crate::hand::top(over).is_some_and(|b| (k.may_hold(mate) & higher(b)).is_empty());
        let ruffed = k
            .trump()
            .is_some_and(|t| t != suit && k.is_void(o, suit) && !(k.certain(o) & CardSet::suit(t)).is_empty());
        beaten || ruffed
    })
}

/// One card choice: the legal cards, who takes the trick for each, and the trick so far.
pub(crate) struct Choice {
    /// Points already in the trick (with the last-trick bonus).
    pub trick_points: f32,
    /// Card points of the cards already in the trick.
    in_trick: f32,
    /// Seats still to play after me, in order.
    pub later: Vec<Seat>,
    /// The card currently winning and its seat (None when I lead).
    pub winner: Option<(Seat, u8)>,
    pub led: Option<Suit>,
    /// The trick outcome over the believed worlds (`outlook::trick_outcomes`), computed once on
    /// first use: P(ours) and the expected trick points per card.
    worlds: std::cell::OnceCell<Option<crate::outlook::TrickOutcomes>>,
}

impl Choice {
    pub fn new(ctx: &Ctx) -> Choice {
        let trick = ctx.k.trick();
        let later: Vec<Seat> = std::iter::successors(Some(next_seat(ctx.me)), |&s| Some(next_seat(s)))
            .take(3 - trick.len())
            .collect();
        let last = ctx.k.hand_len(ctx.me) == 1;
        Choice {
            trick_points: trick.iter().map(|&c| pts(c)).sum::<f32>() + if last { LAST_TRICK_BONUS as f32 } else { 0.0 },
            in_trick: trick.iter().map(|&c| pts(c)).sum::<f32>(),
            later,
            winner: ctx.k.winning(),
            led: trick.first().map(|&c| suit_of(c)),
            worlds: std::cell::OnceCell::new(),
        }
    }

    /// P(seat `o` can and does beat `w` in this trick): it holds a higher card of the led suit,
    /// or it is void there and holds a trump that takes `w`.
    fn p_beats(&self, ctx: &Ctx, o: Seat, w: u8) -> f32 {
        let led = self.led.unwrap_or(suit_of(w));
        let trump = ctx.k.trump();
        let over = |set: CardSet| {
            set.iter()
                .filter(|&b| takes(b, w, trump))
                .fold(CardSet::EMPTY, |a, b| a | CardSet::single(b))
        };
        let in_suit = over(CardSet::suit(led));
        let ruff = trump
            .filter(|&t| t != led)
            .map_or(CardSet::EMPTY, |t| over(CardSet::suit(t)));
        let follow_query = ConceptQuery::new(Concept::Count {
            seat: o,
            cards: in_suit,
            min: 1,
            max: 9,
        });
        let ruff_query = ConceptQuery {
            event: vec![
                Concept::Void { seat: o, suit: led }.into(),
                Concept::Count {
                    seat: o,
                    cards: ruff,
                    min: 1,
                    max: 9,
                }
                .into(),
            ],
            given: vec![],
        };
        match (ctx.joint_probability(&follow_query), ctx.joint_probability(&ruff_query)) {
            // The events are mutually exclusive: a void player cannot also
            // overtake in the led suit. Trump ownership is conditional on void.
            (Some(follows), Some(ruffs)) => (follows + ruffs).min(1.0),
            _ => {
                let follows = p_any(ctx, o, in_suit);
                let void = ctx.mind.p_void(o, led);
                (follows + (1.0 - follows) * void * p_any(ctx, o, ruff)).min(1.0)
            }
        }
    }

    /// The world-resolved trick outcome of `c`, when the outlook is on.
    fn world_outcome(&self, ctx: &Ctx, c: u8) -> Option<&crate::outlook::TrickOutcome> {
        if !ctx.params.outlook_trick {
            return None;
        }
        self.worlds
            .get_or_init(|| crate::outlook::trick_outcomes(ctx))
            .as_ref()
            .and_then(|o| o[c as usize].as_ref())
    }

    /// P(our team takes the trick if I play `c`): over the believed worlds when the outlook is on,
    /// else from the single-seat estimates below.
    pub fn p_ours(&self, ctx: &Ctx, c: u8) -> f32 {
        if let Some(o) = self.world_outcome(ctx, c) {
            return o.p_ours;
        }
        let mate = partner(ctx.me);
        let trump = ctx.k.trump();
        let (seat, w) = match self.winner {
            Some((s, w)) if !takes(c, w, trump) => (s, w),
            _ => (ctx.me, c),
        };
        let mut ours = if seat % 2 == ctx.me % 2 { 1.0 } else { 0.0 };
        for &o in &self.later {
            let b = self.p_beats(ctx, o, w);
            ours = if o == mate {
                ours + (1.0 - ours) * b
            } else {
                ours * (1.0 - b)
            };
        }
        ours
    }

    /// The terms of playing `c` now (higher is better); the value is their sum.
    fn terms(&self, ctx: &Ctx, v: &Valuation, c: u8, leading: bool) -> [(Term, f32); 4] {
        // The trick's worth: over the joint outcomes of the believed worlds (who takes it, with
        // the points it then holds), each valued by the score model before averaging; else from
        // P(ours) and one points estimate.
        let worth = |trick: f32| v.stakes.trick(trick as f64, (trick - self.in_trick) as f64);
        let trick_worth = match self.world_outcome(ctx, c) {
            Some(o) => o
                .outcomes
                .iter()
                .map(|&(ours, pts, p)| {
                    let (take, give) = worth(pts as f32);
                    p * if ours { take } else { give } as f32
                })
                .sum::<f32>(),
            None => {
                let p = self.p_ours(ctx, c);
                let (take, give) = worth(self.trick_points + pts(c) + self.later.len() as f32 * v.avg_card);
                p * take as f32 + (1.0 - p) * give as f32
            }
        };
        let plan = if leading { v.plan[c as usize] } else { 0.0 };
        [
            (Term::Trick, trick_worth + v.lead_value(ctx, c)),
            (Term::Future, -v.future[c as usize]),
            (Term::Pair, -v.pair[c as usize]),
            (Term::Plan, -plan),
        ]
    }

    /// The value of playing `c` now (higher is better).
    pub fn value_of_play(&self, ctx: &Ctx, v: &Valuation, c: u8, leading: bool) -> f32 {
        self.terms(ctx, v, c, leading).iter().map(|t| t.1).sum()
    }

    /// The best card of `set` by value, labelled by the term that separates it most from the
    /// runner-up.
    pub fn best(&self, ctx: &Ctx, v: &Valuation, set: CardSet, leading: bool) -> Best {
        // HL-7 ([RV] "never give a trick away earlier than necessary"): a lead that certainly
        // loses hands the opponents the tempo to cash their standing cards, which no points
        // term prices; such leads are only played when every lead certainly loses.
        let losers = set
            .iter()
            .filter(|&c| leading && certainly_loses(ctx, c))
            .fold(CardSet::EMPTY, |a, c| a | CardSet::single(c));
        let cand = if losers == set { set } else { set - losers };
        // C34 (owner: "you look what your partner drops in colour and usually keep a different
        // one"; "a tie break of what to drop if otherwise undecided, so very small weight"):
        // between cards whose values round to the same whole point (the game's smallest unit), a
        // follower keeps the colours the partner drops, so the team keeps different ones.
        // Rounding is monotone, so without that preference the order is the exact value's.
        let dropped = ctx.k.discarded(partner(ctx.me));
        let drops_partners_colour = |c: u8| !leading && !(dropped & CardSet::suit(suit_of(c))).is_empty();
        let top = |s: CardSet| {
            s.iter()
                .map(|c| (c, self.value_of_play(ctx, v, c, leading)))
                .max_by(|a, b| {
                    a.1.round()
                        .total_cmp(&b.1.round())
                        .then(drops_partners_colour(b.0).cmp(&drops_partners_colour(a.0)))
                        .then(a.1.total_cmp(&b.1))
                        .then(b.0.cmp(&a.0))
                })
        };
        let (card, value) = top(cand).unwrap_or((0, 0.0));
        let runner_up = top(set - CardSet::single(card)).map(|r| r.0);
        let term = match runner_up {
            // The value preferred a certain loser.
            Some(r) if losers.contains(r) && !losers.contains(card) && top(set).is_some_and(|u| u.0 == r) => Term::Win,
            Some(r) => {
                // The term with the largest gain over the runner-up; on a tie, the trick.
                let (a, b) = (self.terms(ctx, v, card, leading), self.terms(ctx, v, r, leading));
                (0..a.len())
                    .filter(|&i| a[i].1 > b[i].1)
                    .max_by(|&i, &j| (a[i].1 - b[i].1).total_cmp(&(a[j].1 - b[j].1)))
                    .map_or(Term::Trick, |i| a[i].0)
            }
            None => Term::Trick,
        };
        Best {
            card,
            value,
            term,
            runner_up,
        }
    }
}

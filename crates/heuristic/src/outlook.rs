//! The plan layer's chain at a lead (KB Batch 3/4/7 and the owner's game reviews, 2026-10-12):
//! for each candidate card, over the believed worlds (the shared `DealModel` pool of `routes`):
//!
//! 1. **Trick outcome:** the engine's legal replies (follow, overtake, trump) are applied exactly;
//!    genuine choices use a level-0 response (`respond`): with the partner winning, play cheap
//!    (the last player feeds points that are not a live pair's half); otherwise win with the
//!    cheapest winning card, else discard the cheapest card, keeping pair halves.
//! 2. **Next leader and what it can do (`activation`):** at every trick start the leader
//!    activates what the rules allow and its knowledge leaves open: an own pair (`order`: the
//!    shorter suit first, at equal length the smaller pair, so the biggest and longest stays
//!    trump); else "Yours" while the partner may still hold a pair; else the most valuable half
//!    whose other half the partner may hold. The asked seat answers truthfully from its own
//!    cards (`answer0`, the same order); a "no" leaves the trump but spends the rights.
//! 3. **The rest of the game (`playout`):** the leader cashes a card that stands (every higher
//!    card of its suit is played) unless an opponent may ruff it, else leads into the partner's
//!    ruff (a side suit the partner is proven void in while it may hold trump, and not where
//!    the opponent behind it may ruff too), else its cheapest card outside its live pairs; the
//!    others respond as in 1.
//!
//! **Knowledge first (owner, 2026-10-12):** the actors decide from what is known, never from the
//! sampled hands (strategy fusion, Astra): `Known` carries every seat's possible cards from the
//! knowledge layer and narrows them along the line with the engine's play-level exclusions (a
//! card off the led suit proves the void). The world only resolves what stays open: which cards
//! a seat really holds and so what it can play and answer. `Known` starts from my seat's
//! knowledge for every actor, an approximation of each actor's own view.
//!
//! So pairs live or die by their guards and by the discard material in the suits their holder
//! is void in; one won lead can start two activations (the owner's game 938); and asker and
//! answerer can cross-ruff while the trump changes (the owner's game 1187) where the default
//! order happens to produce it. Competing authored routes for that are the next step.
//!
//! At the root (`route_values`) my own activation options and answers are compared over the
//! believed worlds: the same choice in every world.
//!
//! The value of a candidate is our expected points (cards, last trick, pairs) minus theirs over
//! those worlds: an estimate under level-0 play, not a proof.

use engine::{
    partner, suit_of, AnswerType, CardSet, FastState, Move, PlayPhase, QuestionType, Suit, CARD_POINTS, SUITS,
};
use knowledge::World;

use crate::rules::RuleId;
use crate::{decision, fact, Ctx, Decision};
use engine::GameAction;

/// The strongest card of `set` in suit `s` (no other card of `set` in `s` beats it).
fn top_of(set: CardSet, s: Suit) -> Option<u8> {
    (set & CardSet::suit(s)).iter().max_by_key(|&c| beats_key(c))
}

/// Order of strength within a suit, as the engine's `takes` defines it.
fn beats_key(c: u8) -> u8 {
    let mut key = 0;
    for other in CardSet::suit(suit_of(c)).iter() {
        if other != c && engine::takes(c, other, None) {
            key += 1;
        }
    }
    key
}

/// Halves of the uncalled pairs a seat still holds complete (cards it should not throw away).
fn live_halves(hand: CardSet, called: u8) -> CardSet {
    hand.pair_suits()
        .filter(|&s| called >> s as u8 & 1 == 0)
        .fold(CardSet::EMPTY, |set, s| set | CardSet::halves(s))
}

/// Level-0 response of the seat to move in `state` (its legal cards under the engine's rules).
fn respond(state: &FastState) -> u8 {
    let seat = state.to_move();
    let legal = state.legal_cards();
    if legal.len() == 1 {
        return legal.iter().next().expect("one legal card");
    }
    let trick = state.trick();
    let keep = live_halves(state.hand(seat), state.called());
    let cheapest = |set: CardSet| {
        let open = match set - keep {
            o if o.is_empty() => set,
            o => o,
        };
        open.iter()
            .min_by_key(|&c| (CARD_POINTS[c as usize], beats_key(c)))
            .expect("nonempty")
    };
    if trick.is_empty() {
        return cheapest(legal);
    }
    let leader = state.leader();
    let winner_seat = engine::winning_seat(leader, trick, state.trump());
    let winning_card = trick[((winner_seat + 4 - leader) % 4) as usize];
    let last = trick.len() == 3;
    if winner_seat % 2 == seat % 2 {
        if last {
            // The trick is ours: feed points that are not a live pair's half.
            let feed = legal - keep;
            if let Some(c) = feed.iter().max_by_key(|&c| (CARD_POINTS[c as usize], beats_key(c))) {
                return c;
            }
        }
        return cheapest(legal);
    }
    let winners = legal
        .iter()
        .filter(|&c| engine::takes(c, winning_card, state.trump()))
        .fold(CardSet::EMPTY, |s, c| s | CardSet::single(c));
    if !winners.is_empty() {
        return winners
            .iter()
            .min_by_key(|&c| {
                (
                    beats_key(c) as i32 + if keep.contains(c) { 100 } else { 0 },
                    CARD_POINTS[c as usize],
                )
            })
            .expect("nonempty");
    }
    cheapest(legal)
}

/// What every seat may still hold: the knowledge layer's possible cards, narrowed along a
/// played line by the engine's play-level exclusions (the rule engine's facts).
#[derive(Clone, Copy)]
struct Known {
    may: [CardSet; 4],
}

impl Known {
    fn new(ctx: &Ctx) -> Known {
        Known {
            may: std::array::from_fn(|s| ctx.k.may_hold(s as u8)),
        }
    }

    /// The seat to move plays `card` into `state`'s trick (called before `state.play`).
    fn play(&mut self, state: &FastState, card: u8) {
        let seat = state.to_move() as usize;
        let first = state.tricks_done() == 0;
        self.may[seat] -= engine::play_levels(state.trick(), state.trump(), first).excluded_by(card);
        self.may.iter_mut().for_each(|m| *m -= CardSet::single(card));
    }

    fn may_hold_suit(&self, seat: u8, s: Suit) -> bool {
        !(self.may[(seat % 4) as usize] & CardSet::suit(s)).is_empty()
    }
}

/// The activation order of a seat's own pairs: the shorter suit first, at equal length the
/// smaller pair, so the biggest and longest stays trump.
fn order(hand: CardSet, s: Suit) -> (u32, i32) {
    ((hand & CardSet::suit(s)).len(), engine::trump_points(s, false))
}

/// The leader's activation at a trick start (see the module doc, step 2).
fn activation(state: &FastState, known: &Known) -> Option<Move> {
    if state.phase() != PlayPhase::Start {
        return None;
    }
    let leader = state.leader();
    let mate = partner(leader) as usize;
    let mine = state.hand(leader);
    let open = |s: Suit| state.called() >> s as u8 & 1 == 0;
    let own = state.announcements();
    if let Some(s) = SUITS
        .into_iter()
        .filter(|&s| own >> s as u8 & 1 == 1)
        .min_by_key(|&s| order(mine, s))
    {
        return Some(Move::Announce(s));
    }
    let legal = |q: QuestionType| state.moves().any(|m| m == Move::Question(q));
    let may_pair = SUITS
        .into_iter()
        .any(|s| open(s) && (known.may[mate] & CardSet::halves(s)) == CardSet::halves(s));
    if may_pair && legal(QuestionType::Yours) {
        return Some(Move::Question(QuestionType::Yours));
    }
    // A half question spends the remaining rights: the most valuable half the partner may complete.
    SUITS
        .into_iter()
        .rev()
        .find(|&s| {
            let h = CardSet::halves(s);
            open(s) && !(mine & h).is_empty() && (mine & h) != h && !(known.may[mate] & (h - mine)).is_empty()
        })
        .map(QuestionType::YourHalf)
        .filter(|&q| legal(q))
        .map(Move::Question)
}

/// The truthful level-0 answer of the asked seat (its own cards decide, in `order`).
fn answer0(state: &FastState) -> Move {
    let held = state.hand(state.to_move());
    let open = |s: Suit| state.called() >> s as u8 & 1 == 0;
    Move::Answer(match state.phase() {
        PlayPhase::Answer(QuestionType::YourHalf(s)) if !(held & CardSet::halves(s)).is_empty() => {
            AnswerType::YesHalf(s)
        }
        PlayPhase::Answer(QuestionType::YourHalf(s)) => AnswerType::NoHalf(s),
        _ => held
            .pair_suits()
            .filter(|&s| open(s))
            .min_by_key(|&s| order(held, s))
            .map_or(AnswerType::NoPair, AnswerType::YesPair),
    })
}

/// Applies `m` and, after a question, the asked seat's answer.
fn apply(state: &mut FastState, m: Move) {
    state.apply(m);
    if let PlayPhase::Answer(_) = state.phase() {
        let a = answer0(state);
        state.apply(a);
    }
}

/// `team`'s points minus the other team's.
fn margin(state: &FastState, team: u8) -> i32 {
    state.points(team) - state.points(1 - team)
}

/// The level-0 lead (see the module doc, step 3).
fn lead0(state: &FastState, known: &Known) -> u8 {
    let seat = state.leader();
    let all = state.in_hands();
    let trump = state.trump();
    let legal = state.legal_cards();
    // As far as the knowledge goes: proven void in the side suit and possibly holding trump.
    let ruffs =
        |p: u8, s: Suit| trump.is_some_and(|t| t != s && !known.may_hold_suit(p, s) && known.may_hold_suit(p, t));
    if let Some(c) = legal
        .iter()
        .filter(|&c| top_of(all, suit_of(c)) == Some(c) && ![1u8, 3].iter().any(|&o| ruffs(seat + o, suit_of(c))))
        .max_by_key(|&c| (CARD_POINTS[c as usize], Some(suit_of(c)) != trump))
    {
        return c;
    }
    let keep = live_halves(state.hand(seat), state.called());
    if let Some(c) = (legal - keep)
        .iter()
        .filter(|&c| ruffs(partner(seat), suit_of(c)) && !ruffs(seat + 3, suit_of(c)))
        .min_by_key(|&c| (CARD_POINTS[c as usize], beats_key(c)))
    {
        return c;
    }
    respond(state)
}

/// Plays the game to its end at level 0 (see the module doc, steps 2 and 3).
fn playout(state: &mut FastState, known: &mut Known) {
    while !state.is_over() {
        let c = if state.trick().is_empty() {
            if let Some(m) = activation(state, known) {
                apply(state, m);
            }
            lead0(state, known)
        } else {
            respond(state)
        };
        known.play(state, c);
        state.play(c);
    }
}

/// Option values over the believed worlds: per option its value in each world (worlds where
/// an option can't be evaluated are dropped for all), and the worlds' probabilities.
pub(crate) struct WorldValues<K> {
    pub options: Vec<K>,
    values: Vec<Vec<f64>>,
    p: Vec<f64>,
}

impl<K: Copy + PartialEq> WorldValues<K> {
    /// Evaluates every option in every world; `None` without worlds or options.
    fn over(worlds: &[World], options: Vec<K>, mut value: impl FnMut(&World, &K) -> Option<i32>) -> Option<Self> {
        let total: f64 = worlds.iter().map(|w| w.weight).sum();
        if options.is_empty() || total <= 0.0 {
            return None;
        }
        let mut values = vec![Vec::new(); options.len()];
        let mut p = Vec::new();
        for w in worlds {
            let row: Option<Vec<i32>> = options.iter().map(|o| value(w, o)).collect();
            if let Some(row) = row {
                values.iter_mut().zip(row).for_each(|(v, x)| v.push(x as f64));
                p.push(w.weight / total);
            }
        }
        let mass: f64 = p.iter().sum();
        if mass <= 0.0 {
            return None;
        }
        p.iter_mut().for_each(|x| *x /= mass);
        Some(WorldValues { options, values, p })
    }

    pub fn index(&self, k: K) -> Option<usize> {
        self.options.iter().position(|&o| o == k)
    }

    pub fn mean(&self, i: usize) -> f64 {
        self.values[i].iter().zip(&self.p).map(|(v, p)| v * p).sum()
    }

    /// The option with the highest expected value (the first on a tie).
    pub fn best(&self) -> usize {
        (0..self.options.len()).fold(0, |b, i| if self.mean(i) > self.mean(b) { i } else { b })
    }

    /// Option `a` beats option `b` beyond the sampling noise of the worlds: the paired
    /// difference's expectation exceeds twice its standard error (effective sample size
    /// 1 / Σp²). Below that, the plan layer has no evidence against the rule it would overrule.
    pub fn clearly_better(&self, a: usize, b: usize) -> bool {
        let d: Vec<f64> = self.values[a].iter().zip(&self.values[b]).map(|(x, y)| x - y).collect();
        let mean: f64 = d.iter().zip(&self.p).map(|(x, p)| x * p).sum();
        let var: f64 = d.iter().zip(&self.p).map(|(x, p)| p * (x - mean).powi(2)).sum();
        let n = 1.0 / self.p.iter().map(|p| p * p).sum::<f64>();
        mean > 2.0 * (var / n).sqrt()
    }
}

/// The value (our points minus theirs from now on) of each of my non-card moves now (an
/// announcement, a question, an answer), and of `None`: leading a card instead (at a trick
/// start). Per believed world; the partner answers my question from its own cards; after an
/// answer of mine, the asker leads.
fn route_values(ctx: &Ctx, phase: PlayPhase, moves: &[Move]) -> Option<WorldValues<Option<Move>>> {
    let _timing = beliefs::profile::scope("outlook.route");
    let public = ctx.k.public_state(ctx.me, phase);
    let us = ctx.me % 2;
    let known = Known::new(ctx);
    let options: Vec<Option<Move>> = moves
        .iter()
        .copied()
        .map(Some)
        .chain((phase == PlayPhase::Start).then_some(None))
        .collect();
    ctx.with_worlds(|worlds: &[World]| {
        WorldValues::over(worlds, options, |w, &option| {
            let state = FastState::from_public(public, w.hands).ok()?;
            let (mut s, mut k) = (state, known);
            match option {
                Some(m) if s.moves().any(|x| x == m) => apply(&mut s, m),
                Some(_) => return None,
                None => {
                    let c = lead0(&s, &k);
                    k.play(&s, c);
                    s.play(c);
                }
            }
            playout(&mut s, &mut k);
            Some(margin(&s, us) - margin(&state, us))
        })
    })
}

/// HO-2: my activation or answer by route. `planned` is the rule's choice (`None`: a card);
/// it stands unless another option is clearly better (`WorldValues::clearly_better`).
/// Returns `None` to keep the plan, `Some(None)` for a card, `Some(Some(d))` for the route's
/// activation or answer.
pub(crate) fn route_choice(
    ctx: &Ctx,
    legal: &[GameAction],
    phase: PlayPhase,
    moves: &[Move],
    planned: Option<Move>,
) -> Option<Option<Decision>> {
    let values = route_values(ctx, phase, moves)?;
    let best = values.best();
    let plan = values.index(planned)?;
    if best == plan || !values.clearly_better(best, plan) {
        return None;
    }
    let Some(m) = values.options[best] else {
        return Some(None);
    };
    let action = legal.iter().find(|a| Move::from_action(&a.action_type) == Some(m))?;
    Some(Some(decision(
        action,
        RuleId::HO2,
        vec![
            fact("expected_difference", values.mean(best)),
            fact("over_plan", values.mean(best) - values.mean(plan)),
        ],
    )))
}

/// Per card: P(our team takes the trick) and the expected points in it.
pub(crate) type TrickOutcomes = [Option<(f32, f32)>; 36];

/// The trick outcome of playing each of my cards now, over the believed worlds: P(our team
/// takes the trick) and the expected points in the trick (cards and a last-trick bonus).
/// The engine's legal replies are exact; genuine choices use the level-0 response.
pub(crate) fn trick_outcomes(ctx: &Ctx) -> Option<TrickOutcomes> {
    let _timing = beliefs::profile::scope("outlook.trick");
    let public = ctx.k.public_state(ctx.me, PlayPhase::Cards);
    let us = ctx.me % 2;
    let mine = ctx.k.certain(ctx.me);
    ctx.with_worlds(|worlds: &[World]| {
        let total: f64 = worlds.iter().map(|w| w.weight).sum();
        if worlds.is_empty() || total <= 0.0 {
            return None;
        }
        let mut out = [None; 36];
        for c in mine.iter() {
            let (mut p_ours, mut pts, mut mass) = (0.0f64, 0.0f64, 0.0f64);
            for w in worlds {
                let Ok(mut state) = FastState::from_public(public, w.hands) else {
                    continue;
                };
                if !state.legal_cards().contains(c) {
                    continue;
                }
                let before = state.card_points(0) + state.card_points(1);
                let tricks = state.tricks_done();
                state.play(c);
                while state.tricks_done() == tricks && !state.is_over() {
                    let r = respond(&state);
                    state.play(r);
                }
                let p = w.weight / total;
                mass += p;
                pts += p * (state.card_points(0) + state.card_points(1) - before) as f64;
                if state.leader() % 2 == us {
                    p_ours += p;
                }
            }
            if mass > 0.0 {
                out[c as usize] = Some(((p_ours / mass) as f32, (pts / mass) as f32));
            }
        }
        Some(out)
    })
}

/// The value (our points minus theirs from now on) of leading each card in `candidates`, per
/// believed world. `None` when there are no worlds.
pub(crate) fn lead_values(ctx: &Ctx, candidates: CardSet) -> Option<WorldValues<u8>> {
    let _timing = beliefs::profile::scope("outlook");
    let public = ctx.k.public_state(ctx.me, PlayPhase::Cards);
    let us = ctx.me % 2;
    let known = Known::new(ctx);
    let options: Vec<u8> = candidates.iter().collect();
    ctx.with_worlds(|worlds: &[World]| {
        WorldValues::over(worlds, options, |w, &c| {
            let mut state = FastState::from_public(public, w.hands).ok()?;
            if !state.legal_cards().contains(c) {
                return None;
            }
            let before = margin(&state, us);
            let mut k = known;
            k.play(&state, c);
            state.play(c);
            playout(&mut state, &mut k);
            Some(margin(&state, us) - before)
        })
    })
}

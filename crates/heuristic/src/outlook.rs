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
//! a seat really holds and so what it can play and answer. `Known` starts from the public
//! facts (what every seat knows; each actor also knows its own cards from the world), so no
//! actor uses my private knowledge.
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
/// Taking the trick from the opponents, it prefers a winner the opponents still to play can't
/// beat as far as it knows (Astra on game 1346: the partner ruffs with the ace, not with a nine
/// the last opponent may overruff); else the cheapest winner.
fn respond(state: &FastState, known: &Known) -> u8 {
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
        let own = state.hand(seat);
        let later = (trick.len() as u8 + 1..4)
            .map(|i| (leader + i) % 4)
            .filter(|&o| o % 2 != seat % 2)
            .fold(CardSet::EMPTY, |set, o| set | known.may(own, o));
        // The cards that beat `c`: higher in its suit, and any trump over a side card.
        let secure = |c: u8| {
            let over = match state.trump() {
                Some(t) if t != suit_of(c) => crate::hand::higher(c) | CardSet::suit(t),
                _ => crate::hand::higher(c),
            };
            (later & over).is_empty()
        };
        return winners
            .iter()
            .min_by_key(|&c| (keep.contains(c), !secure(c), beats_key(c), CARD_POINTS[c as usize]))
            .expect("nonempty");
    }
    cheapest(legal)
}

/// What every seat may still hold as far as the table knows: the public facts' possible cards,
/// narrowed along a played line by the engine's play-level exclusions (the rule engine's facts).
/// `read`: per seat, the suits its partner reads it blank in from its pass forth (`blank_signs`),
/// until it follows one of them. `held`: per seat, the cards its partner knows it holds from the
/// exchange (the team's private transfer facts; Astra's review 2026-10-13), until played.
#[derive(Clone, Copy)]
struct Known {
    may: [CardSet; 4],
    read: [u8; 4],
    held: [CardSet; 4],
}

/// The suits of `cards` as bits.
fn suits_of(cards: CardSet) -> u8 {
    cards.iter().fold(0, |bits, c| bits | 1 << suit_of(c) as u8)
}

/// The suits the giver's partner reads it blank in: those of a Lusche its pass forth contains
/// (the owner 2026-10-13: "It should just be high probabilty of blank when already one lusche in
/// the passed cards"; a Lusche from a kept suit breaks the convention, the store's
/// `lusche_from_kept_suit`), except where the exact transfer facts or its play say otherwise: a
/// suit returned to it, a suit it played (Astra's review 2026-10-13). A fallible reading: the
/// actors act on it until the giver follows the suit.
fn blank_signs(forth: CardSet, back: CardSet, played: CardSet) -> u8 {
    suits_of(forth & beliefs::conventions::LUSCHE) & !suits_of(back) & !suits_of(played)
}

impl Known {
    /// The table's knowledge, and the pass forth my team exchanged if I saw it.
    fn new(ctx: &Ctx) -> Known {
        let mut known = Known {
            may: std::array::from_fn(|s| ctx.mind.public().may_hold(s as u8)),
            read: [0; 4],
            held: [CardSet::EMPTY; 4],
        };
        let seen = |p: knowledge::Passed| match p {
            knowledge::Passed::Seen(cards) => Some(cards),
            _ => None,
        };
        if let Some(first) = ctx.k.first_passer().filter(|f| f % 2 == ctx.me % 2) {
            if let Some(forth) = seen(ctx.k.given(first)) {
                let back = seen(ctx.k.received(first)).unwrap_or(CardSet::EMPTY);
                let played = [ctx.k.played_by(first), ctx.k.played_by(partner(first))];
                known.exchanged(first, forth, back, played);
            }
        }
        known
    }

    /// My team's exchange: `giver` passed `forth`, its partner returned `back`; `played` are the
    /// cards each of them (giver, receiver) has played since. Each partner knows what the other
    /// holds of the transfers; the giver's blank signs are read against them.
    fn exchanged(&mut self, giver: u8, forth: CardSet, back: CardSet, played: [CardSet; 2]) {
        let receiver = partner(giver) as usize;
        self.read[giver as usize] = blank_signs(forth, back, played[0]);
        self.held[giver as usize] = back - played[0];
        self.held[receiver] = forth - back - played[1];
    }

    /// The seat to move plays `card` into `state`'s trick (called before `state.play`).
    fn play(&mut self, state: &FastState, card: u8) {
        let seat = state.to_move() as usize;
        let first = state.tricks_done() == 0;
        self.may[seat] -= engine::play_levels(state.trick(), state.trump(), first).excluded_by(card);
        self.may.iter_mut().for_each(|m| *m -= CardSet::single(card));
        self.read[seat] &= !(1 << suit_of(card) as u8);
        self.held[seat] -= CardSet::single(card);
    }

    /// The view the actors use in one world: the table's knowledge, or with
    /// `outlook_world_known` the world's hands themselves (the classic Monte-Carlo actors that
    /// play each sampled deal open; strategy fusion is then the stated approximation).
    fn in_world(self, ctx: &Ctx, hands: &[CardSet; 4]) -> Known {
        if ctx.params.outlook_world_known {
            Known { may: *hands, ..self }
        } else {
            self
        }
    }

    /// What `seat` may hold as an actor holding `own` sees it: the table's knowledge without
    /// the actor's own cards (Astra: an actor reasons with its own hand, not only the public).
    fn may(&self, own: CardSet, seat: u8) -> CardSet {
        self.may[(seat % 4) as usize] - own
    }

    fn may_hold_suit(&self, own: CardSet, seat: u8, s: Suit) -> bool {
        !(self.may(own, seat) & CardSet::suit(s)).is_empty()
    }
}

/// The activation order of a seat's own pairs: the shorter suit first, at equal length the
/// smaller pair, so the biggest and longest stays trump.
fn order(hand: CardSet, s: Suit) -> (u32, i32) {
    ((hand & CardSet::suit(s)).len(), engine::trump_points(s, false))
}

/// One step at the selecting actor's next activation opportunity. Its partner
/// does not receive this private plan; each seat acts from its own information.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Own,
    Ask,
    Wait,
}

/// How many of the actor's next trick starts a plan fixes; default activation follows.
const PLAN_STEPS: usize = 2;

/// A private activation plan for one actor, fixed across the believed worlds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Plan {
    actor: u8,
    steps: [Step; PLAN_STEPS],
    next: usize,
}

impl Plan {
    /// Every plan for `actor`.
    fn all(actor: u8) -> impl Iterator<Item = Plan> {
        let steps = [Step::Own, Step::Ask, Step::Wait];
        steps.into_iter().flat_map(move |a| {
            steps.into_iter().map(move |b| Plan {
                actor,
                steps: [a, b],
                next: 0,
            })
        })
    }

    /// The step for the trick start in `state`, if the plan's actor leads, steps are left and the
    /// leader has an `eligible` activation (Astra: a step belongs to an actual activation
    /// opportunity; the empty trick after an announcement or answer is not one).
    fn take(plan: &mut Option<Plan>, state: &FastState, eligible: bool) -> Option<Step> {
        if !eligible {
            return None;
        }
        let p = plan
            .as_mut()
            .filter(|p| p.actor == state.leader() && p.next < PLAN_STEPS)?;
        p.next += 1;
        Some(p.steps[p.next - 1])
    }

    /// The plan as digits for the decision facts (own 1, ask 2, wait 0; first step first).
    fn code(&self) -> u32 {
        self.steps.iter().fold(0, |n, s| {
            10 * n
                + match s {
                    Step::Own => 1,
                    Step::Ask => 2,
                    Step::Wait => 0,
                }
        })
    }
}

/// The leader's own announcement: its own pair in `order`.
fn announce(state: &FastState) -> Option<Move> {
    let own = state.announcements();
    SUITS
        .into_iter()
        .filter(|&s| own >> s as u8 & 1 == 1)
        .min_by_key(|&s| order(state.hand(state.leader()), s))
        .map(Move::Announce)
}

/// The leader's activation at a trick start: the plan's `step`, or the default (see the module
/// doc, step 2).
fn activation(state: &FastState, known: &Known, step: Option<Step>) -> Option<Move> {
    if state.phase() != PlayPhase::Start {
        return None;
    }
    match step {
        Some(Step::Wait) => None,
        Some(Step::Own) => announce(state),
        Some(Step::Ask) => ask(state, known),
        None => announce(state).or_else(|| {
            let question = ask(state, known)?;
            if question != Move::Question(QuestionType::Yours) {
                return Some(question);
            }
            let actor = state.leader();
            let own = state.hand(actor);
            let others = (0..4)
                .filter(|&seat| seat != actor)
                .fold(CardSet::EMPTY, |cards, seat| cards | known.may(own, seat));
            if crate::leads::cash_before_question(own, state.trump(), others, lead0(state, known)) {
                None
            } else {
                Some(question)
            }
        }),
    }
}

/// The leader's question: "Yours" while the partner may still hold a pair, else the most
/// valuable half the partner may complete (a half question spends the remaining rights).
fn ask(state: &FastState, known: &Known) -> Option<Move> {
    let leader = state.leader();
    let mate = partner(leader) as usize;
    let mine = state.hand(leader);
    let open = |s: Suit| state.called() >> s as u8 & 1 == 0;
    let legal = |q: QuestionType| state.moves().any(|m| m == Move::Question(q));
    let may_pair = SUITS
        .into_iter()
        .any(|s| open(s) && (known.may(mine, mate as u8) & CardSet::halves(s)) == CardSet::halves(s));
    if may_pair && legal(QuestionType::Yours) {
        return Some(Move::Question(QuestionType::Yours));
    }
    SUITS
        .into_iter()
        .rev()
        .find(|&s| {
            let h = CardSet::halves(s);
            open(s)
                && !(mine & h).is_empty()
                && (mine & h) != h
                && !(known.may(mine, mate as u8) & (h - mine)).is_empty()
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
fn apply(state: &mut FastState, known: &mut Known, m: Move) {
    let asker = state.leader();
    state.apply(m);
    if let PlayPhase::Answer(q) = state.phase() {
        let a = answer0(state);
        state.apply(a);
        // A "no half" proves the answerer lacks the half the asker doesn't hold. A "no pair" is a
        // joint fact (no complete uncalled pair) that the per-seat possible cards can't hold.
        if let (QuestionType::YourHalf(s), Move::Answer(AnswerType::NoHalf(_))) = (q, a) {
            let missing = CardSet::halves(s) - state.hand(asker);
            known.may[partner(asker) as usize] -= missing;
        }
    }
}

/// The contract a playout is judged by: the playing team and the game value (`None` before a
/// contract exists).
type Contract = Option<(u8, i32)>;

fn contract(ctx: &Ctx) -> Contract {
    ctx.k.declarer().map(|d| (d % 2, ctx.k.value()))
}

/// The objective (the owner's choice; Astra's review §2): `us`'s engine score difference at the
/// end of the played-out game under `contract` (made or failed, the defenders' rounded points,
/// schwarz), from `engine::game_scores`, the one owner of the scoring; without a contract, the
/// engine's all-pass scoring (each team's rounded points).
fn score(state: &FastState, contract: Contract, us: u8) -> i32 {
    let outcome = engine::GameOutcome {
        playing_team: contract.map(|c| c.0),
        game_value: contract.map_or(0, |c| c.1),
        team_points: [state.points(0), state.points(1)],
        team_tricks: [state.tricks_won(0), state.tricks_won(1)],
    };
    let s = engine::game_scores(&outcome, engine::SCHWARZ_FACTOR_FIFTHS);
    s[us as usize] - s[1 - us as usize]
}

/// The level-0 lead (see the module doc, step 3).
fn lead0(state: &FastState, known: &Known) -> u8 {
    let seat = state.leader();
    // The union is public: the deck minus publicly played cards. No ownership
    // from another seat's sampled hand is used to identify a standing rank.
    let all = state.in_hands();
    let trump = state.trump();
    let legal = state.legal_cards();
    let own = state.hand(seat);
    // As far as the knowledge goes: proven void in the side suit (or, for my partner, read blank
    // from its pass forth) and possibly holding trump.
    let void = |p: u8, s: Suit| {
        !known.may_hold_suit(own, p, s)
            || (p % 4 == partner(seat) && known.read[partner(seat) as usize] >> s as u8 & 1 == 1)
    };
    let ruffs = |p: u8, s: Suit| trump.is_some_and(|t| t != s && void(p, s) && known.may_hold_suit(own, p, t));
    // The owner's order: standing side-suit cards before standing trumps.
    if let Some(c) = legal
        .iter()
        .filter(|&c| top_of(all, suit_of(c)) == Some(c) && ![1u8, 3].iter().any(|&o| ruffs(seat + o, suit_of(c))))
        .max_by_key(|&c| (Some(suit_of(c)) != trump, CARD_POINTS[c as usize]))
    {
        return c;
    }
    let keep = live_halves(state.hand(seat), state.called());
    // A low lead toward my partner's ace that I know of from the exchange, where no opponent may
    // ruff it (Astra's review 2026-10-13: a route to the giver's kept entry the receiver can read).
    let mate = partner(seat);
    let known_ace = |s: Suit| !(known.held[mate as usize] & CardSet::ACES & CardSet::suit(s)).is_empty();
    if let Some(c) = (legal - keep)
        .iter()
        .filter(|&c| {
            let s = suit_of(c);
            Some(s) != trump && known_ace(s) && ![1u8, 3].iter().any(|&o| ruffs(seat + o, s))
        })
        .min_by_key(|&c| (CARD_POINTS[c as usize], beats_key(c)))
    {
        return c;
    }
    if let Some(c) = (legal - keep)
        .iter()
        .filter(|&c| ruffs(partner(seat), suit_of(c)) && !ruffs(seat + 3, suit_of(c)))
        .min_by_key(|&c| (CARD_POINTS[c as usize], beats_key(c)))
    {
        return c;
    }
    respond(state, known)
}

/// Plays the game to its end at level 0 (see the module doc, steps 2 and 3).
fn playout(state: &mut FastState, known: &mut Known, mut plan: Option<Plan>) {
    while !state.is_over() {
        let c = if state.trick().is_empty() {
            let eligible =
                state.phase() == PlayPhase::Start && (announce(state).is_some() || ask(state, known).is_some());
            let step = Plan::take(&mut plan, state, eligible);
            if let Some(m) = activation(state, known, step) {
                apply(state, known, m);
            }
            lead0(state, known)
        } else {
            respond(state, known)
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

/// At least this many worlds are evaluated before a deadline may stop an evaluation (so the
/// guard's standard error has a sample to stand on).
const MIN_WORLDS: usize = 8;

/// The decision's deadline has passed: an evaluation not yet started is skipped (its rule
/// decides), one under way stops after `MIN_WORLDS`.
fn past(ctx: &Ctx) -> bool {
    ctx.deadline.is_some_and(|d| std::time::Instant::now() >= d)
}

impl<K: Copy + PartialEq> WorldValues<K> {
    /// Evaluates every option in every world, until the decision's deadline (after
    /// `MIN_WORLDS`); `None` without worlds or options. The pool is a sample, so a prefix is one.
    fn over(
        ctx: &Ctx,
        worlds: &[World],
        options: Vec<K>,
        mut value: impl FnMut(&World, &K) -> Option<i32>,
    ) -> Option<Self> {
        let total: f64 = worlds.iter().map(|w| w.weight).sum();
        if options.is_empty() || total <= 0.0 || past(ctx) {
            return None;
        }
        let mut values = vec![Vec::new(); options.len()];
        let mut p = Vec::new();
        for (i, w) in worlds.iter().enumerate() {
            if i >= MIN_WORLDS && past(ctx) {
                break;
            }
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

    /// Per distinct `key` of the options, the option with the best expected value: the rows
    /// of the chosen options keyed by `key`, and the chosen options.
    fn best_per<J: Copy + PartialEq>(self, key: impl Fn(&K) -> J) -> (WorldValues<J>, Vec<K>) {
        let mut chosen: Vec<(J, usize)> = Vec::new();
        for i in 0..self.options.len() {
            let j = key(&self.options[i]);
            match chosen.iter_mut().find(|(k, _)| *k == j) {
                Some(c) if self.mean(i) > self.mean(c.1) => c.1 = i,
                Some(_) => {}
                None => chosen.push((j, i)),
            }
        }
        let picked: Vec<K> = chosen.iter().map(|&(_, i)| self.options[i]).collect();
        let values = WorldValues {
            options: chosen.iter().map(|&(j, _)| j).collect(),
            values: chosen.iter().map(|&(_, i)| self.values[i].clone()).collect(),
            p: self.p,
        };
        (values, picked)
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

/// The value of each of my moves now (an announcement, a question, an answer or a concrete
/// card), each with my best activation plan for my next trick starts (`Plan`), the
/// same plan in every world. Per believed world; the partner answers my question from its own
/// cards; after an answer of mine, the asker leads. Also returns the plan chosen per option.
fn route_values(ctx: &Ctx, phase: PlayPhase, moves: &[Move]) -> Option<(WorldValues<Option<Move>>, Vec<Plan>)> {
    let _timing = beliefs::profile::scope("outlook.route");
    let public = ctx.k.public_state(ctx.me, phase);
    let us = ctx.me % 2;
    let known = Known::new(ctx);
    let game = contract(ctx);
    let options: Vec<(Option<Move>, Plan)> = moves
        .iter()
        .copied()
        .map(Some)
        .flat_map(|m| Plan::all(ctx.me).map(move |p| (m, p)))
        .collect();
    let values = ctx.with_worlds(|worlds: &[World]| {
        WorldValues::over(ctx, worlds, options, |w, &(option, plan)| {
            let state = FastState::from_public(public, w.hands).ok()?;
            let (mut s, mut k) = (state, known.in_world(ctx, &w.hands));
            match option {
                Some(Move::Card(c)) if s.legal_cards().contains(c) => {
                    k.play(&s, c);
                    s.play(c);
                }
                Some(m) if s.moves().any(|x| x == m) => apply(&mut s, &mut k, m),
                Some(_) => return None,
                None => {
                    let c = lead0(&s, &k);
                    k.play(&s, c);
                    s.play(c);
                }
            }
            playout(&mut s, &mut k, Some(plan));
            Some(score(&s, game, us))
        })
    })?;
    let (values, picked) = values.best_per(|&(m, _)| m);
    Some((values, picked.into_iter().map(|(_, p)| p).collect()))
}

/// HO-2: my move by route among `moves` (activations, answers, concrete cards). `planned` is the
/// rule's move; it stands unless another is clearly better (`WorldValues::clearly_better`).
/// Returns `None` to keep the plan, else the route's move as played (the evaluated move).
pub(crate) fn route_choice(
    ctx: &Ctx,
    legal: &[GameAction],
    phase: PlayPhase,
    moves: &[Move],
    planned: Move,
) -> Option<Decision> {
    let (values, plans) = route_values(ctx, phase, moves)?;
    let best = values.best();
    let plan = values.index(Some(planned))?;
    if best == plan || !values.clearly_better(best, plan) {
        return None;
    }
    let m = values.options[best]?;
    let action = legal.iter().find(|a| Move::from_action(&a.action_type) == Some(m))?;
    Some(decision(
        action,
        RuleId::HO2,
        vec![
            fact("expected_score_difference", values.mean(best)),
            fact("over_plan", values.mean(best) - values.mean(plan)),
            fact("actor_plan", plans[best].code()),
            fact("plan_actor", ctx.me),
        ],
    ))
}

/// The game's end per believed world from the current position (before a card of the trick is
/// played), under my best activation plan: the joint distribution of both teams'
/// points and tricks (Astra: a route-derived distribution, not a sum of bonuses; it keeps a
/// failed entry, an unactivated pair and a lost last trick together, and the tricks for schwarz).
pub(crate) struct Outcomes {
    pub p: Vec<f64>,
    pub points: Vec<[i32; 2]>,
    pub tricks: Vec<[u8; 2]>,
}

impl Outcomes {
    /// Our expected engine score difference if our team plays the contract at `value`.
    pub fn score_difference(&self, us: u8, value: i32) -> f64 {
        let us = (us % 2) as usize;
        self.p
            .iter()
            .zip(self.points.iter().zip(&self.tricks))
            .map(|(p, (points, tricks))| {
                let outcome = engine::GameOutcome {
                    playing_team: Some(us as u8),
                    game_value: value,
                    team_points: *points,
                    team_tricks: *tricks,
                };
                let s = engine::game_scores(&outcome, engine::SCHWARZ_FACTOR_FIFTHS);
                p * (s[us] - s[1 - us]) as f64
            })
            .sum()
    }

    /// Our team's expected final points.
    pub fn mean_points(&self, us: u8) -> f64 {
        self.p
            .iter()
            .zip(&self.points)
            .map(|(p, pts)| p * pts[(us % 2) as usize] as f64)
            .sum()
    }
}

/// `Outcomes` before my lead: every actor-local plan is played out in every world;
/// the same plan with the best expected engine score difference is selected.
pub(crate) fn final_outcomes(ctx: &Ctx) -> Option<Outcomes> {
    let _timing = beliefs::profile::scope("outlook.outcomes");
    let public = ctx.k.public_state(ctx.me, PlayPhase::Cards);
    let us = ctx.me % 2;
    let known = Known::new(ctx);
    let game = contract(ctx);
    let ends = |w: &World, plan: Plan| {
        let mut s = FastState::from_public(public, w.hands).ok()?;
        let mut k = known.in_world(ctx, &w.hands);
        playout(&mut s, &mut k, Some(plan));
        Some(s)
    };
    let plans: Vec<Plan> = Plan::all(ctx.me).collect();
    ctx.with_worlds(|worlds: &[World]| {
        // Per world, every plan's end; worlds past the deadline (after `MIN_WORLDS`) are left out.
        let mut rows: Vec<(f64, Vec<FastState>)> = Vec::new();
        if past(ctx) {
            return None;
        }
        for (i, w) in worlds.iter().enumerate() {
            if i >= MIN_WORLDS && past(ctx) {
                break;
            }
            if let Some(ends) = plans.iter().map(|&plan| ends(w, plan)).collect::<Option<Vec<_>>>() {
                rows.push((w.weight, ends));
            }
        }
        let mass: f64 = rows.iter().map(|r| r.0).sum();
        if mass <= 0.0 {
            return None;
        }
        let mean = |j: usize| rows.iter().map(|(p, e)| p * score(&e[j], game, us) as f64).sum::<f64>();
        let best = (0..plans.len()).fold(0, |b, j| if mean(j) > mean(b) { j } else { b });
        Some(Outcomes {
            p: rows.iter().map(|r| r.0 / mass).collect(),
            points: rows
                .iter()
                .map(|r| [r.1[best].points(0), r.1[best].points(1)])
                .collect(),
            tricks: rows
                .iter()
                .map(|r| [r.1[best].tricks_won(0), r.1[best].tricks_won(1)])
                .collect(),
        })
    })
}

/// `take_vs_defend` (Astra and the owner, 2026-10-12): our expected engine score difference if
/// we take the contract at `ours` (I declare), against defending the opponents' `theirs`
/// (`their_declarer` plays), per believed world: each side's level-0 exchange, reading the
/// auction by the convention (`passing::level0_exchange`), then the playout to the end with
/// the default activations, and
/// the engine's scoring (failure, the defenders' points, schwarz). Options: [take, defend].
/// Not modelled: the auction continuing after the bid, and what the opponents' pass tells.
pub(crate) fn take_or_defend(ctx: &Ctx, ours: i32, their_declarer: u8, theirs: i32) -> Option<WorldValues<bool>> {
    let _timing = beliefs::profile::scope("outlook.take");
    let us = ctx.me % 2;
    let conf = ctx.params.partner_half_conf;
    // The exchange moves cards between partners: either may now hold the other's possible cards.
    let known = |declarer: u8| {
        let mut k = Known::new(ctx);
        let (d, p) = (declarer as usize, partner(declarer) as usize);
        let team = k.may[d] | k.may[p];
        k.may[d] = team;
        k.may[p] = team;
        k
    };
    let (mine, theirs_known) = (known(ctx.me), known(their_declarer));
    // Each seat as its partner reads it from the auction, and the pairs it told.
    let likelihood: [_; 4] = std::array::from_fn(|s| ctx.mind.class_likelihood(s as u8, partner(s as u8)));
    let told: [[bool; 4]; 4] = {
        let record = ctx.mind.record();
        std::array::from_fn(|s| crate::passing::known_pairs(&record.seats[s].bids))
    };
    ctx.with_worlds(|worlds: &[World]| {
        WorldValues::over(ctx, worlds, vec![true, false], |w, &take| {
            let (declarer, value, k) = if take {
                (ctx.me, ours, mine)
            } else {
                (their_declarer, theirs, theirs_known)
            };
            let hands = crate::passing::level0_exchange(w.hands, declarer, conf, &likelihood, told);
            let mut s = FastState::new(hands, declarer);
            let mut k = k.in_world(ctx, &hands);
            playout(&mut s, &mut k, None);
            let outcome = engine::GameOutcome {
                playing_team: Some(declarer % 2),
                game_value: value,
                team_points: [s.points(0), s.points(1)],
                team_tricks: [s.tricks_won(0), s.tricks_won(1)],
            };
            let sc = engine::game_scores(&outcome, engine::SCHWARZ_FACTOR_FIFTHS);
            Some(sc[us as usize] - sc[1 - us as usize])
        })
    })
}

/// The pass forth by route (Astra's review 2026-10-13, S12/S13; the owner's game 3): per
/// candidate of mine and believed world, the declarer receives it, passes back at level 0
/// reading it (`passing::level0_back`), and the game is played out from its first lead with our
/// own best activation plan; the declarer reads the candidate's blank signs (`blank_signs`).
/// A pass's legibility is valued by what the receiver does with it, not by counting blanks.
pub(crate) fn pass_values(ctx: &Ctx, candidates: &[CardSet]) -> Option<WorldValues<CardSet>> {
    let _timing = beliefs::profile::scope("outlook.pass");
    let (me, declarer) = (ctx.me, partner(ctx.me));
    let us = me % 2;
    let game = contract(ctx);
    let conf = ctx.params.partner_half_conf;
    let reads_me = ctx.mind.class_likelihood(me, declarer);
    // The exchange moves cards between partners: either may now hold the other's possible cards.
    let mut known = Known::new(ctx);
    let team = known.may[me as usize] | known.may[declarer as usize];
    known.may[me as usize] = team;
    known.may[declarer as usize] = team;
    let options: Vec<(CardSet, Plan)> = candidates
        .iter()
        .flat_map(|&x| Plan::all(me).map(move |p| (x, p)))
        .collect();
    // The plans of one candidate follow each other: its exchange is computed once per world.
    let mut last: Option<(*const World, CardSet, [CardSet; 4])> = None;
    let values = ctx.with_worlds(|worlds: &[World]| {
        WorldValues::over(ctx, worlds, options, |w, &(x, plan)| {
            if !x.is_subset(w.hands[me as usize]) {
                return None;
            }
            let hands = match last {
                Some((p, y, h)) if std::ptr::eq(p, w) && y == x => h,
                _ => {
                    let h = crate::passing::level0_back(w.hands, declarer, x, conf, reads_me.as_deref());
                    last = Some((w, x, h));
                    h
                }
            };
            let mut s = FastState::new(hands, declarer);
            let mut k = known.in_world(ctx, &hands);
            let back = hands[me as usize] - (w.hands[me as usize] - x);
            k.exchanged(me, x, back, [CardSet::EMPTY; 2]);
            playout(&mut s, &mut k, Some(plan));
            Some(score(&s, game, us))
        })
    })?;
    Some(values.best_per(|&(x, _)| x).0)
}

/// Compare concrete returns from the declarer's visible thirteen cards. The
/// sampled partner remainder only resolves its own later play; it never selects
/// the root return or receives the root's private activation plan.
pub(crate) fn return_values(ctx: &Ctx, candidates: &[CardSet]) -> Option<WorldValues<CardSet>> {
    let _timing = beliefs::profile::scope("outlook.return");
    let (me, mate) = (ctx.me, partner(ctx.me));
    let knowledge::Passed::Seen(forth) = ctx.k.received(me) else {
        return None;
    };
    let game = contract(ctx);
    let mut known = Known::new(ctx);
    let team = known.may[me as usize] | known.may[mate as usize];
    known.may[me as usize] = team;
    known.may[mate as usize] = team;
    let options: Vec<_> = candidates
        .iter()
        .flat_map(|&cards| Plan::all(me).map(move |plan| (cards, plan)))
        .collect();
    let values = ctx.with_worlds(|worlds: &[World]| {
        WorldValues::over(ctx, worlds, options, |w, &(cards, plan)| {
            if cards.len() != 4 || !cards.is_subset(w.hands[me as usize]) {
                return None;
            }
            let mut hands = w.hands;
            hands[me as usize] -= cards;
            hands[mate as usize] |= cards;
            if hands.iter().any(|h| h.len() != engine::HAND_SIZE as u32) {
                return None;
            }
            let mut state = FastState::new(hands, me);
            let mut k = known.in_world(ctx, &hands);
            k.exchanged(mate, forth, cards, [CardSet::EMPTY; 2]);
            playout(&mut state, &mut k, Some(plan));
            Some(score(&state, game, me % 2))
        })
    })?;
    Some(values.best_per(|&(cards, _)| cards).0)
}

/// Both teams' final points when `hands` are played out from the first trick with `leader` on
/// lead by the level-0 actors seeing the hands (offline diagnostics, `diagnostics`).
pub(crate) fn open_playout(hands: [CardSet; 4], leader: u8) -> [i32; 2] {
    let mut s = FastState::new(hands, leader);
    let mut k = Known {
        may: hands,
        read: [0; 4],
        held: [CardSet::EMPTY; 4],
    };
    playout(&mut s, &mut k, None);
    [s.points(0), s.points(1)]
}

/// One card's trick over the believed worlds, kept joint: per outcome (our team takes it or not,
/// the points it holds: cards and a last-trick bonus) its probability.
#[derive(Clone, Debug, Default)]
pub(crate) struct TrickOutcome {
    /// P(our team takes the trick).
    pub p_ours: f32,
    /// (ours, points, probability), summing to 1.
    pub outcomes: Vec<(bool, i32, f32)>,
}

/// Per card: its trick outcome.
pub(crate) type TrickOutcomes = [Option<TrickOutcome>; 36];

/// The trick outcome of playing each of my cards now, over the believed worlds. The engine's
/// legal replies are exact; genuine choices use the level-0 response.
pub(crate) fn trick_outcomes(ctx: &Ctx) -> Option<TrickOutcomes> {
    let _timing = beliefs::profile::scope("outlook.trick");
    let public = ctx.k.public_state(ctx.me, PlayPhase::Cards);
    let us = ctx.me % 2;
    let mine = ctx.k.certain(ctx.me);
    let known = Known::new(ctx);
    ctx.with_worlds(|worlds: &[World]| {
        let total: f64 = worlds.iter().map(|w| w.weight).sum();
        if worlds.is_empty() || total <= 0.0 {
            return None;
        }
        let mut out: TrickOutcomes = std::array::from_fn(|_| None);
        for c in mine.iter() {
            let (mut p_ours, mut mass) = (0.0f64, 0.0f64);
            let mut outcomes: Vec<(bool, i32, f64)> = Vec::new();
            for w in worlds {
                let Ok(mut state) = FastState::from_public(public, w.hands) else {
                    continue;
                };
                if !state.legal_cards().contains(c) {
                    continue;
                }
                let before = state.card_points(0) + state.card_points(1);
                let tricks = state.tricks_done();
                let mut k = known.in_world(ctx, &w.hands);
                k.play(&state, c);
                state.play(c);
                while state.tricks_done() == tricks && !state.is_over() {
                    let r = respond(&state, &k);
                    k.play(&state, r);
                    state.play(r);
                }
                let p = w.weight / total;
                mass += p;
                let ours = state.leader() % 2 == us;
                let points = state.card_points(0) + state.card_points(1) - before;
                if ours {
                    p_ours += p;
                }
                match outcomes.iter_mut().find(|o| o.0 == ours && o.1 == points) {
                    Some(o) => o.2 += p,
                    None => outcomes.push((ours, points, p)),
                }
            }
            if mass > 0.0 {
                out[c as usize] = Some(TrickOutcome {
                    p_ours: (p_ours / mass) as f32,
                    outcomes: outcomes
                        .into_iter()
                        .map(|(o, pts, p)| (o, pts, (p / mass) as f32))
                        .collect(),
                });
            }
        }
        Some(out)
    })
}

/// The value of leading each card in `candidates`, per believed world: the same route
/// evaluation as HO-2, with my best activation plan (one continuation policy for
/// both comparisons). `None` when there are no worlds.
pub(crate) fn lead_values(ctx: &Ctx, candidates: CardSet) -> Option<WorldValues<u8>> {
    let moves: Vec<Move> = candidates.iter().map(Move::Card).collect();
    let (values, _) = route_values(ctx, PlayPhase::Cards, &moves)?;
    let options = values
        .options
        .iter()
        .map(|m| match m {
            Some(Move::Card(c)) => *c,
            _ => unreachable!("a lead option is a card"),
        })
        .collect();
    Some(WorldValues {
        options,
        values: values.values,
        p: values.p,
    })
}

/// Per card of `candidates` I lead now: the believed probability that an opponent holding both
/// halves of the led suit's uncalled pair must play one of them in this trick (its legal cards
/// are only halves; a guard it may play instead keeps the pair), the trick played by the
/// level-0 responses (Astra's review 2026-10-13, §3).
pub(crate) fn forced_half(ctx: &Ctx, candidates: CardSet) -> Vec<(u8, f64)> {
    let public = ctx.k.public_state(ctx.me, PlayPhase::Cards);
    let known = Known::new(ctx);
    let opponent = |s: u8| s % 2 != ctx.me % 2;
    ctx.with_worlds(|worlds: &[World]| {
        let total: f64 = worlds.iter().map(|w| w.weight).sum();
        if total <= 0.0 {
            return Vec::new();
        }
        candidates
            .iter()
            .map(|c| {
                let s = suit_of(c);
                let halves = CardSet::halves(s);
                let mut p = 0.0;
                for w in worlds {
                    let Ok(mut state) = FastState::from_public(public, w.hands) else {
                        continue;
                    };
                    if !state.legal_cards().contains(c) || state.called() >> s as u8 & 1 == 1 {
                        continue;
                    }
                    let mut k = known.in_world(ctx, &w.hands);
                    k.play(&state, c);
                    state.play(c);
                    let mut forced = false;
                    while !state.trick().is_empty() && !state.is_over() {
                        let seat = state.to_move();
                        let legal = state.legal_cards();
                        forced |= opponent(seat) && state.hand(seat) & halves == halves && (legal - halves).is_empty();
                        let r = respond(&state, &k);
                        k.play(&state, r);
                        state.play(r);
                    }
                    if forced {
                        p += w.weight / total;
                    }
                }
                (c, p)
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cards(names: &[&str]) -> CardSet {
        CardSet::from_cards(&names.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>())
    }

    #[test]
    fn a_private_plan_does_not_direct_the_partner() {
        let state = FastState::new([cards(&["r-K"]), cards(&["g-A"]), cards(&["r-O"]), cards(&["g-Z"])], 2);
        let mut plan = Some(Plan {
            actor: 0,
            steps: [Step::Ask, Step::Own],
            next: 0,
        });
        assert_eq!(Plan::take(&mut plan, &state, true), None);
        assert_eq!(plan.unwrap().next, 0);
    }

    #[test]
    fn a_running_own_pair_is_announced_before_default_cash_can_consume_a_half() {
        let mut state = FastState::new(
            [
                cards(&["s-A", "s-Z", "s-K", "s-O", "s-9"]),
                cards(&["s-6", "s-U", "g-6", "g-7", "g-8"]),
                cards(&["s-7", "e-6", "e-7", "e-8", "e-9"]),
                cards(&["s-8", "r-6", "r-7", "r-8", "r-9"]),
            ],
            0,
        );
        let mut known = Known {
            may: [state.in_hands(); 4],
            read: [0; 4],
            held: [CardSet::EMPTY; 4],
        };
        for name in ["s-A", "s-6", "s-7", "s-8"] {
            let c = name.parse::<engine::Card>().unwrap().index();
            known.play(&state, c);
            state.play(c);
        }
        assert_eq!(activation(&state, &known, None), Some(Move::Announce(Suit::Bells)));
        // Also protect the later default opportunity after an explicit cash of
        // the non-half Ten. The next standing cash would break the own pair.
        for name in ["s-Z", "s-U", "e-6", "r-6"] {
            let c = name.parse::<engine::Card>().unwrap().index();
            known.play(&state, c);
            state.play(c);
        }
        assert!(CardSet::halves(Suit::Bells).contains(lead0(&state, &known)));
        let m = activation(&state, &known, None).unwrap();
        assert_eq!(m, Move::Announce(Suit::Bells));
        apply(&mut state, &mut known, m);
        assert_eq!(state.points(0), 11 + 12 + engine::trump_points(Suit::Bells, false));
    }

    #[test]
    fn a_safe_nonhalf_run_cash_can_still_precede_a_speculative_question() {
        let mut state = FastState::new(
            [
                cards(&["s-A", "s-Z", "s-K", "s-7"]),
                cards(&["s-O", "g-6", "g-7", "g-8"]),
                cards(&["s-9", "e-K", "e-O", "g-9"]),
                cards(&["s-U", "r-6", "r-7", "r-8"]),
            ],
            0,
        );
        let mut known = Known {
            may: [state.in_hands(); 4],
            read: [0; 4],
            held: [CardSet::EMPTY; 4],
        };
        for name in ["s-A", "s-O", "s-9", "s-U"] {
            let c = name.parse::<engine::Card>().unwrap().index();
            known.play(&state, c);
            state.play(c);
        }
        assert_eq!(activation(&state, &known, None), None);
        assert_eq!(lead0(&state, &known), "s-Z".parse::<engine::Card>().unwrap().index());
        assert_eq!(
            activation(&state, &known, Some(Step::Ask)),
            Some(Move::Question(QuestionType::Yours))
        );
    }

    fn control_opening(enemy: [&str; 4]) -> (FastState, Known) {
        let mut state = FastState::new(
            [
                cards(&["s-A", "r-A", "r-K", "r-O"]),
                cards(&["s-6", "r-6", enemy[0], enemy[1]]),
                cards(&["s-7", "e-8", "g-K", "g-O"]),
                cards(&["s-8", "r-7", enemy[2], enemy[3]]),
            ],
            0,
        );
        let mut known = Known {
            may: [state.in_hands(); 4],
            read: [0; 4],
            held: [CardSet::EMPTY; 4],
        };
        for name in ["s-A", "s-6", "s-7", "s-8"] {
            let c = name.parse::<engine::Card>().unwrap().index();
            known.play(&state, c);
            state.play(c);
        }
        state.announce(Suit::Red);
        for name in ["r-A", "r-6", "e-8", "r-7"] {
            let c = name.parse::<engine::Card>().unwrap().index();
            known.play(&state, c);
            state.play(c);
        }
        (state, known)
    }

    #[test]
    fn current_control_does_not_suppress_the_default_half_question() {
        let mut state = FastState::new(
            [
                cards(&["s-A", "g-K", "g-O", "g-A", "r-O"]),
                cards(&["s-6", "g-6", "e-6", "e-7", "e-8"]),
                cards(&["s-7", "g-7", "r-K", "r-6", "r-7"]),
                cards(&["s-8", "g-8", "e-9", "e-Z", "e-A"]),
            ],
            0,
        );
        let mut known = Known {
            may: [state.in_hands(); 4],
            read: [0; 4],
            held: [CardSet::EMPTY; 4],
        };
        for name in ["s-A", "s-6", "s-7", "s-8"] {
            let c = name.parse::<engine::Card>().unwrap().index();
            known.play(&state, c);
            state.play(c);
        }
        state.announce(Suit::Green);
        for name in ["g-A", "g-6", "g-7", "g-8"] {
            let c = name.parse::<engine::Card>().unwrap().index();
            known.play(&state, c);
            state.play(c);
        }
        assert_eq!(
            activation(&state, &known, None),
            Some(Move::Question(QuestionType::YourHalf(Suit::Red)))
        );
    }

    /// At two cards partner has no spare beyond its possible pair. Even though
    /// the actual Green answer is harmful here, resource-based deferral is not
    /// justified; the engine-scored alternatives must resolve that tradeoff.
    #[test]
    fn preserve_current_control_before_a_truthful_but_harmful_pair_answer() {
        let (mut state, mut known) = control_opening(["g-A", "g-6", "g-Z", "g-7"]);
        assert_eq!(
            activation(&state, &known, None),
            Some(Move::Question(QuestionType::Yours))
        );
        assert_eq!(
            activation(&state, &known, Some(Step::Ask)),
            Some(Move::Question(QuestionType::Yours))
        );
        let (mut cash, mut cash_known) = (state, known);
        playout(
            &mut cash,
            &mut cash_known,
            Some(Plan {
                actor: 0,
                steps: [Step::Wait; PLAN_STEPS],
                next: 0,
            }),
        );
        apply(&mut state, &mut known, Move::Question(QuestionType::Yours));
        assert_eq!(state.trump(), Some(Suit::Green));
        playout(&mut state, &mut known, None);
        assert!(score(&cash, Some((0, 120)), 0) > score(&state, Some((0, 120)), 0));
    }

    #[test]
    fn valuable_partner_activation_remains_a_scored_alternative_to_control() {
        let (mut state, mut known) = control_opening(["e-A", "e-6", "e-Z", "e-7"]);
        // Safe current trump is a resource, not proof that waiting is best:
        // partner can ruff our lead under Green and the extra pair makes 180.
        let question = activation(&state, &known, None).unwrap();
        assert_eq!(question, Move::Question(QuestionType::Yours));
        let (mut cash, mut cash_known) = (state, known);
        playout(
            &mut cash,
            &mut cash_known,
            Some(Plan {
                actor: 0,
                steps: [Step::Wait; PLAN_STEPS],
                next: 0,
            }),
        );
        apply(&mut state, &mut known, question);
        playout(&mut state, &mut known, None);
        assert!(score(&state, Some((0, 180)), 0) > score(&cash, Some((0, 180)), 0));
    }

    #[test]
    fn actors_choose_the_same_lead_after_hidden_opponent_cards_are_swapped() {
        let hands = [
            cards(&["s-A", "e-9", "g-6"]),
            cards(&["s-6", "e-A", "g-9"]),
            cards(&["s-7", "e-K", "g-Z"]),
            cards(&["s-8", "e-Z", "g-U"]),
        ];
        let mut state = FastState::new(hands, 0);
        let mut known = Known {
            may: [state.in_hands(); 4],
            read: [0; 4],
            held: [CardSet::EMPTY; 4],
        };
        for name in ["s-A", "s-6", "s-7", "s-8"] {
            let c = name.parse::<engine::Card>().unwrap().index();
            known.play(&state, c);
            state.play(c);
        }
        let mut swapped = hands;
        swapped.swap(1, 3);
        let mut other = FastState::new(swapped, 0);
        for name in ["s-A", "s-8", "s-7", "s-6"] {
            other.play(name.parse::<engine::Card>().unwrap().index());
        }
        assert_eq!(lead0(&state, &known), lead0(&other, &known));
        assert_eq!(activation(&state, &known, None), activation(&other, &known, None));
    }

    /// A plan step belongs to an activation opportunity (Astra's review, B): without one, the
    /// step is not used up, so the alternatives keep the same horizon.
    #[test]
    fn a_plan_step_is_only_used_at_an_activation_opportunity() {
        let hands = [
            CardSet::from_cards(&["r-K", "r-O", "s-6"].map(|c| c.parse().unwrap())),
            CardSet::from_cards(&["g-6", "g-7", "g-8"].map(|c| c.parse().unwrap())),
            CardSet::from_cards(&["e-6", "e-7", "e-8"].map(|c| c.parse().unwrap())),
            CardSet::from_cards(&["s-7", "s-8", "s-9"].map(|c| c.parse().unwrap())),
        ];
        let state = FastState::new(hands, 0);
        let mut plan = Some(Plan {
            actor: 0,
            steps: [Step::Ask, Step::Own],
            next: 0,
        });
        assert_eq!(Plan::take(&mut plan, &state, false), None);
        assert_eq!(plan.unwrap().next, 0);
        assert_eq!(Plan::take(&mut plan, &state, true), Some(Step::Ask));
        assert_eq!(plan.unwrap().next, 1);
    }

    /// A Lusche passed forth reads as a blank sign; a card of that suit returned to the giver, or
    /// one it played, overrides the reading (the owner 2026-10-13; Astra's review 2026-10-13).
    #[test]
    fn exact_transfers_override_a_blank_sign() {
        let cards = |c: &[&str]| CardSet::from_cards(&c.iter().map(|c| c.parse().unwrap()).collect::<Vec<_>>());
        let forth = cards(&["e-6", "e-7", "s-6", "s-7"]);
        let (acorns, bells) = (1 << Suit::Acorns as u8, 1 << Suit::Bells as u8);
        assert_eq!(blank_signs(forth, CardSet::EMPTY, CardSet::EMPTY), acorns | bells);
        assert_eq!(blank_signs(forth, cards(&["s-A"]), CardSet::EMPTY), acorns);
        assert_eq!(blank_signs(forth, CardSet::EMPTY, cards(&["e-9"])), bells);
        let one = cards(&["e-6", "r-A", "r-Z", "r-K"]);
        assert_eq!(blank_signs(one, CardSet::EMPTY, CardSet::EMPTY), acorns);
        let high = cards(&["r-A", "r-Z", "r-K", "g-A"]);
        assert_eq!(blank_signs(high, CardSet::EMPTY, CardSet::EMPTY), 0);
    }

    /// The receiver leads low toward the giver's ace it returned (it knows the ace is there) and
    /// not without that knowledge (Astra's review 2026-10-13: a readable entry route).
    #[test]
    fn a_known_partner_ace_is_reached_with_a_low_lead() {
        let cards = |c: &[&str]| CardSet::from_cards(&c.iter().map(|c| c.parse().unwrap()).collect::<Vec<_>>());
        let lead = |held: CardSet| {
            let hands = [
                cards(&["s-A", "e-7", "g-6"]),
                cards(&["s-6", "e-8", "g-9"]),
                cards(&["s-7", "e-A", "g-Z"]),
                cards(&["s-8", "e-9", "g-U"]),
            ];
            let mut state = FastState::new(hands, 0);
            for c in ["s-A", "s-6", "s-7", "s-8"] {
                state.play(c.parse::<engine::Card>().unwrap().index());
            }
            let known = Known {
                may: [CardSet::ALL - hands[0]; 4],
                read: [0; 4],
                held: [CardSet::EMPTY, CardSet::EMPTY, held, CardSet::EMPTY],
            };
            engine::Card::from_index(lead0(&state, &known)).to_string()
        };
        assert_eq!(lead(cards(&["e-A"])), "e-7");
        assert_eq!(lead(CardSet::EMPTY), "g-6");
    }

    /// Game 1346 T3 as Astra's test (2026-10-13): after Green is trump, seat 0 leads r-6, seat 1
    /// takes with r-K; the partner ruffs with the ace while the last opponent may hold g-Z, and
    /// with the nine once it can't.
    #[test]
    fn a_ruff_is_secured_against_the_last_opponent() {
        let cards = |c: &[&str]| CardSet::from_cards(&c.iter().map(|c| c.parse().unwrap()).collect::<Vec<_>>());
        let ruff = |last: &str| {
            let hands = [
                cards(&["s-A", "g-K", "g-O", "r-6"]),
                cards(&["s-6", "r-K", "e-6", "e-7"]),
                cards(&["s-7", "g-9", "g-A", "e-8"]),
                cards(&["s-8", last, "e-9", "e-Z"]),
            ];
            let mut state = FastState::new(hands, 0);
            for c in ["s-A", "s-6", "s-7", "s-8"] {
                state.play(c.parse::<engine::Card>().unwrap().index());
            }
            state.announce(Suit::Green);
            for c in ["r-6", "r-K"] {
                state.play(c.parse::<engine::Card>().unwrap().index());
            }
            let known = Known {
                may: state.hands(),
                read: [0; 4],
                held: [CardSet::EMPTY; 4],
            };
            engine::Card::from_index(respond(&state, &known)).to_string()
        };
        assert_eq!(ruff("g-Z"), "g-A");
        assert_eq!(ruff("g-6"), "g-9");
    }
}

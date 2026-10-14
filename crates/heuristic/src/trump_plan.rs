//! Announcement timing: preserve a concrete lead and activation route, or act now.
//! Reach is the existing approximate longer-horizon activation model.

use engine::Suit;
use engine::{partner, CardSet, SUITS};
use engine::{ActionType, GameAction, Move, PlayPhase, QuestionType};

use beliefs::conventions::pair_value;
use beliefs::{Concept, ConceptBudget, ConceptQuery, Literal, PerspectiveBudget};

use crate::rules::RuleId;
use crate::valuation::Valuation;
use crate::{decision, fact, Ctx, Decision};
use engine::PlayerTrumpPossibilities;

/// The player with the lead performs this action. A respondent's rights are
/// irrelevant: the engine owns truthful answers and the resulting activation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Activation {
    Announce,
    AskPair,
    AskHalf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Alternatives {
    own: Option<Activation>,
    partner: Option<Activation>,
}

impl Alternatives {
    /// Conditional on the live pair belonging to our team in this allocation.
    fn new(mine: u32, own: PlayerTrumpPossibilities, partner: PlayerTrumpPossibilities) -> Self {
        match mine {
            2 => Self {
                own: own.may_announce().then_some(Activation::Announce),
                partner: partner.may_ask_pair().then_some(Activation::AskPair),
            },
            1 => Self {
                own: Some(Activation::AskHalf),
                partner: Some(Activation::AskHalf),
            },
            _ => Self {
                own: own.may_ask_pair().then_some(Activation::AskPair),
                partner: partner.may_announce().then_some(Activation::Announce),
            },
        }
    }

    fn available(self, partner_lead: bool) -> bool {
        self.own.is_some() || (partner_lead && self.partner.is_some())
    }
}

/// Approximate future openings belonging to each actor, not interchangeable
/// team tricks and not a joint probability. Exact next-opening activation is
/// separately evaluated by Routes through the engine's trick-control solver.
#[derive(Clone, Copy)]
pub(crate) struct Control {
    pub own: f32,
    pub partner: f32,
}

impl Control {
    pub fn with_own_lead(self) -> Self {
        Self {
            own: self.own + 1.0,
            ..self
        }
    }
}

/// What our team can still reach, from my view.
pub(crate) struct Reach {
    /// My uncalled pairs.
    pub my_pairs: Vec<Suit>,
    /// P(the partner holds an uncalled pair); 0 after its "no pair".
    pub partner_pair: f32,
    /// My live standalone halves: (suit, P(the partner holds the other half)), best first.
    pub halves: Vec<(Suit, f32)>,
    /// Access estimate: the existing live-Ace proxy, supplemented by concrete
    /// next-trick entries when partner access is required. Not a guarantee.
    pub p_partner_lead: f32,
}

impl Reach {
    pub fn new(ctx: &Ctx, legal: &[GameAction]) -> Reach {
        let _timing = beliefs::profile::scope("pair.reach");
        let k = ctx.k;
        let mate = partner(ctx.me);
        let hand = k.certain(ctx.me);
        let uncalled = SUITS.into_iter().filter(|&s| !k.is_called(s));
        let mine = k.pairs(ctx.me).certain;
        let my_pairs: Vec<Suit> = uncalled.clone().filter(|&s| mine & 1 << s as u8 != 0).collect();
        // The believed pair is 1 or 0 where the facts decide (`beliefs`).
        let belief = ctx.mind.features(mate);
        let pair_p = |s: Suit| belief.pair[s as usize];
        let no_partner_pair = ConceptQuery {
            event: uncalled
                .clone()
                .map(|suit| Literal {
                    concept: Concept::Pair { seat: mate, suit },
                    value: false,
                })
                .collect(),
            given: vec![],
        };
        let partner_pair = ctx
            .joint_probability(&no_partner_pair)
            .map(|p| 1.0 - p)
            .unwrap_or_else(|| 1.0 - uncalled.clone().map(|s| 1.0 - pair_p(s)).product::<f32>());
        let mut halves: Vec<(Suit, f32)> = uncalled
            .filter_map(|s| {
                let mine = hand & CardSet::halves(s);
                if mine.len() != 1 {
                    return None;
                }
                let other = (CardSet::halves(s) - mine).iter().next()?;
                Some((s, ctx.mind.p_holds(mate, other)))
            })
            .collect();
        halves.sort_by(|a, b| {
            (b.1 * pair_value(b.0) as f32)
                .total_cmp(&(a.1 * pair_value(a.0) as f32))
                .then((a.0 as u8).cmp(&(b.0 as u8)))
        });
        let mut p_partner_lead = ctx.mind.features(mate).ace;
        let requires_partner =
            (!my_pairs.is_empty() && !k.rights(ctx.me).may_announce() && k.rights(mate).may_ask_pair())
                || (partner_pair > 0.0 && !k.rights(ctx.me).may_ask_pair() && k.rights(mate).may_announce());
        if requires_partner {
            // Establish the hard entry first. Its existence is sufficient for
            // this access proxy; do not build a sampled Routes DP to rediscover it.
            let known_entry = legal.iter().any(|action| match action.action_type {
                ActionType::CardPlayed(c) if !CardSet::HALVES.contains(c.index()) => {
                    !(k.partner_entry_winners(ctx.me, c.index()) - CardSet::HALVES).is_empty()
                }
                _ => false,
            });
            if known_entry {
                p_partner_lead = 1.0;
            } else {
                p_partner_lead = entry_access(
                    p_partner_lead,
                    legal.iter().filter_map(|action| {
                        if let ActionType::CardPlayed(c) = action.action_type {
                            Some(ctx.entry(c.index()).lead[mate as usize])
                        } else {
                            None
                        }
                    }),
                );
            }
        }
        Reach {
            my_pairs,
            partner_pair,
            halves,
            p_partner_lead,
        }
    }

    fn alternatives(&self, ctx: &Ctx, s: Suit, own: PlayerTrumpPossibilities) -> Alternatives {
        Alternatives::new(
            (ctx.k.certain(ctx.me) & CardSet::halves(s)).len(),
            own,
            ctx.k.rights(partner(ctx.me)),
        )
    }

    /// Reserve openings for higher-valued known pairs that have only this actor's
    /// route. This does not solve uncertain declaration order, but prevents one
    /// expected opening being reused for every actor-exclusive known pair.
    fn openings_needed(&self, ctx: &Ctx, s: Suit, own_actor: bool) -> f32 {
        let k = ctx.k;
        let known = k.team_pairs(ctx.me);
        1.0 + SUITS
            .into_iter()
            .filter(|&other| {
                other != s && pair_value(other) > pair_value(s) && known.certain() & (1 << other as u8) != 0
            })
            .filter(|&other| {
                let routes = self.alternatives(ctx, other, k.rights(ctx.me));
                if own_actor {
                    routes.own.is_some() && routes.partner.is_none()
                } else {
                    routes.partner.is_some() && routes.own.is_none()
                }
            })
            .count() as f32
    }

    /// Conditional access estimate, with each lead assigned to its actual actor.
    /// Alternative routes share the same pair; use the stronger estimate rather
    /// than adding correlated events. Pair ownership is charged once by callers.
    pub fn p_activate(&self, ctx: &Ctx, s: Suit, control: Control) -> f32 {
        let routes = self.alternatives(ctx, s, ctx.k.rights(ctx.me));
        let own = if routes.own.is_some() {
            (control.own / self.openings_needed(ctx, s, true)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let partner = if routes.partner.is_some() {
            (control.partner / self.openings_needed(ctx, s, false)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        own.max(partner)
    }

    /// Whether the partner may hold a pair worth a "Yours" (P10a "may need").
    fn partner_may(&self, ctx: &Ctx) -> bool {
        self.partner_pair > 0.0 && self.partner_pair >= ctx.params.partner_pair_min
    }

    /// Ask "Yours" for a believed pair unless the partner can announce it itself: it keeps
    /// its right and probably gets a lead (`p_partner_lead` >= `partner_lead_conf`). The
    /// human games: on a believed pair the players never asked (0 of 1,021 positions, even
    /// at P >= 0.9); they leave it to the partner (owner #175: "let me go and play my stuff
    /// and then announce the pair"). That needs the partner's lead, so without it I ask.
    fn asks_yours(&self, ctx: &Ctx) -> bool {
        let mate = partner(ctx.me);
        let partner_announces = ctx.k.pairs(mate).certain == 0
            && ctx.k.rights(mate).may_announce()
            && self.p_partner_lead >= ctx.params.partner_lead_conf;
        self.partner_may(ctx) && !partner_announces
    }

    /// Existing attempt model: uncertain questions before activation. The timing
    /// guard compares it with a certified run, not independent team winners.
    fn attempts(&self, ctx: &Ctx) -> u32 {
        let yours = self.asks_yours(ctx) as u32;
        if !self.my_pairs.is_empty() || ctx.k.pairs(partner(ctx.me)).certain != 0 {
            0
        } else if self.halves.iter().any(|h| h.1 >= 1.0) {
            1 + yours
        } else {
            self.open_halves(ctx).count() as u32 + yours
        }
    }

    /// Live halves the partner may complete (HC-13: a known "no" is no question).
    /// Half questions worth asking: P(the partner holds the other half) at least
    /// `half_question_min` (the human games: below 0.4 the players rarely ask).
    fn open_halves(&self, ctx: &Ctx) -> impl Iterator<Item = &(Suit, f32)> {
        let min = ctx.params.half_question_min;
        self.halves.iter().filter(move |h| h.1 > 0.0 && h.1 >= min)
    }
}

/// Entry clamps every estimate to [0, 1]. Once this access proxy is exactly
/// saturated, remaining entries cannot change it; their full data stays lazy.
fn entry_access(mut lead: f32, mut entries: impl Iterator<Item = f32>) -> f32 {
    while lead != 1.0 {
        let Some(entry) = entries.next() else { break };
        lead = lead.max(entry);
    }
    lead
}

/// P10a: asking `ask` leaves every pair the team may still need as activatable as before. Only
/// rights the question spends count: a question that changes nothing (my right is spent
/// already) is never blocked.
fn keeps_needed_rights(ctx: &Ctx, reach: &Reach, ask: QuestionType) -> bool {
    let me = ctx.me;
    let mate = partner(me);
    let (before, theirs) = (ctx.k.rights(me), ctx.k.rights(mate));
    let after = before.after_question(ask);
    // The partner will plausibly win a lead before the end (P10a).
    let lead = reach.p_partner_lead >= ctx.params.partner_lead_conf;
    let my_pair = |r| reach.my_pairs.is_empty() || Alternatives::new(2, r, theirs).available(lead);
    let partner_pair = |r| !reach.partner_may(ctx) || Alternatives::new(0, r, theirs).available(lead);
    let lost_mine = my_pair(before) && !my_pair(after);
    let lost_theirs = ask != QuestionType::Yours && partner_pair(before) && !partner_pair(after);
    !lost_mine && !lost_theirs
}

fn find(legal: &[GameAction], t: ActionType) -> Option<&GameAction> {
    legal.iter().find(|a| a.action_type == t)
}

/// The plan at a lead where questions are legal.
pub(crate) fn timing(
    ctx: &Ctx,
    legal: &[GameAction],
    reach: &Reach,
    val: &Valuation,
    lead_card: Option<u8>,
) -> Option<Decision> {
    let planned = action(ctx, legal, reach);
    let cards = legal
        .iter()
        .filter_map(|a| match a.action_type {
            ActionType::CardPlayed(c) => Some(c.index()),
            _ => None,
        })
        .fold(CardSet::EMPTY, |s, c| s | CardSet::single(c));
    let retained = (ctx.k.retaining_lead(ctx.me) & (cards - val.plan_cards))
        .iter()
        .filter(|&card| planned.as_ref().is_some_and(|p| preserves_activation(ctx, p, card)))
        .fold(CardSet::EMPTY, |set, card| set | CardSet::single(card));
    // With no trump, all these cards remain own winners. Under trump, only
    // the trump run plus ONE side winner is certified; later side rounds can
    // become ruffable. Cash that side card first, then the trump run survives.
    let (first, count) = match ctx.k.trump() {
        None => (retained, retained.len()),
        Some(t) => {
            let trumps = retained & CardSet::suit(t);
            let side = retained - trumps;
            (
                if side.is_empty() { trumps } else { side },
                trumps.len() + u32::from(!side.is_empty()),
            )
        }
    };
    let count = count.min(ctx.k.hand_len(ctx.me).saturating_sub(1) as u32);
    let attempts = reach.attempts(ctx);
    // HC-4: cash a card that retains my own lead before the activation. A candidate with the
    // rule as prior, not a gate (Astra's review of the reference players, S20).
    let cash = (ctx.params.lead_cash_standing && attempts < count)
        .then(|| crate::leads::cash_card(cards, first))
        .flatten()
        .and_then(|c| find(legal, ActionType::CardPlayed(engine::Card::from_index(c))))
        .map(|a| {
            decision(
                a,
                RuleId::HC4,
                vec![
                    fact("retains_own_lead", 1),
                    fact("retained_leads", count),
                    fact("attempts", attempts),
                ],
            )
        });
    // Deferral belongs to the actual cash action, never just to a hand shape.
    // A positive partner activation remains an explicit HO-2 alternative.
    let defer_yours = planned
        .as_ref()
        .filter(|d| ctx.params.lead_cash_standing
            && d.action.action_type == ActionType::Question(QuestionType::Yours))
        .and(lead_card)
        .filter(|&card| {
            let others = (0..4)
                .filter(|&seat| seat != ctx.me)
                .fold(CardSet::EMPTY, |set, seat| set | ctx.k.may_hold(seat));
            crate::leads::cash_before_question(ctx.k.certain(ctx.me), ctx.k.trump(), others, card)
        })
        .and_then(|card| find(legal, ActionType::CardPlayed(engine::Card::from_index(card))))
        .map(|a| {
            decision(
                a,
                RuleId::HC4,
                vec![fact("retains_own_lead", 1), fact("partner_pair_spare", 1)],
            )
        });
    let chosen = cash
        .or(defer_yours)
        .or_else(|| connected_entry(ctx, legal, cards - val.plan_cards, reach, planned.as_ref()))
        .or(planned);
    if !ctx.params.outlook_activation {
        return chosen;
    }
    // HO-2 compares the rule's choice with the activations open to me and with the card I would
    // lead, each as the concrete move it is (a baseline's choice is a prior, not a gate).
    match route_activation(ctx, legal, chosen.as_ref(), lead_card) {
        Some(d) => Some(d),
        None => chosen,
    }
}

/// Follow an available conditional route even after my question rights have
/// been spent: the pair owner can announce, or the other actor can ask Yours.
/// This proves availability, not a commitment by a privately modelled partner.
fn connected_entry(
    ctx: &Ctx,
    legal: &[GameAction],
    cards: CardSet,
    reach: &Reach,
    planned: Option<&Decision>,
) -> Option<Decision> {
    let mate = partner(ctx.me);
    if !ctx.params.lead_cash_standing || ctx.k.hand_len(mate) <= 1 {
        return None;
    }
    let defer_yours = !ctx.k.rights(ctx.me).may_ask_pair()
        || planned.is_some_and(|p| p.action.action_type == ActionType::Question(QuestionType::Yours));
    let known = ctx.k.team_pairs(ctx.me);
    let connection = SUITS
        .into_iter()
        .filter(|&s| known.certain() & (1 << s as u8) != 0)
        .filter_map(|suit| {
            let route = reach.alternatives(ctx, suit, ctx.k.rights(ctx.me)).partner?;
            let usable = match route {
                Activation::Announce => defer_yours,
                Activation::AskPair => !ctx.k.rights(ctx.me).may_announce() && partner_knows_pair(ctx, suit),
                // Split access remains in the solver's joint entry valuation.
                Activation::AskHalf => false,
            };
            usable.then_some((Some(suit), route))
        })
        .max_by_key(|&(suit, _)| suit.map(pair_value))
        .or_else(|| {
            // Preserve the existing uncertain pair-owner alternative. A
            // non-half entry preserves any pair it has; it does not prove that
            // the partner has one. No suit or known-pair fact is published here.
            (defer_yours && reach.partner_may(ctx) && ctx.k.rights(mate).may_announce())
                .then_some((None, Activation::Announce))
        });
    let (suit, route) = connection?;
    let (card, winner) = (cards - CardSet::HALVES)
        .iter()
        .filter_map(|c| {
            let winners = ctx.k.partner_entry_winners(ctx.me, c) - CardSet::HALVES;
            crate::leads::cash_card(winners, winners).map(|winner| (c, winner))
        })
        .min_by_key(|&(c, _)| (engine::CARD_POINTS[c as usize], c))?;
    let action = find(legal, ActionType::CardPlayed(engine::Card::from_index(card)))?;
    let mut why = vec![
        fact("partner_entry", winner),
        fact("partner_can_announce", u8::from(route == Activation::Announce)),
        fact("partner_can_ask", u8::from(route == Activation::AskPair)),
        fact("entry_consumes_no_half", 1),
    ];
    if let Some(suit) = suit {
        why.extend([fact("activation_suit", suit as u8), fact("pair_survives_entry", 1)]);
    }
    Some(decision(action, RuleId::HC4, why))
}

/// A guaranteed next own lead is useful only if its activation resources remain.
/// For a known partner resource, forcing follow in another suit preserves it.
/// A speculative half question retains its existing attempt model; no certainty
/// about that unknown half is inferred from this guard.
fn preserves_activation(ctx: &Ctx, planned: &Decision, card: u8) -> bool {
    let k = ctx.k;
    let mate = partner(ctx.me);
    let ours = k.certain(ctx.me) - CardSet::single(card);
    match planned.action.action_type {
        ActionType::AnnounceTrump(suit) => {
            CardSet::halves(suit).is_subset(ours)
                && Alternatives::new(2, k.rights(ctx.me), k.rights(mate)).own == Some(Activation::Announce)
        }
        ActionType::Question(QuestionType::Yours) => {
            if !k.rights(ctx.me).may_ask_pair() {
                return false;
            }
            let known = k.pairs(mate).certain;
            known == 0
                || SUITS
                    .into_iter()
                    .any(|suit| known & (1 << suit as u8) != 0 && partner_keeps_pair(ctx, mate, suit, card))
        }
        ActionType::Question(QuestionType::YourHalf(suit)) => {
            !(ours & CardSet::halves(suit)).is_empty()
                && (k.known_half(mate, suit) != Some(true) || partner_keeps_pair(ctx, mate, suit, card))
        }
        _ => false,
    }
}

/// After I lead `card`, the partner keeps its known pair of `pair`: the lead is another suit and
/// the partner has a card to play other than the two halves: it follows the led suit, ruffs, or
/// discards another card, knowing its own pair (Astra's review of the reference players, S19:
/// a blank partner preserves its pair as well as a following one).
fn partner_keeps_pair(ctx: &Ctx, mate: u8, pair: Suit, card: u8) -> bool {
    engine::suit_of(card) != pair && ctx.k.hand_len(mate) > 2
}

/// Root's proof of viewer knowledge, never a probability rounded to one. This
/// shortcut requests no traversal and cannot reveal a private root pair to mate.
fn partner_knows_pair(ctx: &Ctx, suit: Suit) -> bool {
    ctx.mind
        .perspective_concept(
            partner(ctx.me),
            &ConceptQuery {
                event: vec![Concept::Pair { seat: ctx.me, suit }.into()],
                given: vec![],
            },
            PerspectiveBudget {
                outer_nodes: 0,
                inner: ConceptBudget { max_nodes: 0 },
                max_hypotheses: 0,
            },
        )
        .ok()
        .and_then(|p| p.known_by_viewer)
        == Some(true)
}

/// HO-2: the rule's choice (`planned`: an activation, a card, or none) against the activations
/// open to me, where the route over the believed worlds is clearly better. Questions the partner certainly
/// answers no are not options. `None` keeps the plan; `Some(None)` plays a card.
fn route_activation(
    ctx: &Ctx,
    legal: &[GameAction],
    planned: Option<&Decision>,
    lead_card: Option<u8>,
) -> Option<Decision> {
    let k = ctx.k;
    let mate = partner(ctx.me);
    let mine = k.certain(ctx.me);
    let moves: Vec<Move> = legal
        .iter()
        .filter_map(|a| Move::from_action(&a.action_type))
        .filter(|m| match *m {
            Move::Announce(_) => true,
            Move::Question(QuestionType::Yours) => k.pairs(mate).possible != 0,
            Move::Question(QuestionType::YourHalf(s)) => {
                !k.is_called(s) && !(mine & CardSet::halves(s)).is_empty() && k.known_half(mate, s) != Some(false)
            }
            _ => false,
        })
        .collect();
    let rule = planned.and_then(|d| Move::from_action(&d.action.action_type));
    let mut moves = moves;
    // The concrete cards: the rule's (if it chose one), the card I would lead, and every card that
    // certainly stands (cash now, activate later: `cash_before_activation`, Astra's review of the
    // reference players S16/S18/S19), also in the suit of a possible pair; whether cashing it
    // forces a half out is the route's to weigh, not a filter's.
    let standing = ctx.k.standing(ctx.me).iter().map(Move::Card);
    // And the lead into the partner's ruff: the lowest card of each side suit the partner is
    // proven void in while it may still hold trump (S14: hand it the lead through its trump and
    // keep my own controls).
    let mine = ctx.k.certain(ctx.me);
    let ruff_entries = ctx.k.trump().into_iter().flat_map(|t| {
        SUITS
            .into_iter()
            .filter(move |&s| s != t && ctx.k.is_void(mate, s) && !ctx.k.is_void(mate, t))
            .filter_map(move |s| {
                (mine & CardSet::suit(s))
                    .iter()
                    .min_by_key(|&c| engine::CARD_POINTS[c as usize])
            })
            .map(Move::Card)
    });
    for m in rule
        .into_iter()
        .chain(lead_card.map(Move::Card))
        .chain(standing)
        .chain(ruff_entries)
    {
        if matches!(m, Move::Card(_)) && !moves.contains(&m) {
            moves.push(m);
        }
    }
    let plan = rule.or(lead_card.map(Move::Card))?;
    if moves.len() < 2 {
        return None;
    }
    crate::outlook::route_choice(ctx, legal, PlayPhase::from_actions(legal), &moves, plan)
}

fn action(ctx: &Ctx, legal: &[GameAction], reach: &Reach) -> Option<Decision> {
    let mut why = Vec::new();
    // Mine: my most valuable pair (HC-5 when there are several).
    let announce = reach
        .my_pairs
        .iter()
        .filter_map(|&s| find(legal, ActionType::AnnounceTrump(s)).map(|a| (pair_value(s), a)))
        .max_by_key(|&(v, _)| v);
    if let Some((v, a)) = announce {
        why.push(fact("pair_value", v));
        let rule = if reach.my_pairs.len() > 1 {
            RuleId::HC5
        } else {
            RuleId::HC2
        };
        return Some(decision(a, rule, why));
    }
    // Yours.
    if reach.asks_yours(ctx) && keeps_needed_rights(ctx, reach, QuestionType::Yours) {
        if let Some(a) = find(legal, ActionType::Question(QuestionType::Yours)) {
            why.push(fact("partner_pair", reach.partner_pair));
            return Some(decision(a, RuleId::HQ3, why));
        }
    }
    // Ours: the half the partner most likely completes, by pair value.
    let skipped = reach.halves.len() - reach.open_halves(ctx).count();
    if skipped > 0 {
        why.push(fact("hc13_skipped", skipped as u32));
    }
    for &(s, p) in reach.open_halves(ctx) {
        if !keeps_needed_rights(ctx, reach, QuestionType::YourHalf(s)) {
            why.push(fact("p10a_blocked", 1));
            break;
        }
        if let Some(a) = find(legal, ActionType::Question(QuestionType::YourHalf(s))) {
            why.push(fact("p_other_half", p));
            return Some(decision(a, RuleId::HQ3, why));
        }
    }
    None
}

#[cfg(test)]
#[path = "../tests/internal/activation_resources.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/internal/reach_saturation.rs"]
mod saturation_tests;

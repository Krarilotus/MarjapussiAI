//! Decision-local, lazily evaluated entry conditions over the same joint deals.
//! Probabilities estimate exposed-deal capability; they are not guarantees of
//! informed future play or calibrated probabilities of human choices.

use engine::{opponents, suit_of, FastState, Move, PlayPhase, SUITS};
use knowledge::{Belief, Hands, World};

use crate::{Ctx, Rng};

#[derive(Clone, Copy, Default)]
pub(crate) struct Entry {
    pub lead: [f32; 4],
    pub activation: [f32; 4],
    pub both_follow: f32,
}

pub(crate) struct Routes {
    worlds: Vec<World>,
    entries: [Option<Entry>; 36],
}

impl Routes {
    fn new(ctx: &Ctx) -> Self {
        let _timing = beliefs::profile::scope("routes");
        // A computational budget, not a fitted playing-strength parameter.
        let worlds_max = ctx.params.outlook_worlds as usize;
        let model = ctx.mind.deal_model();
        let (space, weights) = (model.space(), model.weights());
        // D1: the partner's raise is read once into these worlds (its likelihood given the
        // partner's hand at that decision: its current cards and those it played since). With a
        // reading, `RAISE_DRAWS` times as many deals are drawn and resampled by weight, so enough
        // worlds remain that fit the raise. Scope (Astra 17.7 C): this conditions the route
        // worlds (outlook, entries); the Mind's other belief queries don't read the raise yet.
        let raise = crate::raising::partner_raise(ctx);
        let floor = ctx.mind.params().conventions.convention_floor;
        let read = |hands: &Hands| {
            raise.as_ref().map_or(1.0, |r| {
                r.likelihood(hands[r.raiser as usize] | ctx.k.played_by(r.raiser), floor)
            })
        };
        let worlds = if space.count() <= worlds_max as u64 {
            let mut worlds = Vec::new();
            space.for_each_deal(|hands| {
                worlds.push(World {
                    hands: *hands,
                    weight: weights.weight(hands) * read(hands),
                })
            });
            worlds
        } else if raise.is_some() {
            let mut rng = Rng::new(ctx.seed);
            let drawn: Vec<World> = space
                .sample_worlds(&mut rng, RAISE_DRAWS * worlds_max, weights)
                .into_iter()
                .map(|w| World {
                    weight: w.weight * read(&w.hands),
                    ..w
                })
                .collect();
            resample(&drawn, worlds_max, &mut rng)
        } else {
            space.sample_worlds(&mut Rng::new(ctx.seed), worlds_max, weights)
        };
        Self {
            worlds,
            entries: [None; 36],
        }
    }

    fn entry(&mut self, ctx: &Ctx, card: u8) -> Entry {
        let _timing = beliefs::profile::scope("routes.entry");
        if let Some(entry) = self.entries[card as usize] {
            return entry;
        }
        let public = ctx.k.public_state(ctx.me, PlayPhase::Cards);
        let mut entry = Entry::default();
        let total: f64 = self.worlds.iter().map(|w| w.weight).sum();
        for world in &self.worlds {
            let state = FastState::from_public(public, world.hands).expect("consistent deal");
            let goals = solver::trick_control(state, Move::Card(card));
            let p = (world.weight / total) as f32;
            for seat in 0..4 {
                if goals.lead(seat) {
                    entry.lead[seat as usize] += p;
                }
            }
            for s in SUITS {
                if goals.activation(ctx.me % 2, s) {
                    entry.activation[s as usize] += p;
                }
            }
            if opponents(ctx.me)
                .iter()
                .all(|&o| !(world.hands[o as usize] & engine::CardSet::suit(suit_of(card))).is_empty())
            {
                entry.both_follow += p;
            }
        }
        // Accumulating f32 sample masses can round a certain event above one.
        entry
            .lead
            .iter_mut()
            .chain(entry.activation.iter_mut())
            .chain(std::iter::once(&mut entry.both_follow))
            .for_each(|p| *p = p.clamp(0.0, 1.0));
        self.entries[card as usize] = Some(entry);
        entry
    }
}

/// Candidate deals per world when a raise is read: a matching combination of the raiser's
/// pairs can be rare under the uniform draw (game 1187: about 0.5 %); drawing is cheap (2,048
/// deals in under 1 ms), the playouts are not, so only the resampled pool is played out.
const RAISE_DRAWS: usize = 32;

/// `n` worlds drawn from `drawn` with probability proportional to weight (systematic
/// resampling), each then of equal weight: the importance weights become sample frequency.
fn resample(drawn: &[World], n: usize, rng: &mut Rng) -> Vec<World> {
    let total: f64 = drawn.iter().map(|w| w.weight).sum();
    if drawn.is_empty() || total <= 0.0 {
        return drawn.iter().take(n).copied().collect();
    }
    let step = total / n as f64;
    let mut at = (rng.next() >> 11) as f64 / (1u64 << 53) as f64 * step;
    let (mut i, mut acc) = (0, drawn[0].weight);
    let mut out: Vec<World> = Vec::with_capacity(n);
    for _ in 0..n {
        while acc < at && i + 1 < drawn.len() {
            i += 1;
            acc += drawn[i].weight;
        }
        match out.last_mut() {
            // A world drawn again is one world with more weight, not new evidence: the decision
            // guard's effective sample size then counts it once (Astra 17.7 D).
            Some(last) if last.hands == drawn[i].hands => last.weight += 1.0,
            _ => out.push(World {
                weight: 1.0,
                ..drawn[i]
            }),
        }
        at += step;
    }
    out
}

impl Ctx<'_> {
    /// The decision's shared believed worlds (the same pool as the entry estimates).
    pub(crate) fn with_worlds<T>(&self, f: impl FnOnce(&[World]) -> T) -> T {
        let mut routes = self.routes.borrow_mut();
        let routes = routes.get_or_insert_with(|| Routes::new(self));
        f(&routes.worlds)
    }

    pub(crate) fn entry(&self, card: u8) -> Entry {
        self.routes
            .borrow_mut()
            .get_or_insert_with(|| Routes::new(self))
            .entry(self, card)
    }
}

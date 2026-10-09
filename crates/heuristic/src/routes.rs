//! Decision-local, lazily evaluated entry conditions over the same joint deals.
//! Probabilities estimate exposed-deal capability; they are not guarantees of
//! informed future play or calibrated probabilities of human choices.

use engine::{opponents, suit_of, FastState, Move, PlayPhase, SUITS};
use knowledge::{Belief, World};

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
        // A computational budget, not a fitted playing-strength parameter.
        const WORLDS: usize = 64;
        let space = ctx.k.deals();
        let weights = ctx.mind.deal_weights();
        let worlds = if space.count() <= WORLDS as u64 {
            let mut worlds = Vec::new();
            space.for_each_deal(|hands| {
                worlds.push(World {
                    hands: *hands,
                    weight: weights.weight(hands),
                })
            });
            worlds
        } else {
            space.sample_worlds(&mut Rng::new(ctx.seed), WORLDS, &weights)
        };
        Self {
            worlds,
            entries: [None; 36],
        }
    }

    fn entry(&mut self, ctx: &Ctx, card: u8) -> Entry {
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

impl Ctx<'_> {
    pub(crate) fn entry(&self, card: u8) -> Entry {
        self.routes
            .borrow_mut()
            .get_or_insert_with(|| Routes::new(self))
            .entry(self, card)
    }
}

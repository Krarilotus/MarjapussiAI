//! Bounded traversal of equivalent allocations, refining the existing fact DP.
//! Labels are supplied by the consumer: every predicate and weight it evaluates
//! must be invariant under swapping equally labelled cards within a group.

use engine::{CardSet, CARDS};

use crate::sample::{for_each_split, minus, Class};
use crate::{DealSpace, Hands, Knowledge};

/// Exact accounting even when traversal stops before visiting every allocation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AllocationVisit {
    pub total_deals: u64,
    pub covered_deals: u64,
    pub classes: usize,
    pub nodes: usize,
}

impl AllocationVisit {
    pub fn complete(&self) -> bool {
        self.covered_deals == self.total_deals
    }

    pub fn unresolved_deals(&self) -> u64 {
        self.total_deals - self.covered_deals
    }
}

struct Group {
    cards: CardSet,
    sig: u8,
}

struct Layout<'a> {
    space: &'a DealSpace,
    tied: Vec<Vec<Group>>,
    free: Vec<Vec<Group>>,
}

struct Work {
    limit: usize,
    visit: AllocationVisit,
}

impl Work {
    fn enter(&mut self) -> bool {
        if self.visit.nodes == self.limit {
            return false;
        }
        self.visit.nodes += 1;
        true
    }
}

impl DealSpace {
    /// Partition by `viewer`'s full private current hand FIRST, then count
    /// equivalent opponent allocations using the usual labels/constraint DP.
    /// `k` must be the knowledge from which this space was constructed.
    /// Nodes include subset recursion and grouped traversal, not construction
    /// of the conditional DP. `max_hands` bounds distinct supported hypotheses.
    pub fn visit_private_allocations(
        &self,
        k: &Knowledge,
        viewer: u8,
        labels: &[u64; CARDS],
        max_nodes: usize,
        max_hands: usize,
        mut f: impl FnMut(&Hands, u64) -> bool,
    ) -> AllocationVisit {
        debug_assert_eq!(self.known, k.certain);
        let mut work = Work {
            limit: max_nodes,
            visit: AllocationVisit {
                total_deals: self.count(),
                ..Default::default()
            },
        };
        if viewer >= 4 || k.me() >= 4 {
            return work.visit;
        }
        let known = k.certain(viewer);
        let cards: Vec<_> = (k.may_hold(viewer) - known).iter().collect();
        let needed = k.hand_len(viewer).saturating_sub(known.len() as u8);
        let mut hypotheses = 0;
        choose_hand(&cards, needed, known, &mut work, &mut |hand, work| {
            if hypotheses == max_hands {
                return true;
            }
            let Some(conditioned) = k.with_current_hand(viewer, hand) else {
                return false;
            };
            let space = conditioned.deals();
            if space.count() == 0 {
                return false;
            }
            hypotheses += 1;
            let mut stopped = false;
            let visit = space.visit_allocations(labels, work.limit - work.visit.nodes, |hands, ways| {
                stopped = f(hands, ways);
                stopped
            });
            work.visit.nodes += visit.nodes;
            work.visit.classes += visit.classes;
            work.visit.covered_deals += visit.covered_deals;
            stopped || !visit.complete()
        });
        work.visit
    }

    /// Visits one representative and its exact multiplicity per allocation class.
    /// Equal labels only merge cards already equivalent under the hard facts.
    /// `max_nodes` bounds recursive work; returning true from `f` also stops early.
    /// Unvisited deals are reported, not sampled or silently renormalised away.
    pub fn visit_allocations(
        &self,
        labels: &[u64; CARDS],
        max_nodes: usize,
        mut f: impl FnMut(&Hands, u64) -> bool,
    ) -> AllocationVisit {
        let refine = |class: &Class| {
            let mut groups: Vec<(u64, Group)> = Vec::new();
            for &card in &self.pool[class.start as usize..(class.start + class.n) as usize] {
                let label = labels[card as usize];
                if let Some((_, group)) = groups.iter_mut().find(|(l, _)| *l == label) {
                    group.cards |= CardSet::single(card);
                } else {
                    groups.push((
                        label,
                        Group {
                            cards: CardSet::single(card),
                            sig: class.sig,
                        },
                    ));
                }
            }
            groups.into_iter().map(|(_, group)| group).collect()
        };
        let layout = Layout {
            space: self,
            tied: self.groups.iter().map(refine).collect(),
            free: self.classes.iter().map(refine).collect(),
        };
        let mut work = Work {
            limit: max_nodes,
            visit: AllocationVisit {
                total_deals: self.count(),
                ..Default::default()
            },
        };
        for leaf in 0..self.leaves.len() {
            if tied(&layout, leaf, 0, (self.known, 1), &mut work, &mut f) {
                break;
            }
        }
        work.visit
    }
}

fn choose_hand(
    cards: &[u8],
    needed: u8,
    hand: CardSet,
    work: &mut Work,
    f: &mut dyn FnMut(CardSet, &mut Work) -> bool,
) -> bool {
    if !work.enter() {
        return true;
    }
    if needed == 0 {
        return f(hand, work);
    }
    if cards.len() < needed as usize {
        return false;
    }
    for i in 0..=cards.len() - needed as usize {
        if choose_hand(&cards[i + 1..], needed - 1, hand | CardSet::single(cards[i]), work, f) {
            return true;
        }
    }
    false
}

fn tied(
    layout: &Layout,
    leaf: usize,
    at: usize,
    placement: (Hands, u64),
    work: &mut Work,
    f: &mut dyn FnMut(&Hands, u64) -> bool,
) -> bool {
    if !work.enter() {
        return true;
    }
    let Some(block) = layout.tied.get(at) else {
        return free(layout, 0, layout.space.leaves[leaf].rest, placement, work, f);
    };
    let split = layout.space.leaf_splits[leaf * layout.tied.len() + at];
    refine(block, 0, split, placement, layout.space.hidden, work, &mut |p, w| {
        tied(layout, leaf, at + 1, p, w, f)
    })
}

fn free(
    layout: &Layout,
    at: usize,
    rest: [u8; 3],
    placement: (Hands, u64),
    work: &mut Work,
    f: &mut dyn FnMut(&Hands, u64) -> bool,
) -> bool {
    if !work.enter() {
        return true;
    }
    let Some(class) = layout.space.classes.get(at) else {
        work.visit.classes += 1;
        work.visit.covered_deals += placement.1;
        return f(&placement.0, placement.1);
    };
    let mut stopped = false;
    for_each_split(class.sig, class.n, rest, |split, _| {
        let remaining = minus(rest, split);
        if layout.space.ways(at + 1, remaining) > 0 {
            stopped = refine(
                &layout.free[at],
                0,
                split,
                placement,
                layout.space.hidden,
                work,
                &mut |p, w| free(layout, at + 1, remaining, p, w, f),
            );
        }
        stopped
    });
    stopped
}

fn refine(
    block: &[Group],
    at: usize,
    rest: [u8; 3],
    placement: (Hands, u64),
    hidden: [u8; 3],
    work: &mut Work,
    next: &mut dyn FnMut((Hands, u64), &mut Work) -> bool,
) -> bool {
    if !work.enter() {
        return true;
    }
    let Some(group) = block.get(at) else {
        debug_assert_eq!(rest, [0; 3]);
        return next(placement, work);
    };
    let mut stopped = false;
    for_each_split(group.sig, group.cards.len() as u8, rest, |split, ways| {
        let remaining = minus(rest, split);
        // All cards in this block have the same allowed-holder signature. A
        // representative suffices; binomial factors count the omitted permutations.
        let mut h = placement.0;
        let mut cards = group.cards.iter();
        for (seat, n) in hidden.into_iter().zip(split) {
            for _ in 0..n {
                h[seat as usize] |= CardSet::single(cards.next().expect("split cardinality"));
            }
        }
        stopped = refine(block, at + 1, remaining, (h, placement.1 * ways), hidden, work, next);
        stopped
    });
    stopped
}

//! `Mind`: one agent seat's facts, events and cached believed forms.

use std::cell::{Ref, RefCell};

use agent::SeatKind;
use engine::{CardSet, SUITS};
use knowledge::{Contradiction, Knowledge, Tracker};
use observation::{Seat, SeatEvent};

use crate::band::{Band, Bands};
use crate::calibrate::Calibration;
use crate::conventions::{self, bid_table, HandClass, N_CLASSES};
use crate::factors::Factors;
use crate::features::{self, DealtParts, ExchangeInputs, ExchangeTerms, Kernel, Leaves, Likelihood, Posterior};
use crate::reads::{CountDist, SeatFeatures};
use crate::record::Record;
use crate::table;
use crate::Table;

/// How the feature cards (aces, kings, obers) enter the card table (F6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureCards {
    /// The DP's exact posterior marginals, then the per-card factors, renormalised with the rest.
    Renormalised,
    /// The DP's exact posterior marginals, kept: factors and renormalisation move only the rest.
    Pinned,
}

/// Which signals the believed forms read (the arena A/B switch).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signals {
    /// Bids, the partner's pass and early leads: the exact base × the signal factors.
    Full,
    /// The facts only: the exact n-over-k base for cards, the feature DP without signals (and
    /// the uniform exchange) for seat features.
    BaseOnly,
}

/// A fitted part per seat kind (D-f): humans, and HP1 seats read through HP1's own encoder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerKind<T> {
    pub human: T,
    pub agent: T,
}

impl<T: Copy> PerKind<T> {
    pub fn of(&self, kind: SeatKind) -> T {
        match kind {
            SeatKind::Heuristic => self.agent,
            _ => self.human,
        }
    }
}

/// Parameters of the believed forms.
#[derive(Clone, Debug, PartialEq)]
pub struct BeliefParams {
    pub bands: Bands,
    pub signals: Signals,
    /// The convention models (encoders and readings, the trust per kind); `convention_floor` is
    /// the ε that keeps a belief from becoming a fact.
    pub conventions: conventions::Params,
    /// Per-card factors, by the kind of the seat they read.
    pub factors: PerKind<Factors>,
    /// The exchange kernel, by the kind of the passing team.
    pub kernel: PerKind<Kernel>,
    pub feature_cards: FeatureCards,
    pub calibration: Calibration,
}

impl Default for BeliefParams {
    fn default() -> Self {
        BeliefParams {
            bands: Bands::default(),
            signals: Signals::Full,
            conventions: conventions::Params::default(),
            // C32 (owner: no self-play or human-game learning in v1): no fitted card factors,
            // the uniform exchange, no calibration.
            factors: PerKind {
                human: Factors::NONE,
                agent: Factors::NONE,
            },
            kernel: PerKind {
                human: Kernel::UNIFORM,
                agent: Kernel::UNIFORM,
            },
            feature_cards: FeatureCards::Renormalised,
            calibration: Calibration::NONE,
        }
    }
}

impl BeliefParams {
    /// Check parameter domains before constructing a mind from external or modified
    /// configuration. This does not impose a workload budget or tune the model.
    pub fn validate(&self) -> Result<(), String> {
        self.conventions.validate()?;
        if !(0.0..=1.0).contains(&self.bands.low)
            || !(0.0..=1.0).contains(&self.bands.high)
            || self.bands.low > self.bands.high
        {
            return Err("bands must satisfy 0 <= low <= high <= 1".into());
        }
        for (kind, factors, kernel) in [
            ("human", &self.factors.human, &self.kernel.human),
            ("agent", &self.factors.agent, &self.kernel.agent),
        ] {
            if factors
                .pass
                .iter()
                .flatten()
                .chain(factors.lead.iter())
                .any(|&v| !v.is_finite() || v < 0.0)
            {
                return Err(format!("{kind} card factors must be finite and nonnegative"));
            }
            if kernel.stay.iter().flatten().any(|v| !(0.0..=1.0).contains(v)) {
                return Err(format!("{kind} exchange probabilities must be finite and in [0, 1]"));
            }
        }
        if self.calibration.temperature.iter().any(|&t| !t.is_finite() || t <= 0.0) {
            return Err("calibration temperatures must be finite and positive".into());
        }
        Ok(())
    }

    /// The default parameters with the facts only (`Signals::BaseOnly`).
    pub fn base_only() -> Self {
        BeliefParams {
            signals: Signals::BaseOnly,
            ..BeliefParams::default()
        }
    }
}

/// The cache keys (F3, F12): the fact epoch moves when the facts change (or on undo), the
/// signal epoch on every event other than a card play. Neither value ever repeats.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Epochs {
    pub fact: u32,
    pub signal: u64,
}

/// Everything computed once per epoch; reads are lookups into it.
#[derive(Clone, Debug)]
pub(crate) struct Derived {
    pub key: Epochs,
    pub holds: Table,
    pub counts: [[CountDist; 4]; 4],
    pub now: [SeatFeatures; 4],
    /// What the dealt features are made of (computed on their first read).
    pub dealt: DealtParts,
    pub z: f64,
}

#[derive(Default)]
pub(crate) struct Cache {
    pub concepts: Option<(Epochs, u32, crate::concepts::ConceptStore)>,
    pub perspective_facts: [Option<(Epochs, crate::perspective::ViewerFacts)>; 4],
    /// The exact base, keyed by the fact epoch.
    pub base: Option<(u32, Table)>,
    /// The signal record and each seat's dealt-class likelihood, keyed by the signal epoch.
    /// Keyed by both epochs: `bluffing` reads the facts too.
    pub signals: Option<(Epochs, Record, [Likelihood; 4])>,
    /// The hidden exchange's terms, keyed by what they depend on: the passers, their bid
    /// likelihoods and the kernel (so questions, answers and card plays reuse them).
    pub exchange: Option<(ExchangeInputs, ExchangeTerms)>,
    /// The dealt features, keyed by both epochs (computed on the first read).
    pub dealt: Option<(Epochs, [SeatFeatures; 4])>,
    /// The feature DP's leaves, keyed by the fact epoch (reused across bids).
    pub leaves: Option<(u32, Leaves)>,
    pub derived: Option<Derived>,
    /// Per viewed seat j: (public fact epoch, my epochs) and j's card table.
    pub views: [Option<((u32, Epochs), Table)>; 4],
}

/// One agent seat's mind: its own facts and events, the public facts, and the believed forms
/// computed from them. Built only from the seat's start hand and its own events (privacy).
/// `Send`, not `Sync` (lazy caches).
pub struct Mind {
    pub(crate) seat: Seat,
    pub(crate) own: Tracker,
    pub(crate) public: Tracker,
    /// The seat's events with undone ones removed: `Tracker` decides every undo, and the event
    /// it retracts is always the last one applied (the only undo owner).
    pub(crate) events: Vec<SeatEvent>,
    pub(crate) kinds: [SeatKind; 4],
    pub(crate) params: BeliefParams,
    signal: u64,
    next_signal: u64,
    pub(crate) cache: RefCell<Cache>,
    /// Independent borrow owner: model reads can coexist with concept cache updates.
    pub(crate) model: RefCell<Option<(Epochs, u32, crate::DealModel)>>,
    /// Shared only by exact marginals and the joint model of this Mind.
    pub(crate) fact_space: RefCell<Option<((u32, u32), std::sync::Arc<crate::deals::FactSpace>)>>,
}

impl Mind {
    /// Construct from trusted parameters; validate externally supplied or modified
    /// settings with `BeliefParams::validate` first.
    pub fn new(seat: Seat, hand: CardSet, kinds: [SeatKind; 4], params: BeliefParams) -> Mind {
        Mind {
            seat,
            own: Tracker::new(seat, hand),
            public: Tracker::public(),
            events: vec![],
            kinds,
            params,
            signal: 0,
            next_signal: 1,
            cache: RefCell::new(Cache::default()),
            model: RefCell::new(None),
            fact_space: RefCell::new(None),
        }
    }

    /// Applies one of the seat's own events (`observation::SeatFilter`).
    pub fn apply(&mut self, event: &SeatEvent) -> Result<(), Contradiction> {
        self.own.apply(event)?;
        let public_event = match *event {
            SeatEvent::Pass { from, to, .. } => SeatEvent::Pass { from, to, cards: None },
            ref e => e.clone(),
        };
        self.public.apply(&public_event)?;
        if *event == SeatEvent::Undone {
            self.events.pop();
        } else {
            self.events.push(event.clone());
        }
        if !matches!(event, SeatEvent::CardPlayed { .. }) {
            self.signal = self.next_signal;
            self.next_signal += 1;
        }
        Ok(())
    }

    pub fn seat(&self) -> Seat {
        self.seat
    }

    /// The facts and their known forms (K1–K6).
    pub fn knowledge(&self) -> &Knowledge {
        self.own.knowledge()
    }

    /// The public facts (what every seat knows).
    pub fn public(&self) -> &Knowledge {
        self.public.knowledge()
    }

    /// The seat's own event stream (bids, passes, plays, questions, …), undone events removed.
    pub fn events(&self) -> &[SeatEvent] {
        &self.events
    }

    /// Who plays each seat (public table metadata, M3 P3).
    pub fn kinds(&self) -> [SeatKind; 4] {
        self.kinds
    }

    pub fn params(&self) -> &BeliefParams {
        &self.params
    }

    pub fn epochs(&self) -> Epochs {
        Epochs {
            fact: self.own.fact_epoch(),
            signal: self.signal,
        }
    }

    pub fn band(&self, p: f32) -> Band {
        Band::of(p, self.params.bands)
    }

    /// The signal record (auction, passes, melds, questions per seat), cached per signal epoch.
    pub fn record(&self) -> Ref<'_, Record> {
        self.signals();
        Ref::map(self.cache.borrow(), |c| &c.signals.as_ref().expect("computed above").1)
    }

    pub(crate) fn signals(&self) {
        let key = self.epochs();
        if self.cache.borrow().signals.as_ref().map(|s| s.0) == Some(key) {
            return;
        }
        let p = &self.params.conventions;
        let record = Record::new(&self.events, self.kinds, p);
        let lik = std::array::from_fn(|y| {
            let y = y as Seat;
            if y == self.seat {
                return None;
            }
            self.seat_likelihood(&record, y, self.seat)
        });
        self.cache.borrow_mut().signals = Some((key, record, lik));
    }

    /// The auction's likelihood of `y`'s dealt-hand class as `reader` reads it: `y`'s steps by
    /// the convention, trusted by the reader's relation to `y` (partner or opponent), at the
    /// floor for a bluffing seat. Public: only the bids. `None` without bids.
    fn seat_likelihood(&self, record: &Record, y: Seat, reader: Seat) -> Option<Box<[f32; N_CLASSES]>> {
        let p = &self.params.conventions;
        let s = &record.seats[y as usize];
        if s.bids.is_empty() || self.params.signals == Signals::BaseOnly {
            return None;
        }
        // KB `bluffing(y)`: a seat whose step has no support under the facts is read at the
        // floor ε for the rest of the game ([O] Q1b).
        let trust = if self.bluffs(record, y) {
            p.convention_floor
        } else {
            p.trust(s.kind, y == engine::partner(reader))
        };
        let mut l = [1.0f64; N_CLASSES];
        for obs in &s.bids {
            for (x, t) in l.iter_mut().zip(bid_table(obs, trust, p)) {
                *x *= t as f64;
            }
        }
        let max = l.iter().cloned().fold(0.0, f64::max);
        (max > 0.0).then(|| Box::new(l.map(|x| (x / max) as f32)))
    }

    /// `seat`'s dealt-hand class likelihood as `reader` reads the auction (any seat, mine
    /// included: my partner reads me too). For simulating another seat's view.
    pub fn class_likelihood(&self, seat: Seat, reader: Seat) -> Option<Box<[f32; N_CLASSES]>> {
        self.signals();
        let cache = self.cache.borrow();
        let record = &cache.signals.as_ref().expect("computed above").1;
        self.seat_likelihood(record, seat, reader)
    }

    /// KB `bluffing(y)`: some step of `y` carries a message that no hand class of `y`'s dealt
    /// hand can hold under the facts (e.g. "ace" while `y` can have been dealt no ace; "pair"
    /// while a half of every such pair was certainly dealt elsewhere).
    fn bluffs(&self, record: &Record, y: Seat) -> bool {
        let k = self.knowledge();
        let (certain, possible) = (k.dealt_certain(y), k.dealt_possible(y));
        let fits = |c: HandClass| {
            let aces =
                (c.ace || (certain & CardSet::ACES).is_empty()) && (!c.ace || !(possible & CardSet::ACES).is_empty());
            aces && SUITS.iter().all(|&s| {
                let h = c.halves[s as usize] as u32;
                (certain & CardSet::halves(s)).len() <= h && h <= (possible & CardSet::halves(s)).len()
            })
        };
        let p = &self.params.conventions;
        record.seats[y as usize].bids.iter().any(|obs| {
            let Some(m) = obs.nominal else { return false };
            let in_window = p.tell_window == 0 || obs.ctx.n_before < p.tell_window;
            !(0..N_CLASSES)
                .map(HandClass::from_index)
                .any(|c| fits(c) && m.holds(c, p.strict_two_halves, in_window))
        })
    }

    /// KB `bluffing(y)` for the current facts and signals.
    pub fn bluffing(&self, y: Seat) -> bool {
        y != self.seat && self.bluffs(&self.record(), y)
    }

    /// The exact base table (facts only), cached per fact epoch.
    pub(crate) fn base(&self) -> Table {
        let epoch = self.own.fact_epoch();
        if let Some((e, t)) = self.cache.borrow().base {
            if e == epoch {
                return t;
            }
        }
        let _timing = crate::profile::scope("belief.base");
        let facts = self.fact_space();
        let t = table::base(facts.space());
        self.cache.borrow_mut().base = Some((epoch, t));
        t
    }

    /// The believed forms of this epoch, computed on the first read.
    pub(crate) fn derived(&self) -> Ref<'_, Derived> {
        let key = self.epochs();
        if self.cache.borrow().derived.as_ref().map(|d| d.key) != Some(key) {
            let d = self.compute(key);
            self.cache.borrow_mut().derived = Some(d);
        }
        Ref::map(self.cache.borrow(), |c| c.derived.as_ref().expect("computed above"))
    }

    /// The feature DP of this epoch (also for the study).
    pub(crate) fn posterior(&self) -> Posterior {
        let _timing = crate::profile::scope("belief.posterior");
        self.signals();
        let fact = self.own.fact_epoch();
        let leaves = match self.cache.borrow_mut().leaves.take() {
            Some((epoch, leaves)) if epoch == fact => leaves,
            _ => Leaves::new(self.knowledge()),
        };
        let k = self.knowledge();
        let kernel = match (self.params.signals, k.first_passer()) {
            (Signals::Full, Some(m)) => self.params.kernel.of(self.kinds[m as usize]),
            _ => Kernel::UNIFORM,
        };
        let inputs = {
            let cache = self.cache.borrow();
            let lik = &cache.signals.as_ref().expect("computed above").2;
            ExchangeInputs::of(k, lik, kernel)
        };
        let mut terms = match self.cache.borrow_mut().exchange.take() {
            Some((i, terms)) if i == inputs => terms,
            _ => ExchangeTerms::default(),
        };
        let post = {
            let cache = self.cache.borrow();
            let lik = &cache.signals.as_ref().expect("computed above").2;
            features::posterior(&leaves, lik, kernel, &mut terms)
        };
        let mut cache = self.cache.borrow_mut();
        cache.exchange = Some((inputs, terms));
        cache.leaves = Some((fact, leaves));
        post
    }

    /// The dealt features of this epoch, computed on the first read: the passers of a hidden
    /// exchange need every term's class masses, which nothing else reads.
    pub(crate) fn dealt(&self) -> [SeatFeatures; 4] {
        let key = self.epochs();
        if let Some((e, f)) = self.cache.borrow().dealt {
            if e == key {
                return f;
            }
        }
        let (parts, z) = {
            let d = self.derived();
            (d.dealt.clone(), d.z)
        };
        let cal = match self.params.signals {
            Signals::Full => self.params.calibration,
            Signals::BaseOnly => Calibration::NONE,
        };
        let mut cache = self.cache.borrow_mut();
        let (inputs, terms) = cache.exchange.as_mut().expect("computed with the epoch");
        let m = features::dealt(&parts, inputs, terms);
        let f = std::array::from_fn(|y| SeatFeatures::of(&m[y], z, &cal));
        cache.dealt = Some((key, f));
        f
    }

    fn compute(&self, key: Epochs) -> Derived {
        let _timing = crate::profile::scope("belief.derived");
        let k = self.knowledge();
        let post = self.posterior();
        let holds = self.card_table(&post);
        let counts = std::array::from_fn(|x| {
            std::array::from_fn(|s| {
                let suit = SUITS[s];
                CountDist {
                    p: table::count_dist(
                        &holds[x],
                        CardSet::suit(suit) - k.played(),
                        k.count_bounds(x as Seat, suit),
                    ),
                }
            })
        });
        let cal = match self.params.signals {
            Signals::Full => &self.params.calibration,
            Signals::BaseOnly => &Calibration::NONE,
        };
        Derived {
            key,
            holds,
            counts,
            now: std::array::from_fn(|y| SeatFeatures::of(&post.now[y], post.z, cal)),
            dealt: post.dealt,
            z: post.z,
        }
    }

    /// F4: the exact base, the DP's posterior on the feature cards (F6), the per-card factors,
    /// renormalised. Facts (0 and 1) are never moved.
    fn card_table(&self, post: &Posterior) -> Table {
        let mut t = self.base();
        if post.z <= 0.0 || self.params.signals == Signals::BaseOnly {
            return t;
        }
        let open = table::open_cells(&t);
        for x in 0..4 {
            for c in (open[x] & post.split).iter() {
                t[x][c as usize] = table::belief(post.cards[x][c as usize]);
            }
        }
        let pinned = match self.params.feature_cards {
            FeatureCards::Renormalised => CardSet::EMPTY,
            FeatureCards::Pinned => post.split,
        };
        let movable = open.map(|o| o - pinned);
        let f = Factors::table(
            &self.params.factors,
            self.knowledge(),
            self.kinds,
            self.params.conventions.convention_floor,
        );
        for x in 0..4 {
            for c in movable[x].iter() {
                t[x][c as usize] *= f[x][c as usize];
            }
        }
        table::renormalise(&mut t, table::sizes(self.knowledge()), movable);
        t
    }
}

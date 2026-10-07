//! Game statistics computed the same way for any source (HP1 self-play, the human games),
//! from the engine's `GameFinishedInfo`, for side-by-side comparison (C14).

use std::collections::BTreeMap;

use engine::GameFinishedInfo;
use engine::{ActionType, GameCallback};
use engine::{CardSet, SUITS};

use crate::stats::{mean_ci, wilson};

#[derive(Default, Clone)]
pub struct GameStats {
    /// The bid barriers that bucket the game values (the caller's convention, e.g.
    /// `beliefs::conventions::BARRIERS`); none: every value is its own bucket.
    pub barriers: &'static [i32],
    pub games: u64,
    pub pass_games: u64,
    pub contracts: u64,
    pub made: u64,
    pub schwarz: u64,
    pub raised: u64,
    /// Opening action of the game (seat 0's first action), as "pass" or the bid.
    pub opening: BTreeMap<String, u64>,
    /// First step of each seat that bid at all ("pass", "+5", …).
    pub first_step: BTreeMap<String, u64>,
    /// Final game value of games with a contract, bucketed.
    pub value: BTreeMap<Bucket, u64>,
    pub values: Vec<f64>,
    pub raise_amounts: Vec<f64>,
    pub declarer_points: Vec<f64>,
    pub defender_points: Vec<f64>,
    pub declarer_tricks: Vec<f64>,
    pub melds_per_game: Vec<f64>,
    pub pair_points_per_game: Vec<f64>,
    pub questions_per_game: Vec<f64>,
    /// Trick number (1-9) in which the first meld of the game happened.
    pub first_meld_trick: BTreeMap<u8, u64>,
    /// Shape of the 4 cards passed forth.
    pub forth_shape: BTreeMap<&'static str, u64>,
    /// Suits MH is void in right after its pass forth.
    pub mh_voids: BTreeMap<u32, u64>,
    /// The first card of the game: an ace, a green card, or other.
    pub first_lead: BTreeMap<&'static str, u64>,
}

pub fn shape(x: CardSet) -> &'static str {
    let mut lens: Vec<u32> = SUITS
        .iter()
        .map(|&s| (x & CardSet::suit(s)).len())
        .filter(|&l| l > 0)
        .collect();
    lens.sort_by(|a, b| b.cmp(a));
    match lens.as_slice() {
        [4] => "4",
        [3, 1] => "3-1",
        [2, 2] => "2-2",
        [2, 1, 1] => "2-1-1",
        _ => "1-1-1-1",
    }
}

/// A game-value bucket: ordered by its lowest value, shown by its label.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Bucket(i32, String);

impl std::fmt::Display for Bucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.1)
    }
}

/// "< first", each barrier on its own, the ranges between, and "> last".
fn value_bucket(v: i32, barriers: &[i32]) -> Bucket {
    let (Some(&first), Some(&last)) = (barriers.first(), barriers.last()) else {
        return Bucket(v, v.to_string());
    };
    if v < first {
        return Bucket(i32::MIN, format!("< {first}"));
    }
    if v > last {
        return Bucket(last + 1, format!("> {last}"));
    }
    if barriers.contains(&v) {
        return Bucket(v, v.to_string());
    }
    let hi = barriers.iter().find(|&&b| b > v).copied().unwrap_or(last);
    let lo = barriers.iter().rev().find(|&&b| b < v).copied().unwrap_or(first);
    Bucket(lo + 1, format!("{lo} < v < {hi}"))
}

impl GameStats {
    /// Empty statistics bucketing game values at `barriers`.
    pub fn with_barriers(barriers: &'static [i32]) -> GameStats {
        GameStats {
            barriers,
            ..GameStats::default()
        }
    }

    pub fn add(&mut self, info: &GameFinishedInfo) {
        self.games += 1;
        let start: [CardSet; 4] = std::array::from_fn(|s| CardSet::from_cards(&info.info.player_start_cards[s]));
        let mut value = engine::NO_BID.0;
        let mut acted = [false; 4];
        let mut passed = false;
        let mut auction_top = engine::NO_BID.0;
        let (mut melds, mut questions, mut cards) = (0u32, 0u32, 0u32);
        let mut pair_points = 0i32;
        for e in &info.all_events {
            let seat = e.last_action.player.0 as usize;
            match &e.last_action.action_type {
                ActionType::NewBid(v) if !passed => {
                    let label = format!("+{}", v - value);
                    if !acted[seat] {
                        *self.first_step.entry(label).or_default() += 1;
                        if seat == 0 {
                            *self.opening.entry(v.to_string()).or_default() += 1;
                        }
                    }
                    acted[seat] = true;
                    value = *v;
                    auction_top = *v;
                }
                ActionType::StopBidding if !passed => {
                    if !acted[seat] {
                        *self.first_step.entry("pass".into()).or_default() += 1;
                        if seat == 0 {
                            *self.opening.entry("pass".into()).or_default() += 1;
                        }
                    }
                    acted[seat] = true;
                }
                ActionType::NewBid(v) => {
                    self.raised += 1;
                    self.raise_amounts.push((v - auction_top) as f64);
                }
                ActionType::Pass(x) => {
                    if !passed {
                        let x = CardSet::from_cards(x);
                        *self.forth_shape.entry(shape(x)).or_default() += 1;
                        let after = start[seat] - x;
                        let voids = SUITS.iter().filter(|&&s| (after & CardSet::suit(s)).is_empty()).count() as u32;
                        *self.mh_voids.entry(voids).or_default() += 1;
                    }
                    passed = true;
                }
                ActionType::CardPlayed(c) => {
                    if cards == 0 {
                        let label = if c.value == engine::Value::Ace {
                            "ace"
                        } else if c.suit == engine::Suit::Green {
                            "green, no ace"
                        } else {
                            "other"
                        };
                        *self.first_lead.entry(label).or_default() += 1;
                    }
                    cards += 1;
                }
                ActionType::Question(_) => questions += 1,
                _ => {}
            }
            if let Some(GameCallback::NewTrump(s)) = e.callback {
                pair_points += engine::PAIR_POINTS[s as usize];
                if melds == 0 {
                    *self.first_meld_trick.entry((cards / 4 + 1) as u8).or_default() += 1;
                }
                melds += 1;
            }
        }
        self.melds_per_game.push(melds as f64);
        self.pair_points_per_game.push(pair_points as f64);
        self.questions_per_game.push(questions as f64);
        match &info.playing_party {
            None => self.pass_games += 1,
            Some(party) => {
                let s = party.0 as usize % 2;
                self.contracts += 1;
                self.made += info.won.unwrap_or(false) as u64;
                self.values.push(info.game_value.0 as f64);
                *self
                    .value
                    .entry(value_bucket(info.game_value.0, self.barriers))
                    .or_default() += 1;
                self.declarer_points.push(info.team_points[s] as f64);
                self.defender_points.push(info.team_points[1 - s] as f64);
                let tricks = info.tricks.iter().filter(|t| t.winner.0 as usize % 2 == s).count();
                self.declarer_tricks.push(tricks as f64);
            }
        }
        self.schwarz += info.schwarz_game as u64;
    }
}

fn pct(k: u64, n: u64) -> String {
    let (lo, hi) = wilson(k, n);
    format!(
        "{:.1} % [{:.1}, {:.1}]",
        100.0 * k as f64 / n.max(1) as f64,
        100.0 * lo,
        100.0 * hi
    )
}

fn mean(xs: &[f64]) -> String {
    let (m, ci) = mean_ci(xs);
    format!("{m:.1} ± {ci:.1}")
}

fn dist<K: Ord + std::fmt::Display + Clone>(a: &BTreeMap<K, u64>, b: &BTreeMap<K, u64>, out: &mut String, title: &str) {
    let na: u64 = a.values().sum();
    let nb: u64 = b.values().sum();
    out.push_str(&format!("\n{title}\n\n| | A | B |\n|---|---|---|\n"));
    let mut keys: Vec<K> = a.keys().chain(b.keys()).cloned().collect();
    keys.sort();
    keys.dedup();
    for k in keys {
        let (x, y) = (a.get(&k).copied().unwrap_or(0), b.get(&k).copied().unwrap_or(0));
        out.push_str(&format!(
            "| {k} | {:.1} % ({x}) | {:.1} % ({y}) |\n",
            100.0 * x as f64 / na.max(1) as f64,
            100.0 * y as f64 / nb.max(1) as f64
        ));
    }
}

/// A markdown comparison of two sources (A, B) with 95 % intervals.
pub fn compare(a: &GameStats, b: &GameStats, name_a: &str, name_b: &str) -> String {
    let mut out = format!(
        "A = {name_a} ({} games), B = {name_b} ({} games).\n\n",
        a.games, b.games
    );
    out.push_str("| | A | B |\n|---|---|---|\n");
    let rows: Vec<(&str, String, String)> = vec![
        (
            "all passed (no contract)",
            pct(a.pass_games, a.games),
            pct(b.pass_games, b.games),
        ),
        ("contract made", pct(a.made, a.contracts), pct(b.made, b.contracts)),
        ("schwarz", pct(a.schwarz, a.games), pct(b.schwarz, b.games)),
        (
            "raised after the exchange",
            pct(a.raised, a.contracts),
            pct(b.raised, b.contracts),
        ),
        (
            "raise amount when raised",
            mean(&a.raise_amounts),
            mean(&b.raise_amounts),
        ),
        ("game value (contracts)", mean(&a.values), mean(&b.values)),
        ("declarers' points", mean(&a.declarer_points), mean(&b.declarer_points)),
        ("defenders' points", mean(&a.defender_points), mean(&b.defender_points)),
        ("declarers' tricks", mean(&a.declarer_tricks), mean(&b.declarer_tricks)),
        ("melds per game", mean(&a.melds_per_game), mean(&b.melds_per_game)),
        (
            "pair points per game",
            mean(&a.pair_points_per_game),
            mean(&b.pair_points_per_game),
        ),
        (
            "questions per game",
            mean(&a.questions_per_game),
            mean(&b.questions_per_game),
        ),
    ];
    for (k, x, y) in rows {
        out.push_str(&format!("| {k} | {x} | {y} |\n"));
    }
    dist(
        &a.opening,
        &b.opening,
        &mut out,
        "Opening action (seat 0's first action):",
    );
    dist(&a.first_step, &b.first_step, &mut out, "Each seat's first step:");
    dist(
        &a.value,
        &b.value,
        &mut out,
        "Final game value (games with a contract):",
    );
    dist(
        &a.first_meld_trick,
        &b.first_meld_trick,
        &mut out,
        "Trick of the first meld (games with a meld):",
    );
    dist(&a.forth_shape, &b.forth_shape, &mut out, "Shape of the pass forth:");
    dist(
        &a.mh_voids,
        &b.mh_voids,
        &mut out,
        "MH's void suits right after its pass forth:",
    );
    dist(&a.first_lead, &b.first_lead, &mut out, "First card of the game:");
    out
}

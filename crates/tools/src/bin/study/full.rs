//! `study full` (C38): at every human decision of every game (bid, raise, pass forth, pass
//! back, question, answer, announcement, card), HP1 is asked once for its action from that
//! seat's view at that moment: one step, counterfactual, the game goes on with the human's
//! action. Writes `decisions.ndjson` and `games.ndjson` (no names) to the output directory and
//! returns the Markdown overview. Only games that replay to the end are compared.
//!
//! Speed without a change in decisions (checked by the decision hash): one `Mind` per seat
//! per game, read on event by event; games in parallel, each on its own thread's four agents,
//! whose endgame solvers keep their transposition tables (keyed by the whole position, so
//! exact) from game to game; HP1's facts are written out only for disagreements. The concept
//! values are read from a separate agent per seat, so the deciding agent sees exactly the calls
//! of a game of its own. `Fresh` switches the shortcuts off to check them by the decision hash.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use beliefs::conventions::{follow_target, info_to_tell, next_context, HandClass, Message};
use beliefs::Mind;
use engine::{suit_of, ActionType, CardSet, GameFinishedInfo, Suit, SUITS};
use heuristic::{HeuristicAgent, Params, RuleId};
use knowledge::Knowledge;
use observation::Seat;
use serde_json::{json, Map, Value};
use tools::legacy::{meta, replay, same, seat_groups};

use crate::bidding::{action_key, band, bid_of, dist, hand_class};

const SEATS: [&str; 4] = ["N", "E", "S", "W"];
const PHASES: [&str; 9] = [
    "first bid",
    "later bid",
    "pass forth",
    "pass back",
    "raise",
    "trump or lead",
    "answer",
    "lead",
    "follow",
];
const ROLES: [&str; 5] = ["VH", "MH", "defender after VH", "defender before VH", "no game"];

/// How the agents are kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fresh {
    /// The fast way: four agents per thread for all its games, one `Mind` per seat per game.
    No,
    /// New agents for every game (the solvers' tables start empty).
    Game,
    /// A new agent at every decision: its `Mind` is built from the start hand and all the
    /// seat's events, its solver starts empty, and nothing else ever reads it.
    Decision,
}

pub struct Opts {
    pub out: std::path::PathBuf,
    pub threads: usize,
    pub fresh: Fresh,
}

/// What `run` measured besides the overview.
pub struct Done {
    pub overview: String,
    pub hash: u64,
    pub games: usize,
    pub decisions: u64,
    /// Phase → (decisions, the seconds HP1 took to decide them, summed over the threads).
    pub decide_s: BTreeMap<&'static str, (u64, f64)>,
}

/// FNV-1a, 64 bits: the decision hash.
#[derive(Clone, Copy)]
struct Fnv(u64);

impl Fnv {
    fn new() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 = (self.0 ^ x as u64).wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn u64(&mut self, x: u64) {
        self.bytes(&x.to_le_bytes());
    }
}

/// One compared decision, for the overview (its NDJSON line holds the rest).
struct Row {
    step: usize,
    phase: &'static str,
    seat: Seat,
    agree: bool,
    /// Only one legal action (HP1's HX-1).
    forced: bool,
    rule: &'static str,
    /// The situation cell; `with_role` puts the seat's role in front once the game is over.
    cell: String,
    with_role: bool,
    /// The human's and HP1's action by the phase's measure, for the cell's distributions.
    human_key: String,
    hp1_key: String,
    human: String,
    hp1: String,
    /// HP1's facts, only on a disagreement.
    facts: Option<String>,
    /// The time HP1 took to decide.
    nanos: u64,
}

struct GameOut {
    id: usize,
    era: &'static str,
    groups: [&'static str; 4],
    roles: [&'static str; 4],
    rows: Vec<Row>,
    lines: String,
    summary: String,
    hash: u64,
}

pub(crate) fn era(month: &str) -> &'static str {
    if month <= "2023-12" {
        "≤ 2023"
    } else {
        "≥ 2024"
    }
}

/// The seat's role in the game that was played: VH (the declarer), MH (the declarer's
/// partner), a defender, or no game (all passed).
fn role(declarer: Option<Seat>, seat: Seat) -> &'static str {
    match declarer {
        None => ROLES[4],
        Some(d) if seat == d => ROLES[0],
        Some(d) if seat == (d + 2) % 4 => ROLES[1],
        Some(d) if seat == (d + 1) % 4 => ROLES[2],
        Some(_) => ROLES[3],
    }
}

fn team(role: &str) -> &'static str {
    match role {
        "VH" | "MH" => "declarer team",
        "no game" => "no game",
        _ => "defenders",
    }
}

fn text(a: &ActionType) -> String {
    match a {
        ActionType::NewBid(v) => v.to_string(),
        ActionType::StopBidding => "pass".into(),
        ActionType::Pass(cards) => cards.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" "),
        ActionType::CardPlayed(c) => c.to_string(),
        ActionType::Question(q) => format!("question {q:?}"),
        ActionType::Answer(x) => format!("answer {x:?}"),
        ActionType::AnnounceTrump(s) => format!("trump {s}"),
        other => format!("{other:?}"),
    }
}

fn kind(a: &ActionType) -> &'static str {
    match a {
        ActionType::NewBid(_) => "bid",
        ActionType::StopBidding => "pass",
        ActionType::Pass(_) => "cards passed",
        ActionType::CardPlayed(_) => "card",
        ActionType::Question(_) => "question",
        ActionType::Answer(_) => "answer",
        ActionType::AnnounceTrump(_) => "trump",
        _ => "other",
    }
}

/// beliefs' value classes: A, 10, K/O, the rest.
fn value_class(card: u8) -> usize {
    beliefs::factors::value_class(card)
}

/// The suits the passer holds and has none of after passing `passed`.
fn made_blank(hand: CardSet, passed: CardSet) -> Vec<Suit> {
    SUITS
        .into_iter()
        .filter(|&s| !(hand & CardSet::suit(s)).is_empty() && ((hand - passed) & CardSet::suit(s)).is_empty())
        .collect()
}

/// A fact for the NDJSON, to 4 decimals (the hash takes the exact bits).
fn round(x: f32) -> f64 {
    (x as f64 * 1e4).round() / 1e4
}

fn trick_band(trick: usize) -> &'static str {
    match trick {
        1 => "trick 1",
        2..=5 => "tricks 2-5",
        _ => "tricks 6-9",
    }
}

/// The start hand's features (from the cards alone).
fn features(seat: Seat, hand: CardSet) -> Value {
    let ace_ten = SUITS
        .into_iter()
        .filter(|&s| {
            let suit = CardSet::suit(s);
            !(hand & suit & CardSet::ACES).is_empty() && !(hand & suit & CardSet::TENS).is_empty()
        })
        .count();
    let pairs = SUITS
        .into_iter()
        .filter(|&s| CardSet::halves(s).is_subset(hand))
        .count();
    json!({
        "seat": SEATS[seat as usize],
        "aces": (hand & CardSet::ACES).len(),
        "tens": (hand & CardSet::TENS).len(),
        "ace_ten": ace_ten,
        "halves": (hand & CardSet::HALVES).len(),
        "pairs": pairs,
        "standing_against_us": Knowledge::new(seat, hand).standing_against_us(seat),
    })
}

fn message_name(m: Message) -> &'static str {
    match m {
        Message::Ace => "ace",
        Message::TwoHalves => "two halves",
        Message::SmallOrHalves => "small pair or 3+ halves",
        Message::BigPair => "big pair",
        Message::RedPair => "red pair",
    }
}

/// The bidding concept values of the seat's `Mind` (C38 addendum): the message B-tell would
/// send first and its bid, the messages so far as read, the team's pairs, the hand's strength.
fn concepts(o: &mut Map<String, Value>, mind: &Mind, conv: &beliefs::conventions::Params) {
    let me = mind.seat();
    let k = mind.knowledge();
    let hand = k.certain(me);
    let (tell, open, read, value) = {
        let record = mind.record();
        let bctx = next_context(&record.auction, me, conv);
        let tell = info_to_tell(HandClass::of(hand), &bctx, conv)
            .next()
            .map(|m| (m, follow_target(m, &bctx, conv)));
        let read: [Vec<String>; 4] = std::array::from_fn(|y| {
            record.seats[y]
                .bids
                .iter()
                .map(|b| match (b.bid, b.nominal) {
                    (None, _) => "pass".to_string(),
                    (Some(_), Some(m)) => message_name(m).to_string(),
                    (Some(v), None) => format!("unread {v}"),
                })
                .collect()
        });
        (tell, bctx.info_channel_open(), read, bctx.value)
    };
    o.insert("tell_message".into(), json!(tell.map(|t| message_name(t.0))));
    o.insert("tell_bid".into(), json!(tell.map(|t| t.1)));
    o.insert("tell_step".into(), json!(tell.map(|t| t.1 - value)));
    o.insert("info_channel_open".into(), json!(open));
    o.insert("partner_read".into(), json!(read[((me + 2) % 4) as usize]));
    o.insert("left_read".into(), json!(read[((me + 1) % 4) as usize]));
    o.insert("right_read".into(), json!(read[((me + 3) % 4) as usize]));
    for v in [140, 145, 150] {
        o.insert(format!("team_pair_cover_{v}"), json!(round(mind.team_pair_cover(v))));
    }
    let pairs: Map<String, Value> = SUITS
        .into_iter()
        .map(|s| (s.to_string(), json!(round(mind.team_pair(s)))))
        .collect();
    o.insert("team_pair".into(), Value::Object(pairs));
    o.insert("own_standing".into(), json!(k.own_standing(me).len()));
    o.insert("aces".into(), json!((hand & CardSet::ACES).len()));
    o.insert(
        "ace_ten".into(),
        json!(SUITS.into_iter().filter(|&s| k.ace_ten(me, s)).count()),
    );
    o.insert("standing_against_us".into(), json!(k.standing_against_us(me)));
    o.insert("trick_confidence".into(), json!(round(k.trick_confidence(me))));
}

/// One game. The agents' `Mind`s start afresh by themselves: a new start hand is a new game.
fn game(id: usize, rec: &Value, players: &mut [HeuristicAgent; 4], fresh: Fresh) -> Option<GameOut> {
    let m = meta(rec).ok()?;
    let era = era(&m.month);
    let groups = seat_groups(rec);
    let params = players[0].params.clone();
    let conv = params.conv().clone();
    let mut rows: Vec<Row> = vec![];
    let mut lines: Vec<Map<String, Value>> = vec![];
    let mut hash = Fnv::new();
    let mut step = 0usize;
    let mut first: Option<Seat> = None;
    // The concept values come from a reader per seat, never from the deciding agent, whose
    // `Mind` is so touched only by `decide`, as in a game of its own.
    let mut readers: [HeuristicAgent; 4] = std::array::from_fn(|_| HeuristicAgent::new(params.clone()));
    let r = replay(rec, |s| {
        let at = step;
        step += 1;
        let state = &s.table.game.state;
        let seat = s.seat as usize;
        let view = s.table.view(s.seat);
        if fresh == Fresh::Decision {
            players[seat] = HeuristicAgent::new(params.clone());
        }
        let t0 = std::time::Instant::now();
        let d = players[seat].decide(&view, s.legal);
        let nanos = t0.elapsed().as_nanos() as u64;
        let agree = same(&d.action, s.human);
        let (ht, at_) = (&s.human.action_type, &d.action.action_type);
        let (human, hp1) = (text(ht), text(at_));
        hash.u64(at as u64);
        hash.bytes(hp1.as_bytes());
        hash.bytes(d.rule.code().as_bytes());
        for f in &d.why {
            hash.bytes(f.key.as_bytes());
            hash.u64(f.value.to_bits() as u64);
        }
        let mut o = Map::new();
        o.insert("game".into(), json!(id));
        o.insert("step".into(), json!(at));
        o.insert("phase".into(), json!(s.phase));
        o.insert("seat".into(), json!(SEATS[seat]));
        o.insert("group".into(), json!(groups[seat]));
        o.insert("era".into(), json!(era));
        o.insert("human".into(), json!(human));
        o.insert("hp1".into(), json!(hp1));
        o.insert("agree".into(), json!(agree));
        o.insert("rule".into(), json!(d.rule.code()));
        let forced = d.rule == RuleId::HX1;
        o.insert("forced".into(), json!(forced));
        let bidding = matches!(s.phase, "first bid" | "later bid");
        if bidding && first.is_none() {
            first = Some(s.seat);
        }
        let (mut human_key, mut hp1_key) = (kind(ht).to_string(), kind(at_).to_string());
        let (cell, with_role) = if bidding {
            let value = state.value.0;
            let mate = (s.seat + 2) % 4;
            let partner = match state.bidding_history.iter().rev().find(|(_, p)| p.0 == mate) {
                None => "none",
                Some((ActionType::NewBid(_), _)) => "step",
                Some(_) => "pass",
            };
            let opp: Vec<_> = state
                .bidding_history
                .iter()
                .filter(|(_, p)| p.0 % 2 != s.seat % 2)
                .collect();
            let opponents = if opp.iter().any(|(t, _)| matches!(t, ActionType::NewBid(_))) {
                "step"
            } else if opp.is_empty() {
                "none"
            } else {
                "pass"
            };
            let pos = (s.seat + 4 - first.unwrap_or(s.seat)) % 4 + 1;
            let class = hand_class(s.table.hand(s.seat));
            o.insert("value".into(), json!(value));
            o.insert("pos".into(), json!(pos));
            o.insert("partner".into(), json!(partner));
            o.insert("opponents".into(), json!(opponents));
            o.insert("hand_class".into(), json!(class));
            concepts(&mut o, readers[seat].mind(&view), &conv);
            human_key = action_key(bid_of(ht), value);
            hp1_key = action_key(bid_of(at_), value);
            (
                format!(
                    "pos {pos} | partner {partner} | opponents {opponents} | {class} | {}",
                    band(value)
                ),
                false,
            )
        } else if let (ActionType::Pass(h), ActionType::Pass(a)) = (ht, at_) {
            let hand = s.table.hand(s.seat);
            let (h, a) = (CardSet::from_cards(h), CardSet::from_cards(a));
            let overlap = (h & a).len();
            let (hb, ab) = (made_blank(hand, h), made_blank(hand, a));
            o.insert("overlap".into(), json!(overlap));
            o.insert("same_blank".into(), json!(hb == ab));
            o.insert(
                "human_blank".into(),
                json!(hb.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
            );
            o.insert(
                "hp1_blank".into(),
                json!(ab.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
            );
            human_key = format!("overlap {overlap}");
            hp1_key = if hb == ab {
                "same blank suits"
            } else {
                "other blank suits"
            }
            .into();
            (d.rule.code().to_string(), true)
        } else {
            let trick = state.all_tricks.len() + 1;
            o.insert("trick".into(), json!(trick));
            o.insert("endgame".into(), json!(d.rule == RuleId::HE1));
            if let (ActionType::CardPlayed(h), ActionType::CardPlayed(a)) = (ht, at_) {
                let (h, a) = (h.index(), a.index());
                let same_suit = suit_of(h) == suit_of(a);
                let same_class = value_class(h) == value_class(a);
                o.insert("same_suit".into(), json!(same_suit));
                o.insert("same_value_class".into(), json!(same_class));
                human_key = match (h == a, same_suit, same_class) {
                    (true, _, _) => "same card",
                    (_, true, true) => "same suit and class",
                    (_, true, false) => "same suit",
                    _ => "other suit",
                }
                .into();
                hp1_key = d.rule.code().into();
            }
            let cell = match s.phase {
                "lead" | "follow" => format!("{} | {}", trick_band(trick), d.rule.code()),
                _ => format!("human {} → HP1 {} | {}", kind(ht), kind(at_), d.rule.code()),
            };
            (cell, true)
        };
        let facts = (!agree).then(|| {
            let mut f = Map::new();
            for x in &d.why {
                let mut key = x.key.to_string();
                while f.contains_key(&key) {
                    key.push('\'');
                }
                f.insert(key, json!(round(x.value)));
            }
            let txt = d
                .why
                .iter()
                .map(|x| format!("{}={}", x.key, x.value))
                .collect::<Vec<_>>()
                .join(" ");
            o.insert("facts".into(), Value::Object(f));
            txt
        });
        lines.push(o);
        rows.push(Row {
            step: at,
            phase: s.phase,
            seat: s.seat,
            agree,
            forced,
            rule: d.rule.code(),
            cell,
            with_role,
            human_key,
            hp1_key,
            human,
            hp1,
            facts,
            nanos,
        });
    })
    .ok()?;
    let state = &r.table.game.state;
    let declarer = state.declarer().map(|p| p.0);
    let trumps: Vec<String> = state.trump_called.iter().map(|s| s.to_string()).collect();
    let roles: [&'static str; 4] = std::array::from_fn(|y| role(declarer, y as Seat));
    let mut out = String::new();
    let mut by_phase: BTreeMap<&str, (u64, u64)> = BTreeMap::new();
    for (row, mut o) in rows.iter_mut().zip(lines) {
        let rl = roles[row.seat as usize];
        o.insert("role".into(), json!(rl));
        o.insert("team".into(), json!(team(rl)));
        if row.with_role {
            row.cell = format!("{rl} | {}", row.cell);
        }
        let e = by_phase.entry(row.phase).or_default();
        e.0 += 1;
        e.1 += row.agree as u64;
        out.push_str(&Value::Object(o).to_string());
        out.push('\n');
    }
    let info = GameFinishedInfo::from(r.table.game);
    let (result, value) = match info.playing_party {
        None => ("all pass", Value::Null),
        Some(p) => {
            let pts = info.team_points[(p.0 % 2) as usize];
            (
                if pts >= info.game_value.0 { "made" } else { "failed" },
                json!(info.game_value.0),
            )
        }
    };
    let agreement: Map<String, Value> = by_phase
        .iter()
        .map(|(p, (n, a))| (p.to_string(), json!({"n": n, "agree": a})))
        .collect();
    let seats: Vec<Value> = (0..4)
        .map(|y| {
            let mut f = features(y as Seat, CardSet::from_cards(&m.hands[y]));
            f["group"] = json!(groups[y]);
            f["role"] = json!(roles[y]);
            f
        })
        .collect();
    let summary = json!({
        "game": id,
        "month": m.month,
        "era": era,
        "declarer": declarer.map(|d| SEATS[d as usize]),
        "declarer_group": declarer.map(|d| groups[d as usize]),
        "value": value,
        "trumps": trumps,
        "result": result,
        "schwarz": info.schwarz_game,
        "team_points": info.team_points,
        "agreement": agreement,
        "seats": seats,
    })
    .to_string();
    hash.u64(id as u64);
    Some(GameOut {
        id,
        era,
        groups,
        roles,
        rows,
        lines: out,
        summary,
        hash: hash.0,
    })
}

/// Runs the comparison and writes the NDJSON files.
pub fn run(recs: &[Value], opts: &Opts) -> std::io::Result<Done> {
    let params = Params::default();
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<Option<GameOut>>> = Mutex::new((0..recs.len()).map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..opts.threads.max(1) {
            scope.spawn(|| {
                let fresh = || std::array::from_fn(|_| HeuristicAgent::new(params.clone()));
                let mut players: [HeuristicAgent; 4] = fresh();
                loop {
                    let id = next.fetch_add(1, Ordering::Relaxed);
                    if id >= recs.len() {
                        break;
                    }
                    if opts.fresh != Fresh::No {
                        players = fresh();
                    }
                    let g = game(id, &recs[id], &mut players, opts.fresh);
                    done.lock().unwrap()[id] = g;
                }
            });
        }
    });
    let games: Vec<GameOut> = done.into_inner().unwrap().into_iter().flatten().collect();
    std::fs::create_dir_all(&opts.out)?;
    let mut decisions = std::io::BufWriter::new(std::fs::File::create(opts.out.join("decisions.ndjson"))?);
    let mut summaries = std::io::BufWriter::new(std::fs::File::create(opts.out.join("games.ndjson"))?);
    let mut hash = Fnv::new();
    let mut decide_s: BTreeMap<&'static str, (u64, f64)> = BTreeMap::new();
    for g in &games {
        use std::io::Write as _;
        decisions.write_all(g.lines.as_bytes())?;
        summaries.write_all(g.summary.as_bytes())?;
        summaries.write_all(b"\n")?;
        hash.u64(g.hash);
        for r in &g.rows {
            let e = decide_s.entry(r.phase).or_default();
            e.0 += 1;
            e.1 += r.nanos as f64 * 1e-9;
        }
    }
    Ok(Done {
        overview: overview(&games, recs.len()),
        hash: hash.0,
        games: games.len(),
        decisions: games.iter().map(|g| g.rows.len() as u64).sum(),
        decide_s,
    })
}

#[derive(Default)]
struct Cell {
    n: u64,
    agree: u64,
    /// The decisions with more than one legal action, and the agreements among them.
    free: (u64, u64),
    rules: BTreeMap<String, u64>,
    human: BTreeMap<String, u64>,
    hp1: BTreeMap<String, u64>,
    /// (game, step, group, era, human, HP1, rule, facts)
    examples: Vec<String>,
}

/// (player, era) → role column → (decisions, agreements).
type Grid<'a> = BTreeMap<(&'a str, &'a str), BTreeMap<&'a str, (u64, u64)>>;

fn overview(games: &[GameOut], records: usize) -> String {
    let pct = |a: u64, n: u64| 100.0 * a as f64 / n.max(1) as f64;
    // phase → (player, era) → role column → (n, agree)
    let mut grid: BTreeMap<&str, Grid> = BTreeMap::new();
    let mut cells: BTreeMap<&str, BTreeMap<&str, Cell>> = BTreeMap::new();
    let mut phases: BTreeMap<&str, Cell> = BTreeMap::new();
    for g in games {
        for r in &g.rows {
            let seat = r.seat as usize;
            let role = g.roles[seat];
            let col = match role {
                "defender after VH" | "defender before VH" => "defenders",
                x => x,
            };
            for c in [col, "all"] {
                let e = grid
                    .entry(r.phase)
                    .or_default()
                    .entry((g.groups[seat], g.era))
                    .or_default()
                    .entry(c)
                    .or_default();
                e.0 += 1;
                e.1 += r.agree as u64;
            }
            for c in [
                phases.entry(r.phase).or_default(),
                cells.entry(r.phase).or_default().entry(r.cell.as_str()).or_default(),
            ] {
                c.n += 1;
                c.agree += r.agree as u64;
                if !r.forced {
                    c.free.0 += 1;
                    c.free.1 += r.agree as u64;
                }
                *c.rules.entry(r.rule.to_string()).or_default() += 1;
                *c.human.entry(r.human_key.clone()).or_default() += 1;
                *c.hp1.entry(r.hp1_key.clone()).or_default() += 1;
            }
            let c = cells.get_mut(r.phase).unwrap().get_mut(r.cell.as_str()).unwrap();
            if !r.agree && c.examples.len() < 2 {
                c.examples.push(format!(
                    "game {} step {} ({}, {}, {}): human {}, HP1 {} by {} [{}]",
                    g.id,
                    r.step,
                    g.groups[seat],
                    g.era,
                    role,
                    r.human,
                    r.hp1,
                    r.rule,
                    r.facts.as_deref().unwrap_or("")
                ));
            }
        }
    }
    let order = |p: &&str| PHASES.iter().position(|x| x == p).unwrap_or(PHASES.len());
    let mut phase_names: Vec<&str> = phases.keys().copied().collect();
    phase_names.sort_by_key(order);
    let (n, a) = phases.values().fold((0, 0), |(n, a), c| (n + c.n, a + c.agree));
    let (nf, af) = phases.values().fold((0, 0), |(n, a), c| (n + c.free.0, a + c.free.1));
    let mut out = String::new();
    let _ = writeln!(
        out,
        "- **{} games replayed to the end** (of {records} records), **{n} human decisions, HP1 agrees on {:.1} %** (the same action; passes: the same four cards); without the forced ones (one legal action, HX-1): {nf} decisions, {:.1} %.\n",
        games.len(),
        pct(a, n),
        pct(af, nf)
    );
    let _ = writeln!(
        out,
        "## Agreement by phase\n\n| phase | decisions | agreement | not forced: decisions, agreement | HP1's deciding rules | humans (by the phase's measure) |\n|---|---|---|---|---|---|"
    );
    for p in &phase_names {
        let c = &phases[p];
        let mut rules: Vec<_> = c.rules.iter().collect();
        rules.sort_by(|x, y| y.1.cmp(x.1));
        let top: Vec<String> = rules
            .iter()
            .take(6)
            .map(|(r, k)| format!("{r} {:.0} %", pct(**k, c.n)))
            .collect();
        let _ = writeln!(
            out,
            "| {p} | {} | {:.1} % | {}, {:.1} % | {} | {} |",
            c.n,
            pct(c.agree, c.n),
            c.free.0,
            pct(c.free.1, c.free.0),
            top.join(", "),
            dist(&c.human, c.n)
        );
    }
    let _ = writeln!(
        out,
        "\nPasses: the human's measure is the card overlap (0–4); cards: the same card, else the same suit and value class (A, 10, K/O, the rest), else the same suit."
    );
    let _ = writeln!(
        out,
        "\n## Agreement by phase × player × era × role\n\nRole in the game that was played: VH = the declarer, MH = the declarer's partner, defenders, no game = all passed. Each entry: agreement (decisions)."
    );
    for p in &phase_names {
        let _ = writeln!(
            out,
            "\n### {p}\n\n| player | era | VH | MH | defenders | no game | all |\n|---|---|---|---|---|---|---|"
        );
        for ((grp, e), roles) in &grid[p] {
            let cols: Vec<String> = ["VH", "MH", "defenders", "no game", "all"]
                .iter()
                .map(|c| match roles.get(c) {
                    Some((n, a)) => format!("{:.0} % ({n})", pct(*a, *n)),
                    None => "–".into(),
                })
                .collect();
            let _ = writeln!(out, "| {grp} | {e} | {} |", cols.join(" | "));
        }
    }
    let _ = writeln!(
        out,
        "\n## The 10 cells with the most disagreements, per phase\n\nCells: bidding = auction position | the partner's last action | the opponents' actions | hand class (A = an ace) | value band (as in KB-01); passes = role | HP1's rule; cards = role | tricks | HP1's rule; the rest = role | the human's and HP1's kind of action | HP1's rule. \"humans\"/\"HP1\": the actions by the phase's measure (passes: HP1 = whether it makes the same suits blank; cards: HP1 = its rule)."
    );
    for p in &phase_names {
        let _ = writeln!(out, "\n### {p}");
        let mut ranked: Vec<_> = cells[p].iter().filter(|(_, c)| c.n > c.agree).collect();
        ranked.sort_by(|x, y| (y.1.n - y.1.agree).cmp(&(x.1.n - x.1.agree)).then(x.0.cmp(y.0)));
        for (i, (key, c)) in ranked.iter().take(10).enumerate() {
            let _ = writeln!(
                out,
                "\n**{}. {key}**: n = {}, agreement {:.0} %, {} disagreements.",
                i + 1,
                c.n,
                pct(c.agree, c.n),
                c.n - c.agree
            );
            let _ = writeln!(out, "- HP1's deciding rule: {}", dist(&c.rules, c.n));
            let _ = writeln!(out, "- humans: {}", dist(&c.human, c.n));
            let _ = writeln!(out, "- HP1: {}", dist(&c.hp1, c.n));
            for e in &c.examples {
                let _ = writeln!(out, "- example: {e}");
            }
        }
    }
    out
}

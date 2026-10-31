//! Four fixed policy slots, whole-deal worker shards, and one complete NDJSON game at a time.

use std::io::{BufWriter, Write};

use agent::{Agent, SeriesScore};
use beliefs::{conventions::Message, Signals};
use engine::{GamePhase, SeriesSettings, SeriesTally};
use eval::{
    matchstats::{MatchObserver, MatchStats},
    play_table_with,
    suite::DealSuite,
    RandomAgent, Rng, Table,
};
use heuristic::{HeuristicAgent, Params};
use serde_json::{json, Value};

use crate::{
    cli::Cli,
    record::{action_json, game_record, today, Meta, Note},
};

pub const SCHEMA: &str = "marjapussi.selfplay/1";

/// What a policy factory receives for one seat of one game; true-hand diagnostics never
/// reach it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Setup {
    /// The deal/slot-derived stream seed, or the run's fixed `--policy-seed`.
    pub seed: u64,
    /// The physical seat.
    pub seat: u8,
    /// The owner's wall-clock deadlines (`--deadlines wall`, live behaviour) or only the
    /// policy's logical work limits (`--deadlines off`: reproducible under any CPU load).
    pub deadlines: bool,
}

/// A registered policy. The factory is called afresh for every seat of every game.
pub struct Policy {
    pub name: String,
    pub parameters: String,
    pub make: Box<dyn Fn(Setup) -> Box<dyn Agent>>,
}

fn hp1(name: &str, params: Params) -> Policy {
    let parameters = format!(
        "{}# signals = {:?}\n# effective seed is supplied by worker; see policy_seed\n",
        params.to_toml(),
        params.beliefs.signals
    );
    Policy {
        name: name.into(),
        parameters,
        make: Box::new(move |setup| {
            let mut params = params.clone();
            params.seed = setup.seed;
            params.deadlines = setup.deadlines;
            Box::new(HeuristicAgent::new(params))
        }),
    }
}

pub fn builtin_policies() -> Vec<Policy> {
    let fast = Params {
        endgame_tricks: 0,
        ..Params::default()
    };
    let mut facts = Params::default();
    facts.beliefs.signals = Signals::BaseOnly;
    vec![
        hp1("hp1", Params::default()),
        hp1("hp1-fast", fast),
        hp1("hp1-facts", facts),
        Policy {
            name: "random".into(),
            parameters: "uniform legal actions; SplitMix64 stream per deal and slot".into(),
            make: Box::new(|setup| Box::new(RandomAgent(Rng::new(setup.seed)))),
        },
    ]
}

struct Options {
    slots: [usize; 4],
    games: u64,
    suite: DealSuite,
    worker_index: u64,
    workers: usize,
    rotation: Rotation,
    policy_seed: Option<u64>,
    /// Omit the full game record and per-decision facts (scores, timings and rules remain).
    compact: bool,
    /// `Setup::deadlines` for every policy.
    deadlines: bool,
}

/// Games of one series in `--rotation series`: the site's tournament round (`SeriesSettings`).
const SERIES_GAMES: u64 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rotation {
    Teams,
    TeamsAndSeats,
    /// Four consecutive deals form one series, played in order with the running series score
    /// public; the opening seat moves through all four slots. The whole series is replayed with
    /// the partnerships swapped on the same hands.
    Series,
}
impl Rotation {
    fn games_per_deal(self) -> u64 {
        match self {
            Self::Teams | Self::Series => 2,
            Self::TeamsAndSeats => 4,
        }
    }
    /// Consecutive deals one worker plays together: a series stays on one worker.
    fn deals_per_block(self) -> u64 {
        if self == Self::Series {
            SERIES_GAMES
        } else {
            1
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Teams => "teams",
            Self::TeamsAndSeats => "teams-and-seats",
            Self::Series => "series",
        }
    }
    /// The policy lineup rotation (`lineup`) and whether the hands turn by two seats.
    /// In a series, game `g` opens with slot `[0, 3, 2, 1][g]`; the replay swaps partnerships
    /// (rotation `g ^ 1`, openers `[3, 0, 1, 2]`) so each deal's hands are played by both teams
    /// from the same seats.
    fn seating(self, deal_index: u64, game: u8) -> (u8, bool) {
        let rotation = match self {
            Self::Series => (deal_index % SERIES_GAMES) as u8 ^ game,
            _ => game,
        };
        (rotation, self != Self::Teams && rotation >= 2)
    }
}

impl Options {
    fn parse(cli: &Cli, policies: &[Policy]) -> Result<Self, String> {
        let default = policies.first().ok_or("no policies registered")?.name.as_str();
        for (i, policy) in policies.iter().enumerate() {
            if policy.name.is_empty() || policies[..i].iter().any(|p| p.name == policy.name) {
                return Err(format!("empty or duplicate policy name: {}", policy.name));
            }
        }
        let mut slots = [0; 4];
        for (slot, arg) in ["--seat0", "--seat1", "--seat2", "--seat3"].into_iter().enumerate() {
            let name = cli.str(arg).unwrap_or(default);
            slots[slot] = policies
                .iter()
                .position(|p| p.name == name)
                .ok_or_else(|| format!("{arg}: unknown policy {name}"))?;
        }
        let number = |arg, default| -> Result<u64, String> {
            cli.str(arg).map_or(Ok(default), |v| {
                v.parse().map_err(|_| format!("{arg}: invalid integer {v}"))
            })
        };
        let games = number("--games", 1_000)?;
        let seed = number("--seed", 1)?;
        let workers = number("--workers", 1)?;
        let worker_index = number("--worker-index", 0)?;
        let rotation = match cli.str("--rotation").unwrap_or("teams-and-seats") {
            "teams" => Rotation::Teams,
            "teams-and-seats" => Rotation::TeamsAndSeats,
            "series" => Rotation::Series,
            other => return Err(format!("--rotation: unknown mode {other}")),
        };
        let series_block = rotation.games_per_deal() * rotation.deals_per_block();
        if rotation == Rotation::Series && games % series_block != 0 {
            return Err(format!(
                "--rotation series needs whole series: --games a multiple of {series_block}"
            ));
        }
        let policy_seed = cli
            .str("--policy-seed")
            .map(|v| {
                v.parse::<u64>()
                    .map_err(|_| format!("--policy-seed: invalid integer {v}"))
            })
            .transpose()?;
        if workers == 0 || worker_index >= workers {
            return Err("--workers must be positive and --worker-index smaller than --workers".into());
        }
        let workers = usize::try_from(workers).map_err(|_| "--workers exceeds this platform's range")?;
        let suite = DealSuite::named(&format!("seed{seed}"), games.div_ceil(rotation.games_per_deal()))?;
        Ok(Self {
            slots,
            games,
            suite,
            worker_index,
            workers,
            rotation,
            policy_seed,
            compact: cli.flag("--compact"),
            deadlines: match cli.str("--deadlines").unwrap_or("wall") {
                "wall" => true,
                "off" => false,
                other => return Err(format!("--deadlines: unknown mode {other} (wall, off)")),
            },
        })
    }
}

/// One series of the `series` rotation, kept per partnership (team 0 = slots 0 and 2), since
/// the slots move between physical seats from game to game.
struct Series {
    index: u64,
    tally: SeriesTally,
}

impl Series {
    fn new(index: u64) -> Self {
        Series {
            index,
            tally: SeriesTally::new(SeriesSettings::default()),
        }
    }

    /// The public score as the table's physical teams (team 0 = seats 0 and 2) see it.
    fn view(&self, lineup: [u8; 4]) -> SeriesScore {
        let mut score = SeriesScore::from(&self.tally);
        if lineup[0] % 2 == 1 {
            score.team_totals.swap(0, 1);
        }
        score
    }

    /// Adds one game's physical team scores; returns the series record for that game.
    fn add(&mut self, lineup: [u8; 4], mut scores: [i32; 2], game: u64) -> Value {
        let before = self.tally.totals();
        if lineup[0] % 2 == 1 {
            scores.swap(0, 1);
        }
        self.tally.add_scores(scores);
        let mut record = json!({"index": self.index, "game": game, "totals_before": before});
        if self.tally.is_over() {
            let r = self.tally.result();
            record["result"] = json!({
                "games": r.games, "difference": r.difference, "bonus": r.bonus, "total": r.total,
            });
        }
        record
    }
}

/// Opposite slots stay partners. Rotations 2/3 separately turn the deal by two seats,
/// as duplicate::Rotation::TeamsAndSeats does. A heterogeneous slot receives two initial
/// hands across the two team roles, not all four hand allocations.
fn lineup(rotation: u8) -> [u8; 4] {
    std::array::from_fn(|seat| (seat as u8 + 4 - rotation) % 4)
}

fn stream_seed(deal_seed: u64, slot: u8) -> u64 {
    // Reuse the eval RNG's mixing; stream identity follows the slot through all rotations.
    Rng::new(deal_seed.wrapping_add(u64::from(slot).wrapping_mul(0x9E37_79B9_7F4A_7C15))).next_u64()
}

pub(crate) fn phase(phase: &GamePhase) -> &'static str {
    match phase {
        GamePhase::WaitingForStart => "waiting_for_start",
        GamePhase::Bidding => "bidding",
        GamePhase::PassingForth => "passing_forth",
        GamePhase::PassingBack => "passing_back",
        GamePhase::Raising => "raising",
        GamePhase::Trick => "trick",
        GamePhase::StartTrick => "start_trick",
        GamePhase::AnsweringPair => "answering_pair",
        GamePhase::AnsweringHalf(_) => "answering_half",
        GamePhase::Ended => "ended",
        GamePhase::PendingUndo(_) => "pending_undo",
    }
}

fn message(message: Message) -> &'static str {
    match message {
        Message::Ace => "ace",
        Message::TwoHalves => "two_halves",
        Message::SmallOrHalves => "small_or_halves",
        Message::BigPair => "big_pair",
        Message::RedPair => "red_pair",
    }
}

pub(crate) fn metrics_json(m: &MatchStats) -> Value {
    let seats: Vec<_> = m
        .seats
        .iter()
        .map(|s| {
            json!({
                "auction_bids": s.auction_bids, "auction_passes": s.auction_passes,
                "raise_opportunities": s.raise_opportunities, "raises": s.raises,
                "raise_amount": s.raise_amount,
                "questions": s.questions, "melds": s.melds, "negative_answers": s.negative_answers,
            })
        })
        .collect();
    let readings: Vec<_> = m
        .communication
        .readings
        .iter()
        .map(|r| {
            json!({
                "seat": r.seat, "target": r.target, "message": r.message.map(message),
                "supported": r.supported, "ambiguous": r.ambiguous,
                "candidates": r.candidates.iter().copied().map(message).collect::<Vec<_>>(),
                "available_at_pass": r.available_at_pass.iter().copied().map(message).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "scores": m.scores, "team_points": m.team_points,
        "card_points": m.card_points, "pair_points": m.pair_points,
        "card_point_ceiling": m.card_point_ceiling, "pair_point_ceiling": m.pair_point_ceiling,
        "tricks": m.tricks, "schwarz": m.schwarz, "schwarz_teams": m.schwarz_teams,
        "contract": m.contract.as_ref().map(|c| json!({
            "declarer": c.declarer, "team": c.team, "auction_value": c.auction_value,
            "final_value": c.final_value, "raise_amount": c.raise_amount, "made": c.made,
        })),
        "seats": seats,
        "communication": {
            "basis": "common_default_convention_support_not_receiver_understanding",
            "recognizable": m.communication.recognizable,
            "supported": m.communication.supported, "unsupported": m.communication.unsupported,
            "unrecognized_bids": m.communication.unrecognized_bids,
            "ambiguous_bids": m.communication.ambiguous_bids,
            "passes_with_available_message": m.communication.passes_with_available_message,
            "readings": readings,
        },
    })
}

fn game(
    policies: &[Policy],
    options: &Options,
    deal_index: u64,
    game: u8,
    index: u64,
    mut series: Option<&mut Series>,
) -> Result<Value, String> {
    let (rotation, hands_turned) = options.rotation.seating(deal_index, game);
    let lineup = lineup(rotation);
    let seed = options.suite.deal_seed(deal_index);
    let mut hands = options.suite.hands(deal_index);
    if hands_turned {
        hands.rotate_left(2);
    }
    let mut players: Vec<_> = lineup
        .iter()
        .enumerate()
        .map(|(seat, &slot)| {
            (policies[options.slots[slot as usize]].make)(Setup {
                seed: options.policy_seed.unwrap_or_else(|| stream_seed(seed, slot)),
                seat: seat as u8,
                deadlines: options.deadlines,
            })
        })
        .collect();
    let mut phases = Vec::new();
    let mut event_counts = Vec::new();
    let mut observer = MatchObserver::default();
    let mut table = Table::new(&hands);
    table.kinds = std::array::from_fn(|s| players[s].kind());
    if let Some(series) = &series {
        table.series = series.view(lineup);
    }
    let (info, log) = play_table_with(table, &mut players, usize::MAX, |table, seat, _decision| {
        phases.push(phase(&table.game.state.phase));
        event_counts.push(table.view(seat).events.len());
        observer.observe(table);
    })
    .map_err(|e| format!("game {index}: {e}"))?;
    let series = series
        .as_deref_mut()
        .map(|s| s.add(lineup, info.scores(), deal_index % SERIES_GAMES));
    let metrics = metrics_json(&observer.finish(&info));
    // The runner logs only decisions, not the four engine START events. game_record filters
    // precisely those START events, so these notes stay aligned with its action indices.
    let notes: Vec<_> = log
        .iter()
        .map(|l| Note {
            decision: Some(l.decision.clone()),
            ..Note::default()
        })
        .collect();
    let meta = Meta {
        source: "selfplay",
        id: format!("selfplay-s{}-g{index}", options.suite.seed),
        seed: Some(seed),
        params_hash: None,
        date: today(),
        players: lineup.map(|slot| policies[options.slots[slot as usize]].name.clone()),
    };
    let decisions: Vec<_> = log
        .iter()
        .zip(phases)
        .zip(event_counts)
        .enumerate()
        .map(|(step, ((l, phase), event_count))| {
            let facts = l.decision.explanation.facts.iter();
            let facts: Vec<_> = if options.compact {
                facts.filter(|f| REPORTED_FACTS.contains(&f.key)).collect()
            } else {
                facts.collect()
            };
            let mut d = json!({
                "step": step, "seat": l.seat, "phase": phase,
                "ms": l.nanos as f64 / 1_000_000.0,
                "rule": l.decision.explanation.label,
                "facts": facts.iter().map(|f| json!({"key": f.key, "value": f.value})).collect::<Vec<_>>(),
            });
            if !options.compact {
                d["event_count"] = json!(event_count);
                d["action"] = action_json(&l.decision.action, phase == "raising");
            }
            d
        })
        .collect();
    let slots: Vec<_> = options
        .slots
        .iter()
        .enumerate()
        .map(|(slot, &i)| {
            let mut p = json!({
                "name": policies[i].name,
                "stream_seed": stream_seed(seed, slot as u8),
                "policy_seed": options.policy_seed.unwrap_or_else(|| stream_seed(seed, slot as u8)),
            });
            if !options.compact {
                p["parameters"] = json!(policies[i].parameters);
            }
            p
        })
        .collect();
    let mut envelope = json!({
        "schema": SCHEMA, "index": index, "deal_index": deal_index, "deal_seed": seed,
        "rotation": rotation, "hands_turned": hands_turned, "lineup": lineup, "policies": slots,
        "rotation_mode": options.rotation.name(), "games_per_deal": options.rotation.games_per_deal(),
        "decisions": decisions, "metrics": metrics,
    });
    if !options.compact {
        envelope["record"] = game_record(&meta, &info, &notes);
    }
    if let Some(series) = series {
        envelope["series"] = series;
    }
    Ok(envelope)
}

/// The decision facts the benchmark report reads; `--compact` keeps only these.
const REPORTED_FACTS: [&str; 5] = [
    "forecast_complete",
    "forecast_decisions",
    "forecast_reused_decisions",
    "complete",
    "deadline_hit",
];

fn write_games(policies: &[Policy], options: &Options, writer: &mut impl Write) -> Result<(), String> {
    let mut emit = |envelope: Value, index: u64| -> Result<(), String> {
        serde_json::to_writer(&mut *writer, &envelope).map_err(|e| format!("write game {index}: {e}"))?;
        writeln!(writer)
            .and_then(|_| writer.flush())
            .map_err(|e| format!("flush game {index}: {e}"))
    };
    let per_deal = options.rotation.games_per_deal();
    let per_block = options.rotation.deals_per_block();
    for block in (options.worker_index..options.suite.deals.div_ceil(per_block)).step_by(options.workers) {
        let deals = block * per_block..((block + 1) * per_block).min(options.suite.deals);
        if options.rotation == Rotation::Series {
            // Each orientation plays its own series through the block's deals in order.
            for orientation in 0..per_deal as u8 {
                let mut series = Series::new(block);
                for deal_index in deals.clone() {
                    let index = deal_index * per_deal + u64::from(orientation);
                    emit(
                        game(policies, options, deal_index, orientation, index, Some(&mut series))?,
                        index,
                    )?;
                }
            }
            continue;
        }
        for deal_index in deals {
            for rotation in 0..per_deal as u8 {
                let Some(index) = (deal_index * per_deal).checked_add(u64::from(rotation)) else {
                    break;
                };
                if index >= options.games {
                    break;
                }
                emit(game(policies, options, deal_index, rotation, index, None)?, index)?;
            }
        }
    }
    Ok(())
}

/// Parse the shared Cli options and flush one NDJSON envelope after every completed game.
/// Supplied policies form a registry; --seat0..3 select the four partnership slots by name.
pub fn run_with(policies: Vec<Policy>) -> Result<(), String> {
    let cli = Cli::parse(
        &[
            "--seat0",
            "--seat1",
            "--seat2",
            "--seat3",
            "--games",
            "--seed",
            "--worker-index",
            "--workers",
            "--rotation",
            "--policy-seed",
            "--deadlines",
        ],
        &["--compact"],
    );
    let mut policies = policies;
    for arg in ["--seat0", "--seat1", "--seat2", "--seat3"] {
        if let Some(name) = cli.str(arg) {
            if !policies.iter().any(|p| p.name == name) {
                policies.extend(variant(name)?);
            }
        }
    }
    let options = Options::parse(&cli, &policies)?;
    write_games(&policies, &options, &mut BufWriter::new(std::io::stdout().lock()))
}

/// `hp1@<file>`: the current HP1 with the flat TOML overrides of `<file>` (`Params::from_toml`),
/// an ad-hoc variant for parameter A/B runs; `None` for any other unregistered name.
fn variant(name: &str) -> Result<Option<Policy>, String> {
    let Some(path) = name.strip_prefix("hp1@") else {
        return Ok(None);
    };
    let text = std::fs::read_to_string(path).map_err(|e| format!("{name}: {e}"))?;
    let params = Params::from_toml(&text).map_err(|e| format!("{name}: {e}"))?;
    Ok(Some(hp1(name, params)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent::{Decision, Explanation, Fact, SeatView};
    use engine::GameAction;
    use std::{cell::RefCell, rc::Rc};

    struct First(&'static str);
    impl Agent for First {
        fn act(&mut self, _view: &SeatView, legal: &[GameAction]) -> Decision {
            Decision {
                action: legal[0],
                explanation: Explanation {
                    label: self.0,
                    facts: vec![Fact {
                        key: "precision",
                        value: 0.12345678,
                    }],
                },
            }
        }
    }

    fn policies() -> Vec<Policy> {
        ["a", "b", "c", "d"]
            .map(|name| Policy {
                name: name.into(),
                parameters: "first legal".into(),
                make: Box::new(move |_| Box::new(First(name))),
            })
            .into_iter()
            .collect()
    }

    fn options(worker: u64) -> Options {
        let args = [
            "--seat0",
            "a",
            "--seat1",
            "b",
            "--seat2",
            "c",
            "--seat3",
            "d",
            "--games",
            "7",
            "--seed",
            "5",
            "--workers",
            "2",
            "--worker-index",
        ];
        let cli = Cli::from_args(
            args.into_iter().map(String::from).chain([worker.to_string()]),
            &[
                "--seat0",
                "--seat1",
                "--seat2",
                "--seat3",
                "--games",
                "--seed",
                "--workers",
                "--worker-index",
            ],
            &[],
        );
        Options::parse(&cli, &policies()).unwrap()
    }

    #[test]
    fn fixed_slots_and_turned_hands_match_duplicate_and_notes_skip_starts() {
        let policies = policies();
        let options = options(0);
        let games: Vec<_> = (0..4)
            .map(|r| game(&policies, &options, 0, r, u64::from(r), None).unwrap())
            .collect();
        assert_eq!(games[0]["lineup"], json!([0, 1, 2, 3]));
        assert_eq!(games[1]["lineup"], json!([3, 0, 1, 2]));
        assert_eq!(games[2]["lineup"], json!([2, 3, 0, 1]));
        assert_eq!(games[3]["lineup"], json!([1, 2, 3, 0]));
        let mut allocations: [std::collections::BTreeSet<usize>; 4] = std::array::from_fn(|_| Default::default());
        for (rotation, g) in games.iter().enumerate() {
            assert_eq!(g["hands_turned"], rotation >= 2);
            assert_eq!(g["index"], rotation);
            assert_eq!(g["deal_seed"], games[0]["deal_seed"]);
            let lineup = lineup(rotation as u8);
            for seat in 0..4 {
                let hand = (seat + if rotation >= 2 { 2 } else { 0 }) % 4;
                assert_eq!(g["record"]["deal"][seat], games[0]["record"]["deal"][hand]);
                allocations[lineup[seat] as usize].insert(hand);
            }
            assert_eq!((lineup[0] + 2) % 4, lineup[2]);
            assert_eq!((lineup[1] + 2) % 4, lineup[3]);
            let actions = g["record"]["actions"].as_array().unwrap();
            let decisions = g["decisions"].as_array().unwrap();
            assert_eq!(actions.len(), decisions.len());
            assert_eq!(actions[0]["type"], "pass_bid");
            for (a, d) in actions.iter().zip(decisions) {
                assert_eq!(a["seat"], d["seat"]);
                assert_eq!(a["rule"], d["rule"]);
                let seat = d["seat"].as_u64().unwrap() as usize;
                let slot = lineup[seat] as usize;
                assert_eq!(d["rule"], policies[options.slots[slot]].name);
                assert_eq!(g["record"]["players"][seat], policies[options.slots[slot]].name);
                assert_eq!(d["facts"][0]["value"], json!(0.12345678f32));
                assert_ne!(d["facts"][0]["value"], a["why"]["precision"]);
            }
            assert!(actions.iter().all(|a| a["type"] != "start"));
            assert_eq!(g["metrics"]["scores"], g["record"]["result"]["scores"]);
        }
        assert!(allocations.iter().all(|hands| hands.len() == 2));
        // No rotation/worker component enters a slot's random stream identity.
        assert_ne!(stream_seed(5, 0), stream_seed(5, 1));
    }

    #[test]
    fn workers_keep_whole_blocks_and_exact_requested_count() {
        let mut indices = Vec::new();
        for worker in 0..2 {
            let mut bytes = Vec::new();
            write_games(&policies(), &options(worker), &mut bytes).unwrap();
            for line in std::str::from_utf8(&bytes).unwrap().lines() {
                let g: Value = serde_json::from_str(line).unwrap();
                assert_eq!(g["deal_index"].as_u64().unwrap() % 2, worker);
                indices.push(g["index"].as_u64().unwrap());
            }
        }
        indices.sort_unstable();
        assert_eq!(indices, (0..7).collect::<Vec<_>>());
        assert!(Options::parse(
            &Cli::from_args(["--workers".into(), "0".into()], &["--workers"], &[]),
            &policies()
        )
        .is_err());
    }

    #[test]
    fn series_rotate_the_opener_swap_partnerships_and_keep_a_public_tally() {
        let policies = policies();
        let mut rows = Vec::new();
        for worker in 0..2 {
            let mut options = options(worker);
            options.games = 32;
            options.rotation = Rotation::Series;
            options.suite.deals = 16;
            options.compact = true;
            let mut bytes = Vec::new();
            write_games(&policies, &options, &mut bytes).unwrap();
            for line in std::str::from_utf8(&bytes).unwrap().lines() {
                let row: Value = serde_json::from_str(line).unwrap();
                assert_eq!(row["series"]["index"].as_u64().unwrap() % 2, worker);
                assert!(row.get("record").is_none());
                rows.push(row);
            }
        }
        rows.sort_by_key(|g| g["index"].as_u64().unwrap());
        assert_eq!(rows.len(), 32);
        for (index, row) in rows.iter().enumerate() {
            let (deal, orientation) = (index as u64 / 2, index as u64 % 2);
            assert_eq!(row["deal_index"], deal);
            assert_eq!(row["series"]["index"], deal / SERIES_GAMES);
            assert_eq!(row["series"]["game"], deal % SERIES_GAMES);
            let rotation = (deal % SERIES_GAMES) ^ orientation;
            assert_eq!(row["rotation"], rotation);
            assert_eq!(row["hands_turned"], rotation >= 2);
            // The opening seat 0 holds slots 0, 3, 2, 1 through a series (3, 0, 1, 2 in the
            // replay): every slot opens once and the opening partnership alternates.
            let opener = row["lineup"][0].as_u64().unwrap();
            let openers = if orientation == 0 { [0, 3, 2, 1] } else { [3, 0, 1, 2] };
            assert_eq!(opener, openers[(deal % SERIES_GAMES) as usize]);
            assert_eq!(opener % 2, (deal + orientation) % 2);
        }
        for (deal, pair) in rows.chunks_exact(2).enumerate() {
            // Same hands at the same seats, partnerships swapped: physical scores agree.
            assert_eq!(pair[0]["hands_turned"], pair[1]["hands_turned"], "deal {deal}");
            assert_ne!(
                pair[0]["lineup"][0].as_u64().unwrap() % 2,
                pair[1]["lineup"][0].as_u64().unwrap() % 2
            );
        }
        for series in rows.chunks_exact(2 * SERIES_GAMES as usize) {
            for orientation in 0..2 {
                let games: Vec<_> = series.iter().skip(orientation).step_by(2).collect();
                let mut totals = [0i64; 2];
                for g in &games {
                    assert_eq!(g["series"]["totals_before"], json!(totals));
                    let scores = g["metrics"]["scores"].as_array().unwrap();
                    let a = (g["lineup"][0].as_u64().unwrap() % 2) as usize;
                    totals[a] += scores[0].as_i64().unwrap();
                    totals[1 - a] += scores[1].as_i64().unwrap();
                }
                let result = &games[3]["series"]["result"];
                assert_eq!(result["games"], json!(totals));
                assert!(games[..3].iter().all(|g| g["series"].get("result").is_none()));
            }
        }
    }

    #[test]
    fn series_need_whole_series() {
        let args = ["--rotation", "series", "--games", "12"].map(String::from);
        let cli = Cli::from_args(args, &["--rotation", "--games"], &[]);
        assert!(Options::parse(&cli, &policies()).is_err());
    }

    #[test]
    fn teams_shards_128_original_deals_twice_and_supplies_live_seed_zero() {
        let received = Rc::new(RefCell::new(Vec::new()));
        let mut policies = policies();
        for policy in &mut policies {
            let received = received.clone();
            policy.make = Box::new(move |setup| {
                received.borrow_mut().push(setup.seed);
                Box::new(First("first"))
            });
        }
        let mut rows = Vec::new();
        for worker in 0..2 {
            let mut options = options(worker);
            options.games = 256;
            options.rotation = Rotation::Teams;
            options.policy_seed = Some(0);
            options.slots = [0, 1, 0, 1];
            options.suite.deals = 128;
            let mut bytes = Vec::new();
            write_games(&policies, &options, &mut bytes).unwrap();
            for line in std::str::from_utf8(&bytes).unwrap().lines() {
                let row: Value = serde_json::from_str(line).unwrap();
                assert_eq!(row["deal_index"].as_u64().unwrap() % 2, worker);
                assert_eq!(row["hands_turned"], false);
                assert_eq!(row["rotation_mode"], "teams");
                assert!(row["policies"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|p| p["policy_seed"] == 0));
                rows.push(row);
            }
        }
        rows.sort_by_key(|g| g["index"].as_u64().unwrap());
        assert_eq!(rows.len(), 256);
        assert_eq!(received.borrow().len(), 1024);
        assert!(received.borrow().iter().all(|&seed| seed == 0));
        for (deal, pair) in rows.chunks_exact(2).enumerate() {
            assert_eq!(pair[0]["index"], 2 * deal);
            assert_eq!(pair[1]["index"], 2 * deal + 1);
            assert_eq!(pair[0]["deal_index"], deal);
            assert_eq!(pair[1]["deal_index"], deal);
            assert_eq!(pair[0]["record"]["deal"], pair[1]["record"]["deal"]);
            assert_eq!(pair[0]["lineup"], json!([0, 1, 2, 3]));
            assert_eq!(pair[1]["lineup"], json!([3, 0, 1, 2]));
            assert_eq!(pair[0]["record"]["players"], json!(["a", "b", "a", "b"]));
            assert_eq!(pair[1]["record"]["players"], json!(["b", "a", "b", "a"]));
        }
    }
}

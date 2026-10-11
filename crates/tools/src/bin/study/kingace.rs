//! `study kingace` (C44): the owner's "king instead of ace" pattern (Spiel 5) in 4×HP1
//! self-play. A card play where HP1 plays the K or O of a suit s that is not called yet, while
//! it holds the ace of s, may play it, and the other half of s is unseen and may be with the
//! partner (`Knowledge::may_hold`): the half it gives up could have made a pair with the
//! partner, and the ace would have taken the trick. Owner: "Taking tricks with aces is mostly
//! good." Writes every hit to `OUT/kingace.ndjson` and returns the Markdown summary.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use agent::Agent;
use engine::{partner, suit_of, takes, trick_winner, ActionType, Card, CardSet};
use eval::suite::DealSuite;
use eval::Table;
use heuristic::{HeuristicAgent, Params};
use serde_json::{json, Value};

/// One hit, with what the overview needs.
struct Hit {
    deal: u64,
    seed: u64,
    step: usize,
    rule: &'static str,
    lead: bool,
    trick: usize,
    ace_wins_now: bool,
    opp_may_ruff: bool,
    /// The K or O played takes the trick for sure: it wins now and no opponent still to play
    /// can beat it (a higher card of s, or a ruff).
    card_safe: bool,
    p_other: f64,
    declarers: bool,
    line: Value,
}

impl Hit {
    /// The owner's case: the ace would take the trick (it wins now, and no opponent still to
    /// play can ruff).
    fn strict(&self) -> bool {
        self.ace_wins_now && !self.opp_may_ruff
    }
}

const ACE: u8 = 8;

fn cards(set: CardSet) -> Vec<String> {
    set.iter().map(|c| Card::from_index(c).to_string()).collect()
}

/// The pattern at one decision, if it is there.
fn check(table: &Table, seat: u8, legal: &[engine::GameAction], d: &agent::Decision, params: &Params) -> Option<Value> {
    let ActionType::CardPlayed(card) = d.action.action_type else {
        return None;
    };
    let c = card.index();
    let s = suit_of(c);
    let halves = CardSet::halves(s);
    if !halves.contains(c) {
        return None;
    }
    let view = table.view(seat);
    let k = view.knowledge;
    let hand = k.certain(seat);
    let ace = s as u8 * 9 + ACE;
    let ace_legal = legal
        .iter()
        .any(|a| matches!(a.action_type, ActionType::CardPlayed(x) if x.index() == ace));
    let other = (halves - CardSet::single(c)).iter().next()?;
    let mate = partner(seat);
    if !hand.contains(ace) || !ace_legal || k.is_called(s) || hand.contains(other) || !k.may_hold(mate).contains(other)
    {
        return None;
    }
    let trick = k.trick();
    let trump = k.trump();
    let with = |x: u8| {
        let mut t = trick.to_vec();
        t.push(x);
        trick_winner(&t, trump) == trick.len()
    };
    // The seats still to play after me in this trick.
    let later: Vec<u8> = (1..4 - trick.len() as u8).map(|i| (seat + i) % 4).collect();
    let opp_may_ruff = trump.is_some_and(|t| {
        t != s
            && later.iter().any(|&y| {
                y % 2 != seat % 2 && k.count_bounds(y, s).0 == 0 && !(k.may_hold(y) & CardSet::suit(t)).is_empty()
            })
    });
    // An opponent still to play may beat the K/O: a higher card of s it may hold, or a ruff.
    let card_beatable = later
        .iter()
        .any(|&y| y % 2 != seat % 2 && (k.may_hold(y) & CardSet::suit(s)).iter().any(|b| takes(b, c, trump)))
        || opp_may_ruff;
    let partner_winning = !trick.is_empty() && {
        let w = trick_winner(trick, trump);
        (k.trick_leader() + w as u8) % 4 == mate
    };
    let p_partner_other = HeuristicAgent::new(params.clone()).mind(&view).p_holds(mate, other);
    let facts: serde_json::Map<String, Value> = d
        .explanation
        .facts
        .iter()
        .map(|f| (f.key.to_string(), json!((f.value as f64 * 1e3).round() / 1e3)))
        .collect();
    Some(json!({
        "seat": seat,
        "declarers": k.declarer().map(|v| v % 2 == seat % 2),
        "trick": k.trick_winners().len() + 1,
        "lead": trick.is_empty(),
        "trick_so_far": trick.iter().map(|&x| Card::from_index(x).to_string()).collect::<Vec<_>>(),
        "trump": trump.map(|t| t.to_string()),
        "hand": cards(hand),
        "played": card.to_string(),
        "ace": Card::from_index(ace).to_string(),
        "other_half": Card::from_index(other).to_string(),
        "p_partner_other_half": (p_partner_other as f64 * 1e3).round() / 1e3,
        "card_wins_now": with(c),
        "card_beatable_later": card_beatable,
        "ace_wins_now": with(ace),
        "partner_winning": partner_winning,
        "opp_may_ruff": opp_may_ruff,
        "ace_beats_card": takes(ace, c, trump),
        "rule": d.explanation.label,
        "why": facts,
    }))
}

fn game(suite: &DealSuite, g: u64, params: &Params) -> (Vec<Hit>, u64) {
    let mut players: Vec<Box<dyn Agent>> = (0..4)
        .map(|_| Box::new(HeuristicAgent::new(params.clone())) as Box<dyn Agent>)
        .collect();
    let mut hits = vec![];
    let mut step = 0usize;
    let mut plays = 0u64;
    eval::play_game_with(&suite.hands(g), &mut players, |table, seat, d| {
        let at = step;
        step += 1;
        let Some((_, legal)) = table.to_act() else { return };
        plays += matches!(d.action.action_type, ActionType::CardPlayed(_)) as u64;
        if let Some(mut line) = check(table, seat, legal, d, params) {
            line["suite"] = json!(suite.name);
            line["deal"] = json!(g);
            line["deal_seed"] = json!(suite.deal_seed(g));
            line["step"] = json!(at);
            hits.push(Hit {
                deal: g,
                seed: suite.deal_seed(g),
                step: at,
                rule: d.explanation.label,
                lead: line["lead"] == true,
                trick: line["trick"].as_u64().unwrap_or(0) as usize,
                ace_wins_now: line["ace_wins_now"] == true,
                opp_may_ruff: line["opp_may_ruff"] == true,
                card_safe: line["card_wins_now"] == true && line["card_beatable_later"] == false,
                p_other: line["p_partner_other_half"].as_f64().unwrap_or(0.0),
                declarers: line["declarers"] == true,
                line,
            });
        }
    });
    (hits, plays)
}

/// Runs `deals` games of `suite` on `threads` threads; writes the hits; returns the summary.
pub fn run(suite: &DealSuite, threads: usize, out: &std::path::Path) -> std::io::Result<String> {
    let params = Params::default();
    let next = AtomicU64::new(0);
    let plays = AtomicU64::new(0);
    let all: Mutex<Vec<Hit>> = Mutex::new(vec![]);
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| loop {
                let g = next.fetch_add(1, Ordering::Relaxed);
                if g >= suite.deals {
                    break;
                }
                let (hits, n) = game(suite, g, &params);
                plays.fetch_add(n, Ordering::Relaxed);
                all.lock().unwrap().extend(hits);
            });
        }
    });
    let mut hits = all.into_inner().unwrap();
    hits.sort_by_key(|h| (h.deal, h.step));
    std::fs::create_dir_all(out)?;
    let mut text = String::new();
    for h in &hits {
        text.push_str(&h.line.to_string());
        text.push('\n');
    }
    std::fs::write(out.join("kingace.ndjson"), text)?;
    let n = suite.deals as f64;
    let games_hit = {
        let mut d: Vec<u64> = hits.iter().map(|h| h.deal).collect();
        d.dedup();
        d.len()
    };
    let per_1000 = |k: usize| 1000.0 * k as f64 / n;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# C44: \"king instead of ace\" in 4×HP1 self-play\n\nSuite `{}` (seed {}), {} games, {} card plays. Pattern: HP1 plays the K or O of an uncalled suit s while it holds the ace of s, may play it, and the other half of s is unseen and may be with the partner.\n",
        suite.name,
        suite.seed,
        suite.deals,
        plays.load(Ordering::Relaxed)
    );
    let strict: Vec<&Hit> = hits.iter().filter(|h| h.strict()).collect();
    let count = |f: &dyn Fn(&Hit) -> bool| strict.iter().filter(|h| f(h)).count();
    let strict_games = {
        let mut d: Vec<u64> = strict.iter().map(|h| h.deal).collect();
        d.dedup();
        d.len()
    };
    let _ = writeln!(
        out,
        "- Loose pattern: {} hits in {} games, {:.1} per 1,000 games. In most of them the ace would not take the trick (it was trumped already, or an opponent still to play may ruff): then keeping the ace is right.
- **The owner's case (the ace takes the trick: it wins now and no opponent still to play can ruff): {} hits in {} games, {:.1} per 1,000 games.**
  - the K/O played does not take the trick for sure (an opponent still to play may beat it): {} ({:.1} per 1,000); it takes it for sure too: {};
  - lead {}, follow {}; declarer team {}, defenders {};
  - HP1's belief that the partner holds the other half: < 0.05: {}, 0.05–0.25: {}, 0.25–0.5: {}, ≥ 0.5: {}.
",
        hits.len(),
        games_hit,
        per_1000(hits.len()),
        strict.len(),
        strict_games,
        per_1000(strict.len()),
        count(&|h| !h.card_safe),
        per_1000(count(&|h| !h.card_safe)),
        count(&|h| h.card_safe),
        count(&|h| h.lead),
        count(&|h| !h.lead),
        count(&|h| h.declarers),
        count(&|h| !h.declarers),
        count(&|h| h.p_other < 0.05),
        count(&|h| (0.05..0.25).contains(&h.p_other)),
        count(&|h| (0.25..0.5).contains(&h.p_other)),
        count(&|h| h.p_other >= 0.5),
    );
    let mut by_rule: BTreeMap<&str, [usize; 3]> = BTreeMap::new();
    for h in &hits {
        let e = by_rule.entry(h.rule).or_default();
        e[0] += 1;
        e[1] += h.strict() as usize;
        e[2] += (h.strict() && !h.card_safe) as usize;
    }
    let mut rules: Vec<_> = by_rule.into_iter().collect();
    rules.sort_by(|a, b| b.1[1].cmp(&a.1[1]).then(b.1[0].cmp(&a.1[0])));
    let _ = writeln!(
        out,
        "## The deciding rule

| HP1's rule | loose hits | the owner's case | per 1,000 games | … the K/O not sure to take it |
|---|---|---|---|---|"
    );
    for (r, [all, k, lose]) in rules {
        let _ = writeln!(out, "| {r} | {all} | {k} | {:.1} | {lose} |", per_1000(k));
    }
    let mut by_trick: BTreeMap<usize, usize> = BTreeMap::new();
    for h in &strict {
        *by_trick.entry(h.trick).or_default() += 1;
    }
    let _ = writeln!(
        out,
        "
The owner's case by trick: {}.
",
        by_trick
            .iter()
            .map(|(t, k)| format!("{t}: {k}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let _ = writeln!(
        out,
        "## Examples of the owner's case, the K/O not sure to take the trick first (replay: `arena --suite {} --games N`; deal = game index, step = decision index)
",
        suite.name
    );
    let mut shown: Vec<&Hit> = strict.iter().copied().filter(|h| !h.card_safe).collect();
    shown.extend(strict.iter().copied().filter(|h| h.card_safe));
    for h in shown.into_iter().take(5) {
        let l = &h.line;
        let _ = writeln!(
            out,
            "- deal {} (deal seed {}), step {}: seat {} ({}), trick {}, {} {}; trump {}; hand {}; played **{}** ({}) instead of {}; the other half {} with the partner at P = {}; rule **{}** {}",
            h.deal,
            h.seed,
            h.step,
            l["seat"],
            if h.declarers { "declarer team" } else { "defender" },
            h.trick,
            if h.lead { "lead" } else { "follow to" },
            if h.lead { String::new() } else { l["trick_so_far"].to_string() },
            l["trump"],
            l["hand"],
            l["played"].as_str().unwrap_or(""),
            match (h.card_safe, h.line["card_wins_now"] == true) {
                (true, _) => "takes the trick for sure",
                (false, true) => "wins the trick so far, may still be beaten",
                (false, false) => "loses the trick",
            },
            l["ace"].as_str().unwrap_or(""),
            l["other_half"].as_str().unwrap_or(""),
            l["p_partner_other_half"],
            h.rule,
            l["why"]
        );
    }
    Ok(out)
}

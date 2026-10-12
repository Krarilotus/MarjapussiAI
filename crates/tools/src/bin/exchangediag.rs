//! Where the take-or-defend forecast loses points (Astra's diagnostic, offline): on the human
//! games' contracts, the same level-0 play after the real exchange and after the simulated
//! exchange (`passing::level0_exchange`, no auction read) from the real deal, against what the
//! humans scored. Actual hidden hands are used here only as a diagnostic.
//!
//! `exchangediag` reads `MARJA_GAMES` and prints the declaring teams' mean points and made shares
//! by contract band.

use std::io::BufRead;

use engine::{Card, CardSet};
use serde_json::Value;

fn set(v: &Value) -> Option<CardSet> {
    let mut s = CardSet::EMPTY;
    for c in v.as_array()? {
        s |= CardSet::single(c.as_str()?.parse::<Card>().ok()?.index());
    }
    Some(s)
}

#[derive(Default)]
struct Band {
    n: u32,
    human: i64,
    real: i64,
    sim: i64,
    made_human: u32,
    made_real: u32,
    made_sim: u32,
}

fn main() {
    let path = std::env::var("MARJA_GAMES").expect("MARJA_GAMES");
    let conf = heuristic::Params::default().partner_half_conf;
    let none: [Option<Box<[f32; beliefs::conventions::N_CLASSES]>>; 4] = Default::default();
    let mut bands: std::collections::BTreeMap<&str, Band> = Default::default();
    for line in std::io::BufReader::new(std::fs::File::open(path).expect("games")).lines() {
        let Ok(g) = serde_json::from_str::<Value>(&line.expect("line")) else { continue };
        let names: Vec<&str> = g["players"].as_array().unwrap().iter().filter_map(|n| n.as_str()).collect();
        let Some(decl) = g["playing_player"].as_str().and_then(|p| names.iter().position(|n| *n == p)) else {
            continue;
        };
        let value = g["game_value"].as_i64().unwrap_or(0);
        if value <= 0 {
            continue;
        }
        let Some(dealt) = names.iter().map(|n| set(&g["cards"][n])).collect::<Option<Vec<_>>>() else { continue };
        let dealt: [CardSet; 4] = [dealt[0], dealt[1], dealt[2], dealt[3]];
        let (Some(forth), Some(back)) = (set(&g["passed_cards"]["forth"]), set(&g["passed_cards"]["back"])) else {
            continue;
        };
        if forth.len() != 4 || back.len() != 4 {
            continue;
        }
        let (d, p) = (decl, (decl + 2) % 4);
        let mut real = dealt;
        real[p] -= forth;
        real[d] |= forth;
        real[d] -= back;
        real[p] |= back;
        if real.iter().any(|h| h.len() != 9) {
            continue;
        }
        let sim = heuristic::passing::level0_exchange(dealt, d as u8, conf, &none, [[false; 4]; 4]);
        let team = d % 2;
        let pts = &g["players_points"];
        let human = pts[names[d]].as_i64().unwrap_or(0) + pts[names[p]].as_i64().unwrap_or(0);
        let real_pts = heuristic::diagnostics::playout_points(real, d as u8)[team] as i64;
        let sim_pts = heuristic::diagnostics::playout_points(sim, d as u8)[team] as i64;
        let band = if value < 145 { "< 145" } else if value < 180 { "145-175" } else { "180+" };
        let b = bands.entry(band).or_default();
        b.n += 1;
        b.human += human;
        b.real += real_pts;
        b.sim += sim_pts;
        b.made_human += (human >= value) as u32;
        b.made_real += (real_pts >= value) as u32;
        b.made_sim += (sim_pts >= value) as u32;
    }
    println!("contract   n     human pts/made    real exchange + level-0 play    simulated exchange + level-0 play");
    for (band, b) in bands {
        let n = b.n.max(1) as f64;
        println!(
            "{band:8} {:5}   {:6.1} / {:3.0} %       {:6.1} / {:3.0} %                  {:6.1} / {:3.0} %",
            b.n,
            b.human as f64 / n,
            100.0 * b.made_human as f64 / n,
            b.real as f64 / n,
            100.0 * b.made_real as f64 / n,
            b.sim as f64 / n,
            100.0 * b.made_sim as f64 / n
        );
    }
}

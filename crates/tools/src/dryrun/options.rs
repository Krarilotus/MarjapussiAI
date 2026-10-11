use std::{
    collections::BTreeSet,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::PathBuf,
};

use eval::suite::DealSuite;
use serde_json::Value;

use super::{
    fixture::{identity, Fixture},
    Mode, Replacement,
};
use crate::{cli::Cli, selfplay::Policy};

pub(super) struct Case {
    pub index: u64,
    pub steps: Option<Vec<usize>>,
}

pub(super) struct Options {
    pub input: PathBuf,
    pub output: PathBuf,
    pub declared_sha256: Option<String>,
    pub suite: DealSuite,
    pub cases: Vec<Case>,
    pub candidates: Vec<usize>,
    pub baseline: [Option<usize>; 2],
    pub mode: Mode,
    pub replacement: Replacement,
    pub steps: String,
    pub phases: BTreeSet<String>,
    pub max_runs: usize,
    pub candidate_team: String,
}

pub(super) const OPTIONS: &[&str] = &[
    "--input",
    "--output",
    "--input-sha256",
    "--fixtures",
    "--cases",
    "--steps",
    "--phases",
    "--candidate",
    "--baseline-new",
    "--baseline-old",
    "--mode",
    "--replacement",
    "--max-runs",
    "--seed",
    "--deals",
    "--candidate-team",
];

fn policy(policies: &[Policy], name: &str) -> Result<usize, String> {
    policies
        .iter()
        .position(|p| p.name == name)
        .ok_or_else(|| format!("unregistered policy: {name}"))
}

impl Options {
    pub fn parse(cli: &Cli, policies: &[Policy]) -> Result<Self, String> {
        for (i, p) in policies.iter().enumerate() {
            if p.name.is_empty() || policies[..i].iter().any(|q| q.name == p.name) {
                return Err("policy names must be nonempty and unique".into());
            }
        }
        let number = |arg, default| {
            cli.str(arg).map_or(Ok(default), |s| {
                s.parse::<u64>().map_err(|_| format!("invalid {arg}: {s}"))
            })
        };
        let seed = number("--seed", 20261011)?;
        let deals = number("--deals", 1000)?;
        if deals == 0 || deals > 1000 {
            return Err("--deals must be 1..1000".into());
        }
        let mode = match cli.str("--mode").unwrap_or("teacher") {
            "teacher" => Mode::Teacher,
            "continue" => Mode::Continue,
            "full" => Mode::Full,
            s => return Err(format!("unknown mode: {s}")),
        };
        let replacement = match cli.str("--replacement").unwrap_or("action") {
            "action" => Replacement::Action,
            "seat" => Replacement::Seat,
            "partnership" => Replacement::Partnership,
            s => return Err(format!("unknown replacement: {s}")),
        };
        if mode == Mode::Full && replacement == Replacement::Action {
            return Err("full mode needs --replacement seat or partnership".into());
        }
        let candidate_team = cli
            .str("--candidate-team")
            .unwrap_or(if mode == Mode::Full { "new" } else { "focal" })
            .to_string();
        if !["new", "old", "focal"].contains(&candidate_team.as_str()) {
            return Err("--candidate-team must be new, old or focal".into());
        }
        if mode != Mode::Full && candidate_team != "focal" {
            return Err("prefix modes replace the focal actor/team; --candidate-team is for full mode".into());
        }
        let baseline = ["--baseline-new", "--baseline-old"]
            .map(|arg| cli.str(arg).map(|s| policy(policies, s)).transpose())
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        if mode != Mode::Teacher && baseline.iter().any(Option::is_none) {
            return Err("continuation requires explicit --baseline-new and --baseline-old factories".into());
        }
        let candidates = cli
            .str("--candidate")
            .unwrap_or("hp1")
            .split(',')
            .map(|s| policy(policies, s))
            .collect::<Result<Vec<_>, _>>()?;
        if candidates.is_empty()
            || candidates.len() > 8
            || candidates.iter().collect::<BTreeSet<_>>().len() != candidates.len()
        {
            return Err("select 1..8 distinct candidate policies".into());
        }
        let cases = if let Some(path) = cli.str("--fixtures") {
            if cli.str("--cases").is_some() {
                return Err("use --fixtures or --cases, not both".into());
            }
            let mut bytes = Vec::new();
            File::open(path)
                .map_err(|e| format!("fixtures: {e}"))?
                .take(65537)
                .read_to_end(&mut bytes)
                .map_err(|e| format!("fixtures: {e}"))?;
            if bytes.len() > 65536 {
                return Err("fixture selection exceeds 64 KiB".into());
            }
            let value: Value = serde_json::from_slice(&bytes).map_err(|e| format!("fixtures: {e}"))?;
            value
                .as_array()
                .ok_or("fixtures must be an array")?
                .iter()
                .map(|v| {
                    let index = v["index"].as_u64().ok_or("fixture needs index")?;
                    let steps = v
                        .get("steps")
                        .map(|s| {
                            s.as_array()
                                .ok_or("steps must be an array")?
                                .iter()
                                .map(|n| n.as_u64().and_then(|n| usize::try_from(n).ok()).ok_or("invalid step"))
                                .collect::<Result<Vec<_>, _>>()
                        })
                        .transpose()?;
                    Ok(Case { index, steps })
                })
                .collect::<Result<Vec<_>, &str>>()?
        } else {
            cli.str("--cases")
                .unwrap_or("125,2549,2403,1113,2649,3390,3545,162,812,57,664,307")
                .split(',')
                .map(|s| {
                    let (index, steps) = s.split_once(':').map_or((s, None), |(i, steps)| (i, Some(steps)));
                    Ok(Case {
                        index: index.parse().map_err(|_| format!("invalid case: {s}"))?,
                        steps: steps.map(|s| step_list(s, '+')).transpose()?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?
        };
        if cases.is_empty()
            || cases.len() > 32
            || cases.iter().any(|c| c.index >= deals * 4)
            || cases.iter().map(|c| c.index).collect::<BTreeSet<_>>().len() != cases.len()
        {
            return Err("select 1..32 distinct game indices within the suite".into());
        }
        let max_runs = number("--max-runs", 128)?;
        if max_runs == 0 || max_runs > 512 {
            return Err("--max-runs must be 1..512".into());
        }
        let steps = cli
            .str("--steps")
            .unwrap_or(if mode == Mode::Teacher { "all" } else { "focal" })
            .to_string();
        if !["all", "focal"].contains(&steps.as_str()) {
            step_list(&steps, ',')?;
        }
        let phases: BTreeSet<_> = cli
            .str("--phases")
            .map(|s| s.split(',').map(String::from).collect())
            .unwrap_or_default();
        for p in &phases {
            if ![
                "bidding",
                "passing_forth",
                "passing_back",
                "raising",
                "start_trick",
                "trick",
                "answering_pair",
                "answering_half",
            ]
            .contains(&p.as_str())
            {
                return Err(format!("unknown phase: {p}"));
            }
        }
        let declared_sha256 = cli.str("--input-sha256").map(String::from);
        if declared_sha256
            .as_ref()
            .is_some_and(|s| s.len() != 64 || !s.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            return Err("--input-sha256 must contain 64 hexadecimal characters".into());
        }
        Ok(Self {
            input: cli.str("--input").ok_or("--input required")?.into(),
            output: cli.str("--output").ok_or("--output required")?.into(),
            declared_sha256,
            suite: DealSuite::named(&format!("seed{seed}"), deals)?,
            cases,
            candidates,
            baseline: [baseline[0], baseline[1]],
            mode,
            replacement,
            steps,
            phases,
            max_runs: max_runs as usize,
            candidate_team,
        })
    }

    pub fn steps_for(&self, fixture: &Fixture) -> Result<Vec<usize>, String> {
        let case = self
            .cases
            .iter()
            .find(|c| c.index == fixture.index)
            .ok_or("unselected fixture")?;
        let steps = if self.mode == Mode::Full {
            vec![0]
        } else if let Some(steps) = &case.steps {
            steps.clone()
        } else if self.steps == "all" {
            (0..fixture.actions.len()).collect()
        } else if self.steps == "focal" {
            let focal = fixture.phases.iter().position(|p| *p == "start_trick");
            vec![focal.ok_or("no focal step: specify --steps")?]
        } else {
            step_list(&self.steps, ',')?
        };
        if steps.iter().any(|&s| s >= fixture.actions.len())
            || steps.iter().collect::<BTreeSet<_>>().len() != steps.len()
        {
            return Err(format!("game {}: invalid or duplicate steps", fixture.index));
        }
        Ok(steps
            .into_iter()
            .filter(|&s| self.mode == Mode::Full || self.phases.is_empty() || self.phases.contains(fixture.phases[s]))
            .collect())
    }
}

fn step_list(s: &str, separator: char) -> Result<Vec<usize>, String> {
    s.split(separator)
        .map(|s| s.parse::<usize>().map_err(|_| format!("invalid step: {s}")))
        .collect()
}

/// Scan the entire ordered dataset, retaining only selected rows. No policy calls here.
pub(super) fn load(options: &Options) -> Result<Vec<Fixture>, String> {
    load_reader(
        BufReader::new(File::open(&options.input).map_err(|e| format!("input: {e}"))?),
        options,
    )
}

pub(super) fn load_reader(mut reader: impl BufRead, options: &Options) -> Result<Vec<Fixture>, String> {
    let mut selected = Vec::new();
    let mut index = 0u64;
    loop {
        let mut bytes = Vec::new();
        reader
            .by_ref()
            .take(1_048_577)
            .read_until(b'\n', &mut bytes)
            .map_err(|e| format!("read game {index}: {e}"))?;
        if bytes.is_empty() {
            break;
        }
        if bytes.len() > 1_048_576 {
            return Err(format!("game {index}: row exceeds 1 MiB"));
        }
        let row = serde_json::from_slice::<Value>(&bytes).map_err(|e| format!("game {index}: {e}"))?;
        identity(index, &row, &options.suite)?;
        if options.cases.iter().any(|c| c.index == index) {
            selected.push(Fixture::from_qualification(index, row, options.suite.clone())?);
        }
        index += 1;
    }
    if index != options.suite.deals * 4 || selected.len() != options.cases.len() {
        return Err(format!(
            "incomplete dataset: {index} games; expected {}",
            options.suite.deals * 4
        ));
    }
    Ok(selected)
}

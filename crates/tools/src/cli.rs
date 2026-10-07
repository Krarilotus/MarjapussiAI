//! The one command-line parser of the tools binaries (Judge J19 item 7): `--key value` options
//! and `--flag`s, in any order.

use std::collections::BTreeMap;
use std::str::FromStr;

use heuristic::Params;

pub struct Cli {
    values: BTreeMap<String, String>,
    flags: Vec<String>,
}

impl Cli {
    /// The process arguments. `options` take a value, `flags` do not; anything else panics.
    pub fn parse(options: &[&str], flags: &[&str]) -> Cli {
        Cli::from_args(std::env::args().skip(1), options, flags)
    }

    pub fn from_args(args: impl IntoIterator<Item = String>, options: &[&str], flags: &[&str]) -> Cli {
        let mut cli = Cli {
            values: BTreeMap::new(),
            flags: vec![],
        };
        let mut args = args.into_iter();
        while let Some(a) = args.next() {
            if flags.contains(&a.as_str()) {
                cli.flags.push(a);
            } else if options.contains(&a.as_str()) {
                let v = args.next().unwrap_or_else(|| panic!("{a} needs a value"));
                cli.values.insert(a, v);
            } else {
                panic!("unknown argument {a} (options {options:?}, flags {flags:?})");
            }
        }
        cli
    }

    pub fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }

    pub fn str(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// The option parsed as `T`, or `default` when it is absent.
    pub fn get<T: FromStr>(&self, name: &str, default: T) -> T {
        self.str(name).map_or(default, |v| {
            v.parse().unwrap_or_else(|_| panic!("{name}: cannot read {v}"))
        })
    }

    /// The params file of the option (flat TOML over the defaults), or the defaults.
    pub fn params(&self, name: &str) -> Params {
        self.str(name).map_or_else(Params::default, |path| {
            let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
            Params::from_toml(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_and_flags_in_any_order() {
        let args = ["--seats", "--deals", "20", "--suite", "check"].map(String::from);
        let cli = Cli::from_args(args, &["--deals", "--suite"], &["--seats", "--sequential"]);
        assert!(cli.flag("--seats") && !cli.flag("--sequential"));
        assert_eq!(cli.get("--deals", 500u64), 20);
        assert_eq!(cli.str("--suite"), Some("check"));
        assert_eq!(cli.get("--games", 7u64), 7);
    }
}

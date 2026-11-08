use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};

const BUNDLED_COMMON: &str = include_str!("../../../assets/config/common.json");

#[derive(Deserialize)]
struct CommonFile {
    #[serde(default)]
    groups: BTreeMap<String, Group>,
}

#[derive(Deserialize)]
struct Group {
    #[serde(default = "yes")]
    default: bool,
    #[serde(default)]
    ignore: Vec<String>,
}

fn yes() -> bool { true }

/// Patterns from every enabled group. `overrides` (from config.json) beat
/// each group's own `default`. Malformed JSON yields an empty list, never an error.
pub fn common_ignores(overrides: &BTreeMap<String, bool>) -> Vec<String> {
    parse_common(BUNDLED_COMMON, overrides)
}

fn parse_common(json: &str, overrides: &BTreeMap<String, bool>) -> Vec<String> {
    let Ok(file) = serde_json::from_str::<CommonFile>(json) else { return Vec::new() };
    let mut out = Vec::new();
    for (name, g) in file.groups {
        if overrides.get(&name).copied().unwrap_or(g.default) {
            out.extend(g.ignore);
        }
    }
    out
}

/// Drop exact duplicates (e.g. `target/` coming from both a language list and
/// `common.json`), keeping the first occurrence and the original order.
pub fn dedup_patterns(patterns: &mut Vec<String>) {
    let mut seen = HashSet::new();
    patterns.retain(|p| seen.insert(p.trim().to_string()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ignore::engine::should_ignore;

    fn none() -> BTreeMap<String, bool> { BTreeMap::new() }
    fn off(g: &str) -> BTreeMap<String, bool> { BTreeMap::from([(g.to_string(), false)]) }

    #[test]
    fn bundled_file_parses_and_is_not_empty() {
        let f: CommonFile = serde_json::from_str(BUNDLED_COMMON).expect("common.json must parse");
        assert!(!f.groups.is_empty());
        for (name, g) in &f.groups {
            for p in &g.ignore {
                assert!(!p.trim().is_empty(), "{name}: empty pattern");
                assert_eq!(p, p.trim(), "{name}: stray whitespace in {p:?}");
            }
        }
    }

    #[test]
    fn license_and_readme_are_ignored_by_default() {
        let pats = common_ignores(&none());
        for f in ["LICENSE", "LICENSE-MIT", "README.md", "sub/README.md"] {
            assert!(should_ignore(f, &pats), "{f} should be ignored");
        }
    }

    #[test]
    fn a_group_can_be_switched_off() {
        let pats = common_ignores(&off("docs"));
        assert!(!should_ignore("README.md", &pats));
        assert!(should_ignore("Cargo.lock", &pats)); // other groups unaffected
    }

    #[test]
    fn ordinary_source_files_are_never_ignored() {
        let pats = common_ignores(&none());
        for f in ["src/main.rs", "src/lib.rs", "Cargo.toml", "package.json", "pom.xml",
            "src/index.ts", "main.py", ".env.example", "Dockerfile"] {
            assert!(!should_ignore(f, &pats), "{f} must stay included");
        }
    }

    #[test]
    fn bad_json_gives_empty_list() {
        assert!(parse_common("nope", &none()).is_empty());
        assert!(parse_common("{}", &none()).is_empty());
    }

    #[test]
    fn dedup_keeps_first_and_order() {
        let mut v: Vec<String> = ["a", "b", "a", " b "].iter().map(|s| s.to_string()).collect();
        dedup_patterns(&mut v);
        assert_eq!(v, ["a", "b"]);
    }
}
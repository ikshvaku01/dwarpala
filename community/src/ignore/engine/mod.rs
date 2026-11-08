use std::{borrow::Cow, fs, path::Path};

pub fn load_gitignore(root: &Path) -> Vec<String> {
    fs::read_to_string(root.join(".gitignore"))
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('!'))
        .map(String::from)
        .collect()
}

// ── Pattern classification (shared by both entry points) ─────────────────────

/// A pattern after trimming and `\` -> `/` normalisation.
enum Kind<'a> {
    /// One path component, e.g. `target`, `*.lock`, `node_modules/`.
    Single { comp: &'a str, wild: bool, dir_only: bool },
    /// Several components containing `*`, e.g. `community/*.lock`. Root-anchored.
    Glob { glob: &'a str },
    /// Several components, no `*`, e.g. `docs/internal`. Root-anchored.
    Under { p: &'a str },
}

/// `norm` must already be trimmed and have backslashes replaced by `/`.
fn classify(norm: &str) -> Option<Kind<'_>> {
    let dir_only = norm.ends_with('/');
    let p = norm.trim_end_matches('/');
    let mut segs = p.split('/').filter(|s| !s.is_empty());
    let first = segs.next()?; // no non-empty segment -> never matches
    if segs.next().is_none() {
        Some(Kind::Single { comp: first, wild: first.contains('*'), dir_only })
    } else if p.contains('*') {
        Some(Kind::Glob { glob: p })
    } else {
        Some(Kind::Under { p })
    }
}

fn normalise(pattern: &str) -> Option<Cow<'_, str>> {
    let pat = pattern.trim();
    if pat.is_empty() {
        return None;
    }
    Some(if pat.contains('\\') {
        Cow::Owned(pat.replace('\\', "/"))
    } else {
        Cow::Borrowed(pat)
    })
}

// ── Public API ───────────────────────────────────────────────────────────────

pub fn matches_ignore(rel_path: &str, pattern: &str) -> bool {
    let Some(norm) = normalise(pattern) else { return false };
    match classify(&norm) {
        None => false,
        Some(Kind::Single { comp, wild, dir_only }) => single_component(rel_path, comp, wild, dir_only),
        Some(Kind::Glob { glob }) => glob_prefix_at_boundary(glob, rel_path),
        Some(Kind::Under { p }) => exact_or_under(rel_path, p),
    }
}

pub fn should_ignore(rel: &str, patterns: &[String]) -> bool {
    if rel.starts_with(".git/") || rel == ".git" {
        return true;
    }
    patterns.iter().any(|p| matches_ignore(rel, p))
}

/// A pattern parsed once. Build these when a pattern list is reused for many
/// paths (a scan, the watcher, `refilter`).
pub enum Compiled {
    Never,
    Single { comp: String, wild: bool, dir_only: bool },
    Glob { glob: String },
    Under { p: String },
}

impl Compiled {
    pub fn new(pattern: &str) -> Self {
        let Some(norm) = normalise(pattern) else { return Self::Never };
        match classify(&norm) {
            None => Self::Never,
            Some(Kind::Single { comp, wild, dir_only }) => {
                Self::Single { comp: comp.to_owned(), wild, dir_only }
            }
            Some(Kind::Glob { glob }) => Self::Glob { glob: glob.to_owned() },
            Some(Kind::Under { p }) => Self::Under { p: p.to_owned() },
        }
    }

    pub fn is_match(&self, rel_path: &str) -> bool {
        match self {
            Self::Never => false,
            Self::Single { comp, wild, dir_only } => single_component(rel_path, comp, *wild, *dir_only),
            Self::Glob { glob } => glob_prefix_at_boundary(glob, rel_path),
            Self::Under { p } => exact_or_under(rel_path, p),
        }
    }
}

pub fn should_ignore_compiled(rel: &str, patterns: &[Compiled]) -> bool {
    if rel.starts_with(".git/") || rel == ".git" {
        return true;
    }
    patterns.iter().any(|p| p.is_match(rel))
}

// ── Matching primitives ──────────────────────────────────────────────────────

/// Any path component equals / globs `comp`. With `dir_only` the last
/// component (the file itself) is skipped.
fn single_component(rel: &str, comp: &str, wild: bool, dir_only: bool) -> bool {
    let mut parts = rel.split('/').peekable();
    while let Some(part) = parts.next() {
        if dir_only && parts.peek().is_none() {
            continue;
        }
        let hit = if wild { glob_full(comp, part) } else { part == comp };
        if hit {
            return true;
        }
    }
    false
}

/// `rel == p` or `rel` lives under `p/`.
fn exact_or_under(rel: &str, p: &str) -> bool {
    rel == p || rel.strip_prefix(p).is_some_and(|rest| rest.starts_with('/'))
}

/// Old regex: `^<glob>(/|$)` searched in `rel`. I.e. some prefix of `rel`
/// matches `glob` and is followed by `/` or by the end of the text.
fn glob_prefix_at_boundary(glob: &str, text: &str) -> bool {
    let lead_len = glob.find('*').unwrap_or(glob.len());
    if !text.starts_with(&glob[..lead_len]) {
        return false; // cheap reject on the literal part before the first `*`
    }
    let t = text.as_bytes();
    for k in lead_len..t.len() {
        if t[k] == b'/' && glob_full(glob, &text[..k]) {
            return true;
        }
    }
    glob_full(glob, text)
}

/// Whole-string glob match. `*` matches any run of characters except `\n`
/// (that is what regex `.*` did). Works on bytes: every literal in a `&str`
/// pattern starts on a char boundary, so a match can never split a character.
pub fn glob_full(pat: &str, text: &str) -> bool {
    let (p, t) = (pat.as_bytes(), text.as_bytes());
    if t.contains(&b'\n') {
        return glob_full_dp(p, t); // rare; keeps the `\n` rule exactly right
    }
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None; // (pattern index after `*`, text index it has consumed up to)
    loop {
        if pi == p.len() && ti == t.len() {
            return true;
        }
        if pi < p.len() && p[pi] == b'*' {
            star = Some((pi + 1, ti));
            pi += 1;
            continue;
        }
        if pi < p.len() && ti < t.len() && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
            continue;
        }
        match star {
            Some((spi, sti)) if sti < t.len() => {
                star = Some((spi, sti + 1));
                pi = spi;
                ti = sti + 1;
            }
            _ => return false,
        }
    }
}

/// Textbook DP, used only when the text contains `\n`.
fn glob_full_dp(p: &[u8], t: &[u8]) -> bool {
    let mut dp = vec![false; t.len() + 1];
    dp[0] = true;
    for &pc in p {
        if pc == b'*' {
            for j in 1..=t.len() {
                dp[j] = dp[j] || (dp[j - 1] && t[j - 1] != b'\n');
            }
        } else {
            for j in (1..=t.len()).rev() {
                dp[j] = dp[j - 1] && pc == t[j - 1];
            }
            dp[0] = false;
        }
    }
    dp[t.len()]
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod diff_tests;

#[cfg(test)]
mod tests {
    use super::*;

    // ---- verbatim from the repo's ignore/engine tests ----

    fn hit(pattern: &str, path: &str) -> bool {
        matches_ignore(path, pattern)
    }

    #[test]
    fn star_prefix_matches_at_any_depth() {
        assert!(hit("*.lock", "Cargo.lock"));
        assert!(hit("*.lock", "community/Cargo.lock"));
        assert!(hit("*.lock", "a/b/c/Cargo.lock"));
        assert!(!hit("*.lock", "Cargo.toml"));
        assert!(!hit("*.lock", "locks/readme.md"));
    }

    #[test]
    fn star_suffix_matches_at_any_depth() {
        assert!(hit("Cargo.*", "Cargo.lock"));
        assert!(hit("Cargo.*", "x/Cargo.toml"));
        assert!(!hit("Cargo.*", "Cargo"));
        assert!(hit("test_*", "a/test_one.py"));
        assert!(!hit("test_*", "a/my_test_one.py"));
    }

    #[test]
    fn star_on_both_sides_matches_a_substring() {
        assert!(hit("*lock*", "Cargo.lock"));
        assert!(!hit("*lock*", "src/main.rs"));
    }

    #[test]
    fn a_wildcard_also_matches_directory_names() {
        assert!(hit("*.lock", "vendor.lock/x.txt"));
    }

    #[test]
    fn patterns_with_a_slash_and_a_star_are_anchored_to_the_root() {
        assert!(hit("community/*.lock", "community/Cargo.lock"));
        assert!(!hit("community/*.lock", "x/community/Cargo.lock"));
        assert!(hit("community/*.lock", "community/sub/Cargo.lock"));
    }

    #[test]
    fn plain_names_match_whole_components_only() {
        assert!(hit("target", "target/debug/x"));
        assert!(hit("target", "community/target/x"));
        assert!(!hit("target", "src/targets.rs"));
        assert!(hit("target/", "target/x"));
        assert!(!hit("target/", "target"));
    }

    #[test]
    fn matching_is_case_sensitive() {
        assert!(!hit("*.LOCK", "Cargo.lock"));
    }

    #[test]
    fn should_ignore_applies_the_whole_pattern_list() {
        let patterns = vec!["target".to_string(), "*.lock".to_string()];
        assert!(should_ignore("Cargo.lock", &patterns));
        assert!(should_ignore("community/Cargo.lock", &patterns));
        assert!(should_ignore("target/release/dwarpala", &patterns));
        assert!(!should_ignore("community/Cargo.toml", &patterns));
        assert!(should_ignore(".git/config", &[]));
    }

    // ---- quirks of the old implementation that must be preserved ----

    #[test]
    fn quirks_are_preserved() {
        assert!(hit("/target", "a/target/x")); // leading '/' is not an anchor for a single component
        assert!(hit("target\\", "target/x")); // backslash -> '/', so a dir-only pattern
        assert!(!hit("target\\", "target"));
        assert!(!hit("", "a"));
        assert!(!hit("   ", "a"));
        assert!(!hit("/", "a"));
        assert!(!hit("//", "a"));
        assert!(hit("a/b", "a/b"));
        assert!(hit("a/b", "a/b/c"));
        assert!(!hit("a/b", "a/bc"));
        assert!(!hit("a//b", "a/b")); // interior '//' stays literal in the multi-segment path
        assert!(hit("a/*", "a/b"));
        assert!(hit("a/*", "a/b/c")); // star crosses '/'
        assert!(hit("a/*", "a/")); // star can be empty
        assert!(!hit("a/*", "a")); // but the '/' is required
    }

    #[test]
    fn newline_rules_match_regex_dot() {
        assert!(!glob_full("a*b", "a\nb")); // `.` does not match \n
        assert!(glob_full("a\nb", "a\nb")); // literal newline does
        assert!(glob_full("a*", "ab"));
        assert!(!glob_full("*", "x\ny"));
        assert!(glob_full("*", ""));
    }

    #[test]
    fn unicode_is_not_split() {
        assert!(hit("*.日本", "dir/é.日本"));
        assert!(hit("é*", "éa"));
        assert!(!hit("*a", "éb"));
    }

    #[test]
    fn compiled_agrees_with_borrowed() {
        for pat in ["target", "target/", "*.lock", "a/*", "a/b", "/x", "", "a\\b/", "*lock*"] {
            let c = Compiled::new(pat);
            for path in ["target/x", "a/b", "a/b/c", "x", "Cargo.lock", "", "a/", "x/y/Cargo.lock"] {
                assert_eq!(c.is_match(path), matches_ignore(path, pat), "{pat:?} vs {path:?}");
            }
        }
    }
}
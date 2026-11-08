//! Differential tests: the regex-free `matches_ignore` against the ORIGINAL regex implementation.
//!
//! `matches_ignore_regex` below is the original function, verbatim (renamed). It is test-only, so the
//! `regex` crate can stay a dependency without being used on the hot path.
//!
//! Run:    cargo test -p dwarpala diff_
//! Timing: cargo test -p dwarpala --release -- --ignored --nocapture bench_regex_vs_glob

use super::{matches_ignore, Compiled};

// ── The original implementation, verbatim ────────────────────────────────────

fn matches_ignore_regex(rel_path: &str, pattern: &str) -> bool {
    let p = pattern.trim().replace('\\', "/");
    if p.is_empty() { return false; }
    let is_dir_only = p.ends_with('/');
    let p = p.trim_end_matches('/');
    let path_parts: Vec<&str> = rel_path.split('/').collect();
    let pat_parts:  Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
    if pat_parts.is_empty() { return false; }
    if pat_parts.len() == 1 {
        let comp = pat_parts[0];
        if comp.contains('*') {
            let re_str = format!("^{}$", regex::escape(comp).replace("\\*", ".*"));
            if let Ok(re) = regex::Regex::new(&re_str) {
                return path_parts.iter().enumerate().any(|(i, part)| {
                    re.is_match(part) && !(is_dir_only && i == path_parts.len() - 1)
                });
            }
        } else {
            return path_parts.iter().enumerate().any(|(i, part)| {
                *part == comp && !(is_dir_only && i == path_parts.len() - 1)
            });
        }
    } else if p.contains('*') {
        let re_str = format!("^{}(/|$)", regex::escape(p).replace("\\*", ".*"));
        if let Ok(re) = regex::Regex::new(&re_str) { return re.is_match(rel_path); }
    } else {
        return rel_path == p || rel_path.starts_with(&format!("{p}/"));
    }
    false
}

// ── Harness ──────────────────────────────────────────────────────────────────

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize { (self.next() % n as u64) as usize }
    fn string(&mut self, alphabet: &[&str], max_len: usize) -> String {
        let len = self.below(max_len + 1);
        (0..len).map(|_| alphabet[self.below(alphabet.len())]).collect()
    }
}

const PAT_ALPHA: &[&str] = &["a", "a", "a", "b", "b", ".", "*", "*", "/", "/", "\\", " ", "\n", "é"];
const PATH_ALPHA: &[&str] = &["a", "a", "a", "b", "b", ".", "/", "/", "/", "\n", "é", "日", " "];
const FILL: &[&str] = &["a", "b", ".", "/", "é", "\n"];

/// A path that tends to match `pat`: stars filled in, then a random prefix and/or suffix.
fn derive_path(rng: &mut Rng, pat: &str) -> String {
    let p = pat.trim().replace('\\', "/");
    let mut s = String::new();
    for c in p.chars() {
        if c == '*' { s.push_str(&rng.string(FILL, 3)); } else { s.push(c); }
    }
    if rng.below(10) < 4 { s.push('/'); s.push_str(&rng.string(PATH_ALPHA, 10)); }
    if rng.below(10) < 3 { s = format!("{}/{s}", rng.string(PATH_ALPHA, 10)); }
    s
}

#[derive(Default)]
struct Tally { cases: usize, positives: usize, bad: Vec<String> }

impl Tally {
    fn check(&mut self, pat: &str, path: &str) {
        let old = matches_ignore_regex(path, pat);
        let new = matches_ignore(path, pat);
        let comp = Compiled::new(pat).is_match(path);
        self.cases += 1;
        self.positives += old as usize;
        if old != new || old != comp {
            self.bad.push(format!("pattern={pat:?} path={path:?} regex={old} glob={new} compiled={comp}"));
        }
    }
    fn assert_clean(&self, what: &str) {
        assert!(
            self.bad.is_empty(),
            "{what}: {} mismatches out of {} cases, first {}:\n{}",
            self.bad.len(), self.cases, self.bad.len().min(10),
            self.bad.iter().take(10).cloned().collect::<Vec<_>>().join("\n"),
        );
    }
}

// ── Fixtures ─────────────────────────────────────────────────────────────────

const HAND_PATTERNS: &[&str] = &[
    "", " ", "/", "//", "*", "**", "***", "*/", "/*", "*/*", "a", "a/", "a//", "/a", "/a/", "//a", "a/b", "a/b/",
    "a//b", "a/*", "a/*/", "*/a", "*a*", "a*b", "a**b", "\\", "a\\", "a\\b", "\\a", "a\\*", " a ", "\ta", "a\n",
    "a\nb", "*\n", "é*", "*.日本", ".*", "a.b", "a+b", "a(b)", "[a]", "a$", "^a", "a|b",
];
const HAND_PATHS: &[&str] = &[
    "", "/", "a", "a/", "/a", "a/b", "a/b/c", "a//b", "a/b/", "ab", "aab", "a\nb", "a\n", "\na", "a.b", "a+b",
    "a(b)", "[a]", "a$", "^a", "a|b", "é", "éa", "ab/é", "x/a", "x/a/y", "a b", " a", "a\\b", "A", "*", "a*b",
];

/// Shaped like a real pattern list: base ignore, language defaults, gitignore lines, the common list.
const REAL_PATTERNS: &[&str] = &[
    "compiled_output.txt", "compiled_output*.txt", ".git/", ".gitignore", ".env", "*.env", ".env.*.local", ".env.local",
    "node_modules/", "dist/", "build/", "target", "target/", "*.lock", "*.log", "*.jar", "*.class", "*.o", "*.exe",
    "*.pyc", "__pycache__/", ".DS_Store", "Thumbs.db", "*.swp", "*~", ".idea/", ".vscode/", "*.pem", "*.key",
    ".gnupg/", "private-keys-v1.d/", "id_rsa*", "secring.*", "*.gpg", "LICENSE*", "README*", "*.md", "*.png",
    "*.min.js", "*.js.map", "package-lock.json", "service-account*.json", "*.tfstate.*", "npm-debug.log*",
    "docs/internal", "src/generated/*.rs", "community/*.lock", "/build", "*.egg-info/", ".venv/", "*lock*",
];
const REAL_PATHS: &[&str] = &[
    "Cargo.toml", "Cargo.lock", "LICENSE", "LICENSE-MIT", "README.md", ".env", ".env.example", ".env.production.local",
    "config.env", "src/main.rs", "src/lib.rs", "src/core/mod.rs", "src/ui/components/mod.rs", "community/Cargo.lock",
    "community/src/main.rs", "community/assets/config/defaults.json", "enterprise/src/license.rs", "target/debug/x.o",
    "target/release/dwarpala", "node_modules/left-pad/index.js", "dist/app.min.js", "dist/app.js.map", "build/out.o",
    "docs/internal/design.md", "docs/internal", "docs/public/index.md", "src/generated/api.rs", "src/generated/deep/api.rs",
    "src/targets.rs", ".git/config", ".idea/workspace.xml", ".vscode/settings.json", ".gnupg/pubring.kbx",
    ".gnupg/private-keys-v1.d/abc.key", "home/u/.ssh/id_rsa", "id_rsa.pub", "keys/server.pem", "secring.gpg",
    "secring.gpg.bak", "assets/logo.png", "pkg/__pycache__/m.cpython-312.pyc", ".DS_Store", "sub/.DS_Store",
    "Thumbs.db", "notes.txt~", ".main.rs.swp", "package-lock.json", "web/package-lock.json", "mylib.egg-info/PKG-INFO",
    ".venv/bin/python", "npm-debug.log.1", "gcp/service-account-prod.json", "infra/terraform.tfstate.backup",
    "compiled_output_20261108_120000.txt", "a b/c d.txt", "é/日本.txt",
];

// ── Tests ────────────────────────────────────────────────────────────────────

#[test]
fn diff_hand_picked_edge_cases() {
    let mut t = Tally::default();
    for p in HAND_PATTERNS { for r in HAND_PATHS { t.check(p, r); } }
    t.assert_clean("hand-picked");
}

#[test]
fn diff_realistic_pattern_list_against_realistic_paths() {
    let mut t = Tally::default();
    for p in REAL_PATTERNS { for r in REAL_PATHS { t.check(p, r); } }
    t.assert_clean("realistic"); // mismatches first, so a coverage failure can never hide one
    assert!(t.positives > 40, "fixture should contain real matches, got {}", t.positives);
}

#[test]
fn diff_random_fuzz() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut t = Tally::default();
    for _ in 0..200_000 {
        let (p, r) = (rng.string(PAT_ALPHA, 8), rng.string(PATH_ALPHA, 10));
        t.check(&p, &r);
    }
    for _ in 0..200_000 {
        let p = rng.string(PAT_ALPHA, 8);
        let r = derive_path(&mut rng, &p);
        t.check(&p, &r);
    }
    t.assert_clean("fuzz");
    assert!(t.positives > 50_000, "fuzz should exercise matches too, got {}", t.positives);
}

/// Not a correctness test. Same workload through the old and new matchers.
#[test]
#[ignore]
fn bench_regex_vs_glob() {
    use std::{hint::black_box, time::Instant};
    let mut rng = Rng(42);
    let paths: Vec<String> = (0..2_000)
        .map(|_| {
            let base = REAL_PATHS[rng.below(REAL_PATHS.len())];
            format!("{}/{base}", ["", "a", "src/x", "pkg/y/z"][rng.below(4)]).trim_start_matches('/').to_string()
        })
        .collect();
    let compiled: Vec<Compiled> = REAL_PATTERNS.iter().map(|p| Compiled::new(p)).collect();
    let wild = REAL_PATTERNS.iter().filter(|p| p.contains('*')).count();
    println!("\n{} patterns ({} wildcard) x {} paths", REAL_PATTERNS.len(), wild, paths.len());

    let t = Instant::now();
    for r in &paths { black_box(REAL_PATTERNS.iter().any(|p| matches_ignore_regex(black_box(r), p))); }
    let old = t.elapsed();
    let t = Instant::now();
    for r in &paths { black_box(REAL_PATTERNS.iter().any(|p| matches_ignore(black_box(r), p))); }
    let new = t.elapsed();
    let t = Instant::now();
    for r in &paths { black_box(compiled.iter().any(|c| c.is_match(black_box(r)))); }
    let pre = t.elapsed();

    println!("regex per call      : {old:?}");
    println!("glob per call       : {new:?}   ({:.0}x faster)", old.as_secs_f64() / new.as_secs_f64());
    println!("glob precompiled    : {pre:?}   ({:.0}x faster)", old.as_secs_f64() / pre.as_secs_f64());
}
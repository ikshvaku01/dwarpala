use crate::config::language_config_path;
use std::{fs, path::Path};
use walkdir::WalkDir;

/// Per-project-type ignore patterns shipped inside the binary.
/// Shape: `{ "rust": ["target", "*.lock"], "node": [...], ... }`.
const BUNDLED_DEFAULTS: &str = include_str!("../../../assets/config/defaults.json");

#[derive(Debug, Clone, PartialEq)]
pub enum ProjectType {
    Node, SpringBoot, Maven, DotNet, Rails, Python, Cpp, Rust, Unknown,
}

impl ProjectType {
    pub fn detect(root: &Path) -> Self {
        if root.join("package.json").exists() { return Self::Node; }
        if root.join("pom.xml").exists() {
            let is_spring = fs::read_to_string(root.join("pom.xml"))
                .map(|s| s.contains("spring-boot")).unwrap_or(false);
            return if is_spring { Self::SpringBoot } else { Self::Maven };
        }
        // Cheap marker check, so keep it ahead of the .csproj directory walk.
        // Also matches a Cargo workspace root, which has a Cargo.toml too.
        if root.join("Cargo.toml").exists() { return Self::Rust; }
        if WalkDir::new(root).max_depth(3).into_iter()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().ends_with(".csproj"))
        { return Self::DotNet; }
        if root.join("Gemfile").exists()          { return Self::Rails;  }
        if root.join("requirements.txt").exists() { return Self::Python; }
        if root.join("CMakeLists.txt").exists()   { return Self::Cpp;    }
        Self::Unknown
    }

    /// Every project type whose marker file exists (polyglot repos get several).
    pub fn detect_all(root: &Path) -> Vec<Self> {
        let mut v = Vec::new();
        if root.join("package.json").exists() { v.push(Self::Node); }
        if root.join("pom.xml").exists() {
            let is_spring = fs::read_to_string(root.join("pom.xml"))
                .map(|s| s.contains("spring-boot")).unwrap_or(false);
            v.push(if is_spring { Self::SpringBoot } else { Self::Maven });
        }
        if root.join("Cargo.toml").exists() { v.push(Self::Rust); }
        if WalkDir::new(root).max_depth(3).into_iter().filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().ends_with(".csproj"))
        { v.push(Self::DotNet); }
        if root.join("Gemfile").exists()          { v.push(Self::Rails);  }
        if root.join("requirements.txt").exists() { v.push(Self::Python); }
        if root.join("CMakeLists.txt").exists()   { v.push(Self::Cpp);    }
        v
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Node       => "node",
            Self::SpringBoot => "springboot",
            Self::Maven      => "maven",
            Self::DotNet     => "dotnet",
            Self::Rails      => "rails",
            Self::Python     => "python",
            Self::Cpp        => "cpp",
            Self::Rust       => "rust",
            Self::Unknown    => "unknown",
        }
    }

    /// Ignore patterns for this project type.
    ///
    /// 1. A user-written `language-config/<type>.json` replaces everything else.
    /// 2. Otherwise: the built-in fallback plus this type's entries from the
    ///    bundled `defaults.json`.
    pub fn language_ignores(&self) -> Vec<String> {
        let path = language_config_path(self.as_str());
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(arr) = json.get("ignore").and_then(|v| v.as_array()) {
                    return arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
                }
            }
        }
        self.builtin_ignores()
    }

    /// Everything that ships with the binary for this type.
    fn builtin_ignores(&self) -> Vec<String> {
        merge_unique(self.fallback_ignores(), parse_defaults(BUNDLED_DEFAULTS, self.as_str()))
    }

    /// Hard-coded safety net, kept even if `defaults.json` is edited or broken.
    fn fallback_ignores(&self) -> Vec<String> {
        match self {
            Self::Node               => vec!["node_modules/".into(), "dist/".into()],
            Self::SpringBoot | Self::Maven => vec!["target/".into()],
            Self::DotNet             => vec!["bin/".into(), "obj/".into()],
            Self::Rails              => vec!["log/".into(), "tmp/".into()],
            Self::Python             => vec!["__pycache__/".into(), "*.pyc".into()],
            Self::Cpp                => vec!["build/".into()],
            Self::Rust               => vec!["target/".into()],
            Self::Unknown            => vec![],
        }
    }
}

/// Compare patterns ignoring surrounding whitespace and a trailing `/`.
fn norm(p: &str) -> &str {
    p.trim().trim_end_matches('/')
}

/// Append `extra` entries that `base` doesn't already have (`target` is a
/// duplicate of `target/`).
fn merge_unique(mut base: Vec<String>, extra: Vec<String>) -> Vec<String> {
    for p in extra {
        let key = norm(&p);
        if key.is_empty() { continue; }
        if !base.iter().any(|q| norm(q) == key) {
            base.push(p);
        }
    }
    base
}

/// Read `key`'s string list from a `defaults.json` style object.
/// Anything malformed or missing yields an empty list, never an error.
fn parse_defaults(json: &str, key: &str) -> Vec<String> {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    root.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<String>>())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU32, Ordering},
    };

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// Make a throwaway project directory containing empty marker files.
    fn project_with(markers: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "dwarpala-detect-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&dir).unwrap();
        for m in markers {
            fs::write(dir.join(m), "").unwrap();
        }
        dir
    }

    fn detect(markers: &[&str]) -> ProjectType {
        let dir = project_with(markers);
        let found = ProjectType::detect(&dir);
        let _ = fs::remove_dir_all(&dir);
        found
    }

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn cargo_toml_is_a_rust_project() {
        let t = detect(&["Cargo.toml"]);
        assert_eq!(t, ProjectType::Rust);
        assert_eq!(t.as_str(), "rust");
    }

    #[test]
    fn existing_markers_keep_their_precedence() {
        assert_eq!(detect(&["package.json", "Cargo.toml"]), ProjectType::Node);
        assert_eq!(detect(&["pom.xml", "Cargo.toml"]), ProjectType::Maven);
    }

    #[test]
    fn empty_directory_is_still_unknown() {
        assert_eq!(detect(&[]), ProjectType::Unknown);
    }

    #[test]
    fn rust_falls_back_to_ignoring_target() {
        assert_eq!(ProjectType::Rust.fallback_ignores(), strings(&["target/"]));
    }

    #[test]
    fn parse_defaults_reads_one_key() {
        let json = r#"{"rust":["target","*.lock"],"node":[".env.local"]}"#;
        assert_eq!(parse_defaults(json, "rust"), strings(&["target", "*.lock"]));
        assert_eq!(parse_defaults(json, "node"), strings(&[".env.local"]));
    }

    #[test]
    fn parse_defaults_never_fails() {
        assert!(parse_defaults(r#"{"rust":["a"]}"#, "cpp").is_empty());
        assert!(parse_defaults("not json", "rust").is_empty());
        assert!(parse_defaults("", "rust").is_empty());
        assert_eq!(parse_defaults(r#"{"rust":["a",1,null,"b"]}"#, "rust"), strings(&["a", "b"]));
    }

    #[test]
    fn merge_skips_duplicates_with_or_without_trailing_slash() {
        let merged = merge_unique(strings(&["target/"]), strings(&["target", "*.lock", "*.lock", " "]));
        assert_eq!(merged, strings(&["target/", "*.lock"]));
    }

    #[test]
    fn bundled_defaults_file_is_a_valid_json_object() {
        let v: serde_json::Value = serde_json::from_str(BUNDLED_DEFAULTS).expect("defaults.json must parse");
        assert!(v.is_object());
    }

    #[test]
    fn every_bundled_default_reaches_its_project_type() {
        // Guards against defaults.json silently not being applied.
        let all = [
            ProjectType::Node, ProjectType::SpringBoot, ProjectType::Maven,
            ProjectType::DotNet, ProjectType::Rails, ProjectType::Python,
            ProjectType::Cpp, ProjectType::Rust, ProjectType::Unknown,
        ];
        for t in all {
            let applied = t.builtin_ignores();
            for want in parse_defaults(BUNDLED_DEFAULTS, t.as_str()) {
                assert!(
                    applied.iter().any(|q| norm(q) == norm(&want)),
                    "{}: `{want}` from defaults.json was not applied",
                    t.as_str()
                );
            }
        }
    }
    #[test]
    fn defaults_and_common_do_not_overlap() {
        let common = crate::ignore::rules::common_ignores(&Default::default());
        let root: serde_json::Value = serde_json::from_str(BUNDLED_DEFAULTS).unwrap();
        for (ty, list) in root.as_object().unwrap() {
            for p in list.as_array().unwrap().iter().filter_map(|v| v.as_str()) {
                assert!(
                    !common.iter().any(|c| norm(c) == norm(p)),
                    "`{p}` ({ty}) is already in common.json"
                );
            }
        }
    }
}
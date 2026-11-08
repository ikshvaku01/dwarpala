//! Collects dependency versions so the Credits tab can show them without hard-coding.
//!
//! Primary source: `[workspace.dependencies]` in the root Cargo.toml.
//! Fallback: this crate's own `[dependencies]`. When the crate is published, Cargo
//! rewrites `x.workspace = true` into a concrete version there, and the root
//! Cargo.toml is not part of the package.

use std::{collections::BTreeMap, env, fs, path::{Path, PathBuf}};

/// `name -> version` for every entry of the table at `section` in `path` that has a version.
/// Entries without one (`x.workspace = true`, path or git dependencies) are skipped.
/// A missing file or a malformed table yields an empty map.
fn versions_in(path: &Path, section: &[&str]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(raw) = fs::read_to_string(path) else { return out };
    let Ok(root) = toml::from_str::<toml::Table>(&raw) else { return out };

    let mut table = &root;
    for key in section {
        match table.get(*key).and_then(|v| v.as_table()) {
            Some(t) => table = t,
            None    => return out,
        }
    }
    for (name, spec) in table {
        let version = match spec {
            toml::Value::String(v) => Some(v.as_str()),                              // clap = "4"
            toml::Value::Table(t)  => t.get("version").and_then(|v| v.as_str()),     // clap = { version = "4", .. }
            _ => None,
        };
        if let Some(v) = version {
            out.insert(name.clone(), v.trim_start_matches(['^', '~', '=', ' ']).to_string());
        }
    }
    out
}

fn main() {
    let manifest  = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let own       = manifest.join("Cargo.toml");
    let workspace = manifest.join("..").join("Cargo.toml");

    // Only watch files that exist; a missing path would make Cargo rerun this every build.
    for p in [&own, &workspace] {
        if p.exists() { println!("cargo:rerun-if-changed={}", p.display()); }
    }

    let mut versions = versions_in(&workspace, &["workspace", "dependencies"]);
    for (name, v) in versions_in(&own, &["dependencies"]) {
        versions.entry(name).or_insert(v);
    }
    if versions.is_empty() {
        println!("cargo:warning=no dependency versions found; the Credits tab will show `?`");
    }

    let mut src = String::from("static DEP_VERSIONS: &[(&str, &str)] = &[\n");
    for (name, v) in &versions {
        src.push_str(&format!("    ({name:?}, {v:?}),\n"));
    }
    src.push_str("];\n");
    fs::write(PathBuf::from(env::var("OUT_DIR").unwrap()).join("dep_versions.rs"), src).unwrap();
}
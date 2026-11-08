use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};

#[derive(Serialize, Deserialize, Clone)]
pub struct Entry {
    pub kid: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub generated: String,
    #[serde(default)] pub disable: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub enc: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub mask: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub privatekey: String,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Store { #[serde(default)] pub keys: Vec<Entry> }

impl Store {
    pub fn get(&self, kid: &str) -> Option<&Entry> { self.keys.iter().find(|k| k.kid == kid) }

    /// Next unused `k<N>`; disabled kids count, so a kid is never reused.
    pub fn next_kid(&self) -> String {
        let max = self.keys.iter()
            .filter_map(|k| k.kid.strip_prefix('k')?.parse::<u32>().ok())
            .max().unwrap_or(0);
        format!("k{}", max + 1)
    }

    /// Newest enabled key that has a private key.
    pub fn newest_signing_key(&self) -> Option<&Entry> {
        self.keys.iter().rev().find(|k| !k.disable && !k.privatekey.is_empty())
    }
}

pub fn resolve_path(flag: Option<&str>) -> PathBuf {
    if let Some(p) = flag { return PathBuf::from(p); }
    if let Ok(p) = std::env::var("DWARPALA_KEYS_FILE") { return PathBuf::from(p); }
    // tools/sign_license -> tools -> repo root (next to rust-toolchain.toml)
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors().nth(2).expect("repo root")
        .join("license-keys.json")
}

pub fn load(path: &Path) -> Result<Store, String> {
    if !path.exists() { return Ok(Store::default()); }
    let raw = fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("{} is not valid: {e}", path.display()))
}

/// Write to a temp file, then rename, so a crash can't corrupt the store.
pub fn save(path: &Path, store: &Store) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    fs::write(&tmp, json).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&tmp, path).map_err(|e| format!("cannot replace {}: {e}", path.display()))
}

pub fn to_hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }

pub fn from_hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.is_ascii() { return None; }
    let mut out = [0u8; 32];
    for i in 0..32 { out[i] = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?; }
    Some(out)
}
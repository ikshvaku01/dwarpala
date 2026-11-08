use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::PathBuf};
use crate::output::OutputStyle;

#[derive(Deserialize, Serialize, Clone, Default)]
#[serde(default)]
pub struct AppConfig {
    pub output:      OutputConfig,
    pub features:    FeaturesConfig,
    #[serde(rename = "baseIgnore")]
    pub base_ignore: Vec<String>,
    #[serde(rename = "commonIgnore")]
    pub common_ignore: BTreeMap<String, bool>,
    pub privacy:     PrivacyConfig,
    /// Patterns `ensure_base_ignore` added for the compiled output file. Only these may be
    /// taken out of `baseIgnore` again by a reset. Hidden from config.json while empty.
    #[serde(rename = "outputIgnore", skip_serializing_if = "Vec::is_empty")]
    pub output_ignore: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct OutputConfig {
    pub filename:         String,
    #[serde(rename = "timestampSuffix")]
    pub timestamp_suffix: bool,
    pub path:             String,
    #[serde(rename = "customPath")]
    pub custom_path:      String,
    pub style:            OutputStyle,
}
impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            filename:         "compiled_output.txt".into(),
            timestamp_suffix: false,
            path:             "default".into(),
            custom_path:      String::new(),
            style:            OutputStyle::Banner,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Default)]
#[serde(default)]
pub struct FeaturesConfig {
    #[serde(rename = "dirStructure")]
    pub dir_structure:     bool,
    #[serde(rename = "additionalIgnore")]
    pub additional_ignore: bool,
    #[serde(rename = "gitIgnore")]
    pub git_ignore:        bool,
}

#[derive(Deserialize, Serialize, Clone, Default)]
#[serde(default)]
pub struct PrivacyConfig {
    pub ma_sirene:  Vec<String>,
    pub ecoute:     bool,
    pub hateration: Vec<String>,
}

pub fn config_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| dirs::home_dir().expect("no $HOME").join(".config"))
            .join("dwarpala")
    }
    #[cfg(not(target_os = "linux"))]
    {
        dirs::config_dir().expect("no config dir").join("dwarpala")
    }
}

pub fn config_path()           -> PathBuf { config_dir().join("config.json") }
pub fn additional_ignore_path() -> PathBuf {
    config_dir().join("language-config").join("additional-ignore.json")
}
pub fn language_config_path(project_type: &str) -> PathBuf {
    config_dir().join("language-config").join(format!("{project_type}.json"))
}

pub fn default_config() -> AppConfig {
    AppConfig {
        output: OutputConfig::default(),
        features: FeaturesConfig { dir_structure: true, additional_ignore: true, git_ignore: true },
        base_ignore: vec![
            "compiled_output.txt".into(), ".git/".into(),
            ".gitignore".into(), ".gitattributes".into(), ".gitmodules".into(),
        ],
        common_ignore: BTreeMap::new(),
        privacy: PrivacyConfig::default(),
        output_ignore: Vec::new(),
    }
}

pub fn load_or_create_config() -> Result<(AppConfig, bool)> {
    let path = config_path();
    if path.exists() {
        let raw = fs::read_to_string(&path)?;
        let cfg: AppConfig = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("Malformed {}: {e}", path.display()))?;
        return Ok((cfg, false));
    }
    fs::create_dir_all(config_dir())?;
    fs::create_dir_all(config_dir().join("language-config"))?;
    let cfg  = default_config();
    let json = serde_json::to_string_pretty(&cfg)?;
    fs::write(&path, json)?;
    Ok((cfg, true))
}

/// Set one value at a JSON path such as `["output", "style"]`.
fn set_path(root: &mut serde_json::Value, path: &[&str], value: serde_json::Value) {
    let mut node = root;
    for key in path {
        node = &mut node[*key]; // creates missing objects on the way down
    }
    *node = value;
}

/// Persist a single setting, leaving every other key in the file untouched.
pub fn save_setting(path: &[&str], value: serde_json::Value) -> Result<()> {
    load_or_create_config()?;   // creates the file if missing; errors (without overwriting) if malformed
    let file = config_path();
    let mut root: serde_json::Value = serde_json::from_str(&fs::read_to_string(&file)?)?;
    set_path(&mut root, path, value);
    fs::write(&file, serde_json::to_string_pretty(&root)?)?;
    Ok(())
}

/// Append `pattern` to the string list at `root[key]` unless an entry with the same text is
/// already there. Returns true when it was added; creates the list if it is missing.
fn push_unique(root: &mut serde_json::Value, key: &str, pattern: &str) -> bool {
    let mut list = root.get(key)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if list.iter().any(|v| v.as_str().map(str::trim) == Some(pattern)) {
        return false;
    }
    list.push(serde_json::Value::from(pattern));
    set_path(root, &[key], serde_json::Value::Array(list));
    true
}

/// Remove every entry of the string list at `root[key]` that is in `stale`.
/// Returns what was removed; a missing list or no match leaves `root` untouched.
fn drop_from_list(root: &mut serde_json::Value, key: &str, stale: &[String]) -> Vec<String> {
    let Some(list) = root.get(key).and_then(|v| v.as_array()) else { return Vec::new() };
    let (gone, kept): (Vec<serde_json::Value>, Vec<serde_json::Value>) = list.iter().cloned()
        .partition(|v| {
            v.as_str().map(str::trim)
                .is_some_and(|s| stale.iter().any(|x| x.as_str() == s))
        });
    if gone.is_empty() {
        return Vec::new();
    }
    set_path(root, &[key], serde_json::Value::Array(kept));
    gone.iter().filter_map(|v| v.as_str().map(String::from)).collect()
}

/// Make sure `baseIgnore` contains `pattern`, both in `config` and in config.json.
/// A pattern that was missing is also recorded in `outputIgnore`, so a reset can remove it
/// again. Entries that were already there (stock or hand-written) are never recorded.
/// Only those keys of the file are touched. Returns true if anything was added.
/// The in-memory lists are updated first, so this session is covered even if the write fails.
pub fn ensure_base_ignore(config: &mut AppConfig, pattern: &str) -> Result<bool> {
    let mut added = false;
    if !config.base_ignore.iter().any(|p| p.trim() == pattern) {
        config.base_ignore.push(pattern.to_string());
        if !config.output_ignore.iter().any(|p| p.as_str() == pattern) {
            config.output_ignore.push(pattern.to_string());
        }
        added = true;
    }
    load_or_create_config()?;   // creates the file if missing; errors (without overwriting) if malformed
    let file = config_path();
    let mut root: serde_json::Value = serde_json::from_str(&fs::read_to_string(&file)?)?;
    if push_unique(&mut root, "baseIgnore", pattern) {
        push_unique(&mut root, "outputIgnore", pattern);
        fs::write(&file, serde_json::to_string_pretty(&root)?)?;
        added = true;
    }
    Ok(added)
}

/// What a reset may take out of `baseIgnore`: everything in `outputIgnore` plus `also`,
/// minus `keep` and the stock entries.
fn stale_patterns(config: &AppConfig, also: &[String], keep: &str) -> Vec<String> {
    let stock = default_config().base_ignore;
    let mut stale = config.output_ignore.clone();
    stale.extend(also.iter().cloned());
    stale.retain(|p| p != keep && !stock.contains(p));
    stale
}

/// Undo the automatic `baseIgnore` entries after a reset.
///
/// Removes the patterns recorded in `outputIgnore` plus `also` (what the output name could
/// produce before the reset; this also cleans up entries written before `outputIgnore`
/// existed). Never removes `keep`, the stock entries, or anything written by hand.
/// Returns what was removed from `config`; the `Result` is the config.json write.
/// The in-memory lists are updated first, so this session is right even if the write fails.
pub fn release_base_ignore(
    config: &mut AppConfig, also: &[String], keep: &str,
) -> (Vec<String>, Result<()>) {
    let stale = stale_patterns(config, also, keep);
    let mut removed = Vec::new();
    config.base_ignore.retain(|p| {
        let gone = stale.iter().any(|s| s.as_str() == p.trim());
        if gone { removed.push(p.clone()); }
        !gone
    });
    config.output_ignore.retain(|p| !stale.contains(p));
    let written = write_release(&stale);
    (removed, written)
}

fn write_release(stale: &[String]) -> Result<()> {
    load_or_create_config()?;   // errors (without overwriting) if config.json is malformed
    let file = config_path();
    let mut root: serde_json::Value = serde_json::from_str(&fs::read_to_string(&file)?)?;
    let mut changed = !drop_from_list(&mut root, "baseIgnore", stale).is_empty();
    changed |= !drop_from_list(&mut root, "outputIgnore", stale).is_empty();
    // An empty tracking list is deleted, so config.json goes back to its stock shape.
    // `shift_remove` keeps the other keys in order (the workspace enables preserve_order).
    let emptied = root.get("outputIgnore")
        .and_then(|v| v.as_array())
        .is_some_and(|a| a.is_empty());
    if emptied {
        if let Some(obj) = root.as_object_mut() { obj.shift_remove("outputIgnore"); }
        changed = true;
    }
    if changed {
        fs::write(&file, serde_json::to_string_pretty(&root)?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_defaults_to_banner() {
        let c: AppConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(c.output.style, OutputStyle::Banner);
    }

    #[test]
    fn style_is_read_from_output_block() {
        let c: AppConfig = serde_json::from_str(r#"{"output":{"style":"comment"}}"#).unwrap();
        assert_eq!(c.output.style, OutputStyle::Comment);
        assert_eq!(c.output.filename, "compiled_output.txt"); // siblings still default
    }

    #[test]
    fn unknown_style_is_rejected() {
        assert!(serde_json::from_str::<AppConfig>(r#"{"output":{"style":"fancy"}}"#).is_err());
    }

    #[test]
    fn set_path_touches_only_its_key() {
        let mut v = serde_json::json!({
        "output": { "style": "banner", "filename": "x.txt" },
        "features": { "dirStructure": true }
    });
        set_path(&mut v, &["output", "style"], serde_json::json!("comment"));
        assert_eq!(v["output"]["style"], "comment");
        assert_eq!(v["output"]["filename"], "x.txt");
        assert_eq!(v["features"]["dirStructure"], true);
    }

    #[test]
    fn push_unique_adds_once_and_keeps_the_rest() {
        let mut v = serde_json::json!({
            "baseIgnore": [".git/"],
            "output": { "style": "comment" }
        });
        assert!(push_unique(&mut v, "baseIgnore", "mini_*.txt"));
        assert!(!push_unique(&mut v, "baseIgnore", "mini_*.txt"));
        assert_eq!(v["baseIgnore"], serde_json::json!([".git/", "mini_*.txt"]));
        assert_eq!(v["output"]["style"], "comment");
    }

    #[test]
    fn push_unique_creates_a_missing_list() {
        let mut v = serde_json::json!({});
        assert!(push_unique(&mut v, "baseIgnore", "a.txt"));
        assert_eq!(v["baseIgnore"], serde_json::json!(["a.txt"]));
    }

    #[test]
    fn drop_from_list_removes_only_the_stale_entries() {
        let mut v = serde_json::json!({
            "baseIgnore": [".git/", "a.txt", "a_*.txt", "mine.txt"],
            "output": { "style": "comment" }
        });
        let stale = vec!["a.txt".to_string(), "a_*.txt".to_string()];
        assert_eq!(drop_from_list(&mut v, "baseIgnore", &stale), ["a.txt", "a_*.txt"]);
        assert_eq!(v["baseIgnore"], serde_json::json!([".git/", "mine.txt"]));
        assert_eq!(v["output"]["style"], "comment");
        assert!(drop_from_list(&mut v, "baseIgnore", &stale).is_empty());
        assert!(drop_from_list(&mut v, "missing", &stale).is_empty());
    }

    #[test]
    fn stale_patterns_skip_stock_entries_and_the_one_to_keep() {
        let mut c = default_config();
        c.output_ignore = vec!["mini.txt".into(), "mini_*.txt".into()];
        let also = vec!["compiled_output.txt".to_string(), "compiled_output_*.txt".to_string()];
        assert_eq!(
            stale_patterns(&c, &also, "mini.txt"),
            ["mini_*.txt", "compiled_output_*.txt"]
        );
    }

    #[test]
    fn output_ignore_is_hidden_until_it_is_used() {
        let v = serde_json::to_value(default_config()).unwrap();
        assert!(v.get("outputIgnore").is_none());
        let c: AppConfig = serde_json::from_str(r#"{"outputIgnore":["a.txt"]}"#).unwrap();
        assert_eq!(c.output_ignore, ["a.txt"]);
    }
}
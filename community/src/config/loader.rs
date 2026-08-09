use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Deserialize, Serialize, Clone, Default)]
#[serde(default)]
pub struct AppConfig {
    pub output:      OutputConfig,
    pub features:    FeaturesConfig,
    #[serde(rename = "baseIgnore")]
    pub base_ignore: Vec<String>,
    pub privacy:     PrivacyConfig,
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
}
impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            filename:         "compiled_output.txt".into(),
            timestamp_suffix: false,
            path:             "default".into(),
            custom_path:      String::new(),
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

fn default_config() -> AppConfig {
    AppConfig {
        output: OutputConfig::default(),
        features: FeaturesConfig { dir_structure: true, additional_ignore: true, git_ignore: true },
        base_ignore: vec![
            "compiled_output.txt".into(), ".git/".into(),
            ".gitignore".into(), ".gitattributes".into(), ".gitmodules".into(),
        ],
        privacy: PrivacyConfig::default(),
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
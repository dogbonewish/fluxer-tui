use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_API_BASE_URL: &str = "https://api.fluxer.app/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    pub clock_12h: bool,
    #[serde(default, skip_serializing, rename = "image_display")]
    legacy_image_display: Option<String>,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            clock_12h: false,
            legacy_image_display: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_api_base_url")]
    pub api_base_url: String,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub last_server_id: Option<String>,
    #[serde(default)]
    pub last_channel_id: Option<String>,
    #[serde(default)]
    pub ui: UiSettings,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_base_url: default_api_base_url(),
            token: None,
            last_server_id: None,
            last_channel_id: None,
            ui: UiSettings::default(),
        }
    }
}

pub fn default_api_base_url() -> String {
    DEFAULT_API_BASE_URL.to_string()
}

pub fn default_config_path() -> Result<PathBuf> {
    let base = dirs::config_dir().context("could not determine config directory")?;
    Ok(base.join("fluxer-tui").join("config.toml"))
}

pub fn load_config(path: &Path) -> Result<AppConfig> {
    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;
    let config = toml::from_str::<AppConfig>(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(config)
}

pub fn save_config(path: &Path, config: &AppConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config directory {}", parent.display()))?;
    }

    let serialized = toml::to_string_pretty(config).context("failed to serialize config")?;
    fs::write(path, serialized).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

use serde::Deserialize;
use std::path::Path;

use crate::terminal::terminal::DisplayMode;

#[derive(Debug, Deserialize, Default)]
pub struct GlobalConfig {
    #[serde(default = "default_terminal_type")]
    pub default_terminal_type: String,
    #[serde(default = "default_timeout")]
    pub default_timeout: u32,
    #[serde(default = "default_display_mode")]
    pub default_display_mode: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_terminal_type() -> String {
    "xterm-256color".to_string()
}
fn default_timeout() -> u32 {
    30
}
fn default_display_mode() -> String {
    "raw".to_string()
}
fn default_log_level() -> String {
    "info".to_string()
}

#[derive(Debug, Deserialize, Default)]
pub struct ServerConfig {
    #[serde(default)]
    pub name: String,
    pub hostname: Option<String>,
    #[serde(default = "default_port")]
    pub port: u16,
    pub terminal_type: Option<String>,
    pub username: Option<String>,
}

fn default_port() -> u16 {
    23
}

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub global: GlobalConfig,
    #[serde(default)]
    pub servers: std::collections::HashMap<String, ServerConfig>,
}

impl Config {
    pub fn load(path: &str) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)?;
        log::info!("Loaded config from: {}", path);
        Ok(config)
    }

    pub fn load_default() -> Option<Self> {
        let path = Self::get_default_config_path()?;
        if !Path::new(&path).exists() {
            log::debug!("Default config not found: {}", path);
            return None;
        }
        match Self::load(&path) {
            Ok(config) => Some(config),
            Err(e) => {
                log::warn!("Failed to load default config: {}", e);
                None
            }
        }
    }

    pub fn get_default_config_path() -> Option<String> {
        let exe = std::env::current_exe().ok()?;
        let exe_dir = exe.parent()?;
        let config_path = exe_dir
            .parent()?
            .join("etc")
            .join("telcli")
            .join("telcli.json");
        Some(config_path.to_string_lossy().to_string())
    }

    pub fn display_mode(&self) -> DisplayMode {
        match self.global.default_display_mode.as_str() {
            "ignore" => DisplayMode::Ignore,
            "hex" => DisplayMode::Hex,
            "placeholder" => DisplayMode::Placeholder,
            _ => DisplayMode::Raw,
        }
    }
}

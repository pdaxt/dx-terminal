use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Polling interval in milliseconds
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,

    /// Number of lines to capture from pane
    #[serde(default = "default_capture_lines")]
    pub capture_lines: u32,

    /// Hub API URL (e.g. http://localhost:3100)
    #[serde(default)]
    pub api_url: Option<String>,

    /// Native PTY mode — own processes directly instead of using tmux
    #[serde(default)]
    pub native_mode: bool,
}

fn default_poll_interval() -> u64 {
    500
}

fn default_capture_lines() -> u32 {
    100
}

impl Default for Config {
    fn default() -> Self {
        Self {
            poll_interval_ms: default_poll_interval(),
            capture_lines: default_capture_lines(),
            api_url: None,
            native_mode: false,
        }
    }
}

impl Config {
    /// Returns the default config file path
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("dx-terminal").join("config.toml"))
    }

    /// Loads config from the default path or returns defaults
    pub fn load() -> Self {
        Self::default_path()
            .and_then(|path| {
                if path.exists() {
                    Self::load_from(&path).ok()
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }

    /// Loads config from a specific path
    pub fn load_from(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Saves config to the default path
    pub fn save(&self) -> Result<()> {
        if let Some(path) = Self::default_path() {
            self.save_to(&path)?;
        }
        Ok(())
    }

    /// Saves config to a specific path
    pub fn save_to(&self, path: &PathBuf) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.poll_interval_ms, 500);
        assert_eq!(config.capture_lines, 100);
        assert!(!config.native_mode);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(config.poll_interval_ms, parsed.poll_interval_ms);
    }
}

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelProvider {
    Anthropic,
    OpenAI,
    Google,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider: ModelProvider,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            provider: ModelProvider::Anthropic,
            model: "claude-sonnet-4-20250514".to_string(),
            api_key: None,
            base_url: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DxosConfig {
    pub provider: ProviderConfig,
    pub max_turns: usize,
    pub permission_mode: String,
    pub data_dir: PathBuf,
}

impl Default for DxosConfig {
    fn default() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("dxos");
        Self {
            provider: ProviderConfig::default(),
            max_turns: 16,
            permission_mode: "workspace-write".to_string(),
            data_dir,
        }
    }
}

impl DxosConfig {
    pub fn load() -> Self {
        let config_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("dxos")
            .join("config.toml");

        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(config) = toml_from_str(&content) {
                    return config;
                }
            }
        }

        Self::default()
    }
}

fn toml_from_str(s: &str) -> std::result::Result<DxosConfig, String> {
    // Minimal TOML parsing — will upgrade to `toml` crate when config grows
    let _ = s;
    Ok(DxosConfig::default())
}

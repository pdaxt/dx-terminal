use std::process::Command;

use dx_agent_core::{ModelProvider, ProviderConfig, Result};
use dx_agent_harness::{ApiClient, ApiRequest, AssistantEvent};

use crate::anthropic::AnthropicClient;
use crate::ollama::OllamaClient;

/// Provider-neutral client that dispatches to the correct backend.
pub enum ProviderClient {
    Anthropic(AnthropicClient),
    Local(OllamaClient),
    OpenAI { client: OllamaClient },
}

impl ProviderClient {
    pub fn from_config(config: &ProviderConfig) -> Result<Self> {
        match config.provider {
            ModelProvider::Anthropic => {
                let api_key = resolve_api_key("ANTHROPIC_API_KEY", &config.api_key)?;
                Ok(Self::Anthropic(AnthropicClient::new(
                    api_key,
                    config.model.clone(),
                    config.base_url.clone(),
                )))
            }
            ModelProvider::Local => {
                tracing::info!("Using local model via Ollama: {}", config.model);
                Ok(Self::Local(OllamaClient::new(
                    config.model.clone(),
                    config.base_url.clone(),
                )))
            }
            ModelProvider::OpenAI => {
                let api_key = resolve_api_key("OPENAI_API_KEY", &config.api_key)?;
                let base_url = config
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://api.openai.com".to_string());
                // OpenAI-compatible: works with OpenAI, OpenRouter, Together, etc.
                tracing::info!("Using OpenAI-compatible API at {base_url}");
                Ok(Self::OpenAI {
                    client: OllamaClient::new_with_key(
                        config.model.clone(),
                        Some(base_url),
                        Some(api_key),
                    ),
                })
            }
            ModelProvider::Google => Err(dx_agent_core::DxosError::Config(
                "Google provider coming soon".into(),
            )),
        }
    }

    /// Auto-detect the best available provider.
    /// Priority: Ollama (free) → Anthropic (env/vault) → error
    pub fn auto_detect(model_hint: Option<&str>) -> Result<(Self, String)> {
        // 1. Check if Ollama is running with a suitable model
        if let Ok(output) = Command::new("ollama").args(["list"]).output() {
            if output.status.success() {
                let list = String::from_utf8_lossy(&output.stdout);
                // Prefer qwen3, then deepseek-r1, then llama, then mistral
                // Order: best coding models first
                let preferred = [
                    "qwen2.5-coder",
                    "qwen3-coder",
                    "devstral",
                    "qwen3",
                    "llama3",
                    "mistral",
                ];

                if let Some(hint) = model_hint {
                    // User specified a model — check if it's available locally
                    if list.contains(hint) {
                        let config = ProviderConfig {
                            provider: ModelProvider::Local,
                            model: hint.to_string(),
                            api_key: None,
                            base_url: None,
                        };
                        return Ok((Self::from_config(&config)?, hint.to_string()));
                    }
                }

                for pref in &preferred {
                    for line in list.lines().skip(1) {
                        if line.contains(pref) {
                            let model_name = line.split_whitespace().next().unwrap_or(pref);
                            let config = ProviderConfig {
                                provider: ModelProvider::Local,
                                model: model_name.to_string(),
                                api_key: None,
                                base_url: None,
                            };
                            return Ok((Self::from_config(&config)?, model_name.to_string()));
                        }
                    }
                }
            }
        }

        // 2. Check for Anthropic API key
        if resolve_api_key("ANTHROPIC_API_KEY", &None).is_ok() {
            let model = model_hint.unwrap_or("claude-sonnet-4-20250514").to_string();
            let config = ProviderConfig {
                provider: ModelProvider::Anthropic,
                model: model.clone(),
                api_key: None,
                base_url: None,
            };
            return Ok((Self::from_config(&config)?, model));
        }

        // 3. Check for OpenAI API key
        if resolve_api_key("OPENAI_API_KEY", &None).is_ok() {
            let model = model_hint.unwrap_or("gpt-4o").to_string();
            let config = ProviderConfig {
                provider: ModelProvider::OpenAI,
                model: model.clone(),
                api_key: None,
                base_url: None,
            };
            return Ok((Self::from_config(&config)?, model));
        }

        // 4. Check for OpenRouter API key (100+ models through one key)
        if let Ok(key) = resolve_api_key("OPENROUTER_API_KEY", &None) {
            let model = model_hint
                .unwrap_or("anthropic/claude-sonnet-4")
                .to_string();
            let client = OllamaClient::new_with_key(
                model.clone(),
                Some("https://openrouter.ai/api".to_string()),
                Some(key),
            );
            tracing::info!("Using OpenRouter: {model}");
            return Ok((Self::OpenAI { client }, model));
        }

        Err(dx_agent_core::DxosError::Config(
            "No model available. Options:\n  \
             1. Install Ollama (ollama.com) — free, local\n  \
             2. export ANTHROPIC_API_KEY=sk-...\n  \
             3. export OPENAI_API_KEY=sk-...\n  \
             4. export OPENROUTER_API_KEY=sk-... (100+ models)"
                .into(),
        ))
    }
}

/// Resolve an API key from (in order):
/// 1. Explicit config value
/// 2. Environment variable
/// 3. PQVault CLI
/// 4. macOS Keychain
fn resolve_api_key(env_name: &str, config_value: &Option<String>) -> Result<String> {
    if let Some(key) = config_value {
        if !key.is_empty() {
            return Ok(key.clone());
        }
    }

    if let Ok(key) = std::env::var(env_name) {
        if !key.is_empty() {
            return Ok(key);
        }
    }

    if let Ok(output) = Command::new("pqvault").args(["get", env_name]).output() {
        if output.status.success() {
            let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !key.is_empty() && key.starts_with("sk-") {
                tracing::info!("Loaded {env_name} from PQVault");
                return Ok(key);
            }
        }
    }

    if let Ok(output) = Command::new("security")
        .args(["find-generic-password", "-s", env_name, "-w"])
        .output()
    {
        if output.status.success() {
            let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !key.is_empty() && key.starts_with("sk-") {
                tracing::info!("Loaded {env_name} from macOS Keychain");
                return Ok(key);
            }
        }
    }

    Err(dx_agent_core::DxosError::Config(format!(
        "{env_name} not found"
    )))
}

impl ApiClient for ProviderClient {
    fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>> {
        match self {
            Self::Anthropic(client) => client.stream(request),
            Self::Local(client) => client.stream(request),
            Self::OpenAI { client } => client.stream(request),
        }
    }

    fn stream_with_callback(
        &mut self,
        request: ApiRequest,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<Vec<AssistantEvent>> {
        match self {
            Self::Anthropic(client) => client.stream_with_callback(request, on_token),
            Self::Local(client) => client.stream_with_callback(request, on_token),
            Self::OpenAI { client } => client.stream_with_callback(request, on_token),
        }
    }
}

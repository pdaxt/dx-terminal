use anyhow::Result;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const BRIDGE_VERSION: u32 = 1;
const SHARED_SOURCE: &str = "dx";
const CLAUDE_SOURCE: &str = "claude";
const CODEX_SOURCE: &str = "codex";
const GEMINI_SOURCE: &str = "gemini";

#[derive(Debug, Clone)]
pub struct ProviderImportSource {
    pub provider: String,
    pub mode: String,
    pub path: PathBuf,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderPluginTarget {
    pub provider: String,
    pub label: String,
    pub mode: String,
    pub format: String,
    pub path: String,
    pub exists: bool,
    pub import_supported: bool,
    pub export_supported: bool,
    pub imported_servers: usize,
    pub available_servers: usize,
}

pub fn claude_plugin_path() -> PathBuf {
    crate::config::home_dir()
        .join(".claude")
        .join("dx-provider-plugin.json")
}

pub fn codex_plugin_path() -> PathBuf {
    crate::config::home_dir()
        .join(".codex")
        .join("dx-provider-plugin.toml")
}

pub fn gemini_plugin_path() -> PathBuf {
    crate::config::home_dir()
        .join(".gemini")
        .join("dx-provider-plugin.json")
}

pub fn normalized_provider(provider: &str) -> &'static str {
    match provider.trim().to_lowercase().as_str() {
        "dx" | "shared" | "shared_manifest" => SHARED_SOURCE,
        "openai" | "gpt" | "chatgpt" | "codex" => CODEX_SOURCE,
        "google" | "gemini" => GEMINI_SOURCE,
        _ => CLAUDE_SOURCE,
    }
}

pub fn provider_label(provider: &str) -> &'static str {
    match normalized_provider(provider) {
        SHARED_SOURCE => "DX shared manifest",
        CODEX_SOURCE => "Codex / GPT bridge",
        GEMINI_SOURCE => "Gemini bridge",
        _ => "Claude bridge",
    }
}

pub fn provider_plugin_path(provider: &str) -> PathBuf {
    match normalized_provider(provider) {
        CODEX_SOURCE => codex_plugin_path(),
        GEMINI_SOURCE => gemini_plugin_path(),
        _ => claude_plugin_path(),
    }
}

pub fn catalog_import_sources() -> Vec<ProviderImportSource> {
    let mut sources = Vec::new();
    sources.push(ProviderImportSource {
        provider: CLAUDE_SOURCE.to_string(),
        mode: "native_import".to_string(),
        path: crate::config::claude_json_path(),
        payload: crate::claude::read_claude_config(),
    });

    let claude_plugin = read_json_plugin(&claude_plugin_path());
    if !claude_plugin.is_null() {
        sources.push(ProviderImportSource {
            provider: CLAUDE_SOURCE.to_string(),
            mode: "dx_plugin".to_string(),
            path: claude_plugin_path(),
            payload: claude_plugin,
        });
    }

    let codex_plugin = read_codex_plugin(&codex_plugin_path());
    if !codex_plugin.is_null() {
        sources.push(ProviderImportSource {
            provider: CODEX_SOURCE.to_string(),
            mode: "dx_plugin".to_string(),
            path: codex_plugin_path(),
            payload: codex_plugin,
        });
    }

    let gemini_plugin = read_json_plugin(&gemini_plugin_path());
    if !gemini_plugin.is_null() {
        sources.push(ProviderImportSource {
            provider: GEMINI_SOURCE.to_string(),
            mode: "dx_plugin".to_string(),
            path: gemini_plugin_path(),
            payload: gemini_plugin,
        });
    }
    sources
}

pub fn plugin_inventory() -> Value {
    let catalog = crate::external_mcp::load_external_catalog();
    let mut providers = Vec::new();
    for provider in [CLAUDE_SOURCE, CODEX_SOURCE, GEMINI_SOURCE] {
        let path = provider_plugin_path(provider);
        let imported_servers = catalog
            .iter()
            .filter(|entry| {
                entry.sources.iter().any(|source| {
                    let normalized = normalized_provider(source);
                    normalized == provider
                })
            })
            .count();
        providers.push(ProviderPluginTarget {
            provider: provider.to_string(),
            label: provider_label(provider).to_string(),
            mode: if provider == CLAUDE_SOURCE {
                "native_import_plus_plugin_export".to_string()
            } else {
                "plugin_bridge".to_string()
            },
            format: if provider == CODEX_SOURCE {
                "toml".to_string()
            } else {
                "json".to_string()
            },
            path: path.to_string_lossy().to_string(),
            exists: path.exists(),
            import_supported: true,
            export_supported: true,
            imported_servers,
            available_servers: catalog.len(),
        });
    }

    let translation_matrix = [CLAUDE_SOURCE, CODEX_SOURCE, GEMINI_SOURCE]
        .into_iter()
        .flat_map(|source| {
            [CLAUDE_SOURCE, CODEX_SOURCE, GEMINI_SOURCE]
                .into_iter()
                .filter(move |target| *target != source)
                .map(move |target| {
                    json!({
                        "source": source,
                        "target": target,
                        "via": "dx_shared_manifest",
                    })
                })
        })
        .collect::<Vec<_>>();

    json!({
        "shared_catalog_path": crate::external_mcp::shared_catalog_path(),
        "shared_catalog_count": catalog.len(),
        "providers": providers,
        "translation_matrix": translation_matrix,
        "source_of_truth": "dx_shared_manifest",
        "bridge_contract": {
            "claude": {
                "native_import": crate::config::claude_json_path(),
                "plugin_export": claude_plugin_path(),
            },
            "codex": {
                "plugin_export": codex_plugin_path(),
            },
            "gemini": {
                "plugin_export": gemini_plugin_path(),
            }
        }
    })
}

pub fn convert_provider_plugin(
    source_provider: Option<&str>,
    target_provider: &str,
    dry_run: bool,
) -> Result<Value> {
    let target = normalized_provider(target_provider);
    let source = source_provider
        .map(normalized_provider)
        .map(str::to_string)
        .unwrap_or_else(|| SHARED_SOURCE.to_string());

    let filtered = filtered_catalog(source_provider);
    let target_path = provider_plugin_path(target);
    let payload = match target {
        CODEX_SOURCE => render_codex_plugin_payload(&filtered),
        GEMINI_SOURCE => render_json_plugin_payload(target, &filtered),
        _ => render_json_plugin_payload(target, &filtered),
    };

    if !dry_run {
        match target {
            CODEX_SOURCE => write_codex_plugin(&target_path, &filtered)?,
            _ => write_json_plugin(&target_path, &payload)?,
        }
    }

    Ok(json!({
        "ok": true,
        "source": source,
        "target": target,
        "dry_run": dry_run,
        "path": target_path,
        "exported_servers": filtered.len(),
        "available_servers": crate::external_mcp::load_external_catalog().len(),
        "mode": if target == CLAUDE_SOURCE {
            "plugin_export"
        } else {
            "plugin_bridge"
        },
        "payload": payload,
    }))
}

fn filtered_catalog(source_provider: Option<&str>) -> Vec<crate::external_mcp::ExternalMcpEntry> {
    let normalized_source = source_provider.map(normalized_provider);
    crate::external_mcp::load_external_catalog()
        .into_iter()
        .filter(|entry| match normalized_source {
            None => true,
            Some(source) if source == SHARED_SOURCE => true,
            Some(source) => {
                let has_source = entry
                    .sources
                    .iter()
                    .any(|candidate| normalized_provider(candidate) == source);
                let has_dx = entry
                    .sources
                    .iter()
                    .any(|candidate| candidate.trim().eq_ignore_ascii_case(SHARED_SOURCE));
                has_source || has_dx
            }
        })
        .collect()
}

fn read_json_plugin(path: &Path) -> Value {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Value::Null;
    };
    let Ok(parsed) = serde_json::from_str::<Value>(&raw) else {
        return Value::Null;
    };
    parsed
        .get("dxProviderPlugin")
        .and_then(|value| value.get("mcpServers").map(|servers| json!({ "mcpServers": servers })))
        .unwrap_or(Value::Null)
}

fn read_codex_plugin(path: &Path) -> Value {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Value::Null;
    };
    let Ok(parsed) = raw.parse::<toml::Value>() else {
        return Value::Null;
    };
    let Some(servers) = parsed
        .get("dx_provider_plugin")
        .and_then(|value| value.get("mcp_servers"))
        .and_then(|value| value.as_table())
    else {
        return Value::Null;
    };

    let mut mcp_servers = serde_json::Map::new();
    for (name, server) in servers {
        let Some(table) = server.as_table() else {
            continue;
        };
        let args = table
            .get("args")
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(|value| value.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let env = table
            .get("env")
            .and_then(|value| value.as_table())
            .map(|env| {
                env.iter()
                    .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), json!(value))))
                    .collect::<serde_json::Map<_, _>>()
            })
            .unwrap_or_default();
        let array_field = |key: &str| {
            table
                .get(key)
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(|value| json!(value)))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        mcp_servers.insert(
            name.clone(),
            json!({
                "command": table.get("command").and_then(|value| value.as_str()).unwrap_or(""),
                "args": args,
                "env": env,
                "description": table.get("description").and_then(|value| value.as_str()).unwrap_or(""),
                "capabilities": array_field("capabilities"),
                "projects": array_field("projects"),
                "keywords": array_field("keywords"),
                "category": table.get("category").and_then(|value| value.as_str()).unwrap_or("general"),
            }),
        );
    }
    json!({ "mcpServers": mcp_servers })
}

fn render_json_plugin_payload(
    provider: &str,
    entries: &[crate::external_mcp::ExternalMcpEntry],
) -> Value {
    json!({
        "dxProviderPlugin": {
            "version": BRIDGE_VERSION,
            "provider": provider,
            "sourceOfTruth": SHARED_SOURCE,
            "exportedAt": unix_timestamp(),
            "mcpServers": render_server_map(entries),
        }
    })
}

fn render_codex_plugin_payload(entries: &[crate::external_mcp::ExternalMcpEntry]) -> Value {
    let mut root = serde_json::Map::new();
    let mut plugin = serde_json::Map::new();
    plugin.insert("version".to_string(), json!(BRIDGE_VERSION));
    plugin.insert("provider".to_string(), json!(CODEX_SOURCE));
    plugin.insert("sourceOfTruth".to_string(), json!(SHARED_SOURCE));
    plugin.insert("exportedAt".to_string(), json!(unix_timestamp()));
    plugin.insert("mcpServers".to_string(), render_server_map(entries));
    root.insert("dxProviderPlugin".to_string(), Value::Object(plugin));
    Value::Object(root)
}

fn render_server_map(entries: &[crate::external_mcp::ExternalMcpEntry]) -> Value {
    let mut servers = serde_json::Map::new();
    for entry in entries {
        let env = entry
            .env
            .iter()
            .map(|(key, value)| (key.clone(), json!(value)))
            .collect::<serde_json::Map<_, _>>();
        servers.insert(
            entry.name.clone(),
            json!({
                "command": entry.command,
                "args": entry.args,
                "env": env,
                "description": entry.description,
                "capabilities": entry.capabilities,
                "projects": entry.projects,
                "keywords": entry.keywords,
                "category": entry.category,
            }),
        );
    }
    Value::Object(servers)
}

fn write_json_plugin(path: &Path, payload: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(payload)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn write_codex_plugin(path: &Path, entries: &[crate::external_mcp::ExternalMcpEntry]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut root = toml::map::Map::new();
    let mut plugin = toml::map::Map::new();
    plugin.insert("version".to_string(), toml::Value::Integer(BRIDGE_VERSION.into()));
    plugin.insert("provider".to_string(), toml::Value::String(CODEX_SOURCE.to_string()));
    plugin.insert(
        "source_of_truth".to_string(),
        toml::Value::String(SHARED_SOURCE.to_string()),
    );
    plugin.insert(
        "exported_at".to_string(),
        toml::Value::Integer(unix_timestamp() as i64),
    );

    let mut servers = toml::map::Map::new();
    for entry in entries {
        let mut table = toml::map::Map::new();
        table.insert("command".to_string(), toml::Value::String(entry.command.clone()));
        table.insert(
            "args".to_string(),
            toml::Value::Array(
                entry
                    .args
                    .iter()
                    .cloned()
                    .map(toml::Value::String)
                    .collect::<Vec<_>>(),
            ),
        );
        table.insert(
            "description".to_string(),
            toml::Value::String(entry.description.clone()),
        );
        table.insert(
            "capabilities".to_string(),
            toml::Value::Array(
                entry
                    .capabilities
                    .iter()
                    .cloned()
                    .map(toml::Value::String)
                    .collect::<Vec<_>>(),
            ),
        );
        table.insert(
            "projects".to_string(),
            toml::Value::Array(
                entry
                    .projects
                    .iter()
                    .cloned()
                    .map(toml::Value::String)
                    .collect::<Vec<_>>(),
            ),
        );
        table.insert(
            "keywords".to_string(),
            toml::Value::Array(
                entry
                    .keywords
                    .iter()
                    .cloned()
                    .map(toml::Value::String)
                    .collect::<Vec<_>>(),
            ),
        );
        table.insert("category".to_string(), toml::Value::String(entry.category.clone()));
        if !entry.env.is_empty() {
            let env = entry
                .env
                .iter()
                .map(|(key, value)| (key.clone(), toml::Value::String(value.clone())))
                .collect::<toml::map::Map<_, _>>();
            table.insert("env".to_string(), toml::Value::Table(env));
        }
        servers.insert(entry.name.clone(), toml::Value::Table(table));
    }
    plugin.insert("mcp_servers".to_string(), toml::Value::Table(servers));
    root.insert("dx_provider_plugin".to_string(), toml::Value::Table(plugin));

    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, toml::to_string_pretty(&toml::Value::Table(root))?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_provider_names() {
        assert_eq!(normalized_provider("gpt"), "codex");
        assert_eq!(normalized_provider("openai"), "codex");
        assert_eq!(normalized_provider("google"), "gemini");
        assert_eq!(normalized_provider("claude"), "claude");
    }

    #[test]
    fn renders_json_plugin_payload() {
        let entries = vec![crate::external_mcp::ExternalMcpEntry {
            name: "playwright".to_string(),
            command: "/tmp/playwright".to_string(),
            args: vec!["--headless".to_string()],
            env: HashMap::from([(String::from("P"), String::from("3"))]),
            description: "Browser MCP".to_string(),
            capabilities: vec!["browser".to_string()],
            projects: vec!["dx-terminal".to_string()],
            keywords: vec!["playwright".to_string()],
            category: "testing".to_string(),
            sources: vec![SHARED_SOURCE.to_string()],
        }];

        let payload = render_json_plugin_payload("gemini", &entries);
        assert_eq!(
            payload["dxProviderPlugin"]["mcpServers"]["playwright"]["command"],
            "/tmp/playwright"
        );
    }

    #[test]
    fn reads_codex_plugin_shape() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dx-provider-plugin.toml");
        std::fs::write(
            &path,
            r#"[dx_provider_plugin]
version = 1
provider = "codex"
source_of_truth = "dx"

[dx_provider_plugin.mcp_servers.playwright]
command = "/tmp/playwright"
args = ["--headless"]
description = "Browser MCP"
capabilities = ["browser"]
projects = ["dx-terminal"]
keywords = ["playwright"]
category = "testing"

[dx_provider_plugin.mcp_servers.playwright.env]
P = "5"
"#,
        )
        .unwrap();

        let payload = read_codex_plugin(&path);
        assert_eq!(payload["mcpServers"]["playwright"]["command"], "/tmp/playwright");
        assert_eq!(payload["mcpServers"]["playwright"]["env"]["P"], "5");
    }
}

use dx_types::MCPDescriptor;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

const SHARED_SOURCE: &str = "dx";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalMcpEntry {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub projects: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ExternalMcpCatalog {
    #[serde(default = "default_catalog_version")]
    version: u32,
    #[serde(default)]
    servers: Vec<ExternalMcpEntry>,
}

fn default_catalog_version() -> u32 {
    1
}

fn default_category() -> String {
    "general".into()
}

pub fn shared_catalog_path() -> PathBuf {
    crate::config::dx_root().join("external_mcps.json")
}

/// Load the shared dx-owned external MCP catalog, importing Claude config as a source
/// and DX provider-plugin bridges when needed so all runtimes can consume the same registry.
pub fn load_external_catalog() -> Vec<ExternalMcpEntry> {
    load_external_catalog_from_sources(&shared_catalog_path(), &crate::provider_plugins::catalog_import_sources())
}

/// Refresh the shared dx-owned catalog from known import sources.
pub fn sync_shared_catalog() -> usize {
    load_external_catalog().len()
}

/// Load external MCP servers and normalize them into gateway descriptors.
pub fn load_external_descriptors() -> Vec<MCPDescriptor> {
    let mut descriptors = load_external_catalog()
        .into_iter()
        .filter_map(|entry| descriptor_from_entry(&entry))
        .collect::<Vec<_>>();
    descriptors.sort_by(|left, right| left.name.cmp(&right.name));
    descriptors
}

/// Refresh the gateway with descriptors sourced from the shared dx catalog.
pub fn sync_gateway(gateway: &mut dx_gateway::MCPRegistry) -> usize {
    let descriptors = load_external_descriptors();
    let count = descriptors.len();
    for descriptor in descriptors {
        gateway.register(descriptor);
    }
    count
}

fn load_external_catalog_from_sources(
    shared_path: &Path,
    import_sources: &[crate::provider_plugins::ProviderImportSource],
) -> Vec<ExternalMcpEntry> {
    let entries = merged_catalog_entries_from_sources(shared_path, import_sources);
    let _ = write_shared_catalog_to(shared_path, &entries);
    entries
}

fn merged_catalog_entries_from_sources(
    shared_path: &Path,
    import_sources: &[crate::provider_plugins::ProviderImportSource],
) -> Vec<ExternalMcpEntry> {
    let mut merged: HashMap<String, ExternalMcpEntry> = HashMap::new();

    for entry in read_shared_catalog_from_path(shared_path) {
        let entry = normalize_entry(entry);
        merged.insert(entry.name.clone(), entry);
    }

    for source in import_sources {
        for imported in import_catalog_from_config(&source.payload, &source.provider) {
            let imported = normalize_entry(imported);
            match merged.get_mut(&imported.name) {
                Some(existing) => merge_entry(existing, imported),
                None => {
                    merged.insert(imported.name.clone(), imported);
                }
            }
        }
    }

    let mut entries = merged.into_values().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    entries
}

fn read_shared_catalog_from_path(path: &Path) -> Vec<ExternalMcpEntry> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };

    serde_json::from_str::<ExternalMcpCatalog>(&content)
        .map(|catalog| catalog.servers)
        .unwrap_or_default()
}

fn write_shared_catalog_to(path: &Path, entries: &[ExternalMcpEntry]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let payload = ExternalMcpCatalog {
        version: default_catalog_version(),
        servers: entries.to_vec(),
    };

    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&payload)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

fn import_catalog_from_config(config: &Value, source: &str) -> Vec<ExternalMcpEntry> {
    let Some(servers) = config.get("mcpServers").and_then(|value| value.as_object()) else {
        return Vec::new();
    };

    let mut entries = servers
        .iter()
        .filter_map(|(name, server)| entry_from_value(name, server, source))
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    entries
}

#[cfg(test)]
fn load_external_catalog_from_test_sources(
    shared_path: &Path,
    import_sources: &[crate::provider_plugins::ProviderImportSource],
) -> Vec<ExternalMcpEntry> {
    load_external_catalog_from_sources(shared_path, import_sources)
}

fn entry_from_value(name: &str, server: &Value, source: &str) -> Option<ExternalMcpEntry> {
    let command = server.get("command")?.as_str()?.trim().to_string();
    if command.is_empty() {
        return None;
    }

    let args = string_array(server.get("args"));
    let env = string_map(server.get("env"));
    let mut capabilities = string_array(server.get("capabilities"));
    let projects = string_array(server.get("projects"));
    let mut keywords = string_array(server.get("keywords"));
    let category = server
        .get("category")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let description = server
        .get("description")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if keywords.is_empty() {
        keywords = generate_keywords(name);
    }
    if is_playwright_launcher(name, &command) {
        capabilities.extend([
            "playwright".to_string(),
            "browser".to_string(),
            "testing".to_string(),
        ]);
    }

    Some(ExternalMcpEntry {
        name: name.to_string(),
        command,
        args,
        env,
        description,
        capabilities,
        projects,
        keywords,
        category: if category.is_empty() {
            infer_category(name)
        } else {
            category
        },
        sources: vec![source.to_string()],
    })
}

fn descriptor_from_entry(entry: &ExternalMcpEntry) -> Option<MCPDescriptor> {
    if entry.command.trim().is_empty() {
        return None;
    }

    let mut env = entry.env.clone();
    let normalized_command =
        normalize_launch(&entry.name, &entry.command, entry.args.clone(), &mut env);

    let mut capabilities = BTreeSet::new();
    capabilities.insert("external".to_string());
    capabilities.insert("dx_catalog".to_string());
    for source in &entry.sources {
        capabilities.insert(format!("source:{}", source));
    }
    for capability in &entry.capabilities {
        if !capability.trim().is_empty() {
            capabilities.insert(capability.clone());
        }
    }
    for keyword in &entry.keywords {
        if !keyword.trim().is_empty() {
            capabilities.insert(keyword.clone());
        }
    }

    Some(MCPDescriptor {
        name: entry.name.clone(),
        command: normalized_command,
        capabilities: capabilities.into_iter().collect(),
        auto_start: false,
        env,
        description: if entry.description.trim().is_empty() {
            format!("External MCP from dx catalog: {}", entry.name)
        } else {
            entry.description.clone()
        },
    })
}

fn normalize_entry(mut entry: ExternalMcpEntry) -> ExternalMcpEntry {
    entry.name = entry.name.trim().to_string();
    entry.command = entry.command.trim().to_string();
    if entry.description.trim().is_empty() {
        entry.description = format!("External MCP from dx catalog: {}", entry.name);
    }
    if entry.category.trim().is_empty() {
        entry.category = infer_category(&entry.name);
    }
    if entry.keywords.is_empty() {
        entry.keywords = generate_keywords(&entry.name);
    }
    if entry.sources.is_empty() {
        entry.sources.push(SHARED_SOURCE.to_string());
    }
    if is_playwright_launcher(&entry.name, &entry.command) {
        entry.capabilities.extend([
            "playwright".to_string(),
            "browser".to_string(),
            "testing".to_string(),
        ]);
    }

    entry.capabilities = dedupe_sorted(entry.capabilities);
    entry.projects = dedupe_sorted(entry.projects);
    entry.keywords = dedupe_sorted(entry.keywords);
    entry.sources = dedupe_sorted(entry.sources);
    entry
}

fn merge_entry(existing: &mut ExternalMcpEntry, imported: ExternalMcpEntry) {
    let dx_owned = existing
        .sources
        .iter()
        .any(|source| source == SHARED_SOURCE);
    if !dx_owned {
        existing.command = imported.command.clone();
        existing.args = imported.args.clone();
        existing.env = imported.env.clone();
    }

    if existing.description.trim().is_empty()
        || existing
            .description
            .starts_with("External MCP from dx catalog:")
    {
        existing.description = imported.description.clone();
    }
    if existing.category.trim().is_empty() || existing.category == default_category() {
        existing.category = imported.category.clone();
    }

    existing.capabilities.extend(imported.capabilities);
    existing.projects.extend(imported.projects);
    existing.keywords.extend(imported.keywords);
    existing.sources.extend(imported.sources);

    *existing = normalize_entry(existing.clone());
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|value| value.trim().to_string()))
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn string_map(value: Option<&Value>) -> HashMap<String, String> {
    value
        .and_then(|value| value.as_object())
        .map(|map| {
            map.iter()
                .filter_map(|(key, value)| {
                    value
                        .as_str()
                        .map(|value| (key.to_string(), value.to_string()))
                })
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default()
}

fn dedupe_sorted(values: Vec<String>) -> Vec<String> {
    let mut set = BTreeSet::new();
    for value in values {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            set.insert(trimmed.to_string());
        }
    }
    set.into_iter().collect()
}

fn normalize_launch(
    name: &str,
    command: &str,
    mut args: Vec<String>,
    env: &mut HashMap<String, String>,
) -> Vec<String> {
    if is_playwright_launcher(name, command) {
        let pane = env
            .get("P")
            .cloned()
            .or_else(|| std::env::var("P").ok())
            .unwrap_or_else(|| "99".to_string());
        env.entry("P".to_string()).or_insert_with(|| pane.clone());

        let browser_port = env
            .get("PLAYWRIGHT_PORT")
            .cloned()
            .or_else(|| env.get("DX_BROWSER_PORT").cloned())
            .or_else(|| std::env::var("PLAYWRIGHT_PORT").ok())
            .or_else(|| std::env::var("DX_BROWSER_PORT").ok())
            .or_else(|| {
                pane.parse::<u8>()
                    .ok()
                    .map(crate::config::pane_browser_port)
                    .map(|value| value.to_string())
            })
            .unwrap_or_else(|| "46099".to_string());

        env.insert("PLAYWRIGHT_PORT".to_string(), browser_port.clone());
        env.entry("DX_BROWSER_PORT".to_string())
            .or_insert_with(|| browser_port.clone());

        let browser_profile_root = env
            .get("DX_BROWSER_PROFILE_ROOT")
            .cloned()
            .or_else(|| std::env::var("DX_BROWSER_PROFILE_ROOT").ok())
            .or_else(|| {
                pane.parse::<u8>().ok().map(|value| {
                    crate::config::pane_browser_profile_root(value)
                        .to_string_lossy()
                        .to_string()
                })
            })
            .unwrap_or_else(|| {
                crate::config::pane_browser_profile_root(99)
                    .to_string_lossy()
                    .to_string()
            });
        let browser_artifacts_root = env
            .get("DX_BROWSER_ARTIFACTS_ROOT")
            .cloned()
            .or_else(|| std::env::var("DX_BROWSER_ARTIFACTS_ROOT").ok())
            .or_else(|| {
                pane.parse::<u8>().ok().map(|value| {
                    crate::config::pane_browser_artifacts_root(value)
                        .to_string_lossy()
                        .to_string()
                })
            })
            .unwrap_or_else(|| {
                crate::config::pane_browser_artifacts_root(99)
                    .to_string_lossy()
                    .to_string()
            });
        env.entry("DX_BROWSER_PROFILE_ROOT".to_string())
            .or_insert_with(|| browser_profile_root.clone());
        env.entry("DX_BROWSER_ARTIFACTS_ROOT".to_string())
            .or_insert_with(|| browser_artifacts_root.clone());

        let has_port_arg = args.windows(2).any(|window| window[0] == "--port");
        if !has_port_arg {
            args.push("--port".to_string());
            args.push(browser_port);
        }
        let has_profile_arg = args.windows(2).any(|window| window[0] == "--user-data-dir");
        if !has_profile_arg {
            args.push("--user-data-dir".to_string());
            args.push(browser_profile_root);
        }
        let has_output_arg = args.windows(2).any(|window| window[0] == "--output-dir");
        if !has_output_arg {
            args.push("--output-dir".to_string());
            args.push(browser_artifacts_root);
        }

        let mut wrapped = vec![
            "zsh".to_string(),
            "-o".to_string(),
            "nonomatch".to_string(),
            command.to_string(),
        ];
        wrapped.extend(args);
        return wrapped;
    }

    let mut resolved = vec![command.to_string()];
    resolved.extend(args);
    resolved
}

fn is_playwright_launcher(name: &str, command: &str) -> bool {
    if name.to_ascii_lowercase().contains("playwright") {
        return true;
    }
    Path::new(command)
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value == "playwright-session")
        .unwrap_or(false)
}

fn generate_keywords(name: &str) -> Vec<String> {
    name.replace('-', " ")
        .replace('_', " ")
        .split_whitespace()
        .map(|value| value.to_lowercase())
        .collect()
}

fn infer_category(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("monitor") || lower.contains("metric") || lower.contains("health") {
        "monitoring".into()
    } else if lower.contains("build") || lower.contains("deploy") || lower.contains("ci") {
        "build".into()
    } else if lower.contains("test") || lower.contains("playwright") || lower.contains("qa") {
        "testing".into()
    } else if lower.contains("dns") || lower.contains("server") || lower.contains("infra") {
        "infrastructure".into()
    } else if lower.contains("track") || lower.contains("issue") || lower.contains("sprint") {
        "tracking".into()
    } else if lower.contains("doc") || lower.contains("collab") || lower.contains("diagram") {
        "documentation".into()
    } else if lower.contains("vault") || lower.contains("secret") || lower.contains("auth") {
        "security".into()
    } else if lower.contains("graph") || lower.contains("store") || lower.contains("data") {
        "data".into()
    } else {
        default_category()
    }
}

fn infer_projects(name: &str) -> Vec<String> {
    let lower = name.to_lowercase();
    if let Some(prefix) = lower.split('-').next() {
        if prefix.len() > 2 && lower.contains('-') {
            return vec![prefix.to_string()];
        }
    }
    Vec::new()
}

pub fn entry_to_registry_info(entry: &ExternalMcpEntry) -> crate::mcp_registry::McpInfo {
    crate::mcp_registry::McpInfo {
        name: entry.name.clone(),
        description: entry.description.clone(),
        capabilities: entry.capabilities.clone(),
        projects: if entry.projects.is_empty() {
            infer_projects(&entry.name)
        } else {
            entry.projects.clone()
        },
        keywords: entry.keywords.clone(),
        category: if entry.category.trim().is_empty() {
            infer_category(&entry.name)
        } else {
            entry.category.clone()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn wraps_playwright_launcher_with_nonomatch() {
        let descriptor = descriptor_from_entry(&normalize_entry(ExternalMcpEntry {
            name: "playwright".to_string(),
            command: "/Users/pran/bin/playwright-session".to_string(),
            args: vec!["--headless".to_string()],
            env: HashMap::new(),
            description: String::new(),
            capabilities: Vec::new(),
            projects: Vec::new(),
            keywords: Vec::new(),
            category: String::new(),
            sources: vec!["claude".to_string()],
        }))
        .expect("descriptor");

        assert_eq!(
            descriptor.command[0..4],
            [
                "zsh".to_string(),
                "-o".to_string(),
                "nonomatch".to_string(),
                "/Users/pran/bin/playwright-session".to_string(),
            ]
        );
        assert!(descriptor.command.contains(&"--headless".to_string()));
        assert!(descriptor
            .command
            .windows(2)
            .any(|window| { window[0] == "--port" && window[1] == "46099" }));
        assert!(descriptor.command.windows(2).any(|window| {
            window[0] == "--user-data-dir"
                && window[1]
                    == crate::config::pane_browser_profile_root(99)
                        .to_string_lossy()
                        .to_string()
        }));
        assert!(descriptor.command.windows(2).any(|window| {
            window[0] == "--output-dir"
                && window[1]
                    == crate::config::pane_browser_artifacts_root(99)
                        .to_string_lossy()
                        .to_string()
        }));
        assert!(descriptor
            .capabilities
            .iter()
            .any(|capability| capability == "playwright"));
        assert_eq!(descriptor.env.get("P").map(String::as_str), Some("99"));
        assert_eq!(
            descriptor.env.get("PLAYWRIGHT_PORT").map(String::as_str),
            Some("46099")
        );
    }

    #[test]
    fn derives_playwright_port_from_pane_env() {
        let descriptor = descriptor_from_entry(&normalize_entry(ExternalMcpEntry {
            name: "playwright".to_string(),
            command: "/Users/pran/bin/playwright-session".to_string(),
            args: Vec::new(),
            env: HashMap::from([(String::from("P"), String::from("3"))]),
            description: String::new(),
            capabilities: Vec::new(),
            projects: Vec::new(),
            keywords: Vec::new(),
            category: String::new(),
            sources: vec!["claude".to_string()],
        }))
        .expect("descriptor");

        assert!(descriptor.command.windows(2).any(|window| {
            window[0] == "--port" && window[1] == crate::config::pane_browser_port(3).to_string()
        }));
        assert!(descriptor.command.windows(2).any(|window| {
            window[0] == "--user-data-dir"
                && window[1]
                    == crate::config::pane_browser_profile_root(3)
                        .to_string_lossy()
                        .to_string()
        }));
    }

    #[test]
    fn syncs_shared_catalog_from_claude_import() {
        let dx = tempdir().unwrap();
        let shared_path = dx.path().join("external_mcps.json");
        let claude_config = json!({
            "mcpServers": {
                "playwright": {
                    "command": "/Users/pran/bin/playwright-session",
                    "args": ["--headless"]
                }
            }
        });

        let entries = load_external_catalog_from_test_sources(
            &shared_path,
            &[crate::provider_plugins::ProviderImportSource {
                provider: "claude".to_string(),
                mode: "native_import".to_string(),
                path: crate::config::claude_json_path(),
                payload: claude_config,
            }],
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "playwright");
        assert_eq!(entries[0].sources, vec!["claude".to_string()]);

        let persisted = std::fs::read_to_string(&shared_path).unwrap();
        let catalog: ExternalMcpCatalog = serde_json::from_str(&persisted).unwrap();
        assert_eq!(catalog.servers.len(), 1);
        assert_eq!(catalog.servers[0].name, "playwright");
    }

    #[test]
    fn keeps_dx_owned_entry_while_merging_claude_metadata() {
        let dx = tempdir().unwrap();
        let path = dx.path().join("external_mcps.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&ExternalMcpCatalog {
                version: 1,
                servers: vec![ExternalMcpEntry {
                    name: "playwright".to_string(),
                    command: "/custom/playwright".to_string(),
                    args: vec!["--headless".to_string()],
                    env: HashMap::new(),
                    description: "Shared dx override".to_string(),
                    capabilities: vec!["browser".to_string()],
                    projects: vec!["dx-terminal".to_string()],
                    keywords: vec!["browser".to_string()],
                    category: "testing".to_string(),
                    sources: vec![SHARED_SOURCE.to_string()],
                }],
            })
            .unwrap(),
        )
        .unwrap();

        let claude_config = json!({
            "mcpServers": {
                "playwright": {
                    "command": "/Users/pran/bin/playwright-session"
                }
            }
        });

        let entries = load_external_catalog_from_test_sources(
            &path,
            &[crate::provider_plugins::ProviderImportSource {
                provider: "claude".to_string(),
                mode: "native_import".to_string(),
                path: crate::config::claude_json_path(),
                payload: claude_config,
            }],
        );
        assert_eq!(entries[0].command, "/custom/playwright");
        assert!(entries[0]
            .sources
            .iter()
            .any(|value| value == SHARED_SOURCE));
        assert!(entries[0]
            .sources
            .iter()
            .any(|value| value == "claude"));
    }
}

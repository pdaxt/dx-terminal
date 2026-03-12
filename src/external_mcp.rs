use dx_types::MCPDescriptor;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

/// Load external MCP servers configured in Claude Code and normalize them
/// into gateway descriptors so dx can route through the same servers.
pub fn load_external_descriptors() -> Vec<MCPDescriptor> {
    let config = crate::claude::read_claude_config();
    let metadata = crate::mcp_registry::load_registry()
        .into_iter()
        .map(|info| (info.name.clone(), info))
        .collect::<HashMap<_, _>>();

    let mut descriptors = Vec::new();
    let Some(servers) = config.get("mcpServers").and_then(|value| value.as_object()) else {
        return descriptors;
    };

    for (name, server) in servers {
        if let Some(descriptor) = descriptor_from_value(name, server, metadata.get(name)) {
            descriptors.push(descriptor);
        }
    }

    descriptors.sort_by(|left, right| left.name.cmp(&right.name));
    descriptors
}

/// Refresh the gateway with descriptors sourced from Claude's MCP config.
pub fn sync_gateway(gateway: &mut dx_gateway::MCPRegistry) -> usize {
    let descriptors = load_external_descriptors();
    let count = descriptors.len();
    for descriptor in descriptors {
        gateway.register(descriptor);
    }
    count
}

fn descriptor_from_value(
    name: &str,
    server: &Value,
    metadata: Option<&crate::mcp_registry::McpInfo>,
) -> Option<MCPDescriptor> {
    let command = server.get("command")?.as_str()?.trim();
    if command.is_empty() {
        return None;
    }

    let args = server
        .get("args")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|value| value.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut env = server
        .get("env")
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
        .unwrap_or_default();

    let normalized_command = normalize_launch(name, command, args, &mut env);

    let mut capabilities = BTreeSet::new();
    capabilities.insert("external".to_string());
    capabilities.insert("claude_configured".to_string());
    if let Some(info) = metadata {
        if !info.category.trim().is_empty() {
            capabilities.insert(info.category.clone());
        }
        for keyword in &info.keywords {
            if !keyword.trim().is_empty() {
                capabilities.insert(keyword.clone());
            }
        }
        for capability in &info.capabilities {
            if !capability.trim().is_empty() {
                capabilities.insert(capability.clone());
            }
        }
    }
    if is_playwright_launcher(name, command) {
        capabilities.insert("playwright".to_string());
        capabilities.insert("browser".to_string());
        capabilities.insert("testing".to_string());
    }

    Some(MCPDescriptor {
        name: name.to_string(),
        command: normalized_command,
        capabilities: capabilities.into_iter().collect(),
        auto_start: false,
        env,
        description: metadata
            .map(|info| info.description.clone())
            .unwrap_or_else(|| format!("External MCP configured in Claude: {}", name)),
    })
}

fn normalize_launch(
    name: &str,
    command: &str,
    args: Vec<String>,
    env: &mut HashMap<String, String>,
) -> Vec<String> {
    if is_playwright_launcher(name, command) {
        env.entry("P".to_string())
            .or_insert_with(|| std::env::var("P").unwrap_or_else(|_| "99".to_string()));

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn wraps_playwright_launcher_with_nonomatch() {
        let descriptor = descriptor_from_value(
            "playwright",
            &json!({
                "command": "/Users/pran/bin/playwright-session",
                "args": ["--headless"]
            }),
            None,
        )
        .expect("descriptor");

        assert_eq!(
            descriptor.command,
            vec![
                "zsh".to_string(),
                "-o".to_string(),
                "nonomatch".to_string(),
                "/Users/pran/bin/playwright-session".to_string(),
                "--headless".to_string()
            ]
        );
        assert!(descriptor
            .capabilities
            .iter()
            .any(|capability| capability == "playwright"));
        assert_eq!(descriptor.env.get("P").map(String::as_str), Some("99"));
    }

    #[test]
    fn preserves_normal_stdio_launchers() {
        let descriptor = descriptor_from_value(
            "vdd",
            &json!({
                "command": "/tmp/vdd-mcp",
                "env": {"VDD_PROJECTS_ROOT": "/Users/pran/Projects"}
            }),
            None,
        )
        .expect("descriptor");

        assert_eq!(descriptor.command, vec!["/tmp/vdd-mcp".to_string()]);
        assert_eq!(
            descriptor.env.get("VDD_PROJECTS_ROOT").map(String::as_str),
            Some("/Users/pran/Projects")
        );
    }
}

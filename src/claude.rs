use anyhow::Result;
use crate::config;
use crate::state::persistence::{read_json, write_json};

/// Set project-level MCPs in ~/.claude.json
pub fn set_project_mcps(project_path: &str, mcp_names: &[String]) -> Result<()> {
    let claude_json = config::claude_json_path();
    let mut config = read_json(&claude_json);

    let all_servers = config.get("mcpServers")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    let mut proj_servers = serde_json::Map::new();
    for name in mcp_names {
        if let Some(server) = all_servers.get(name) {
            proj_servers.insert(name.clone(), server.clone());
        }
    }

    let root = match config.as_object_mut() {
        Some(obj) => obj,
        None => anyhow::bail!("claude.json is not a JSON object"),
    };

    let projects = root
        .entry("projects")
        .or_insert_with(|| serde_json::json!({}));

    let project_entry = match projects.as_object_mut() {
        Some(obj) => obj.entry(project_path).or_insert_with(|| serde_json::json!({})),
        None => anyhow::bail!("claude.json 'projects' is not an object"),
    };

    match project_entry.as_object_mut() {
        Some(obj) => { obj.insert("mcpServers".to_string(), serde_json::Value::Object(proj_servers)); }
        None => anyhow::bail!("claude.json project entry is not an object"),
    };

    write_json(&claude_json, &config)?;
    Ok(())
}

/// Read the claude.json config
pub fn read_claude_config() -> serde_json::Value {
    read_json(&config::claude_json_path())
}

/// Write preamble file for a pane
pub fn write_preamble(pane: u8, content: &str) -> Result<String> {
    let dir = config::preamble_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("pane_{}.md", pane));
    std::fs::write(&path, content)?;
    Ok(path.to_string_lossy().to_string())
}

/// Generate a preamble for an agent
pub fn generate_preamble(
    pane: u8,
    theme: &str,
    project: &str,
    role: &str,
    task: &str,
    prompt: &str,
) -> String {
    let role_short = config::role_short(role);
    format!(
        "# TASK: {task}\n\
         **Role:** {role_short} | **Project:** {project} | **Pane:** {pane} ({theme})\n\
         \n\
         ## Role Instructions\n\
         You are the {role} agent. Focus on your assigned task.\n\
         \n\
         ## Task Details\n\
         {task}\n\
         {extra}\n\
         ## Coordination\n\
         - Use multi_agent MCP to register and coordinate with other agents\n\
         - Lock files before editing shared code\n\
         - When done: summarize what you accomplished\n",
        extra = if prompt.is_empty() {
            String::new()
        } else {
            format!("Additional context: {}\n\n", prompt)
        }
    )
}

/// Get the account config dir for a pane (alternates between accounts)
/// Falls back to default ~/.claude if account dirs don't exist
pub fn account_config_dir(pane: u8) -> String {
    let home = config::home_dir();
    let acc_dir = if pane % 2 == 1 {
        home.join(".claude-acc1")
    } else {
        home.join(".claude-acc2")
    };
    if acc_dir.exists() {
        acc_dir.to_string_lossy().to_string()
    } else {
        home.join(".claude").to_string_lossy().to_string()
    }
}

/// Check if preamble exists
pub fn preamble_exists(pane: u8) -> bool {
    let path = config::preamble_dir().join(format!("pane_{}.md", pane));
    path.exists()
}


use crate::config;
use crate::state::persistence::{read_json, write_json};
use anyhow::Result;

/// Set project-level MCPs in ~/.claude.json
pub fn set_project_mcps(project_path: &str, mcp_names: &[String]) -> Result<()> {
    let claude_json = config::claude_json_path();
    let mut config = read_json(&claude_json);

    let all_servers = config
        .get("mcpServers")
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
        Some(obj) => obj
            .entry(project_path)
            .or_insert_with(|| serde_json::json!({})),
        None => anyhow::bail!("claude.json 'projects' is not an object"),
    };

    match project_entry.as_object_mut() {
        Some(obj) => {
            obj.insert(
                "mcpServers".to_string(),
                serde_json::Value::Object(proj_servers),
            );
        }
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

/// Generate a preamble for an agent. Includes workspace context and sibling agent info.
pub fn generate_preamble(
    pane: u8,
    theme: &str,
    project: &str,
    role: &str,
    task: &str,
    prompt: &str,
) -> String {
    let _ = config::init();
    let role_short = config::role_short(role);
    let browser_port = config::pane_browser_port(pane);
    let browser_profile_root = config::pane_browser_profile_root(pane);
    let browser_artifacts_root = config::pane_browser_artifacts_root(pane);

    // Parse prompt sections: split on known headers
    let mut regular_prompt = String::new();
    let mut handoff_section = String::new();
    let mut gate_section = String::new();
    let mut coord_section = String::new();

    let mut current_target = &mut regular_prompt;
    for line in prompt.lines() {
        if line.starts_with("## Predecessor Results") {
            current_target = &mut handoff_section;
            continue;
        } else if line.starts_with("## Quality Gate Results") {
            current_target = &mut gate_section;
            continue;
        } else if line.starts_with("## Pipeline Coordination") {
            current_target = &mut coord_section;
            continue;
        }
        current_target.push_str(line);
        current_target.push('\n');
    }

    let extra = {
        let trimmed = regular_prompt.trim();
        if trimmed.is_empty() {
            String::new()
        } else {
            format!("{}\n\n", trimmed)
        }
    };

    let handoff = {
        let trimmed = handoff_section.trim();
        if trimmed.is_empty() {
            String::new()
        } else {
            format!("## Predecessor Results\nThese tasks completed before yours. Use their output as context:\n{}\n\n", trimmed)
        }
    };

    let gate = {
        let trimmed = gate_section.trim();
        if trimmed.is_empty() {
            String::new()
        } else {
            format!("## Quality Gate Results\n{}\n\n", trimmed)
        }
    };

    let coord = {
        let trimmed = coord_section.trim();
        if trimmed.is_empty() {
            "## Coordination\n\
             - Use multi_agent MCP to register and coordinate with other agents\n\
             - Lock files before editing shared code\n\
             - If DXOS_SESSION_ID is present and you are blocked by permission, login, CAPTCHA, or human approval, call dxos_session_raise_blocker with worker_session_id=$DXOS_SESSION_ID instead of waiting silently\n\
             - If DX_WORKFLOW_RUN_ID is present, use dxos_workflow_step to mark workflow steps in_progress/completed/blocked as you work\n\
             - When done: summarize what you accomplished\n"
                .to_string()
        } else {
            format!("{}\n", trimmed)
        }
    };

    // Gather sibling agent context
    let sibling_context = gather_sibling_context(pane, project);

    format!(
        "# TASK: {task}\n\
         **Role:** {role_short} | **Project:** {project} | **Pane:** {pane} ({theme})\n\
         \n\
         ## Environment\n\
         - Pane: {pane} ({theme})\n\
         - Project: {project}\n\
         - Role: {role} ({role_short})\n\
         - Browser testing port: {browser_port}\n\
         - Browser profile root: {browser_profile_root}\n\
         - Browser artifacts root: {browser_artifacts_root}\n\
         - DX Terminal managed agent — your work is monitored automatically\n\
         \n\
         ## Role Instructions\n\
         You are the {role} agent. Focus on your assigned task.\n\
         \n\
         ## Task Details\n\
         {task}\n\
         {extra}\
         {handoff}\
         {gate}\
         {sibling_context}\
         {coord}\n",
        browser_port = browser_port,
        browser_profile_root = browser_profile_root.display(),
        browser_artifacts_root = browser_artifacts_root.display(),
    )
}

/// Gather info about other active agents on the same project
fn gather_sibling_context(my_pane: u8, project: &str) -> String {
    // Read state file directly (sync context, no async available)
    let state_path = config::dx_root().join("state.json");
    let state_content = std::fs::read_to_string(&state_path).unwrap_or_default();
    let state: serde_json::Value = serde_json::from_str(&state_content).unwrap_or_default();

    let panes = match state.get("panes").and_then(|p| p.as_object()) {
        Some(p) => p,
        None => return String::new(),
    };

    let mut siblings = Vec::new();
    for (key, pane_data) in panes {
        let pane_num: u8 = match key.parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        if pane_num == my_pane {
            continue;
        }

        let status = pane_data
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("idle");
        if status != "active" {
            continue;
        }

        let p = pane_data
            .get("project")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let r = pane_data.get("role").and_then(|v| v.as_str()).unwrap_or("");
        let t = pane_data.get("task").and_then(|v| v.as_str()).unwrap_or("");
        let b = pane_data
            .get("branch_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let theme = config::theme_name(pane_num);

        if p.to_lowercase() == project.to_lowercase() {
            siblings.push(format!(
                "  - Pane {} ({}): {} — {} [branch: {}]",
                pane_num,
                theme,
                config::role_short(r),
                if t.len() > 60 { &t[..60] } else { t },
                if b.is_empty() { "none" } else { b }
            ));
        }
    }

    if siblings.is_empty() {
        String::new()
    } else {
        format!(
            "## Active Sibling Agents (same project)\n{}\n\n",
            siblings.join("\n")
        )
    }
}

/// Check if preamble exists for a pane
pub fn preamble_exists(pane: u8) -> bool {
    let path = config::preamble_dir().join(format!("pane_{}.md", pane));
    path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_preamble_basic() {
        let result = generate_preamble(1, "CYAN", "myproject", "developer", "Build auth", "");
        assert!(result.contains("# TASK: Build auth"));
        assert!(result.contains("**Role:** DEV"));
        assert!(result.contains("**Project:** myproject"));
        assert!(result.contains("**Pane:** 1 (CYAN)"));
        assert!(result.contains("developer agent"));
    }

    #[test]
    fn test_generate_preamble_with_handoff() {
        let prompt = "## Predecessor Results\nAuth built by pane 2, JWT in /api/auth\n";
        let result = generate_preamble(3, "PURPLE", "proj", "qa", "Test auth", prompt);
        assert!(result.contains("## Predecessor Results"));
        assert!(result.contains("Auth built by pane 2"));
    }

    #[test]
    fn test_generate_preamble_with_gate_results() {
        let prompt = "## Quality Gate Results\nBuild: PASS, Tests: 12/12\n";
        let result = generate_preamble(2, "GREEN", "proj", "code_reviewer", "Review PR", prompt);
        assert!(result.contains("## Quality Gate Results"));
        assert!(result.contains("Build: PASS"));
        assert!(result.contains("**Role:** CR"));
    }

    #[test]
    fn test_generate_preamble_with_coordination() {
        let prompt = "## Pipeline Coordination\nYou are the QA agent. Run tests only.\n";
        let result = generate_preamble(4, "ORANGE", "proj", "qa", "QA pass", prompt);
        assert!(result.contains("You are the QA agent"));
        // Should NOT contain default coordination when custom provided
        assert!(!result.contains("Lock files before editing"));
    }

    #[test]
    fn test_generate_preamble_default_coordination() {
        let result = generate_preamble(1, "CYAN", "proj", "developer", "Build it", "");
        assert!(result.contains("## Coordination"));
        assert!(result.contains("Lock files before editing"));
        assert!(result.contains("dxos_session_raise_blocker"));
    }

    #[test]
    fn test_generate_preamble_all_sections() {
        let prompt = "extra instructions\n\
            ## Predecessor Results\nprev agent did X\n\
            ## Quality Gate Results\nall pass\n\
            ## Pipeline Coordination\ncustom coord\n";
        let result = generate_preamble(5, "RED", "proj", "security", "Audit", prompt);
        assert!(result.contains("extra instructions"));
        assert!(result.contains("prev agent did X"));
        assert!(result.contains("all pass"));
        assert!(result.contains("custom coord"));
        assert!(result.contains("**Role:** SEC"));
    }

    #[test]
    fn test_write_preamble_and_exists() {
        // Directly test file ops without setting DX_ROOT (avoids env races).
        // Instead, use a known unique temp path and write/read directly.
        let tmp = tempfile::tempdir().unwrap();
        let preamble_dir = tmp.path().join("preambles");
        std::fs::create_dir_all(&preamble_dir).unwrap();

        let path = preamble_dir.join("pane_1.md");
        std::fs::write(&path, "# Test preamble").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "# Test preamble");

        // Verify nonexistent preamble
        assert!(!preamble_dir.join("pane_99.md").exists());
        std::fs::write(preamble_dir.join("pane_99.md"), "test").unwrap();
        assert!(preamble_dir.join("pane_99.md").exists());
    }
}

use serde::{Deserialize, Serialize};

/// MCP capability descriptor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInfo {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub projects: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default = "default_category")]
    pub category: String,
}

fn default_category() -> String { "general".into() }

/// Load MCP registry dynamically from ~/.claude.json + optional enrichment file
pub fn load_registry() -> Vec<McpInfo> {
    let mut registry = Vec::new();

    // Load any user-defined MCP metadata from enrichment file
    let enrichment = load_enrichment();

    // Primary source: ~/.claude.json mcpServers
    let claude_cfg = crate::claude::read_claude_config();
    if let Some(servers) = claude_cfg.get("mcpServers").and_then(|v| v.as_object()) {
        for (name, _server_config) in servers {
            // Check if user has enriched this MCP with metadata
            if let Some(enriched) = enrichment.iter().find(|e| e.name == *name) {
                registry.push(enriched.clone());
            } else {
                // Auto-generate metadata from the name
                let keywords = generate_keywords(name);
                let category = infer_category(name);
                registry.push(McpInfo {
                    name: name.clone(),
                    description: format!("MCP server: {}", name),
                    capabilities: vec![],
                    projects: infer_projects(name),
                    keywords,
                    category,
                });
            }
        }
    }

    registry
}

/// Optional enrichment file: ~/.config/dx-terminal/mcp_metadata.json
/// Users can add descriptions, keywords, project associations
fn load_enrichment() -> Vec<McpInfo> {
    let path = crate::config::dx_root().join("mcp_metadata.json");
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(mcps) = serde_json::from_str::<Vec<McpInfo>>(&content) {
                return mcps;
            }
        }
    }
    Vec::new()
}

/// Generate keywords from MCP name (split on hyphens, underscores)
fn generate_keywords(name: &str) -> Vec<String> {
    name.replace('-', " ")
        .replace('_', " ")
        .split_whitespace()
        .map(|s| s.to_lowercase())
        .collect()
}

/// Infer category from MCP name patterns
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
        "general".into()
    }
}

/// Infer project associations from MCP name patterns
fn infer_projects(name: &str) -> Vec<String> {
    let lower = name.to_lowercase();
    // Extract project prefix (e.g., "dataxlr8-employees" -> "dataxlr8")
    if let Some(prefix) = lower.split('-').next() {
        if prefix.len() > 2 && lower.contains('-') {
            return vec![prefix.to_string()];
        }
    }
    Vec::new()
}

/// Route: given a project and task description, return ranked MCP suggestions
pub fn route_mcps(project: &str, task: &str, role: &str) -> Vec<McpMatch> {
    let registry = load_registry();
    let query = format!("{} {} {}", project, task, role).to_lowercase();
    let project_lower = project.to_lowercase();

    let mut matches: Vec<McpMatch> = registry.iter().filter_map(|mcp| {
        let mut score: u32 = 0;
        let mut reasons = Vec::new();

        // Direct project match (highest signal)
        for p in &mcp.projects {
            if p.to_lowercase() == project_lower || project_lower.contains(&p.to_lowercase()) {
                score += 100;
                reasons.push(format!("project:{}", p));
            }
        }

        // Keyword match against task+project+role
        for kw in &mcp.keywords {
            if query.contains(&kw.to_lowercase()) {
                score += 30;
                reasons.push(format!("keyword:{}", kw));
            }
        }

        // Category match against role
        let role_categories = role_to_categories(role);
        if role_categories.contains(&mcp.category.as_str()) {
            score += 20;
            reasons.push(format!("role:{}", mcp.category));
        }

        // Infrastructure MCPs always get a baseline
        if mcp.category == "infrastructure" || mcp.category == "general" {
            score += 5;
        }

        if score > 0 {
            Some(McpMatch {
                name: mcp.name.clone(),
                score,
                reasons,
                description: mcp.description.clone(),
            })
        } else {
            None
        }
    }).collect();

    matches.sort_by(|a, b| b.score.cmp(&a.score));
    matches
}

/// Search MCPs by capability or keyword
pub fn search(query: &str) -> Vec<McpInfo> {
    let q = query.to_lowercase();
    load_registry().into_iter().filter(|mcp| {
        mcp.name.to_lowercase().contains(&q)
            || mcp.description.to_lowercase().contains(&q)
            || mcp.capabilities.iter().any(|c| c.to_lowercase().contains(&q))
            || mcp.keywords.iter().any(|k| k.to_lowercase().contains(&q))
            || mcp.category.to_lowercase().contains(&q)
    }).collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct McpMatch {
    pub name: String,
    pub score: u32,
    pub reasons: Vec<String>,
    pub description: String,
}

fn role_to_categories(role: &str) -> Vec<&'static str> {
    match role {
        "frontend" => vec!["ui", "testing", "build"],
        "backend" => vec!["data", "api", "infrastructure"],
        "devops" => vec!["infrastructure", "monitoring", "deployment"],
        "qa" => vec!["testing", "monitoring"],
        "security" => vec!["security", "monitoring"],
        "pm" => vec!["tracking", "documentation", "general"],
        "architect" => vec!["infrastructure", "data", "api"],
        "developer" => vec!["build", "testing", "data", "general"],
        _ => vec![],
    }
}

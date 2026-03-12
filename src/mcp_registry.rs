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

fn default_category() -> String {
    "general".into()
}

/// Load MCP registry dynamically from the shared dx-owned external catalog plus
/// optional enrichment metadata.
pub fn load_registry() -> Vec<McpInfo> {
    let enrichment = load_enrichment();
    let mut registry = Vec::new();

    for entry in crate::external_mcp::load_external_catalog() {
        if let Some(enriched) = enrichment.iter().find(|item| item.name == entry.name) {
            registry.push(enriched.clone());
        } else {
            registry.push(crate::external_mcp::entry_to_registry_info(&entry));
        }
    }

    registry.sort_by(|left, right| left.name.cmp(&right.name));
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

/// Route: given a project and task description, return ranked MCP suggestions
pub fn route_mcps(project: &str, task: &str, role: &str) -> Vec<McpMatch> {
    let registry = load_registry();
    let query = format!("{} {} {}", project, task, role).to_lowercase();
    let project_lower = project.to_lowercase();

    let mut matches: Vec<McpMatch> = registry
        .iter()
        .filter_map(|mcp| {
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
        })
        .collect();

    matches.sort_by(|a, b| b.score.cmp(&a.score));
    matches
}

/// Search MCPs by capability or keyword
pub fn search(query: &str) -> Vec<McpInfo> {
    let q = query.to_lowercase();
    load_registry()
        .into_iter()
        .filter(|mcp| {
            mcp.name.to_lowercase().contains(&q)
                || mcp.description.to_lowercase().contains(&q)
                || mcp
                    .capabilities
                    .iter()
                    .any(|c| c.to_lowercase().contains(&q))
                || mcp.keywords.iter().any(|k| k.to_lowercase().contains(&q))
                || mcp.category.to_lowercase().contains(&q)
        })
        .collect()
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

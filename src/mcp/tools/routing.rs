//! MCP routing: mcp_list, mcp_route, mcp_search.

use super::super::types::*;
use crate::app::App;
use crate::claude;
use crate::config;

/// Execute os_mcp_list logic — list available MCPs with metadata
pub async fn mcp_list(_app: &App, req: McpListRequest) -> String {
    let registry = crate::mcp_registry::load_registry();

    let filtered: Vec<_> = registry
        .into_iter()
        .filter(|mcp| {
            if let Some(cat) = &req.category {
                if !mcp.category.eq_ignore_ascii_case(cat) {
                    return false;
                }
            }
            if let Some(proj) = &req.project {
                if !mcp.projects.iter().any(|p| p.eq_ignore_ascii_case(proj)) {
                    return false;
                }
            }
            true
        })
        .collect();

    let items: Vec<serde_json::Value> = filtered
        .iter()
        .map(|mcp| {
            serde_json::json!({
                "name": mcp.name,
                "description": mcp.description,
                "category": mcp.category,
                "capabilities": mcp.capabilities,
                "projects": mcp.projects,
            })
        })
        .collect();

    serde_json::json!({
        "count": items.len(),
        "mcps": items,
    })
    .to_string()
}

/// Execute os_mcp_route logic — smart MCP routing based on project+task+role
pub async fn mcp_route(app: &App, req: McpRouteRequest) -> String {
    let role = req.role.unwrap_or_else(|| "developer".into());
    let matches = crate::mcp_registry::route_mcps(&req.project, &req.task, &role);

    let suggested: Vec<String> = matches
        .iter()
        .filter(|m| m.score >= 20)
        .map(|m| m.name.clone())
        .collect();

    let details: Vec<serde_json::Value> = matches
        .iter()
        .take(15)
        .map(|m| {
            serde_json::json!({
                "name": m.name,
                "score": m.score,
                "reasons": m.reasons,
                "description": m.description,
            })
        })
        .collect();

    let mut applied = false;
    let mut apply_error: Option<String> = None;
    if req.apply.unwrap_or(false) && !suggested.is_empty() {
        app.state
            .set_project_mcps(&req.project, suggested.clone())
            .await;
        let project_path = config::resolve_project_path(&req.project);
        match claude::set_project_mcps(&project_path, &suggested) {
            Ok(()) => {
                applied = true;
            }
            Err(e) => {
                tracing::warn!("mcp_route: failed to persist MCPs to claude.json: {}", e);
                apply_error = Some(e.to_string());
            }
        }
    }

    serde_json::json!({
        "project": req.project,
        "task": req.task,
        "role": role,
        "suggested_mcps": suggested,
        "applied": applied,
        "apply_error": apply_error,
        "details": details,
    })
    .to_string()
}

/// Execute os_mcp_search logic — search MCPs by capability or keyword
pub async fn mcp_search(_app: &App, req: McpSearchRequest) -> String {
    let results = crate::mcp_registry::search(&req.query);

    let items: Vec<serde_json::Value> = results
        .iter()
        .map(|mcp| {
            serde_json::json!({
                "name": mcp.name,
                "description": mcp.description,
                "category": mcp.category,
                "capabilities": mcp.capabilities,
                "projects": mcp.projects,
                "keywords": mcp.keywords,
            })
        })
        .collect();

    serde_json::json!({
        "query": req.query,
        "count": items.len(),
        "results": items,
    })
    .to_string()
}

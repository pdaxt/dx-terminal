//! Gateway tools: discover, call, list micro MCPs via the gateway crate.

use crate::app::App;
use super::super::types::*;
use super::helpers::*;

/// Discover micro MCPs matching a capability
pub async fn gateway_discover(app: &App, req: GatewayDiscoverRequest) -> String {
    let (results, names_to_start) = {
        let gateway = app.gateway.lock().await;
        let matches = gateway.discover(&req.capability);

        if matches.is_empty() {
            return serde_json::json!({
                "status": "no_matches",
                "capability": req.capability,
                "hint": "No MCPs match this capability. Use gateway_list to see available MCPs, or register a new one.",
            }).to_string();
        }

        let results: Vec<serde_json::Value> = matches.iter().map(|d| {
            serde_json::json!({
                "name": d.name,
                "description": d.description,
                "capabilities": d.capabilities,
                "auto_start": d.auto_start,
                "running": gateway.get_tools(&d.name).is_some(),
            })
        }).collect();

        let names: Vec<String> = matches.iter()
            .filter(|d| gateway.get_tools(&d.name).is_none())
            .map(|d| d.name.clone())
            .collect();

        (results, names)
    };

    // Auto-start if requested
    if req.auto_start.unwrap_or(false) {
        let mut gw = app.gateway.lock().await;
        for name in &names_to_start {
            if let Err(e) = gw.ensure_running(name).await {
                return serde_json::json!({
                    "status": "partial",
                    "matches": results,
                    "start_error": format!("Failed to start '{}': {}", name, e),
                }).to_string();
            }
        }
    }

    serde_json::json!({
        "status": "found",
        "count": results.len(),
        "matches": results,
    }).to_string()
}

/// Call a tool on a running micro MCP
pub async fn gateway_call(app: &App, req: GatewayCallRequest) -> String {
    // Ensure the MCP is running
    {
        let mut gw = app.gateway.lock().await;
        if let Err(e) = gw.ensure_running(&req.mcp).await {
            return json_err(&format!("Failed to start MCP '{}': {}", req.mcp, e));
        }
    }

    let gw = app.gateway.lock().await;
    let arguments = req.arguments.and_then(|v| {
        if let serde_json::Value::Object(map) = v {
            Some(map)
        } else {
            None
        }
    });

    match gw.call(&req.mcp, &req.tool, arguments).await {
        Ok(result) => serde_json::json!({
            "status": if result.success { "success" } else { "error" },
            "mcp": result.mcp,
            "tool": result.tool,
            "content": result.content,
            "error": result.error,
        }).to_string(),
        Err(e) => json_err(&format!("Gateway call failed: {}", e)),
    }
}

/// List all MCPs (running and registered)
pub async fn gateway_list(app: &App, req: GatewayListRequest) -> String {
    let gw = app.gateway.lock().await;

    if req.running_only.unwrap_or(false) {
        let running = gw.list_running().await;
        serde_json::json!({
            "running": running.iter().map(|s| serde_json::json!({
                "name": s.name,
                "tool_count": s.tool_count,
                "tools": s.tools,
                "uptime_secs": s.uptime_secs,
                "last_used_secs_ago": s.last_used_secs_ago,
            })).collect::<Vec<_>>(),
            "count": running.len(),
        }).to_string()
    } else {
        let all = gw.list_all();
        let running_count = all.iter().filter(|(_, r)| *r).count();
        serde_json::json!({
            "mcps": all.iter().map(|(name, running)| serde_json::json!({
                "name": name,
                "running": running,
            })).collect::<Vec<_>>(),
            "total": all.len(),
            "running": running_count,
            "registered": all.len() - running_count,
        }).to_string()
    }
}

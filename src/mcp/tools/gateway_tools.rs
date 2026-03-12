//! Gateway tools: discover, call, list micro MCPs via the gateway crate.

use super::super::types::*;
use super::helpers::*;
use crate::app::App;

async fn sync_external_descriptors(app: &App) {
    let mut gateway = app.gateway.lock().await;
    crate::external_mcp::sync_gateway(&mut gateway);
}

/// Discover micro MCPs matching a capability
pub async fn gateway_discover(app: &App, req: GatewayDiscoverRequest) -> String {
    sync_external_descriptors(app).await;
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

        let results: Vec<serde_json::Value> = matches
            .iter()
            .map(|d| {
                serde_json::json!({
                    "name": d.name,
                    "description": d.description,
                    "capabilities": d.capabilities,
                    "auto_start": d.auto_start,
                    "running": gateway.get_tools(&d.name).is_some(),
                })
            })
            .collect();

        let names: Vec<String> = matches
            .iter()
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
                })
                .to_string();
            }
        }
    }

    serde_json::json!({
        "status": "found",
        "count": results.len(),
        "matches": results,
    })
    .to_string()
}

/// Call a tool on a running micro MCP
pub async fn gateway_call(app: &App, req: GatewayCallRequest) -> String {
    sync_external_descriptors(app).await;

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
        })
        .to_string(),
        Err(e) => json_err(&format!("Gateway call failed: {}", e)),
    }
}

/// List all MCPs (running and registered)
pub async fn gateway_list(app: &App, req: GatewayListRequest) -> String {
    sync_external_descriptors(app).await;
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
        })
        .to_string()
    } else {
        let mut descriptors = gw.list_descriptors();
        descriptors.sort_by(|left, right| left.name.cmp(&right.name));
        let running_count = descriptors
            .iter()
            .filter(|descriptor| gw.get_tools(&descriptor.name).is_some())
            .count();
        serde_json::json!({
            "mcps": descriptors.iter().map(|descriptor| serde_json::json!({
                "name": descriptor.name,
                "description": descriptor.description,
                "capabilities": descriptor.capabilities,
                "running": gw.get_tools(&descriptor.name).is_some(),
            })).collect::<Vec<_>>(),
            "total": descriptors.len(),
            "running": running_count,
            "registered": descriptors.len().saturating_sub(running_count),
        })
        .to_string()
    }
}

/// List tools exposed by one MCP, auto-starting it if needed.
pub async fn gateway_tools(app: &App, req: GatewayToolsRequest) -> String {
    sync_external_descriptors(app).await;

    if req.auto_start.unwrap_or(true) {
        let mut gw = app.gateway.lock().await;
        if let Err(e) = gw.ensure_running(&req.mcp).await {
            return json_err(&format!("Failed to start MCP '{}': {}", req.mcp, e));
        }
    }

    let gw = app.gateway.lock().await;
    let tools = match gw.get_tools(&req.mcp) {
        Some(tools) => tools,
        None => return json_err(&format!("MCP '{}' is not running", req.mcp)),
    };

    let descriptor = gw.get_descriptor(&req.mcp);
    let tool_rows: Vec<serde_json::Value> = tools
        .iter()
        .map(|tool| {
            serde_json::json!({
                "name": tool.name,
                "title": tool.title,
                "description": tool.description,
                "input_schema": tool.input_schema.as_ref(),
                "output_schema": tool.output_schema.as_deref(),
            })
        })
        .collect();

    serde_json::json!({
        "mcp": req.mcp,
        "description": descriptor.map(|d| d.description.clone()).unwrap_or_default(),
        "capabilities": descriptor.map(|d| d.capabilities.clone()).unwrap_or_default(),
        "tool_count": tool_rows.len(),
        "tools": tool_rows,
    })
    .to_string()
}

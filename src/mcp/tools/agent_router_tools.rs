use serde_json::json;

use crate::mcp::types::{AddRoutingRuleRequest, AgentStatsRequest, RouteTaskRequest};

pub async fn route_task(req: RouteTaskRequest) -> String {
    match crate::agent_router::route_task(&req.description, req.language.as_deref()) {
        Ok(recommendation) => json!({
            "status": "ok",
            "recommendation": recommendation,
        })
        .to_string(),
        Err(err) => json!({ "error": err.to_string() }).to_string(),
    }
}

pub async fn agent_stats(_req: AgentStatsRequest) -> String {
    match (
        crate::agent_router::agent_stats(),
        crate::agent_router::cost_report(),
    ) {
        (Ok(stats), Ok(costs)) => json!({
            "status": "ok",
            "stats": stats,
            "cost_report": costs,
        })
        .to_string(),
        (Err(err), _) | (_, Err(err)) => json!({ "error": err.to_string() }).to_string(),
    }
}

pub async fn add_routing_rule(req: AddRoutingRuleRequest) -> String {
    match crate::agent_router::add_routing_rule(&req.pattern, &req.provider, &req.reason) {
        Ok(rule) => json!({
            "status": "added",
            "rule": rule,
        })
        .to_string(),
        Err(err) => json!({ "error": err.to_string() }).to_string(),
    }
}

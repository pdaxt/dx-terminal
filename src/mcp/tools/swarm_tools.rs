use serde_json::json;
use std::sync::Arc;

use crate::app::App;
use crate::mcp::types::{SwarmStartRequest, SwarmStatusRequest, SwarmStopRequest};

pub async fn swarm_start(app: Arc<App>, req: SwarmStartRequest) -> String {
    let config = crate::swarm::SwarmConfig {
        repo: req.repo,
        max_agents: req.max_agents.unwrap_or(5),
        issue_labels: req.labels.unwrap_or_default(),
        agent_provider: req.provider.unwrap_or_else(|| "claude".to_string()),
    };

    match crate::swarm::start(app, config).await {
        Ok(report) => json!({
            "status": "started",
            "swarm": report,
        })
        .to_string(),
        Err(err) => json!({ "error": err.to_string() }).to_string(),
    }
}

pub async fn swarm_status(app: &App, _req: SwarmStatusRequest) -> String {
    match crate::swarm::status(app).await {
        Ok(report) => json!({
            "status": "ok",
            "swarm": report,
        })
        .to_string(),
        Err(err) => json!({ "error": err.to_string() }).to_string(),
    }
}

pub async fn swarm_stop(app: &App, _req: SwarmStopRequest) -> String {
    match crate::swarm::stop(app).await {
        Ok(report) => json!({
            "status": "stopped",
            "swarm": report,
        })
        .to_string(),
        Err(err) => json!({ "error": err.to_string() }).to_string(),
    }
}

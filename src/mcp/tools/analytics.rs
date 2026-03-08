//! Analytics micro MCP — token usage, costs, performance metrics.

use serde_json::{json, Value};
use super::MicroMcp;

pub struct AnalyticsMcp;

impl MicroMcp for AnalyticsMcp {
    fn namespace(&self) -> &str {
        "analytics"
    }

    fn tools(&self) -> Vec<Value> {
        vec![
            json!({
                "name": "get_usage",
                "description": "Get token usage and cost for the current session or a time period.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "period": {
                            "type": "string",
                            "description": "Time period: 'session', 'today', 'week', 'month'",
                            "default": "session"
                        },
                        "group_by": {
                            "type": "string",
                            "description": "Group results by: 'total', 'project', 'agent'",
                            "default": "total"
                        }
                    }
                }
            }),
            json!({
                "name": "get_cost_breakdown",
                "description": "Get cost breakdown by project, agent type, or day.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "days": { "type": "integer", "description": "Number of days to include", "default": 7 }
                    }
                }
            }),
            json!({
                "name": "get_system_stats",
                "description": "Get system resource usage: CPU, memory, load average.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
        ]
    }
}

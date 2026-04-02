use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

use crate::app::App;

#[derive(Debug, Clone, Copy)]
pub struct InternalServiceDefinition {
    pub name: &'static str,
    pub service_type: &'static str,
    pub interface: &'static str,
    pub purpose: &'static str,
    pub domain: &'static str,
    pub cli: &'static [&'static str],
    pub depends_on: &'static [&'static str],
    pub servable: bool,
}

const INTERNAL_SERVICES: &[InternalServiceDefinition] = &[
    InternalServiceDefinition {
        name: "mcp",
        service_type: "api_facade",
        interface: "stdio",
        purpose: "Monolithic MCP facade exposing the full internal tool graph.",
        domain: "edge",
        cli: &["dx services serve mcp", "dx mcp"],
        depends_on: &["core", "queue", "tracker", "coord", "intel"],
        servable: true,
    },
    InternalServiceDefinition {
        name: "core",
        service_type: "microservice",
        interface: "stdio",
        purpose: "Agent lifecycle, PTY management, pane control, and low-level runtime ops.",
        domain: "control_plane",
        cli: &["dx services serve core", "dx mcp core"],
        depends_on: &[],
        servable: true,
    },
    InternalServiceDefinition {
        name: "queue",
        service_type: "microservice",
        interface: "stdio",
        purpose: "Task queue, auto-cycle, prioritization, and execution routing.",
        domain: "work_management",
        cli: &["dx services serve queue", "dx mcp queue"],
        depends_on: &["core"],
        servable: true,
    },
    InternalServiceDefinition {
        name: "tracker",
        service_type: "microservice",
        interface: "stdio",
        purpose: "Issue tracking, milestones, sprints, and delivery planning.",
        domain: "planning",
        cli: &["dx services serve tracker", "dx mcp tracker"],
        depends_on: &["core"],
        servable: true,
    },
    InternalServiceDefinition {
        name: "coord",
        service_type: "microservice",
        interface: "stdio",
        purpose: "File locks, ports, messaging, and multi-agent coordination state.",
        domain: "coordination",
        cli: &["dx services serve coord", "dx mcp coord"],
        depends_on: &["core"],
        servable: true,
    },
    InternalServiceDefinition {
        name: "intel",
        service_type: "microservice",
        interface: "stdio",
        purpose: "Analytics, monitoring, quality gates, and vision/reporting workloads.",
        domain: "intelligence",
        cli: &["dx services serve intel", "dx mcp intel"],
        depends_on: &["core", "queue", "tracker", "coord"],
        servable: true,
    },
    InternalServiceDefinition {
        name: "web",
        service_type: "edge_service",
        interface: "http",
        purpose: "HTTP dashboard over the internal runtime state and replicated session feeds.",
        domain: "edge",
        cli: &["dx services serve web --port 3100", "dx web --port 3100"],
        depends_on: &["core", "queue", "tracker", "coord", "intel"],
        servable: true,
    },
    InternalServiceDefinition {
        name: "gateway",
        service_type: "embedded_gateway",
        interface: "in_process",
        purpose: "Embedded registry and lifecycle manager for external micro-MCP services.",
        domain: "integration",
        cli: &["dx services list --kind external", "dx services inspect <service>"],
        depends_on: &[],
        servable: false,
    },
];

pub fn internal_service(name: &str) -> Option<&'static InternalServiceDefinition> {
    let normalized = normalize_internal_service_name(name);
    INTERNAL_SERVICES.iter().find(|service| service.name == normalized)
}

pub fn normalize_internal_service_name(name: &str) -> &str {
    match name.trim().to_ascii_lowercase().as_str() {
        "all" => "mcp",
        other => {
            if other.is_empty() {
                ""
            } else {
                // The owned string can't be returned directly; only aliases are rewritten.
                name.trim()
            }
        }
    }
}

pub async fn list_services(app: &App, kind: &str, running_only: bool) -> Result<Value> {
    let kind = normalize_kind(kind)?;
    sync_external_descriptors(app).await;

    let internal = if matches!(kind, "all" | "internal") && !running_only {
        INTERNAL_SERVICES
            .iter()
            .map(|service| internal_service_value(service))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let (external, running_external, registered_external) = if matches!(kind, "all" | "external") {
        let gateway = app.gateway.lock().await;
        let mut descriptors = gateway.list_descriptors();
        descriptors.sort_by(|left, right| left.name.cmp(&right.name));
        let services = descriptors
            .into_iter()
            .filter(|descriptor| {
                !running_only || gateway.get_tools(&descriptor.name).is_some()
            })
            .map(|descriptor| {
                json!({
                    "name": descriptor.name,
                    "kind": "external",
                    "service_type": "microservice",
                    "interface": "stdio",
                    "domain": "integration",
                    "purpose": descriptor.description,
                    "capabilities": descriptor.capabilities,
                    "running": gateway.get_tools(&descriptor.name).is_some(),
                    "auto_start": descriptor.auto_start,
                    "command": descriptor.command,
                    "cli": [
                        format!("dx services inspect {}", descriptor.name),
                        format!("dx services call {} <tool>", descriptor.name),
                    ],
                })
            })
            .collect::<Vec<_>>();
        (services, gateway.running_count(), gateway.descriptor_count())
    } else {
        (Vec::new(), 0, 0)
    };

    Ok(json!({
        "architecture": "cli_microservices",
        "kind_filter": kind,
        "running_only": running_only,
        "services": [internal, external].concat(),
        "counts": {
            "internal": INTERNAL_SERVICES.len(),
            "external_registered": registered_external,
            "external_running": running_external,
        },
        "notes": [
            "Internal services are CLI-invoked and designed as on-demand processes.",
            "External services are discovered and auto-started through the embedded gateway.",
        ],
    }))
}

pub async fn inspect_service(app: &App, service_name: &str) -> Result<Value> {
    if let Some(service) = internal_service(service_name) {
        return Ok(json!({
            "name": service.name,
            "kind": "internal",
            "service_type": service.service_type,
            "interface": service.interface,
            "domain": service.domain,
            "purpose": service.purpose,
            "depends_on": service.depends_on,
            "servable": service.servable,
            "cli": service.cli,
            "topology_role": if service.name == "mcp" {
                "api_facade"
            } else if service.name == "web" {
                "edge_service"
            } else if service.name == "gateway" {
                "integration_gateway"
            } else {
                "microservice"
            },
        }));
    }

    sync_external_descriptors(app).await;
    let mut gateway = app.gateway.lock().await;
    let descriptor = gateway
        .get_descriptor(service_name)
        .cloned()
        .ok_or_else(|| anyhow!("unknown service '{}'", service_name))?;

    gateway
        .ensure_running(service_name)
        .await
        .with_context(|| format!("start external service '{}'", service_name))?;

    let tools = gateway
        .get_tools(service_name)
        .ok_or_else(|| anyhow!("service '{}' did not report tools after startup", service_name))?;
    let tool_rows = tools
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "title": tool.title,
                "description": tool.description,
                "input_schema": tool.input_schema.as_ref(),
                "output_schema": tool.output_schema.as_deref(),
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "name": descriptor.name,
        "kind": "external",
        "service_type": "microservice",
        "interface": "stdio",
        "domain": "integration",
        "purpose": descriptor.description,
        "capabilities": descriptor.capabilities,
        "running": true,
        "auto_start": descriptor.auto_start,
        "command": descriptor.command,
        "tool_count": tool_rows.len(),
        "tools": tool_rows,
        "cli": [
            format!("dx services inspect {}", descriptor.name),
            format!("dx services call {} <tool>", descriptor.name),
        ],
    }))
}

pub async fn call_service(
    app: &App,
    service_name: &str,
    tool: &str,
    args: Option<String>,
) -> Result<Value> {
    if let Some(service) = internal_service(service_name) {
        bail!(
            "internal service '{}' is CLI-served only; use `{}`",
            service.name,
            service.cli.first().copied().unwrap_or("dx services serve <service>")
        );
    }

    sync_external_descriptors(app).await;
    let mut gateway = app.gateway.lock().await;
    gateway
        .ensure_running(service_name)
        .await
        .with_context(|| format!("start external service '{}'", service_name))?;
    drop(gateway);

    let parsed_args = match args {
        Some(raw) => Some(
            serde_json::from_str::<Value>(&raw)
                .map_err(|e| anyhow!("Invalid --args JSON: {}", e))?,
        ),
        None => None,
    };

    let mut gateway = app.gateway.lock().await;
    let arguments = parsed_args.and_then(|value| match value {
        Value::Object(map) => Some(map),
        _ => None,
    });
    let result = gateway
        .call(service_name, tool, arguments)
        .await
        .with_context(|| format!("call {} on external service '{}'", tool, service_name))?;

    Ok(json!({
        "status": if result.success { "success" } else { "error" },
        "service": result.mcp,
        "tool": result.tool,
        "content": result.content,
        "error": result.error,
    }))
}

pub fn topology() -> Value {
    let services = INTERNAL_SERVICES
        .iter()
        .map(|service| {
            json!({
                "name": service.name,
                "kind": "internal",
                "service_type": service.service_type,
                "interface": service.interface,
                "domain": service.domain,
                "depends_on": service.depends_on,
                "cli": service.cli,
            })
        })
        .collect::<Vec<_>>();

    let edges = INTERNAL_SERVICES
        .iter()
        .flat_map(|service| {
            service.depends_on.iter().map(move |dependency| {
                json!({
                    "from": service.name,
                    "to": dependency,
                    "relationship": "depends_on",
                })
            })
        })
        .chain([
            json!({
                "from": "gateway",
                "to": "external:*",
                "relationship": "spawns",
            }),
            json!({
                "from": "mcp",
                "to": "core",
                "relationship": "aggregates",
            }),
            json!({
                "from": "mcp",
                "to": "queue",
                "relationship": "aggregates",
            }),
            json!({
                "from": "mcp",
                "to": "tracker",
                "relationship": "aggregates",
            }),
            json!({
                "from": "mcp",
                "to": "coord",
                "relationship": "aggregates",
            }),
            json!({
                "from": "mcp",
                "to": "intel",
                "relationship": "aggregates",
            }),
        ])
        .collect::<Vec<_>>();

    json!({
        "architecture": "cli_microservices",
        "entrypoint": "dx services",
        "services": services,
        "edges": edges,
        "notes": [
            "The CLI is the control plane: services are listed, inspected, served, and called through `dx services ...`.",
            "Internal services are split MCP domains; external services are micro-MCPs managed by the embedded gateway.",
        ],
    })
}

fn normalize_kind(kind: &str) -> Result<&str> {
    match kind.trim().to_ascii_lowercase().as_str() {
        "all" => Ok("all"),
        "internal" => Ok("internal"),
        "external" => Ok("external"),
        other => bail!("unknown service kind '{}'; expected all, internal, or external", other),
    }
}

fn internal_service_value(service: &InternalServiceDefinition) -> Value {
    json!({
        "name": service.name,
        "kind": "internal",
        "service_type": service.service_type,
        "interface": service.interface,
        "domain": service.domain,
        "purpose": service.purpose,
        "depends_on": service.depends_on,
        "servable": service.servable,
        "running": Value::Null,
        "cli": service.cli,
    })
}

async fn sync_external_descriptors(app: &App) {
    let mut gateway = app.gateway.lock().await;
    crate::external_mcp::sync_gateway(&mut gateway);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_service_aliases_mcp() {
        let service = internal_service("all").expect("alias should resolve");
        assert_eq!(service.name, "mcp");
    }

    #[test]
    fn topology_exposes_cli_microservices_architecture() {
        let topology = topology();
        assert_eq!(topology["architecture"], "cli_microservices");
        assert!(topology["services"].as_array().unwrap().len() >= 6);
    }
}

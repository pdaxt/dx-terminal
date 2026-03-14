use crate::app::App;
use crate::state::events::StateEvent;
use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request},
};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper_util::{
    client::legacy::{connect::HttpConnector, Client},
    rt::TokioExecutor,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::ServiceExt;
use uuid::Uuid;
const EVENT_COOLDOWN_MS: u64 = 800;

#[derive(Clone)]
struct ContractClient {
    transport: ContractTransport,
    actor: String,
}

#[derive(Clone)]
enum ContractTransport {
    Local {
        app: Arc<App>,
    },
    Remote {
        base_url: String,
        client: Client<HttpConnector, Full<Bytes>>,
    },
}

impl ContractClient {
    fn new(app: Arc<App>) -> Self {
        let actor = crate::config::http_supervisor_id();
        let transport = if let Some(base_url) = crate::config::http_supervisor_base_url() {
            let connector = HttpConnector::new();
            let client = Client::builder(TokioExecutor::new()).build(connector);
            ContractTransport::Remote { base_url, client }
        } else {
            ContractTransport::Local { app }
        };
        Self { transport, actor }
    }

    fn is_remote(&self) -> bool {
        matches!(self.transport, ContractTransport::Remote { .. })
    }

    fn remote_target(&self) -> Option<(String, Client<HttpConnector, Full<Bytes>>)> {
        match &self.transport {
            ContractTransport::Remote { base_url, client } => {
                Some((base_url.clone(), client.clone()))
            }
            ContractTransport::Local { .. } => None,
        }
    }

    async fn request_json(
        &self,
        method: Method,
        path_and_query: &str,
        body: Option<Value>,
    ) -> anyhow::Result<Value> {
        match &self.transport {
            ContractTransport::Local { app } => {
                self.request_json_local(app, method, path_and_query, body)
                    .await
            }
            ContractTransport::Remote { base_url, client } => {
                self.request_json_remote(client, base_url, method, path_and_query, body)
                    .await
            }
        }
    }

    async fn request_json_local(
        &self,
        app: &Arc<App>,
        method: Method,
        path_and_query: &str,
        body: Option<Value>,
    ) -> anyhow::Result<Value> {
        let mut builder = Request::builder()
            .method(method)
            .uri(path_and_query)
            .header(header::ACCEPT, "application/json")
            .header("x-dx-actor", &self.actor);

        if let Some(token) = crate::config::control_token() {
            builder = builder
                .header("x-dx-control-token", &token)
                .header(header::AUTHORIZATION, format!("Bearer {}", token));
        }

        let request = if let Some(payload) = body {
            builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))?
        } else {
            builder.body(Body::empty())?
        };

        let response = crate::web::build_router(Arc::clone(app))
            .oneshot(request)
            .await?;
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let mut value = serde_json::from_slice::<Value>(&bytes)
            .unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&bytes).to_string() }));
        if !status.is_success() {
            if let Some(object) = value.as_object_mut() {
                object.insert("_http_status".to_string(), json!(status.as_u16()));
            }
        }
        Ok(value)
    }

    async fn request_json_remote(
        &self,
        client: &Client<HttpConnector, Full<Bytes>>,
        base_url: &str,
        method: Method,
        path_and_query: &str,
        body: Option<Value>,
    ) -> anyhow::Result<Value> {
        let full_url = format!("{}{}", base_url.trim_end_matches('/'), path_and_query);
        let mut builder = Request::builder()
            .method(method)
            .uri(full_url)
            .header(header::ACCEPT, "application/json")
            .header("x-dx-actor", &self.actor);

        if let Some(token) = crate::config::control_token() {
            builder = builder
                .header("x-dx-control-token", &token)
                .header(header::AUTHORIZATION, format!("Bearer {}", token));
        }

        let request = if let Some(payload) = body {
            builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Full::new(Bytes::from(payload.to_string())))?
        } else {
            builder.body(Full::new(Bytes::new()))?
        };

        let response = client.request(request).await?;
        let status = response.status();
        let bytes = response.into_body().collect().await?.to_bytes();
        let mut value = serde_json::from_slice::<Value>(&bytes)
            .unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&bytes).to_string() }));
        if !status.is_success() {
            if let Some(object) = value.as_object_mut() {
                object.insert("_http_status".to_string(), json!(status.as_u16()));
            }
        }
        Ok(value)
    }

    async fn scheduler_snapshot(
        &self,
        project_name: &str,
        project_path: &str,
    ) -> anyhow::Result<Value> {
        self.request_json(
            Method::GET,
            &format!(
                "/api/dxos/scheduler?project={}&path={}",
                encode_component(project_name),
                encode_component(project_path)
            ),
            None,
        )
        .await
    }

    async fn scheduler_run(&self, project_name: &str, project_path: &str) -> anyhow::Result<Value> {
        let run_id = format!(
            "{}:{}:{}",
            self.actor,
            encode_component(project_name),
            Uuid::new_v4()
        );
        self.request_json(
            Method::POST,
            "/api/dxos/scheduler/run",
            Some(json!({
                "project": project_name,
                "path": project_path,
                "run_id": run_id,
            })),
        )
        .await
    }

    async fn registered_projects(&self) -> anyhow::Result<Vec<(String, String)>> {
        if !self.is_remote() {
            return Ok(local_registered_projects());
        }
        let registry = self
            .request_json(Method::GET, "/api/dxos/registry", None)
            .await?;
        Ok(projects_from_registry(&registry))
    }
}

#[derive(Clone)]
struct Supervisor {
    client: ContractClient,
    last_tick: Arc<Mutex<HashMap<String, tokio::time::Instant>>>,
}

impl Supervisor {
    fn new(app: Arc<App>) -> Self {
        Self {
            client: ContractClient::new(app),
            last_tick: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn tick_project_if_ready(
        &self,
        project_name: &str,
        project_path: &str,
    ) -> anyhow::Result<Value> {
        let scheduler = self
            .client
            .scheduler_snapshot(project_name, project_path)
            .await?;
        let next_launch = scheduler
            .get("scheduler")
            .and_then(|value| value.get("next_launch"))
            .cloned()
            .unwrap_or_else(|| json!(null));
        let ready = next_launch
            .get("ready")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| !next_launch.is_null());
        if !ready || next_launch.is_null() {
            return Ok(json!({
                "project": project_name,
                "project_path": project_path,
                "actor": self.client.actor,
                "action": "no_ready_launch",
            }));
        }
        self.client.scheduler_run(project_name, project_path).await
    }

    async fn tick_project_with_cooldown(
        &self,
        project_name: &str,
        project_path: &str,
    ) -> Option<Value> {
        let now = tokio::time::Instant::now();
        {
            let mut last = self.last_tick.lock().await;
            if let Some(previous) = last.get(project_path) {
                if now.duration_since(*previous)
                    < std::time::Duration::from_millis(EVENT_COOLDOWN_MS)
                {
                    return None;
                }
            }
            last.insert(project_path.to_string(), now);
        }

        match self.tick_project_if_ready(project_name, project_path).await {
            Ok(value) => Some(value),
            Err(error) => Some(json!({
                "project": project_name,
                "project_path": project_path,
                "error": error.to_string(),
            })),
        }
    }
}

fn encode_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{:02X}", byte));
        }
    }
    encoded
}

fn projects_from_registry(registry: &Value) -> Vec<(String, String)> {
    registry
        .get("projects")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let path = item.get("path").and_then(Value::as_str)?.trim().to_string();
                    if path.is_empty() {
                        return None;
                    }
                    let name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .unwrap_or_else(|| {
                            std::path::Path::new(&path)
                                .file_name()
                                .and_then(|value| value.to_str())
                                .unwrap_or("project")
                        })
                        .to_string();
                    Some((name, path))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn local_registered_projects() -> Vec<(String, String)> {
    let registry = serde_json::from_str::<Value>(&crate::dxos::control_plane_registry())
        .unwrap_or_else(|_| json!({}));
    projects_from_registry(&registry)
}

fn event_project(event: &StateEvent) -> Option<String> {
    match event {
        StateEvent::VisionChanged { project, .. }
        | StateEvent::DebateChanged { project, .. }
        | StateEvent::SessionContractChanged { project, .. }
        | StateEvent::WorkflowRunChanged { project, .. } => Some(project.clone()),
        _ => None,
    }
}

fn event_project_from_value(payload: &Value) -> Option<String> {
    payload
        .get("event")
        .and_then(|event| event.get("project"))
        .and_then(Value::as_str)
        .map(|value| value.to_string())
}

async fn stream_remote_events(supervisor: Supervisor) {
    let Some((base_url, client)) = supervisor.client.remote_target() else {
        return;
    };

    loop {
        let mut builder = Request::builder()
            .method(Method::GET)
            .uri(format!("{}/api/events", base_url.trim_end_matches('/')))
            .header(header::ACCEPT, "text/event-stream")
            .header("x-dx-actor", &supervisor.client.actor);
        if let Some(token) = crate::config::control_token() {
            builder = builder
                .header("x-dx-control-token", &token)
                .header(header::AUTHORIZATION, format!("Bearer {}", token));
        }
        let request = match builder.body(Full::new(Bytes::new())) {
            Ok(request) => request,
            Err(error) => {
                tracing::warn!("DXOS supervisor SSE request build failed: {}", error);
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                continue;
            }
        };

        let response = match client.request(request).await {
            Ok(response) => response,
            Err(error) => {
                tracing::warn!("DXOS supervisor SSE connect failed: {}", error);
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                continue;
            }
        };

        if !response.status().is_success() {
            tracing::warn!(
                "DXOS supervisor SSE returned HTTP {}",
                response.status().as_u16()
            );
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            continue;
        }

        let mut body = response.into_body();
        let mut buffer = String::new();
        while let Some(frame) = body.frame().await {
            let Ok(frame) = frame else {
                break;
            };
            let Some(bytes) = frame.data_ref() else {
                continue;
            };
            buffer.push_str(&String::from_utf8_lossy(bytes));

            while let Some(separator) = buffer.find("\n\n") {
                let chunk = buffer[..separator].to_string();
                buffer = buffer[separator + 2..].to_string();
                let data = chunk
                    .lines()
                    .filter_map(|line| line.strip_prefix("data: "))
                    .collect::<Vec<_>>()
                    .join("\n");
                if data.is_empty() {
                    continue;
                }
                let Ok(payload) = serde_json::from_str::<Value>(&data) else {
                    continue;
                };
                let Some(project_name) = event_project_from_value(&payload) else {
                    continue;
                };
                let project_path = match supervisor.client.registered_projects().await {
                    Ok(projects) => projects
                        .into_iter()
                        .find(|(name, _)| name == &project_name)
                        .map(|(_, path)| path)
                        .unwrap_or_else(|| project_name.clone()),
                    Err(_) => project_name.clone(),
                };
                let _ = supervisor
                    .tick_project_with_cooldown(&project_name, &project_path)
                    .await;
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

pub fn start(app: Arc<App>) {
    if !crate::config::http_supervisor_autorun_enabled() {
        tracing::info!(
            "DXOS HTTP supervisor disabled. Set DX_HTTP_SUPERVISOR_AUTORUN=1 to enable contract-driven orchestration."
        );
        return;
    }

    let interval_secs = crate::config::http_supervisor_interval_secs();
    let target = crate::config::http_supervisor_base_url()
        .unwrap_or_else(|| "in-process router".to_string());
    tracing::info!(
        "DXOS HTTP supervisor enabled — id: {}, target: {}, sweep: {}s through the public control contract",
        crate::config::http_supervisor_id(),
        target,
        interval_secs
    );

    let supervisor = Supervisor::new(Arc::clone(&app));

    let periodic = supervisor.clone();
    tokio::spawn(async move {
        let interval = std::time::Duration::from_secs(interval_secs);
        loop {
            tokio::time::sleep(interval).await;
            let Ok(projects) = periodic.client.registered_projects().await else {
                continue;
            };
            for (project_name, project_path) in projects {
                if let Some(result) = periodic
                    .tick_project_with_cooldown(&project_name, &project_path)
                    .await
                {
                    if result
                        .get("result")
                        .and_then(|value| value.get("action"))
                        .and_then(Value::as_str)
                        == Some("launch_attempted")
                    {
                        tracing::info!(
                            "DXOS HTTP supervisor launched queued work for {}",
                            project_name
                        );
                    }
                }
            }
        }
    });

    if supervisor.client.is_remote() {
        let remote = supervisor.clone();
        tokio::spawn(async move {
            stream_remote_events(remote).await;
        });
    } else {
        let evented = supervisor.clone();
        tokio::spawn(async move {
            let mut rx = app.state.event_bus.subscribe();
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let Some(project_name) = event_project(&event) else {
                            continue;
                        };
                        let project_path = local_registered_projects()
                            .into_iter()
                            .find(|(name, _)| name == &project_name)
                            .map(|(_, path)| path)
                            .unwrap_or_else(|| project_name.clone());
                        let _ = evented
                            .tick_project_with_cooldown(&project_name, &project_path)
                            .await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_component_percent_encodes_paths() {
        assert_eq!(encode_component("dx terminal/path"), "dx%20terminal%2Fpath");
    }

    #[test]
    fn event_project_extracts_dxos_scoped_project_names() {
        let event = StateEvent::SessionContractChanged {
            project: "demo".to_string(),
            session_id: "S1".to_string(),
            role: "design".to_string(),
            status: "planned".to_string(),
            action: "session_upserted".to_string(),
        };
        assert_eq!(event_project(&event).as_deref(), Some("demo"));
    }
}

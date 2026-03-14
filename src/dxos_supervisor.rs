use crate::app::App;
use crate::state::events::StateEvent;
use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request},
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::ServiceExt;

const SUPERVISOR_ACTOR: &str = "dxos_http_supervisor";
const EVENT_COOLDOWN_MS: u64 = 800;

#[derive(Clone)]
struct ContractClient {
    app: Arc<App>,
}

impl ContractClient {
    fn new(app: Arc<App>) -> Self {
        Self { app }
    }

    async fn request_json(
        &self,
        method: Method,
        path_and_query: &str,
        body: Option<Value>,
    ) -> anyhow::Result<Value> {
        let mut builder = Request::builder()
            .method(method)
            .uri(path_and_query)
            .header(header::ACCEPT, "application/json")
            .header("x-dx-actor", SUPERVISOR_ACTOR);

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

        let response = crate::web::build_router(Arc::clone(&self.app))
            .oneshot(request)
            .await?;
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let mut value =
            serde_json::from_slice::<Value>(&bytes).unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&bytes).to_string() }));
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

    async fn scheduler_run(
        &self,
        project_name: &str,
        project_path: &str,
    ) -> anyhow::Result<Value> {
        self.request_json(
            Method::POST,
            "/api/dxos/scheduler/run",
            Some(json!({
                "project": project_name,
                "path": project_path,
            })),
        )
        .await
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

    async fn tick_project_if_ready(&self, project_name: &str, project_path: &str) -> anyhow::Result<Value> {
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
                "action": "no_ready_launch",
            }));
        }
        self.client.scheduler_run(project_name, project_path).await
    }

    async fn tick_project_with_cooldown(&self, project_name: &str, project_path: &str) -> Option<Value> {
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

fn registered_projects() -> Vec<(String, String)> {
    let registry = serde_json::from_str::<Value>(&crate::dxos::control_plane_registry())
        .unwrap_or_else(|_| json!({}));
    registry
        .get("projects")
        .and_then(Value::as_array)
        .map(|items| {
            items.iter().filter_map(|item| {
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
            }).collect::<Vec<_>>()
        })
        .unwrap_or_default()
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

pub fn start(app: Arc<App>) {
    if !crate::config::http_supervisor_autorun_enabled() {
        tracing::info!(
            "DXOS HTTP supervisor disabled. Set DX_HTTP_SUPERVISOR_AUTORUN=1 to enable contract-driven orchestration."
        );
        return;
    }

    let interval_secs = crate::config::http_supervisor_interval_secs();
    tracing::info!(
        "DXOS HTTP supervisor enabled — sweeping every {}s through the public control contract",
        interval_secs
    );

    let supervisor = Supervisor::new(Arc::clone(&app));

    let periodic = supervisor.clone();
    tokio::spawn(async move {
        let interval = std::time::Duration::from_secs(interval_secs);
        loop {
            tokio::time::sleep(interval).await;
            for (project_name, project_path) in registered_projects() {
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

    let evented = supervisor.clone();
    tokio::spawn(async move {
        let mut rx = app.state.event_bus.subscribe();
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let Some(project_name) = event_project(&event) else {
                        continue;
                    };
                    let project_path = registered_projects()
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

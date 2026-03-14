use crate::app::App;
use crate::mcp::tools::panes;
use crate::mcp::types::SpawnRequest;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug)]
struct LaunchCandidate {
    project_name: String,
    project_path: String,
    session_id: String,
    role: String,
    provider: Option<String>,
    model: Option<String>,
    runtime_adapter: Option<String>,
    feature_id: Option<String>,
    stage: Option<String>,
    supervisor_session_id: Option<String>,
    objective: String,
}

fn registered_projects() -> Vec<(String, String)> {
    let registry = serde_json::from_str::<Value>(&crate::dxos::control_plane_registry())
        .unwrap_or_else(|_| json!({}));
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

fn next_launch_candidate(project_name: &str, project_path: &str) -> Option<LaunchCandidate> {
    let scheduler = serde_json::from_str::<Value>(&crate::dxos::scheduler_snapshot(
        project_path,
        Some(project_name),
    ))
    .ok()?;
    let next_launch = scheduler
        .get("scheduler")
        .and_then(|value| value.get("next_launch"))?;
    if next_launch.get("ready").and_then(Value::as_bool) == Some(false) {
        return None;
    }
    let session_id = next_launch
        .get("session_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();

    let context =
        crate::dxos::runtime_launch_context(project_path, Some(project_name), &session_id);
    let session = context.get("session")?;
    Some(LaunchCandidate {
        project_name: project_name.to_string(),
        project_path: project_path.to_string(),
        session_id,
        role: session
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("developer")
            .to_string(),
        provider: session
            .get("provider")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        model: session
            .get("model")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        runtime_adapter: session
            .get("runtime_adapter")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        feature_id: session
            .get("feature_id")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        stage: session
            .get("stage")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        supervisor_session_id: session
            .get("supervisor_session_id")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        objective: session
            .get("objective")
            .and_then(Value::as_str)
            .unwrap_or("Advance the assigned DXOS lane.")
            .to_string(),
    })
}

async fn launch_claimed_candidate(app: &App, candidate: &LaunchCandidate) -> Value {
    let request = SpawnRequest {
        pane: "auto".to_string(),
        project: candidate.project_path.clone(),
        role: Some(candidate.role.clone()),
        provider: candidate.provider.clone(),
        model: candidate.model.clone(),
        runtime_adapter: candidate.runtime_adapter.clone(),
        client_request_id: Some(format!("dxos-scheduler:{}", candidate.session_id)),
        session_id: Some(candidate.session_id.clone()),
        feature_id: candidate.feature_id.clone(),
        stage: candidate.stage.clone(),
        supervisor_session_id: candidate.supervisor_session_id.clone(),
        task: Some(candidate.objective.clone()),
        prompt: None,
        autonomous: Some(true),
    };
    let raw = panes::spawn(app, request).await;
    serde_json::from_str(&raw).unwrap_or_else(|_| json!({ "raw": raw }))
}

fn scheduler_run_id(actor: &str, project_name: &str, run_id: Option<&str>) -> String {
    run_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| format!("{}:{}:{}", actor.trim(), project_name, Uuid::new_v4()))
}

fn finalize_scheduler_run(
    project_name: &str,
    project_path: &str,
    actor: &str,
    run_id: &str,
    result: Value,
) -> Value {
    crate::dxos::remember_scheduler_run_result(
        project_path,
        Some(project_name),
        actor,
        run_id,
        result,
    )
}

pub async fn drive_once_for_project(
    app: &App,
    project_name: &str,
    project_path: &str,
    actor: Option<&str>,
    run_id: Option<&str>,
) -> Value {
    let actor = actor
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("dxos_scheduler");
    let run_id = scheduler_run_id(actor, project_name, run_id);
    if let Some(existing) =
        crate::dxos::scheduler_run_replay(project_path, Some(project_name), &run_id)
    {
        return json!({
            "project": project_name,
            "project_path": project_path,
            "actor": actor,
            "run_id": run_id,
            "action": "scheduler_run_replayed",
            "outcome": existing.get("outcome").cloned().unwrap_or_else(|| json!("ok")),
            "result": existing,
        });
    }

    let Some(candidate) = next_launch_candidate(project_name, project_path) else {
        return finalize_scheduler_run(project_name, project_path, actor, &run_id, json!({
            "project": project_name,
            "project_path": project_path,
            "actor": actor,
            "run_id": run_id,
            "action": "no_ready_launch",
            "outcome": "ok",
        }));
    };

    let claim = crate::dxos::claim_session_launch(
        project_path,
        Some(project_name),
        &candidate.session_id,
        Some(actor),
        Some(&run_id),
    );
    let claim_value = serde_json::from_str::<Value>(&claim).unwrap_or_else(|_| json!({}));
    if claim_value.get("error").is_some() {
        return finalize_scheduler_run(project_name, project_path, actor, &run_id, json!({
            "project": project_name,
            "project_path": project_path,
            "actor": actor,
            "run_id": run_id,
            "action": "claim_skipped",
            "outcome": "blocked",
            "claim": claim_value,
        }));
    }

    let launched = launch_claimed_candidate(app, &candidate).await;
    let outcome = if launched.get("error").is_some() {
        "error"
    } else {
        "ok"
    };
    let summary = if outcome == "ok" {
        format!(
            "DXOS scheduler launched {} on {}",
            candidate.session_id, project_name
        )
    } else {
        format!(
            "DXOS scheduler failed to launch {} on {}",
            candidate.session_id, project_name
        )
    };
    let _ = crate::dxos::append_audit_record(
        project_path,
        Some(project_name),
        actor,
        "scheduler_launch",
        &candidate.session_id,
        outcome,
        &summary,
        json!({
            "run_id": run_id,
            "claim": claim_value,
            "launch": launched,
        }),
    );

    finalize_scheduler_run(project_name, project_path, actor, &run_id, json!({
        "project": project_name,
        "project_path": project_path,
        "actor": actor,
        "run_id": run_id,
        "action": "launch_attempted",
        "outcome": outcome,
        "session_id": candidate.session_id,
        "claim": claim_value,
        "launch": launched,
    }))
}

pub async fn drive_once(app: &App) -> Value {
    let mut launched = Vec::new();
    let mut skipped = Vec::new();

    for (project_name, project_path) in registered_projects() {
        let result = drive_once_for_project(app, &project_name, &project_path, None, None).await;
        match result.get("action").and_then(Value::as_str) {
            Some("launch_attempted") => launched.push(result),
            _ => skipped.push(result),
        }
    }

    json!({
        "status": "ok",
        "launched": launched,
        "skipped": skipped,
        "launched_count": launched.len(),
        "skipped_count": skipped.len(),
    })
}

pub fn start(app: Arc<App>) {
    if !crate::config::scheduler_autorun_enabled() {
        tracing::info!(
            "DXOS scheduler autorun disabled. Set DX_SCHEDULER_AUTORUN=1 to enable the local scheduler loop."
        );
        return;
    }

    let interval = std::time::Duration::from_secs(crate::config::scheduler_interval_secs());
    tracing::info!(
        "DXOS scheduler autorun enabled — polling every {}s",
        interval.as_secs()
    );
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            let result = drive_once(app.as_ref()).await;
            let launched = result
                .get("launched_count")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if launched > 0 {
                tracing::info!("DXOS scheduler launched {} queued lane(s)", launched);
            }
        }
    });
}

pub fn kick_project(app: Arc<App>, project_name: String, project_path: String) {
    if !crate::config::scheduler_autorun_enabled() {
        return;
    }
    tokio::spawn(async move {
        let result =
            drive_once_for_project(app.as_ref(), &project_name, &project_path, None, None).await;
        if result.get("action").and_then(Value::as_str) == Some("launch_attempted") {
            tracing::info!(
                "DXOS scheduler kick launched queued lane for {}",
                project_name
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn next_launch_candidate_uses_scheduler_ready_item() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let session = crate::dxos::upsert_session_contract(
            project,
            Some("demo"),
            None,
            "design",
            Some("claude"),
            Some("claude-opus-4.6"),
            Some("guarded_auto"),
            "Prepare design options",
            vec!["mockups".to_string()],
            vec!["docs".to_string()],
            vec![project.to_string()],
            vec![project.to_string()],
            Some(project),
            None,
            None,
            None,
            Some("pty_native_adapter"),
            None,
            Some("F1.1"),
            Some("design"),
            None,
            Some("lead_then_human"),
            Some("planned"),
        );
        let session_value: Value = serde_json::from_str(&session).unwrap();
        let session_id = session_value["session_id"].as_str().unwrap();
        let _ = crate::dxos::delegate_work_order(
            project,
            Some("demo"),
            session_id,
            Some(session_id),
            "Prepare design options",
            "Prepare design options",
            Some("F1.1"),
            Some("design"),
            vec!["docs".to_string()],
            vec!["mockups".to_string()],
        );

        let candidate = next_launch_candidate("demo", project).unwrap();
        assert_eq!(candidate.session_id, session_id);
        assert_eq!(candidate.role, "design");
        assert_eq!(candidate.stage.as_deref(), Some("design"));
    }
}

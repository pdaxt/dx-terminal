use crate::app::App;
use crate::dxos;
use crate::tmux;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct ResolutionDeliveryResult {
    pub status: String,
    pub work_order_id: Option<String>,
    pub worker_session_id: Option<String>,
    pub via: Option<String>,
    pub pane: Option<u8>,
    pub tmux_target: Option<String>,
    pub error: Option<String>,
    pub message: Option<String>,
}

fn compose_resolution_message(work_order: &Value, resolution: &str) -> String {
    let work_order_id = work_order
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("work-order");
    let feature_id = work_order
        .get("feature_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let stage = work_order
        .get("stage")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let permissions = work_order
        .get("requested_permissions")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let blockers = work_order
        .get("blockers")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut lines = vec![format!("DXOS RESOLUTION [{}]", work_order_id)];
    if let Some(feature_id) = feature_id {
        if let Some(stage) = stage {
            lines.push(format!("Scope: {} · {}", feature_id, stage));
        } else {
            lines.push(format!("Scope: {}", feature_id));
        }
    } else if let Some(stage) = stage {
        lines.push(format!("Stage: {}", stage));
    }
    if !blockers.is_empty() {
        lines.push(format!("Cleared blocker: {}", blockers.join(" | ")));
    }
    if !permissions.is_empty() {
        lines.push(format!("Approved permission: {}", permissions.join(", ")));
    }
    lines.push(format!("Guidance: {}", resolution.trim()));
    lines.push(
        "Resume work now and keep going on the next high-value task in scope. If you hit another permission, login, or human-approval gate, raise dxos_session_raise_blocker again."
            .to_string(),
    );
    lines.join("\n")
}

pub async fn deliver_work_order_resolution(
    app: &App,
    project_path: &str,
    project_name: Option<&str>,
    result: &str,
    resolution: Option<&str>,
) -> ResolutionDeliveryResult {
    let value = match serde_json::from_str::<Value>(result) {
        Ok(value) => value,
        Err(error) => {
            return ResolutionDeliveryResult {
                status: "skipped".to_string(),
                work_order_id: None,
                worker_session_id: None,
                via: None,
                pane: None,
                tmux_target: None,
                error: Some(format!("unparseable_result: {}", error)),
                message: None,
            };
        }
    };
    if value.get("error").is_some() {
        return ResolutionDeliveryResult {
            status: "skipped".to_string(),
            work_order_id: None,
            worker_session_id: None,
            via: None,
            pane: None,
            tmux_target: None,
            error: value
                .get("error")
                .and_then(Value::as_str)
                .map(|value| value.to_string()),
            message: None,
        };
    }

    let Some(work_order) = value.get("work_order") else {
        return ResolutionDeliveryResult {
            status: "skipped".to_string(),
            work_order_id: None,
            worker_session_id: None,
            via: None,
            pane: None,
            tmux_target: None,
            error: Some("missing_work_order".to_string()),
            message: None,
        };
    };

    let work_order_id = work_order
        .get("id")
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    let worker_session_id = work_order
        .get("worker_session_id")
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    let resolution = resolution
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Portal approval: blocker cleared. Continue from your current point.");
    let message = compose_resolution_message(work_order, resolution);

    let mut pane = None;
    let mut tmux_target = None;
    if let Some(session_id) = worker_session_id.as_deref() {
        let control = dxos::load_control_plane(project_path, project_name);
        if let Some(session) = control.sessions.iter().find(|item| item.id == session_id) {
            pane = session.pane;
            tmux_target = session.tmux_target.clone();
        }

        if pane.is_none() || tmux_target.is_none() {
            let snapshot = app.state.get_state_snapshot().await;
            for (pane_key, pane_state) in &snapshot.panes {
                if pane_state.dxos_session_id.as_deref() == Some(session_id) {
                    if pane.is_none() {
                        pane = pane_key.parse::<u8>().ok();
                    }
                    if tmux_target.is_none() {
                        tmux_target = pane_state.tmux_target.clone();
                    }
                    break;
                }
            }
        }
    }

    let delivery = if let Some(target) = tmux_target
        .as_ref()
        .filter(|target| tmux::pane_exists(target))
    {
        match tokio::task::spawn_blocking({
            let target = target.clone();
            let message = message.clone();
            move || tmux::send_command(&target, &message)
        })
        .await
        {
            Ok(Ok(())) => Ok("tmux".to_string()),
            Ok(Err(error)) => Err(error.to_string()),
            Err(error) => Err(format!("task join error: {}", error)),
        }
    } else if let Some(pane_num) = pane {
        let send_result = {
            let mut pty = app.pty_lock();
            pty.send_line(pane_num, &message)
        };
        send_result
            .map(|_| "pty".to_string())
            .map_err(|error| error.to_string())
    } else {
        Err("No live worker lane found for the resolved work order.".to_string())
    };

    match delivery {
        Ok(via) => {
            if let Some(pane_num) = pane {
                app.state
                    .log_activity(
                        pane_num,
                        "dxos_resolution",
                        &format!(
                            "Delivered DXOS resolution for {} via {}",
                            work_order_id.as_deref().unwrap_or("work-order"),
                            via
                        ),
                    )
                    .await;
            }
            ResolutionDeliveryResult {
                status: "delivered".to_string(),
                work_order_id,
                worker_session_id,
                via: Some(via),
                pane,
                tmux_target,
                error: None,
                message: Some(message),
            }
        }
        Err(error) => ResolutionDeliveryResult {
            status: "failed".to_string(),
            work_order_id,
            worker_session_id,
            via: None,
            pane,
            tmux_target,
            error: Some(error),
            message: Some(message),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolution_message_mentions_next_high_value_task() {
        let message = compose_resolution_message(
            &json!({
                "id": "WO-1",
                "feature_id": "F1.1",
                "stage": "build",
                "blockers": ["Need approval to continue"],
                "requested_permissions": ["browser_control"]
            }),
            "Permission granted by lead.",
        );

        assert!(message.contains("DXOS RESOLUTION [WO-1]"));
        assert!(message.contains("Approved permission: browser_control"));
        assert!(message.contains("next high-value task"));
    }
}

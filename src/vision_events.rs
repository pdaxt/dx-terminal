use std::path::Path;

use serde_json::Value;

use crate::app::App;
use crate::state::events::StateEvent;

pub fn emit_from_result(
    app: &App,
    project_path: &str,
    result: &str,
    fallback_feature_id: Option<&str>,
) {
    let Ok(value) = serde_json::from_str::<Value>(result) else {
        return;
    };

    if !should_emit(&value) {
        return;
    }

    let feature_id = fallback_feature_id
        .or_else(|| value.get("feature_id").and_then(|v| v.as_str()))
        .or_else(|| value.get("feature").and_then(|v| v.as_str()));

    let project = project_name(project_path);
    let summary = change_summary(project_path, &value, feature_id);

    if let Some(feature_id) = feature_id {
        let readiness = crate::vision::feature_readiness(project_path, feature_id);
        if let Ok(readiness_value) = serde_json::from_str::<Value>(&readiness) {
            if readiness_value.get("error").is_none() {
                let goal_id = readiness_value.get("goal_id").and_then(|v| v.as_str());
                crate::vision_focus::upsert_focus(
                    project_path,
                    Some(project.as_str()),
                    goal_id,
                    Some(feature_id),
                    Some("mutation"),
                );
                app.state.event_bus.send(StateEvent::VisionChanged {
                    project,
                    summary,
                    feature_id: Some(feature_id.to_string()),
                    feature_title: readiness_value
                        .get("title")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    phase: readiness_value
                        .get("phase")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    state: readiness_value
                        .get("state")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    readiness: readiness_value.get("readiness").cloned(),
                });
                return;
            }
        }

        crate::vision_focus::upsert_focus(
            project_path,
            Some(project.as_str()),
            None,
            Some(feature_id),
            Some("mutation"),
        );
    }

    app.state.event_bus.send(StateEvent::VisionChanged {
        project,
        summary,
        feature_id: feature_id.map(|s| s.to_string()),
        feature_title: None,
        phase: value
            .get("phase")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        state: value
            .get("state")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        readiness: None,
    });
}

fn should_emit(value: &Value) -> bool {
    if value.get("error").is_some() {
        return false;
    }

    !matches!(
        value.get("status").and_then(|v| v.as_str()),
        Some("noop") | Some("blocked")
    )
}

fn project_name(project_path: &str) -> String {
    let summary = crate::vision::vision_summary(project_path);
    if let Ok(value) = serde_json::from_str::<Value>(&summary) {
        if let Some(project) = value.get("project").and_then(|v| v.as_str()) {
            return project.to_string();
        }
    }

    Path::new(project_path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "--".to_string())
}

fn change_summary(project_path: &str, result: &Value, feature_id: Option<&str>) -> String {
    if let Some(summary) = result.get("summary").and_then(|v| v.as_str()) {
        return summary.to_string();
    }

    if let Some(feature_id) = feature_id {
        if let Some(phase) = result.get("phase").and_then(|v| v.as_str()) {
            return format!("{} -> {}", feature_id, phase);
        }
        if let Some(state) = result.get("state").and_then(|v| v.as_str()) {
            return format!("{} state -> {}", feature_id, state);
        }
        if let Some(status) = result.get("status").and_then(|v| v.as_str()) {
            return format!("{} {}", feature_id, status);
        }
    }

    let summary = crate::vision::vision_summary(project_path);
    if let Ok(value) = serde_json::from_str::<Value>(&summary) {
        if let Some(change) = value
            .get("recent_changes")
            .and_then(|v| v.as_array())
            .and_then(|changes| changes.first())
        {
            let field = change.get("field").and_then(|v| v.as_str()).unwrap_or("");
            let reason = change
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("Vision updated");
            return if field.is_empty() {
                reason.to_string()
            } else {
                format!("{}: {}", field, reason)
            };
        }
    }

    "Vision updated".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_noop_and_blocked_results() {
        assert!(!should_emit(&serde_json::json!({"status":"noop"})));
        assert!(!should_emit(&serde_json::json!({"status":"blocked"})));
        assert!(should_emit(&serde_json::json!({"status":"updated"})));
    }

    #[test]
    fn builds_feature_summary_from_direct_result() {
        let summary = change_summary(
            "/tmp/demo",
            &serde_json::json!({"status":"started","phase":"discovery"}),
            Some("F1.1"),
        );
        assert_eq!(summary, "F1.1 -> discovery");
    }
}

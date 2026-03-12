use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct VisionFocusEntry {
    pub project_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct VisionFocusStore {
    #[serde(default)]
    entries: Vec<VisionFocusEntry>,
}

fn focus_path() -> PathBuf {
    crate::config::dx_root().join("vision_focus.json")
}

fn now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn trimmed(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn project_name(project_path: &str) -> Option<String> {
    Path::new(project_path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|value| !value.is_empty())
}

fn load_store_from(path: &Path) -> VisionFocusStore {
    serde_json::from_value(crate::state::persistence::read_json(path)).unwrap_or_default()
}

fn save_store_to(path: &Path, store: &VisionFocusStore) -> Option<()> {
    let value = serde_json::to_value(store).ok()?;
    crate::state::persistence::write_json(path, &value).ok()
}

fn upsert_focus_at(
    path: &Path,
    project_path: &str,
    project: Option<&str>,
    goal_id: Option<&str>,
    feature_id: Option<&str>,
    source: Option<&str>,
) -> Option<VisionFocusEntry> {
    let normalized = normalize_project_path(project_path)?;
    let mut store = load_store_from(path);
    let entry = VisionFocusEntry {
        project_path: normalized.clone(),
        project: trimmed(project).or_else(|| project_name(&normalized)),
        goal_id: trimmed(goal_id),
        feature_id: trimmed(feature_id),
        source: trimmed(source),
        updated_at: Some(now()),
    };

    store.entries.retain(|existing| existing.project_path != normalized);
    store.entries.push(entry.clone());
    store
        .entries
        .sort_by(|left, right| left.project_path.cmp(&right.project_path));
    save_store_to(path, &store)?;
    Some(entry)
}

fn read_project_focus_at(path: &Path, project_path: &str) -> Option<VisionFocusEntry> {
    let normalized = normalize_project_path(project_path)?;
    load_store_from(path)
        .entries
        .into_iter()
        .find(|entry| entry.project_path == normalized)
}

pub fn normalize_project_path(project_path: &str) -> Option<String> {
    let raw = project_path.trim();
    if raw.is_empty() {
        return None;
    }

    let candidate = PathBuf::from(raw);
    let absolute = if candidate.is_absolute() {
        candidate
    } else if raw == "." {
        std::env::current_dir().ok()?
    } else {
        let cwd = std::env::current_dir().ok();
        let cwd_candidate = cwd
            .as_ref()
            .map(|cwd| cwd.join(&candidate))
            .unwrap_or_else(|| candidate.clone());
        if cwd_candidate.exists() {
            cwd_candidate
        } else {
            let projects_candidate = crate::config::projects_dir().join(raw);
            if projects_candidate.exists() {
                projects_candidate
            } else {
                cwd_candidate
            }
        }
    };

    Some(
        std::fs::canonicalize(&absolute)
            .unwrap_or(absolute)
            .to_string_lossy()
            .to_string(),
    )
}

pub fn upsert_focus(
    project_path: &str,
    project: Option<&str>,
    goal_id: Option<&str>,
    feature_id: Option<&str>,
    source: Option<&str>,
) -> Option<VisionFocusEntry> {
    upsert_focus_at(
        &focus_path(),
        project_path,
        project,
        goal_id,
        feature_id,
        source,
    )
}

pub fn upsert_feature_focus(
    project_path: &str,
    feature_id: &str,
    source: Option<&str>,
) -> Option<VisionFocusEntry> {
    let readiness = crate::vision::feature_readiness(project_path, feature_id);
    let value = serde_json::from_str::<Value>(&readiness).ok()?;
    if value.get("error").is_some() {
        return None;
    }

    upsert_focus(
        project_path,
        None,
        value.get("goal_id").and_then(|v| v.as_str()),
        value.get("feature_id")
            .and_then(|v| v.as_str())
            .or(Some(feature_id)),
        source,
    )
}

pub fn read_project_focus(project_path: &str) -> Option<VisionFocusEntry> {
    read_project_focus_at(&focus_path(), project_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_focus_replaces_existing_project_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("demo");
        std::fs::create_dir_all(&project).unwrap();
        let store_path = tmp.path().join("vision_focus.json");

        let first = upsert_focus_at(
            &store_path,
            project.to_str().unwrap(),
            Some("demo"),
            Some("G1"),
            Some("F1.1"),
            Some("dashboard"),
        )
        .unwrap();
        let second = upsert_focus_at(
            &store_path,
            project.to_str().unwrap(),
            Some("demo"),
            Some("G1"),
            Some("F1.2"),
            Some("mcp"),
        )
        .unwrap();

        let stored = read_project_focus_at(&store_path, project.to_str().unwrap()).unwrap();
        assert_eq!(stored.feature_id.as_deref(), Some("F1.2"));
        assert_eq!(stored.source.as_deref(), Some("mcp"));
        assert_eq!(first.project_path, second.project_path);
    }

    #[test]
    fn normalize_project_path_resolves_existing_relative_path() {
        let tmp = tempfile::tempdir().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        std::fs::create_dir_all("demo").unwrap();

        let normalized = normalize_project_path("demo").unwrap();

        std::env::set_current_dir(original).unwrap();
        assert_eq!(normalized, tmp.path().join("demo").to_string_lossy());
    }
}

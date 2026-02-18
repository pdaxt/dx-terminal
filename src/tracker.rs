use anyhow::Result;
use crate::config;

/// Load all issues from a tracker space
pub fn load_issues(space: &str) -> Vec<serde_json::Value> {
    let dir = config::collab_root().join("spaces").join(space).join("issues");
    if !dir.exists() {
        return Vec::new();
    }
    let mut issues = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.path().extension().map_or(false, |e| e == "json") {
                if let Ok(contents) = std::fs::read_to_string(entry.path()) {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&contents) {
                        issues.push(v);
                    }
                }
            }
        }
    }
    issues.sort_by(|a, b| {
        let a_id = a.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let b_id = b.get("id").and_then(|v| v.as_str()).unwrap_or("");
        a_id.cmp(b_id)
    });
    issues
}

/// Find a specific issue by ID
pub fn find_issue(space: &str, issue_id: &str) -> Option<serde_json::Value> {
    load_issues(space).into_iter().find(|issue| {
        issue.get("id").and_then(|v| v.as_str()) == Some(issue_id)
    })
}

/// Update an issue file with new fields
pub fn update_issue(space: &str, issue_id: &str, updates: &serde_json::Value) -> Result<bool> {
    let dir = config::collab_root().join("spaces").join(space).join("issues");
    if !dir.exists() {
        return Ok(false);
    }
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                if let Ok(contents) = std::fs::read_to_string(&path) {
                    if let Ok(mut data) = serde_json::from_str::<serde_json::Value>(&contents) {
                        if data.get("id").and_then(|v| v.as_str()) == Some(issue_id) {
                            if let (Some(obj), Some(upd)) = (data.as_object_mut(), updates.as_object()) {
                                for (k, v) in upd {
                                    obj.insert(k.clone(), v.clone());
                                }
                            }
                            let json = serde_json::to_string_pretty(&data)?;
                            std::fs::write(&path, json)?;
                            return Ok(true);
                        }
                    }
                }
            }
        }
    }
    Ok(false)
}

/// Load board summary (status counts) across all spaces
pub fn load_board_summary() -> std::collections::HashMap<String, usize> {
    let mut counts = std::collections::HashMap::new();
    let spaces_dir = config::collab_root().join("spaces");
    if !spaces_dir.exists() {
        return counts;
    }
    if let Ok(entries) = std::fs::read_dir(&spaces_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let space = entry.file_name().to_string_lossy().to_string();
                for issue in load_issues(&space) {
                    let status = issue.get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("backlog")
                        .to_string();
                    *counts.entry(status).or_insert(0) += 1;
                }
            }
        }
    }
    counts
}

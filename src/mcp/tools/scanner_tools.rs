//! Scanner tools: project scan, list, detail, test, deps.
//!
//! Thin wrappers over crate::scanner (and crate::engine::health for tests)
//! so all layers route through one place.

/// Scan ~/Projects for git repos
pub fn project_scan() -> String {
    let reg = crate::scanner::scan_all();
    let summary: Vec<serde_json::Value> = reg
        .projects
        .iter()
        .map(|p| {
            serde_json::json!({
                "name": p.name,
                "tech": p.tech,
                "test_cmd": p.test_cmd,
                "git_dirty": p.git_dirty,
            })
        })
        .collect();
    serde_json::json!({
        "count": reg.projects.len(),
        "projects": summary,
        "last_scan": reg.last_scan,
    })
    .to_string()
}

/// List discovered projects with health grades
pub fn project_list(tech: Option<&str>) -> String {
    let reg = crate::scanner::load_registry();
    let projects: Vec<serde_json::Value> = reg
        .projects
        .iter()
        .filter(|p| {
            if let Some(tech) = tech {
                p.tech.iter().any(|t| t.contains(tech))
            } else {
                true
            }
        })
        .map(|p| {
            let health = crate::quality::project_health(&p.name);
            serde_json::json!({
                "name": p.name,
                "path": p.path,
                "tech": p.tech,
                "test_cmd": p.test_cmd,
                "build_cmd": p.build_cmd,
                "has_ci": p.has_ci,
                "health_grade": health.get("grade").and_then(|v| v.as_str()).unwrap_or("?"),
                "health_score": health.get("health_score").and_then(|v| v.as_i64()).unwrap_or(0),
                "git_dirty": p.git_dirty,
                "git_ahead": p.git_ahead,
                "git_behind": p.git_behind,
                "last_commit": p.last_commit_msg,
                "loc": p.loc,
            })
        })
        .collect();
    serde_json::json!({
        "count": projects.len(),
        "projects": projects,
        "last_scan": reg.last_scan,
    })
    .to_string()
}

/// Full detail for one project
pub fn project_detail(project: &str) -> String {
    crate::scanner::project_detail(project).to_string()
}

/// Run tests for a project
pub async fn project_test(project: &str) -> String {
    let info = match crate::scanner::project_by_name(project) {
        Some(i) => i,
        None => {
            return serde_json::json!({"error": format!("Project '{}' not found", project)})
                .to_string()
        }
    };
    match crate::engine::health::run_tests(&info).await {
        Some(result) => serde_json::json!({
            "project": info.name,
            "success": result.success,
            "total": result.total,
            "passed": result.passed,
            "failed": result.failed,
            "duration_ms": result.duration_ms,
            "output": if result.output.len() > 2000 {
                format!("{}...(truncated)", &result.output[..2000])
            } else {
                result.output
            },
        })
        .to_string(),
        None => {
            serde_json::json!({"error": "No test command available for this project"}).to_string()
        }
    }
}

/// Show dependency graph
pub fn project_deps(project: Option<&str>) -> String {
    let reg = crate::scanner::load_registry();
    if let Some(name) = project {
        if let Some(p) = reg
            .projects
            .iter()
            .find(|p| p.name.to_lowercase() == name.to_lowercase())
        {
            let depended_on_by: Vec<&str> = reg
                .projects
                .iter()
                .filter(|other| other.deps.iter().any(|d| d == &p.name))
                .map(|other| other.name.as_str())
                .collect();
            serde_json::json!({
                "project": p.name,
                "depends_on": p.deps,
                "depended_on_by": depended_on_by,
            })
            .to_string()
        } else {
            serde_json::json!({"error": format!("Project '{}' not found", name)}).to_string()
        }
    } else {
        let graph: Vec<serde_json::Value> = reg
            .projects
            .iter()
            .filter(|p| !p.deps.is_empty())
            .map(|p| serde_json::json!({"project": p.name, "depends_on": p.deps}))
            .collect();
        serde_json::json!({"dependencies": graph}).to_string()
    }
}

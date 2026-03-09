//! Git isolation tools: git_sync, git_status, git_push, git_pr, git_merge.

use crate::app::App;
use crate::config;
use crate::workspace;
use super::super::types::*;
use super::helpers::{json_err, truncate, pane_id_str};

/// Execute os_git_sync — pull latest from base branch into agent's worktree
pub async fn git_sync(app: &App, req: GitSyncRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let pane_data = app.state.get_pane(pane_num).await;
    let (ws, branch) = match (&pane_data.workspace_path, &pane_data.branch_name) {
        (Some(ws), Some(br)) => (ws.clone(), br.clone()),
        _ => return json_err(&format!("Pane {} has no git workspace", pane_num)),
    };

    let base = pane_data.base_branch.clone().unwrap_or_else(|| {
        std::process::Command::new("git")
            .args(["symbolic-ref", "refs/remotes/origin/HEAD", "--short"])
            .current_dir(&ws)
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    Some(s.strip_prefix("origin/").unwrap_or(&s).to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "main".into())
    });

    let result = workspace::sync_from_main(&ws, &base);

    serde_json::json!({
        "pane": pane_num,
        "branch": branch,
        "base_branch": base,
        "result": result.unwrap_or_else(|e| e.to_string()),
    }).to_string()
}

/// Execute os_git_status — show git status/diff for agent's worktree
pub async fn git_status_tool(app: &App, req: GitStatusRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let pane_data = app.state.get_pane(pane_num).await;
    let ws = match &pane_data.workspace_path {
        Some(ws) => ws.clone(),
        None => return json_err(&format!("Pane {} has no git workspace", pane_num)),
    };

    let status = workspace::git_status(&ws).unwrap_or_default();
    let diff = if req.verbose.unwrap_or(false) {
        std::process::Command::new("git")
            .args(["diff"])
            .current_dir(&ws)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default()
    } else {
        workspace::git_diff(&ws).unwrap_or_default()
    };

    serde_json::json!({
        "pane": pane_num,
        "branch": pane_data.branch_name,
        "status": status,
        "diff": truncate(&diff, 5000),
    }).to_string()
}

/// Execute os_git_push — commit and push agent's current work
pub async fn git_push(app: &App, req: GitPushRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let pane_data = app.state.get_pane(pane_num).await;
    let (ws, branch) = match (&pane_data.workspace_path, &pane_data.branch_name) {
        (Some(ws), Some(br)) => (ws.clone(), br.clone()),
        _ => return json_err(&format!("Pane {} has no git workspace", pane_num)),
    };

    let msg = req.message.unwrap_or_else(|| {
        format!("Pane {}: {}", pane_num, truncate(&pane_data.task, 60))
    });

    let commit_result = workspace::commit_all(&ws, &msg);
    let push_result = workspace::push_branch(&ws, &branch);

    serde_json::json!({
        "pane": pane_num,
        "branch": branch,
        "commit": commit_result.unwrap_or_else(|e| e.to_string()),
        "push": push_result.unwrap_or_else(|e| e.to_string()),
    }).to_string()
}

/// Execute os_git_pr — create a PR from agent's branch
pub async fn git_pr(app: &App, req: GitPrRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let pane_data = app.state.get_pane(pane_num).await;
    let (ws, branch) = match (&pane_data.workspace_path, &pane_data.branch_name) {
        (Some(ws), Some(br)) => (ws.clone(), br.clone()),
        _ => return json_err(&format!("Pane {} has no git workspace", pane_num)),
    };

    let _ = workspace::commit_all(&ws, &format!("Pane {}: pre-PR commit", pane_num));
    let push_result = workspace::push_branch(&ws, &branch);

    let title = req.title.unwrap_or_else(|| {
        format!("[Pane {}] {}", pane_num, truncate(&pane_data.task, 50))
    });
    let body = req.body.unwrap_or_else(|| {
        format!("## Task\n{}\n\nAutomated PR from DX Terminal pane {}", pane_data.task, pane_num)
    });
    let pr_result = workspace::create_pr(&ws, &title, &body);

    serde_json::json!({
        "pane": pane_num,
        "branch": branch,
        "push": push_result.unwrap_or_else(|e| e.to_string()),
        "pr": pr_result.unwrap_or_else(|e| e.to_string()),
    }).to_string()
}

/// Merge an agent's branch back into the base branch
pub async fn git_merge(app: &App, req: GitMergeRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let pane_data = app.state.get_pane(pane_num).await;
    let project_path = &pane_data.project_path;
    if project_path.is_empty() {
        return json_err(&format!("Pane {} has no project", pane_num));
    }

    let branch = req.branch.or(pane_data.branch_name.clone())
        .unwrap_or_default();
    if branch.is_empty() {
        return json_err(&format!("Pane {} has no branch to merge", pane_num));
    }

    let base = pane_data.base_branch.clone().unwrap_or_else(|| "main".into());
    let project_name = &pane_data.project;

    match workspace::merge_branch(project_path, &branch, &base) {
        Ok(result) => {
            let _ = crate::multi_agent::git_release_branch(&pane_id_str(pane_num), &branch, project_name);

            serde_json::json!({
                "status": "merged",
                "pane": pane_num,
                "branch": branch,
                "base": base,
                "result": result,
            }).to_string()
        }
        Err(e) => serde_json::json!({
            "status": "failed",
            "pane": pane_num,
            "branch": branch,
            "base": base,
            "error": e.to_string(),
        }).to_string(),
    }
}

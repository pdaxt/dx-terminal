//! Multi-agent coordination tools: ports, agents, locks, git branches, builds, tasks, KB, messaging, lifecycle.
//!
//! Thin wrappers over crate::multi_agent so all layers route through one place.

use super::super::types::*;

// ── Ports ──

/// Allocate a port for a service
pub fn port_allocate(req: &PortAllocateRequest) -> String {
    crate::multi_agent::port_allocate(
        &req.service,
        &req.pane_id,
        req.preferred,
        &req.description.clone().unwrap_or_default(),
    )
    .to_string()
}

/// Release an allocated port
pub fn port_release(port: u16) -> String {
    crate::multi_agent::port_release(port).to_string()
}

/// List all allocated ports
pub fn port_list() -> String {
    crate::multi_agent::port_list().to_string()
}

/// Get port for a service by name
pub fn port_get(service: &str) -> String {
    crate::multi_agent::port_get(service).to_string()
}

// ── Agents ──

/// Register an agent in a pane
pub fn agent_register(req: &AgentRegisterRequest) -> String {
    let files = req.files.clone().unwrap_or_default();
    crate::multi_agent::agent_register(&req.pane_id, &req.project, &req.task, &files).to_string()
}

/// Update an agent's task and files
pub fn agent_update(req: &AgentUpdateRequest) -> String {
    crate::multi_agent::agent_update(&req.pane_id, &req.task, req.files.as_deref()).to_string()
}

/// List registered agents
pub fn agent_list(project: Option<&str>) -> String {
    crate::multi_agent::agent_list(project).to_string()
}

/// Deregister an agent
pub fn agent_deregister(pane_id: &str) -> String {
    crate::multi_agent::agent_deregister(pane_id).to_string()
}

// ── Locks ──

/// Acquire file locks
pub fn lock_acquire(req: &LockAcquireRequest) -> String {
    crate::multi_agent::lock_acquire(
        &req.pane_id,
        &req.files,
        &req.reason.clone().unwrap_or_default(),
    )
    .to_string()
}

/// Release file locks
pub fn lock_release(req: &LockReleaseRequest) -> String {
    let files = req.files.clone().unwrap_or_default();
    crate::multi_agent::lock_release(&req.pane_id, &files).to_string()
}

/// Check lock status
pub fn lock_check(files: &[String]) -> String {
    crate::multi_agent::lock_check(files).to_string()
}

/// Force-steal a file lock
pub fn lock_steal(pane_id: &str, file_path: &str, reason: &str) -> String {
    crate::multi_agent::lock_steal(pane_id, file_path, reason).to_string()
}

// ── Git Branches ──

/// Claim a git branch for exclusive use
pub fn git_claim_branch(req: &GitClaimBranchRequest) -> String {
    crate::multi_agent::git_claim_branch(
        &req.pane_id,
        &req.branch,
        &req.repo,
        &req.purpose.clone().unwrap_or_default(),
    )
    .to_string()
}

/// Release a claimed git branch
pub fn git_release_branch(pane_id: &str, branch: &str, repo: &str) -> String {
    crate::multi_agent::git_release_branch(pane_id, branch, repo).to_string()
}

/// List claimed branches
pub fn git_list_branches(repo: Option<&str>) -> String {
    crate::multi_agent::git_list_branches(repo).to_string()
}

/// Pre-commit conflict check
pub fn git_pre_commit_check(pane_id: &str, repo: &str, files: &[String]) -> String {
    crate::multi_agent::git_pre_commit_check(pane_id, repo, files).to_string()
}

// ── Builds ──

/// Claim exclusive build access
pub fn build_claim(req: &BuildClaimRequest) -> String {
    crate::multi_agent::build_claim(
        &req.pane_id,
        &req.project,
        &req.build_type.clone().unwrap_or_else(|| "default".into()),
    )
    .to_string()
}

/// Release build claim
pub fn build_release(req: &BuildReleaseRequest) -> String {
    crate::multi_agent::build_release(
        &req.pane_id,
        &req.project,
        req.success,
        &req.output.clone().unwrap_or_default(),
    )
    .to_string()
}

/// Check build status
pub fn build_status(project: &str) -> String {
    crate::multi_agent::build_status(project).to_string()
}

/// Get last build result
pub fn build_get_last(project: &str) -> String {
    crate::multi_agent::build_get_last(project).to_string()
}

// ── Inter-Agent Tasks ──

/// Add a shared inter-agent task
pub fn task_add(req: &MaTaskAddRequest) -> String {
    crate::multi_agent::task_add(
        &req.project,
        &req.title,
        &req.description.clone().unwrap_or_default(),
        &req.priority.clone().unwrap_or_else(|| "medium".into()),
        &req.added_by,
    )
    .to_string()
}

/// Claim next pending task
pub fn task_claim(pane_id: &str, project: Option<&str>) -> String {
    crate::multi_agent::task_claim(pane_id, project).to_string()
}

/// Complete a task
pub fn task_complete(req: &MaTaskCompleteRequest) -> String {
    crate::multi_agent::task_complete(
        &req.task_id,
        &req.pane_id,
        &req.result.clone().unwrap_or_default(),
    )
    .to_string()
}

/// List inter-agent tasks
pub fn task_list(status: Option<&str>, project: Option<&str>) -> String {
    crate::multi_agent::task_list(status, project).to_string()
}

// ── Knowledge Base ──

/// Add a KB entry
pub fn kb_add(req: &KbAddRequest) -> String {
    let files = req.files.clone().unwrap_or_default();
    crate::multi_agent::kb_add(
        &req.pane_id,
        &req.project,
        &req.category,
        &req.title,
        &req.content,
        &files,
    )
    .to_string()
}

/// Search KB
pub fn kb_search(query: &str, project: Option<&str>, category: Option<&str>) -> String {
    crate::multi_agent::kb_search(query, project, category).to_string()
}

/// List KB entries
pub fn kb_list(project: Option<&str>, limit: usize) -> String {
    crate::multi_agent::kb_list(project, limit).to_string()
}

// ── Messaging ──

/// Broadcast a message to all agents
pub fn msg_broadcast(req: &MsgBroadcastRequest) -> String {
    crate::multi_agent::msg_broadcast(
        &req.from_pane,
        &req.message,
        &req.priority.clone().unwrap_or_else(|| "info".into()),
    )
    .to_string()
}

/// Send a direct message
pub fn msg_send(from: &str, to: &str, message: &str) -> String {
    crate::multi_agent::msg_send(from, to, message).to_string()
}

/// Get unread messages
pub fn msg_get(pane_id: &str, mark_read: bool) -> String {
    crate::multi_agent::msg_get(pane_id, mark_read).to_string()
}

// ── Lifecycle ──

/// Clean up stale entries
pub fn cleanup_all() -> String {
    crate::multi_agent::cleanup_all().to_string()
}

/// Full status overview
pub fn status_overview(project: Option<&str>) -> String {
    crate::multi_agent::status_overview(project).to_string()
}

/// Send heartbeat
pub fn heartbeat(pane_id: &str, task: Option<&str>, status: Option<&str>) -> String {
    crate::multi_agent::heartbeat(pane_id, task, status).to_string()
}

/// Start tracking session
pub fn session_start(pane_id: &str, project: &str) -> String {
    crate::multi_agent::session_start(pane_id, project).to_string()
}

/// End tracking session
pub fn session_end(session_id: &str, summary: &str) -> String {
    crate::multi_agent::session_end(session_id, summary).to_string()
}

/// List active agents (simple)
pub fn who() -> String {
    crate::multi_agent::who().to_string()
}

/// Detect concurrent file conflicts
pub fn conflict_scan(project: Option<&str>) -> String {
    crate::multi_agent::conflict_scan(project).to_string()
}

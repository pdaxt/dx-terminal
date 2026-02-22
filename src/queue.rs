use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use anyhow::Result;

use crate::config;

/// A task in the queue — everything needed to auto-spawn an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueTask {
    pub id: String,
    pub project: String,
    pub role: String,
    pub task: String,
    pub prompt: String,
    pub priority: u8,           // 1=highest, 5=lowest
    #[serde(default)]
    pub status: QueueStatus,
    #[serde(default)]
    pub pane: Option<u8>,       // assigned pane (when running)
    #[serde(default)]
    pub added_at: String,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<String>, // task IDs that must complete first
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum QueueStatus {
    Pending,
    Running,
    Done,
    Failed,
    Blocked,
}

impl Default for QueueStatus {
    fn default() -> Self { Self::Pending }
}

/// The full queue file
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskQueue {
    pub tasks: Vec<QueueTask>,
}

/// Orchestrator auto-cycle config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoConfig {
    /// Max panes to use simultaneously (1-9)
    pub max_parallel: u8,
    /// Panes reserved (never auto-assigned)
    pub reserved_panes: Vec<u8>,
    /// Auto-complete when agent is done (vs wait for manual review)
    pub auto_complete: bool,
    /// Auto-assign next task when a pane becomes free
    pub auto_assign: bool,
    /// Default role if not specified in task
    pub default_role: String,
    /// Auto-cycle interval in seconds (0 = disabled)
    #[serde(default = "default_cycle_secs")]
    pub cycle_interval_secs: u64,
}

fn default_cycle_secs() -> u64 { 30 }

impl Default for AutoConfig {
    fn default() -> Self {
        Self {
            max_parallel: 6,
            reserved_panes: vec![],
            auto_complete: true,
            auto_assign: true,
            default_role: "developer".into(),
            cycle_interval_secs: 30,
        }
    }
}

fn queue_path() -> PathBuf {
    config::agentos_root().join("queue.json")
}

fn auto_config_path() -> PathBuf {
    config::agentos_root().join("auto_config.json")
}

/// Load the task queue
pub fn load_queue() -> TaskQueue {
    let path = queue_path();
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(q) = serde_json::from_str(&content) {
                return q;
            }
        }
    }
    TaskQueue::default()
}

/// Save the task queue
pub fn save_queue(queue: &TaskQueue) -> Result<()> {
    let path = queue_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(queue)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Load auto-cycle config
pub fn load_auto_config() -> AutoConfig {
    let path = auto_config_path();
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(c) = serde_json::from_str(&content) {
                return c;
            }
        }
    }
    let default = AutoConfig::default();
    let _ = save_auto_config(&default);
    default
}

/// Save auto-cycle config
pub fn save_auto_config(cfg: &AutoConfig) -> Result<()> {
    let path = auto_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(cfg)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Generate a short ID
fn gen_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    format!("t{}", ts % 1_000_000)
}

/// Add a task to the queue
pub fn add_task(project: &str, role: &str, task: &str, prompt: &str, priority: u8, depends_on: Vec<String>) -> Result<QueueTask> {
    let mut queue = load_queue();

    let new_task = QueueTask {
        id: gen_id(),
        project: project.into(),
        role: role.into(),
        task: task.into(),
        prompt: prompt.into(),
        priority: priority.clamp(1, 5),
        status: QueueStatus::Pending,
        pane: None,
        added_at: crate::state::now(),
        started_at: None,
        completed_at: None,
        result: None,
        depends_on,
    };

    queue.tasks.push(new_task.clone());
    save_queue(&queue)?;
    Ok(new_task)
}

/// Get the next task to execute (highest priority pending task with no unresolved deps)
pub fn next_task() -> Option<QueueTask> {
    let queue = load_queue();
    let done_ids: Vec<&str> = queue.tasks.iter()
        .filter(|t| t.status == QueueStatus::Done)
        .map(|t| t.id.as_str())
        .collect();

    let mut pending: Vec<&QueueTask> = queue.tasks.iter()
        .filter(|t| t.status == QueueStatus::Pending)
        .filter(|t| t.depends_on.iter().all(|dep| done_ids.contains(&dep.as_str())))
        .collect();

    pending.sort_by_key(|t| t.priority);
    pending.first().cloned().cloned()
}

/// Mark a task as running on a specific pane
pub fn mark_running(task_id: &str, pane: u8) -> Result<()> {
    let mut queue = load_queue();
    if let Some(task) = queue.tasks.iter_mut().find(|t| t.id == task_id) {
        task.status = QueueStatus::Running;
        task.pane = Some(pane);
        task.started_at = Some(crate::state::now());
    }
    save_queue(&queue)
}

/// Mark a task as done
pub fn mark_done(task_id: &str, result: &str) -> Result<()> {
    let mut queue = load_queue();
    if let Some(task) = queue.tasks.iter_mut().find(|t| t.id == task_id) {
        task.status = QueueStatus::Done;
        task.completed_at = Some(crate::state::now());
        task.result = Some(result.into());
        task.pane = None;
    }
    // Unblock tasks that depend on this one
    let done_ids: Vec<String> = queue.tasks.iter()
        .filter(|t| t.status == QueueStatus::Done)
        .map(|t| t.id.clone())
        .collect();
    for task in &mut queue.tasks {
        if task.status == QueueStatus::Blocked {
            if task.depends_on.iter().all(|dep| done_ids.contains(dep)) {
                task.status = QueueStatus::Pending;
            }
        }
    }
    save_queue(&queue)
}

/// Mark a task as failed
pub fn mark_failed(task_id: &str, reason: &str) -> Result<()> {
    let mut queue = load_queue();
    if let Some(task) = queue.tasks.iter_mut().find(|t| t.id == task_id) {
        task.status = QueueStatus::Failed;
        task.completed_at = Some(crate::state::now());
        task.result = Some(reason.into());
        task.pane = None;
    }
    save_queue(&queue)
}

/// Remove completed/failed tasks older than N entries, keep last N
pub fn prune_queue(keep: usize) -> Result<usize> {
    let mut queue = load_queue();
    let total = queue.tasks.len();
    let mut finished: Vec<usize> = queue.tasks.iter().enumerate()
        .filter(|(_, t)| t.status == QueueStatus::Done || t.status == QueueStatus::Failed)
        .map(|(i, _)| i)
        .collect();

    if finished.len() <= keep {
        return Ok(0);
    }

    // Remove oldest finished, keeping `keep` most recent
    finished.sort();
    let to_remove = finished.len() - keep;
    let remove_indices: Vec<usize> = finished[..to_remove].to_vec();

    // Remove in reverse to preserve indices
    for &idx in remove_indices.iter().rev() {
        queue.tasks.remove(idx);
    }

    save_queue(&queue)?;
    Ok(total - queue.tasks.len())
}

/// Get running task for a pane
pub fn task_for_pane(pane: u8) -> Option<QueueTask> {
    let queue = load_queue();
    queue.tasks.into_iter().find(|t| t.pane == Some(pane) && t.status == QueueStatus::Running)
}

/// Find available pane (not running, not reserved)
pub fn find_free_pane(cfg: &AutoConfig, occupied: &[u8]) -> Option<u8> {
    let max = cfg.max_parallel.min(9);
    for p in 1..=max {
        if !cfg.reserved_panes.contains(&p) && !occupied.contains(&p) {
            return Some(p);
        }
    }
    None
}

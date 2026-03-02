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
    #[serde(default)]
    pub retry_count: u32,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub last_error: Option<String>,
    /// Tracker issue ID this task implements (e.g. "DX-5")
    #[serde(default)]
    pub issue_id: Option<String>,
    /// Tracker space for the linked issue
    #[serde(default)]
    pub space: Option<String>,
    /// Pipeline ID this task belongs to (None = standalone task)
    #[serde(default)]
    pub pipeline_id: Option<String>,
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

fn default_max_retries() -> u32 { 2 }
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

/// Generate a unique task ID (timestamp + random suffix to avoid collisions)
fn gen_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("t{}_{:04x}", ts % 10_000_000, seq % 0xFFFF)
}

/// Add a task to the queue
pub fn add_task(project: &str, role: &str, task: &str, prompt: &str, priority: u8, depends_on: Vec<String>) -> Result<QueueTask> {
    add_task_with_pipeline(project, role, task, prompt, priority, depends_on, None)
}

/// Add a task with an optional pipeline_id set atomically on creation
pub fn add_task_with_pipeline(project: &str, role: &str, task: &str, prompt: &str, priority: u8, depends_on: Vec<String>, pipeline_id: Option<String>) -> Result<QueueTask> {
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
        retry_count: 0,
        max_retries: 2,
        last_error: None,
        issue_id: None,
        space: None,
        pipeline_id,
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

/// Mark a task as failed and cascade failure to dependents
pub fn mark_failed(task_id: &str, reason: &str) -> Result<()> {
    let mut queue = load_queue();
    if let Some(task) = queue.tasks.iter_mut().find(|t| t.id == task_id) {
        task.status = QueueStatus::Failed;
        task.completed_at = Some(crate::state::now());
        task.result = Some(reason.into());
        task.last_error = Some(reason.into());
        task.pane = None;
    }

    // Cascade: fail tasks that depend on this failed task (multi-pass for transitive deps)
    loop {
        let failed_ids: Vec<String> = queue.tasks.iter()
            .filter(|t| t.status == QueueStatus::Failed)
            .map(|t| t.id.clone())
            .collect();
        let mut changed = false;
        for task in &mut queue.tasks {
            if task.status == QueueStatus::Pending || task.status == QueueStatus::Blocked {
                if task.depends_on.iter().any(|dep| failed_ids.contains(dep)) {
                    task.status = QueueStatus::Failed;
                    task.completed_at = Some(crate::state::now());
                    task.result = Some(format!("cascade: dependency {} failed", task_id));
                    task.last_error = Some(format!("cascade: dependency {} failed", task_id));
                    changed = true;
                }
            }
        }
        if !changed { break; }
    }

    save_queue(&queue)
}

/// Requeue a failed task for retry (increments retry_count, resets to Pending)
pub fn requeue_failed(task_id: &str) -> Result<bool> {
    let mut queue = load_queue();
    let mut requeued = false;

    if let Some(task) = queue.tasks.iter_mut().find(|t| t.id == task_id) {
        if task.status != QueueStatus::Failed || task.retry_count >= task.max_retries {
            return Ok(false);
        }
        task.retry_count += 1;
        task.status = QueueStatus::Pending;
        task.pane = None;
        task.started_at = None;
        task.completed_at = None;
        task.last_error = task.result.take();
        requeued = true;
    }

    if requeued {
        // Unblock cascade-failed dependents — set to Pending so they re-enter the queue
        let tid = task_id.to_string();
        for task in &mut queue.tasks {
            if task.status == QueueStatus::Failed {
                if let Some(err) = &task.last_error {
                    if err.contains(&format!("cascade: dependency {} failed", tid)) {
                        task.status = QueueStatus::Pending;
                        task.completed_at = None;
                        task.result = None;
                        task.last_error = None;
                    }
                }
            }
        }
        save_queue(&queue)?;
    }

    Ok(requeued)
}

/// Get running task for a pane
pub fn task_for_pane(pane: u8) -> Option<QueueTask> {
    let queue = load_queue();
    queue.tasks.into_iter().find(|t| t.pane == Some(pane) && t.status == QueueStatus::Running)
}

/// Look up a task by ID
pub fn task_by_id(task_id: &str) -> Option<QueueTask> {
    let queue = load_queue();
    queue.tasks.into_iter().find(|t| t.id == task_id)
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

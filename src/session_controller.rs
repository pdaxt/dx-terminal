use anyhow::{anyhow, bail, Context, Result};
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};
use tokio::sync::{watch, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

static NUMBERED_LIST_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d+\.\s+").expect("valid regex"));

use crate::agents::AgentType;

const CONTROL_LOOP_INTERVAL: Duration = Duration::from_secs(30);
const IDLE_NUDGE_INTERVAL: Duration = Duration::from_secs(120);
const LOW_CONTEXT_THRESHOLD_PCT: u8 = 25;

#[derive(Debug, Clone)]
pub struct PaneWatcher {
    pub pane_id: String,
    pub agent_type: AgentType,
    pub mission: String,
    pub state: WatcherState,
    pub context_pct: Option<u8>,
    pub approvals_given: u32,
    pub corrections_sent: u32,
    pub errors_unblocked: u32,
    pub started_at: Instant,
}

#[derive(Debug, Clone)]
pub enum WatcherState {
    Starting,
    Active(AgentState),
    Stopped { reason: String },
}

#[derive(Debug, Clone)]
pub enum AgentState {
    Working {
        description: String,
        duration_secs: u64,
    },
    Idle {
        prompt_text: String,
    },
    PermissionPrompt {
        command: String,
        options: Vec<String>,
    },
    Error {
        message: String,
    },
    Done {
        summary: String,
    },
    ContextLow {
        pct: u8,
    },
    Exited,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalAction {
    Persist,
    ApproveOnce,
    Deny(String),
}

#[derive(Debug)]
struct WatchHandle {
    stop_tx: watch::Sender<bool>,
    handle: JoinHandle<()>,
}

#[derive(Debug, Default)]
struct LoopHints {
    last_idle_nudge_at: Option<Instant>,
    last_error: Option<String>,
    low_context_warned: bool,
}

pub struct SessionController {
    watchers: RwLock<HashMap<String, Arc<RwLock<PaneWatcher>>>>,
    handles: Mutex<HashMap<String, WatchHandle>>,
}

impl SessionController {
    pub fn new() -> Self {
        Self {
            watchers: RwLock::new(HashMap::new()),
            handles: Mutex::new(HashMap::new()),
        }
    }

    pub async fn start(
        self: &Arc<Self>,
        pane_id: String,
        mission: String,
        agent_type: AgentType,
    ) -> Result<PaneWatcher> {
        let pane_id = pane_id.trim().to_string();
        if pane_id.is_empty() {
            bail!("pane is required");
        }
        if mission.trim().is_empty() {
            bail!("mission is required");
        }
        if !pane_exists(&pane_id) {
            bail!("tmux pane '{}' does not exist", pane_id);
        }

        let _ = self.stop(&pane_id).await;

        let watcher = PaneWatcher {
            pane_id: pane_id.clone(),
            agent_type,
            mission,
            state: WatcherState::Starting,
            context_pct: None,
            approvals_given: 0,
            corrections_sent: 0,
            errors_unblocked: 0,
            started_at: Instant::now(),
        };

        let snapshot = Arc::new(RwLock::new(watcher.clone()));
        self.watchers
            .write()
            .await
            .insert(pane_id.clone(), Arc::clone(&snapshot));

        let (stop_tx, stop_rx) = watch::channel(false);
        let controller = Arc::clone(self);
        let pane_for_task = pane_id.clone();
        let handle = tokio::spawn(async move {
            controller
                .run_managed_control_loop(Arc::clone(&snapshot), stop_rx)
                .await;
            debug!(pane = %pane_for_task, "session control loop stopped");
        });

        self.handles
            .lock()
            .await
            .insert(pane_id.clone(), WatchHandle { stop_tx, handle });

        info!(
            pane = %pane_id,
            agent = %watcher.agent_type.short_name(),
            "started session controller"
        );

        Ok(watcher)
    }

    pub async fn stop(&self, pane_id: &str) -> Result<Option<PaneWatcher>> {
        if let Some(handle) = self.handles.lock().await.remove(pane_id) {
            let _ = handle.stop_tx.send(true);
            handle.handle.abort();
        }

        let snapshot = self.watchers.write().await.remove(pane_id);
        let watcher = match snapshot {
            Some(snapshot) => Some(snapshot.read().await.clone()),
            None => None,
        };

        if watcher.is_some() {
            info!(pane = %pane_id, "stopped session controller");
        }

        Ok(watcher)
    }

    pub async fn status(&self, pane_id: &str) -> Option<PaneWatcher> {
        let snapshot = {
            let watchers = self.watchers.read().await;
            watchers.get(pane_id).cloned()
        }?;

        let watcher = snapshot.read().await.clone();
        Some(watcher)
    }

    pub async fn list(&self) -> Vec<PaneWatcher> {
        let snapshots: Vec<_> = {
            let watchers = self.watchers.read().await;
            watchers.values().cloned().collect()
        };

        let mut result = Vec::with_capacity(snapshots.len());
        for snapshot in snapshots {
            result.push(snapshot.read().await.clone());
        }
        result.sort_by(|a, b| a.pane_id.cmp(&b.pane_id));
        result
    }

    pub async fn send_instruction(&self, pane_id: &str, instruction: &str) -> Result<()> {
        if instruction.trim().is_empty() {
            bail!("instruction is required");
        }

        let snapshot = {
            let watchers = self.watchers.read().await;
            watchers.get(pane_id).cloned()
        }
        .ok_or_else(|| anyhow!("pane '{}' is not supervised", pane_id))?;

        send_instruction(pane_id, instruction)?;

        let mut watcher = snapshot.write().await;
        watcher.corrections_sent = watcher.corrections_sent.saturating_add(1);

        Ok(())
    }

    async fn run_managed_control_loop(
        &self,
        snapshot: Arc<RwLock<PaneWatcher>>,
        mut stop_rx: watch::Receiver<bool>,
    ) {
        let mut hints = LoopHints::default();

        loop {
            if *stop_rx.borrow() {
                let mut watcher = snapshot.write().await;
                watcher.state = WatcherState::Stopped {
                    reason: "supervision stopped".to_string(),
                };
                break;
            }

            let should_continue = {
                let mut watcher = snapshot.write().await;
                control_loop_step(&mut watcher, &mut hints)
            };

            if !should_continue {
                break;
            }

            tokio::select! {
                changed = stop_rx.changed() => {
                    if changed.is_ok() && *stop_rx.borrow() {
                        let mut watcher = snapshot.write().await;
                        watcher.state = WatcherState::Stopped {
                            reason: "supervision stopped".to_string(),
                        };
                        break;
                    }
                }
                _ = tokio::time::sleep(CONTROL_LOOP_INTERVAL) => {}
            }
        }
    }
}

impl Default for SessionController {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn run_control_loop(watcher: &mut PaneWatcher) {
    let mut hints = LoopHints::default();
    loop {
        if !control_loop_step(watcher, &mut hints) {
            break;
        }
        tokio::time::sleep(CONTROL_LOOP_INTERVAL).await;
    }
}

fn control_loop_step(watcher: &mut PaneWatcher, hints: &mut LoopHints) -> bool {
    if !pane_exists(&watcher.pane_id) {
        watcher.state = WatcherState::Active(AgentState::Exited);
        return false;
    }

    let output = capture_pane(&watcher.pane_id, 30);
    let project_dir = pane_current_path(&watcher.pane_id).unwrap_or_else(default_project_dir);
    watcher.context_pct = detect_context_pct(&output);

    let mut state = detect_state(&output, &watcher.agent_type);
    if let AgentState::Working { description, .. } = state {
        state = AgentState::Working {
            description,
            duration_secs: watcher.started_at.elapsed().as_secs(),
        };
    }

    if watcher.context_pct.unwrap_or(100) >= LOW_CONTEXT_THRESHOLD_PCT {
        hints.low_context_warned = false;
    }

    match &state {
        AgentState::Working { .. } => {
            debug!(pane = %watcher.pane_id, "supervised pane is working");
        }
        AgentState::PermissionPrompt { command, options } => {
            let action = decide_approval_for_project(command, &project_dir);
            match send_approval(&watcher.pane_id, &action, options) {
                Ok(()) => match action {
                    ApprovalAction::Persist | ApprovalAction::ApproveOnce => {
                        watcher.approvals_given = watcher.approvals_given.saturating_add(1);
                    }
                    ApprovalAction::Deny(ref reason) => {
                        watcher.corrections_sent = watcher.corrections_sent.saturating_add(1);
                        warn!(
                            pane = %watcher.pane_id,
                            command,
                            reason,
                            "denied supervised approval"
                        );
                    }
                },
                Err(err) => warn!(
                    pane = %watcher.pane_id,
                    error = %err,
                    "failed to answer permission prompt"
                ),
            }
        }
        AgentState::Idle { .. } => {
            let should_nudge = hints
                .last_idle_nudge_at
                .map(|at| at.elapsed() >= IDLE_NUDGE_INTERVAL)
                .unwrap_or(true);
            if should_nudge {
                let instruction = idle_instruction(&watcher.mission, &project_dir);
                if send_instruction(&watcher.pane_id, &instruction).is_ok() {
                    watcher.corrections_sent = watcher.corrections_sent.saturating_add(1);
                    hints.last_idle_nudge_at = Some(Instant::now());
                }
            }
        }
        AgentState::Error { message } => {
            let is_new_error = hints.last_error.as_deref() != Some(message.as_str());
            if is_new_error {
                let instruction = error_instruction(message, &watcher.mission);
                if send_instruction(&watcher.pane_id, &instruction).is_ok() {
                    watcher.corrections_sent = watcher.corrections_sent.saturating_add(1);
                    watcher.errors_unblocked = watcher.errors_unblocked.saturating_add(1);
                    hints.last_error = Some(message.clone());
                }
            }
        }
        AgentState::Done { summary } => {
            info!(
                pane = %watcher.pane_id,
                summary,
                "supervised pane reported done"
            );
        }
        AgentState::ContextLow { pct } => {
            if *pct < LOW_CONTEXT_THRESHOLD_PCT
                && !hints.low_context_warned
                && send_instruction(
                    &watcher.pane_id,
                    "Context is low. Commit and wrap up with a concise handoff.",
                )
                .is_ok()
            {
                watcher.corrections_sent = watcher.corrections_sent.saturating_add(1);
                hints.low_context_warned = true;
            }
        }
        AgentState::Exited => {
            watcher.state = WatcherState::Active(AgentState::Exited);
            return false;
        }
    }

    watcher.state = WatcherState::Active(state);
    true
}

pub fn detect_state(output: &str, agent_type: &AgentType) -> AgentState {
    let recent = tail_lines(output, 30);

    if let Some(message) = detect_error_message(&recent) {
        return AgentState::Error { message };
    }

    if let Some((command, options)) = detect_permission_prompt(&recent, agent_type) {
        return AgentState::PermissionPrompt { command, options };
    }

    if crate::pty::output::check_shell_prompt(&recent) {
        return AgentState::Exited;
    }

    if let Some(summary) = detect_done_summary(&recent) {
        return AgentState::Done { summary };
    }

    if let Some(pct) = detect_context_pct(&recent) {
        if pct < LOW_CONTEXT_THRESHOLD_PCT {
            return AgentState::ContextLow { pct };
        }
    }

    if detect_working(&recent, agent_type) {
        return AgentState::Working {
            description: working_description(&recent),
            duration_secs: 0,
        };
    }

    if let Some(prompt_text) = detect_idle_prompt(&recent, agent_type) {
        return AgentState::Idle { prompt_text };
    }

    AgentState::Idle {
        prompt_text: last_non_empty_line(&recent).unwrap_or_else(|| "idle".to_string()),
    }
}

pub fn detect_context_pct(output: &str) -> Option<u8> {
    let patterns = [
        Regex::new(r"(?i)\b(\d{1,3})%\s*left\b").expect("valid regex"),
        Regex::new(r"(?i)\b(\d{1,3})%\s*context\b").expect("valid regex"),
        Regex::new(r"(?i)context(?:\s+\w+){0,4}\s+(\d{1,3})%").expect("valid regex"),
    ];

    for regex in patterns {
        if let Some(captures) = regex.captures(output) {
            if let Some(matched) = captures.get(1) {
                if let Ok(pct) = matched.as_str().parse::<u8>() {
                    return Some(pct.min(100));
                }
            }
        }
    }

    None
}

pub fn decide_approval(command: &str) -> ApprovalAction {
    let project_dir = default_project_dir();
    decide_approval_for_project(command, &project_dir)
}

fn decide_approval_for_project(command: &str, project_dir: &Path) -> ApprovalAction {
    if command.trim().is_empty() {
        return ApprovalAction::Deny("unable to classify empty command".to_string());
    }

    if command_touches_outside_project(command, project_dir) {
        return ApprovalAction::Deny(format!(
            "command touches paths outside {}",
            project_dir.display()
        ));
    }

    let tokens = tokenize_command(command);
    let root = tokens
        .first()
        .map(|token| executable_name(token))
        .unwrap_or_default();
    let sub = tokens.get(1).map(|value| value.as_str()).unwrap_or("");

    match root.as_str() {
        "cargo" => ApprovalAction::Persist,
        "curl" | "node" | "python" | "python3" | "bun" => ApprovalAction::Persist,
        "git" => match sub {
            "push" | "reset" => ApprovalAction::ApproveOnce,
            "add" | "commit" | "status" | "test" | "build" => ApprovalAction::Persist,
            _ => ApprovalAction::Deny(format!("git subcommand '{}' is not on the allowlist", sub)),
        },
        "rm" => ApprovalAction::ApproveOnce,
        _ => ApprovalAction::Deny(format!("command '{}' is not on the allowlist", root)),
    }
}

pub fn capture_pane(pane: &str, lines: u32) -> String {
    Command::new("tmux")
        .args(["capture-pane", "-t", pane, "-p", "-S", &format!("-{lines}")])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
        .unwrap_or_default()
}

pub fn send_keys(pane: &str, text: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["send-keys", "-t", pane, text, "Enter"])
        .status()
        .with_context(|| format!("send keys to pane '{}'", pane))?;

    if !status.success() {
        bail!("tmux send-keys failed for pane '{}'", pane);
    }

    Ok(())
}

pub fn watcher_to_value(watcher: &PaneWatcher) -> Value {
    json!({
        "pane": watcher.pane_id,
        "agent_type": watcher.agent_type.short_name(),
        "agent_type_label": watcher.agent_type.display_name(),
        "mission": watcher.mission,
        "state": watcher_state_to_value(&watcher.state),
        "context_pct": watcher.context_pct,
        "approvals_given": watcher.approvals_given,
        "corrections_sent": watcher.corrections_sent,
        "errors_unblocked": watcher.errors_unblocked,
        "uptime_secs": watcher.started_at.elapsed().as_secs(),
    })
}

pub fn watchers_to_value(watchers: &[PaneWatcher]) -> Value {
    let values: Vec<_> = watchers.iter().map(watcher_to_value).collect();
    json!({
        "count": values.len(),
        "watchers": values,
    })
}

fn watcher_state_to_value(state: &WatcherState) -> Value {
    match state {
        WatcherState::Starting => json!({"kind": "starting"}),
        WatcherState::Stopped { reason } => json!({
            "kind": "stopped",
            "reason": reason,
        }),
        WatcherState::Active(agent_state) => agent_state_to_value(agent_state),
    }
}

fn agent_state_to_value(state: &AgentState) -> Value {
    match state {
        AgentState::Working {
            description,
            duration_secs,
        } => json!({
            "kind": "working",
            "description": description,
            "duration_secs": duration_secs,
        }),
        AgentState::Idle { prompt_text } => json!({
            "kind": "idle",
            "prompt_text": prompt_text,
        }),
        AgentState::PermissionPrompt { command, options } => json!({
            "kind": "permission_prompt",
            "command": command,
            "options": options,
        }),
        AgentState::Error { message } => json!({
            "kind": "error",
            "message": message,
        }),
        AgentState::Done { summary } => json!({
            "kind": "done",
            "summary": summary,
        }),
        AgentState::ContextLow { pct } => json!({
            "kind": "context_low",
            "pct": pct,
        }),
        AgentState::Exited => json!({"kind": "exited"}),
    }
}

fn pane_exists(pane: &str) -> bool {
    Command::new("tmux")
        .args(["display-message", "-t", pane, "-p", "#{pane_id}"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn pane_current_path(pane: &str) -> Option<PathBuf> {
    let output = Command::new("tmux")
        .args(["display-message", "-t", pane, "-p", "#{pane_current_path}"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

fn send_instruction(pane: &str, instruction: &str) -> Result<()> {
    let text = instruction.replace('\n', " ");
    let typed = Command::new("tmux")
        .args(["send-keys", "-t", pane, "-l", &text])
        .status()
        .with_context(|| format!("send literal instruction to pane '{}'", pane))?;

    if !typed.success() {
        bail!("tmux send-keys -l failed for pane '{}'", pane);
    }

    let enter = Command::new("tmux")
        .args(["send-keys", "-t", pane, "Enter"])
        .status()
        .with_context(|| format!("send Enter to pane '{}'", pane))?;

    if !enter.success() {
        bail!("tmux send-keys Enter failed for pane '{}'", pane);
    }

    Ok(())
}

fn send_approval(pane: &str, action: &ApprovalAction, options: &[String]) -> Result<()> {
    match action {
        ApprovalAction::Persist => {
            if let Some(choice) = persistent_choice(options) {
                send_literal_and_enter(pane, &choice)
            } else if options
                .iter()
                .any(|option| option.to_lowercase().contains("press enter"))
            {
                send_enter(pane)
            } else {
                send_literal_and_enter(pane, "y")
            }
        }
        ApprovalAction::ApproveOnce => {
            if options
                .iter()
                .any(|option| option.to_lowercase().contains("press enter"))
            {
                send_enter(pane)
            } else {
                send_literal_and_enter(pane, "y")
            }
        }
        ApprovalAction::Deny(_) => send_literal_and_enter(pane, "n"),
    }
}

fn send_literal_and_enter(pane: &str, text: &str) -> Result<()> {
    let typed = Command::new("tmux")
        .args(["send-keys", "-t", pane, "-l", text])
        .status()
        .with_context(|| format!("send approval text to pane '{}'", pane))?;

    if !typed.success() {
        bail!("tmux send-keys -l failed for pane '{}'", pane);
    }

    send_enter(pane)
}

fn send_enter(pane: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["send-keys", "-t", pane, "Enter"])
        .status()
        .with_context(|| format!("send Enter to pane '{}'", pane))?;

    if !status.success() {
        bail!("tmux send-keys Enter failed for pane '{}'", pane);
    }

    Ok(())
}

fn persistent_choice(options: &[String]) -> Option<String> {
    let index = options.iter().position(|option| {
        let lower = option.to_lowercase();
        lower.contains("don't ask again")
            || lower.contains("dont ask again")
            || lower.contains("always allow")
            || lower.contains("persist")
    })?;

    Some((index + 1).to_string())
}

fn idle_instruction(mission: &str, project_dir: &Path) -> String {
    let git_status = git_status_summary(project_dir);
    if git_status.trim().is_empty() {
        format!(
            "Continue the mission: {}. State the next concrete step you are taking, execute it now, and only stop if you hit a real blocker.",
            mission
        )
    } else {
        format!(
            "You are idle with uncommitted changes in {}. Review the current diff, align it to the mission '{}', and continue with the next concrete step.",
            project_dir.display(),
            mission
        )
    }
}

fn error_instruction(message: &str, mission: &str) -> String {
    let lower = message.to_lowercase();
    if lower.contains("database is locked") {
        format!(
            "The session is blocked by a database lock. Retry after a short pause, avoid parallel writes, verify the lock is clear, then continue: {}.",
            mission
        )
    } else if lower.contains("hit your limit") || lower.contains("rate limit") {
        "Usage is exhausted. Stop active work, write a concise handoff with changed files and next step, and commit if the work is in a safe state.".to_string()
    } else if lower.contains("panic") || lower.contains("failed") {
        format!(
            "Investigate the failure, isolate the smallest reproducible error, fix it, rerun the failing command, and continue: {}.",
            mission
        )
    } else {
        format!(
            "You hit an error: {}. Diagnose it, fix the root cause, verify the fix, and continue the mission: {}.",
            message, mission
        )
    }
}

fn git_status_summary(project_dir: &Path) -> String {
    let project_dir = match project_dir.to_str() {
        Some(path) => path,
        None => return String::new(),
    };

    Command::new("git")
        .args(["-C", project_dir, "status", "--short"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default()
}

fn detect_error_message(output: &str) -> Option<String> {
    let patterns = [
        "error[",
        "FAILED",
        "panic",
        "database is locked",
        "rate limit",
        "hit your limit",
    ];
    let lower = output.to_lowercase();

    for pattern in patterns {
        if lower.contains(&pattern.to_lowercase()) {
            return recent_matching_line(output, pattern).or_else(|| Some(pattern.to_string()));
        }
    }

    None
}

fn detect_done_summary(output: &str) -> Option<String> {
    let markers = vec!["---DONE---".to_string(), "TASK COMPLETE".to_string()];
    if let Some(marker) = crate::pty::output::check_completion(output, &markers) {
        return Some(marker);
    }

    recent_matching_line(output, "completed")
        .or_else(|| recent_matching_line(output, "done"))
        .filter(|line| !line.contains("% left"))
}

fn detect_permission_prompt(output: &str, agent_type: &AgentType) -> Option<(String, Vec<String>)> {
    let lower = output.to_lowercase();
    let looks_like_prompt = lower.contains("press enter to confirm")
        || lower.contains("yes, proceed")
        || lower.contains("allow?")
        || lower.contains("(y/n)")
        || lower.contains("[y/n]")
        || lower.contains("[yes/no]")
        || lower.contains("yes / no");

    if !looks_like_prompt {
        return None;
    }

    let command = extract_command_from_prompt(output).unwrap_or_else(|| {
        format!(
            "{} prompt awaiting approval",
            agent_type.short_name().to_lowercase()
        )
    });
    let options = extract_prompt_options(output);

    Some((command, options))
}

fn extract_command_from_prompt(output: &str) -> Option<String> {
    let recent_lines: Vec<&str> = output
        .lines()
        .rev()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .take(20)
        .collect();

    for line in &recent_lines {
        if let Some(command) = extract_backticked_segment(line) {
            return Some(command);
        }
        if let Some(command) = line.strip_prefix("Command:") {
            return Some(command.trim().to_string());
        }
        if let Some(command) = line.strip_prefix("command:") {
            return Some(command.trim().to_string());
        }
        if let Some(command) = line.strip_prefix("$ ") {
            return Some(command.trim().to_string());
        }
        if let Some(command) = line.strip_prefix("> ") {
            return Some(command.trim().to_string());
        }
    }

    recent_lines
        .into_iter()
        .find(|line| looks_like_command(line))
        .map(|line| line.to_string())
}

fn extract_backticked_segment(line: &str) -> Option<String> {
    let start = line.find('`')?;
    let rest = &line[start + 1..];
    let end = rest.find('`')?;
    let segment = rest[..end].trim();
    if segment.is_empty() {
        None
    } else {
        Some(segment.to_string())
    }
}

fn extract_prompt_options(output: &str) -> Vec<String> {
    let mut options = Vec::new();
    let recent_lines: Vec<&str> = output.lines().rev().take(12).collect();

    for line in recent_lines.into_iter().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.len() > 80 {
            continue;
        }
        let lower = trimmed.to_lowercase();
        let looks_like_option = lower == "yes"
            || lower == "no"
            || lower.starts_with("yes,")
            || lower.starts_with("no,")
            || lower.starts_with("press enter")
            || lower.starts_with("allow")
            || NUMBERED_LIST_RE.is_match(trimmed);

        if looks_like_option && !options.iter().any(|existing| existing == trimmed) {
            options.push(trimmed.to_string());
        }
    }

    options
}

fn detect_working(output: &str, agent_type: &AgentType) -> bool {
    let lower = output.to_lowercase();
    match agent_type {
        AgentType::CodexCli => {
            output.contains("• Working (") || lower.contains("waiting for background")
        }
        AgentType::ClaudeCode => {
            output.contains('⏳')
                || output.contains('⠋')
                || output.contains('⠙')
                || output.contains('⠸')
                || output.contains('⠴')
                || lower.contains("working")
        }
        _ => {
            lower.contains("working")
                || lower.contains("running")
                || lower.contains("thinking")
                || lower.contains("executing")
        }
    }
}

fn working_description(output: &str) -> String {
    recent_matching_line(output, "working")
        .or_else(|| recent_matching_line(output, "background"))
        .or_else(|| last_non_empty_line(output))
        .unwrap_or_else(|| "working".to_string())
}

fn detect_idle_prompt(output: &str, agent_type: &AgentType) -> Option<String> {
    let lines: Vec<&str> = output
        .lines()
        .rev()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .take(6)
        .collect();

    for line in lines {
        let lower = line.to_lowercase();
        let is_codex_idle = matches!(agent_type, AgentType::CodexCli)
            && (line.starts_with('›') || line.starts_with('❯') || lower.contains("% left"));
        let is_generic_idle =
            line.starts_with('>') || line.starts_with('›') || line.starts_with('❯');
        if is_codex_idle || is_generic_idle {
            return Some(line.to_string());
        }
    }

    None
}

fn recent_matching_line(output: &str, needle: &str) -> Option<String> {
    let needle = needle.to_lowercase();
    output
        .lines()
        .rev()
        .find(|line| line.to_lowercase().contains(&needle))
        .map(|line| line.trim().to_string())
}

fn last_non_empty_line(output: &str) -> Option<String> {
    output.lines().rev().find_map(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn looks_like_command(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = trimmed.to_lowercase();
    lower.starts_with("cargo ")
        || lower.starts_with("git ")
        || lower.starts_with("curl ")
        || lower.starts_with("node ")
        || lower.starts_with("python ")
        || lower.starts_with("python3 ")
        || lower.starts_with("bun ")
        || lower.starts_with("rm ")
}

fn tail_lines(output: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = output.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}

fn command_touches_outside_project(command: &str, project_dir: &Path) -> bool {
    let tokens = tokenize_command(command);
    let mut expect_path = false;

    for (index, token) in tokens.iter().enumerate() {
        let cleaned = clean_token(token);
        if cleaned.is_empty() {
            expect_path = false;
            continue;
        }

        if index == 0 && is_known_executable_path(&cleaned) {
            expect_path = false;
            continue;
        }

        if expect_path || looks_like_path(&cleaned) {
            if let Some(path) = resolve_path_token(&cleaned, project_dir) {
                if !path_within_dir(&path, project_dir) {
                    return true;
                }
            }
        }

        expect_path = matches!(
            cleaned.as_str(),
            "-C" | "--cwd" | "--workdir" | "--project-dir"
        );
    }

    false
}

fn tokenize_command(command: &str) -> Vec<String> {
    command
        .split_whitespace()
        .map(ToString::to_string)
        .collect()
}

fn clean_token(token: &str) -> String {
    token
        .trim_matches(|ch: char| matches!(ch, '"' | '\'' | '`' | ',' | ';' | '(' | ')' | '[' | ']'))
        .to_string()
}

fn executable_name(token: &str) -> String {
    Path::new(token)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(token)
        .to_string()
}

fn looks_like_path(token: &str) -> bool {
    token.starts_with('/')
        || token.starts_with("./")
        || token.starts_with("../")
        || token.starts_with("~/")
}

fn resolve_path_token(token: &str, project_dir: &Path) -> Option<PathBuf> {
    if token == "~" {
        return std::env::var("HOME").ok().map(PathBuf::from);
    }

    if let Some(home_relative) = token.strip_prefix("~/") {
        let home = std::env::var("HOME").ok()?;
        return Some(normalize_path(
            &PathBuf::from(home),
            Path::new(home_relative),
        ));
    }

    if token.starts_with('/') {
        return Some(normalize_absolute(Path::new(token)));
    }

    if token.starts_with("./") || token.starts_with("../") {
        return Some(normalize_path(project_dir, Path::new(token)));
    }

    None
}

fn normalize_absolute(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir => normalized.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
        }
    }
    normalized
}

fn normalize_path(base: &Path, candidate: &Path) -> PathBuf {
    let mut normalized = if candidate.is_absolute() {
        PathBuf::new()
    } else {
        normalize_absolute(base)
    };

    for component in candidate.components() {
        match component {
            Component::RootDir => normalized = PathBuf::from("/"),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
        }
    }

    normalized
}

fn path_within_dir(path: &Path, project_dir: &Path) -> bool {
    let normalized_path = normalize_absolute(path);
    let normalized_project = normalize_absolute(project_dir);
    normalized_path.starts_with(&normalized_project)
}

fn is_known_executable_path(token: &str) -> bool {
    if !token.starts_with('/') {
        return false;
    }

    matches!(
        executable_name(token).as_str(),
        "cargo"
            | "git"
            | "curl"
            | "node"
            | "python"
            | "python3"
            | "bun"
            | "bash"
            | "sh"
            | "zsh"
            | "rm"
    )
}

fn default_project_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
impl SessionController {
    pub(crate) async fn insert_watcher_for_test(&self, watcher: PaneWatcher) {
        self.watchers
            .write()
            .await
            .insert(watcher.pane_id.clone(), Arc::new(RwLock::new(watcher)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_codex_working_state() {
        let state = detect_state(
            "• Working (searching repo)\nWaiting for background task",
            &AgentType::CodexCli,
        );
        match state {
            AgentState::Working { description, .. } => {
                assert!(description.contains("Working") || description.contains("background"));
            }
            other => panic!("expected working state, got {:?}", other),
        }
    }

    #[test]
    fn detects_codex_idle_state() {
        let state = detect_state("89% left\n› continue", &AgentType::CodexCli);
        match state {
            AgentState::Idle { prompt_text } => assert!(prompt_text.contains('›')),
            other => panic!("expected idle state, got {:?}", other),
        }
    }

    #[test]
    fn detects_permission_prompt() {
        let output = "Command: cargo test\nPress enter to confirm";
        let state = detect_state(output, &AgentType::CodexCli);
        match state {
            AgentState::PermissionPrompt { command, options } => {
                assert_eq!(command, "cargo test");
                assert!(options.iter().any(|option| option.contains("Press enter")));
            }
            other => panic!("expected permission prompt, got {:?}", other),
        }
    }

    #[test]
    fn detects_error_state() {
        let state = detect_state("database is locked", &AgentType::ClaudeCode);
        match state {
            AgentState::Error { message } => assert!(message.contains("database is locked")),
            other => panic!("expected error state, got {:?}", other),
        }
    }

    #[test]
    fn detects_low_context_state() {
        let state = detect_state("23% left\n› continue", &AgentType::CodexCli);
        match state {
            AgentState::ContextLow { pct } => assert_eq!(pct, 23),
            other => panic!("expected context low state, got {:?}", other),
        }
    }

    #[test]
    fn persist_for_safe_commands() {
        let project = Path::new("/repo");
        assert_eq!(
            decide_approval_for_project("cargo test", project),
            ApprovalAction::Persist
        );
        assert_eq!(
            decide_approval_for_project("git commit -m test", project),
            ApprovalAction::Persist
        );
    }

    #[test]
    fn approve_once_for_risky_commands() {
        let project = Path::new("/repo");
        assert_eq!(
            decide_approval_for_project("git push origin master", project),
            ApprovalAction::ApproveOnce
        );
        assert_eq!(
            decide_approval_for_project("rm target/tmp.txt", project),
            ApprovalAction::ApproveOnce
        );
    }

    #[test]
    fn deny_commands_outside_project() {
        let project = Path::new("/repo");
        match decide_approval_for_project("cat /etc/passwd", project) {
            ApprovalAction::Deny(reason) => assert!(reason.contains("outside")),
            other => panic!("expected deny, got {:?}", other),
        }
    }
}

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::sync::{watch, RwLock};
use tokio::task::JoinHandle;

use crate::agents::AgentType;
use crate::app::App;
use crate::runtime_broker;
use crate::session_controller::{AgentState, PaneWatcher, WatcherState};
use crate::tmux;

const DEFAULT_MAX_AGENTS: usize = 5;
const FETCH_MULTIPLIER: usize = 5;
const FETCH_LIMIT_CAP: usize = 50;
const MONITOR_INTERVAL: Duration = Duration::from_secs(10);
const SWARM_DONE_MARKER: &str = "SWARM_COMPLETE";
const SWARM_FAIL_MARKER: &str = "SWARM_FAILED";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmConfig {
    pub repo: String,
    pub max_agents: usize,
    pub issue_labels: Vec<String>,
    pub agent_provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmResult {
    pub issue_number: u32,
    pub branch: String,
    pub status: SwarmTaskStatus,
    pub commits: Vec<String>,
    pub pr_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SwarmTaskStatus {
    Queued,
    InProgress { pane: String, context_pct: u8 },
    Complete { pr_url: String },
    Failed { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmStatusReport {
    pub repo: String,
    pub repo_path: String,
    pub provider: String,
    pub max_agents: usize,
    pub active: bool,
    pub started_at: String,
    pub labels: Vec<String>,
    pub results: Vec<SwarmResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmIssue {
    number: u32,
    title: String,
    body: String,
    labels: Vec<String>,
    url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SwarmTaskRecord {
    issue: SwarmIssue,
    branch: String,
    worktree_path: String,
    pane: Option<String>,
    status: SwarmTaskStatus,
    commits: Vec<String>,
    pr_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SwarmSnapshot {
    config: SwarmConfig,
    repo_path: String,
    base_branch: String,
    started_at: String,
    tasks: Vec<SwarmTaskRecord>,
}

struct ActiveSwarm {
    state: Arc<RwLock<SwarmSnapshot>>,
    stop_tx: watch::Sender<bool>,
    handle: JoinHandle<()>,
}

static ACTIVE_SWARM: OnceLock<Mutex<Option<ActiveSwarm>>> = OnceLock::new();

pub async fn start(app: Arc<App>, config: SwarmConfig) -> Result<SwarmStatusReport> {
    let state = {
        let guard = active_swarm()
            .lock()
            .map_err(|_| anyhow!("swarm state lock poisoned"))?;
        if guard.is_some() {
            bail!("a swarm is already running; stop it before starting a new one");
        }
        None::<Arc<RwLock<SwarmSnapshot>>>
    };
    drop(state);

    let config = normalize_config(config)?;
    let repo_path = resolve_repo_path(&config.repo)?;
    let base_branch = current_branch(&repo_path)?;
    let issue_limit = issue_fetch_limit(config.max_agents);
    let issues = fetch_issues(&config.repo, &config.issue_labels, issue_limit)?;
    if issues.is_empty() {
        bail!("no open issues matched the current swarm filters");
    }

    let tasks = issues
        .into_iter()
        .map(|issue| SwarmTaskRecord {
            branch: branch_name(issue.number),
            worktree_path: repo_path
                .join(".worktrees")
                .join(format!("issue-{}", issue.number))
                .to_string_lossy()
                .to_string(),
            pane: None,
            status: SwarmTaskStatus::Queued,
            commits: Vec::new(),
            pr_url: None,
            issue,
        })
        .collect();

    let snapshot = SwarmSnapshot {
        config,
        repo_path: repo_path.to_string_lossy().to_string(),
        base_branch,
        started_at: crate::state::now(),
        tasks,
    };
    let state = Arc::new(RwLock::new(snapshot));
    refresh_swarm_state(app.as_ref(), &state).await?;

    let (stop_tx, stop_rx) = watch::channel(false);
    let monitor_state = Arc::clone(&state);
    let monitor_app = Arc::clone(&app);
    let handle = tokio::spawn(async move {
        monitor_loop(monitor_app, monitor_state, stop_rx).await;
    });

    {
        let mut guard = active_swarm()
            .lock()
            .map_err(|_| anyhow!("swarm state lock poisoned"))?;
        *guard = Some(ActiveSwarm {
            state: Arc::clone(&state),
            stop_tx,
            handle,
        });
    }

    let snapshot = state.read().await.clone();
    Ok(snapshot_to_report(&snapshot))
}

pub async fn status(app: &App) -> Result<SwarmStatusReport> {
    let active = {
        let guard = active_swarm()
            .lock()
            .map_err(|_| anyhow!("swarm state lock poisoned"))?;
        guard.as_ref().map(|active| Arc::clone(&active.state))
    };

    if let Some(state) = active {
        refresh_swarm_state(app, &state).await?;
        let snapshot = state.read().await.clone();
        return Ok(snapshot_to_report(&snapshot));
    }

    let snapshot = load_snapshot()?.ok_or_else(|| anyhow!("no swarm state found"))?;
    Ok(snapshot_to_report(&snapshot))
}

pub async fn stop(app: &App) -> Result<SwarmStatusReport> {
    let active = {
        let mut guard = active_swarm()
            .lock()
            .map_err(|_| anyhow!("swarm state lock poisoned"))?;
        guard.take()
    };

    let Some(active) = active else {
        let snapshot = load_snapshot()?.ok_or_else(|| anyhow!("no swarm state found"))?;
        return Ok(snapshot_to_report(&snapshot));
    };

    let _ = active.stop_tx.send(true);
    active.handle.abort();

    refresh_swarm_state(app, &active.state).await?;

    let mut snapshot = active.state.write().await;
    for task in &mut snapshot.tasks {
        if let Some(pane) = task.pane.as_deref() {
            let _ = app.session_controller.stop(pane).await;
            let _ = tmux::kill_window(pane);
        }
        if !is_terminal(&task.status) {
            task.status = SwarmTaskStatus::Failed {
                reason: "swarm stopped by operator".to_string(),
            };
        }
        let _ = cleanup_worktree(Path::new(&task.worktree_path));
        task.pane = None;
    }
    save_snapshot(&snapshot)?;
    Ok(snapshot_to_report(&snapshot))
}

pub async fn monitor_swarm(app: &App) -> Result<SwarmStatusReport> {
    status(app).await
}

pub fn fetch_issues(repo: &str, labels: &[String], limit: usize) -> Result<Vec<SwarmIssue>> {
    let output = Command::new("gh")
        .args([
            "issue",
            "list",
            "--repo",
            repo,
            "--state",
            "open",
            "--limit",
            &limit.max(1).to_string(),
            "--json",
            "number,title,body,labels",
        ])
        .output()
        .with_context(|| format!("run gh issue list for {}", repo))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!(
            "gh issue list failed for {}: {}",
            repo,
            if stderr.is_empty() {
                "unknown error"
            } else {
                stderr.as_str()
            }
        );
    }

    let raw: Vec<GitHubIssue> =
        serde_json::from_slice(&output.stdout).context("parse gh issue list output")?;
    let mut issues = raw
        .into_iter()
        .map(|issue| SwarmIssue {
            number: issue.number,
            title: issue.title,
            body: issue.body,
            labels: issue.labels.into_iter().map(|label| label.name).collect(),
            url: format!("https://github.com/{repo}/issues/{}", issue.number),
        })
        .collect::<Vec<_>>();

    if !labels.is_empty() {
        issues.retain(|issue| issue_matches_labels(issue, labels));
    }

    Ok(issues)
}

pub fn create_worktree(repo_path: &Path, issue: u32) -> Result<(PathBuf, String)> {
    let branch = branch_name(issue);
    let worktree_path = repo_path
        .join(".worktrees")
        .join(format!("issue-{}", issue));
    if let Some(parent) = worktree_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }

    if worktree_path.exists() {
        let _ = Command::new("git")
            .current_dir(repo_path)
            .args(["worktree", "remove", "--force"])
            .arg(&worktree_path)
            .output();
        if worktree_path.exists() {
            let _ = std::fs::remove_dir_all(&worktree_path);
        }
    }

    let _ = Command::new("git")
        .current_dir(repo_path)
        .args(["branch", "-D", &branch])
        .output();

    let base_branch = current_branch(repo_path)?;
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["worktree", "add", "-b", &branch])
        .arg(&worktree_path)
        .arg(&base_branch)
        .output()
        .with_context(|| format!("create worktree for issue #{}", issue))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!(
            "git worktree add failed for issue #{}: {}",
            issue,
            if stderr.is_empty() {
                "unknown error"
            } else {
                stderr.as_str()
            }
        );
    }

    Ok((worktree_path, branch))
}

pub fn spawn_agent(worktree_path: &Path, issue: &SwarmIssue, provider: &str) -> Result<String> {
    let window_name = format!("swarm-{}", issue.number);
    let prompt = build_issue_prompt(worktree_path, issue);
    let plan = runtime_broker::plan_tmux_launch(
        provider,
        &window_name,
        &worktree_path.to_string_lossy(),
        &prompt,
        true,
        None,
    )?;
    let agent = tmux::spawn_planned_agent(&plan, &[])?;
    Ok(agent.target)
}

pub fn create_pr(worktree: &Path, issue: &SwarmIssue) -> Result<String> {
    let branch = current_branch(worktree)?;
    if let Some(existing) = existing_pr_url(worktree, &branch)? {
        return Ok(existing);
    }

    let title = format!("Fix #{}: {}", issue.number, issue.title);
    let body = format!(
        "Closes #{}\n\n## Summary\n- Automated fix prepared by DX Terminal swarm.\n- Issue URL: {}\n\n## Validation\n- Agent reported tests/build steps complete before PR creation.",
        issue.number, issue.url
    );

    let output = Command::new("gh")
        .current_dir(worktree)
        .args(["pr", "create", "--title", &title, "--body", &body])
        .output()
        .with_context(|| format!("create PR for issue #{}", issue.number))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!(
            "gh pr create failed for issue #{}: {}",
            issue.number,
            if stderr.is_empty() {
                "unknown error"
            } else {
                stderr.as_str()
            }
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let url = stdout
        .lines()
        .find(|line| line.contains("github.com"))
        .map(|line| line.trim().to_string())
        .unwrap_or(stdout);
    if url.is_empty() {
        bail!("gh pr create succeeded but did not return a PR URL");
    }
    Ok(url)
}

pub fn cleanup_worktree(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let output = Command::new("git")
        .current_dir(path)
        .args(["worktree", "remove", "--force"])
        .arg(path)
        .output()
        .with_context(|| format!("remove worktree {}", path.display()))?;

    if !output.status.success() && path.exists() {
        std::fs::remove_dir_all(path).with_context(|| format!("remove {}", path.display()))?;
    }

    let _ = Command::new("git")
        .current_dir(path.parent().unwrap_or(path))
        .args(["worktree", "prune"])
        .output();

    Ok(())
}

async fn monitor_loop(
    app: Arc<App>,
    state: Arc<RwLock<SwarmSnapshot>>,
    mut stop_rx: watch::Receiver<bool>,
) {
    loop {
        if *stop_rx.borrow() {
            break;
        }

        if let Err(err) = refresh_swarm_state(app.as_ref(), &state).await {
            tracing::warn!(error = %err, "swarm monitor refresh failed");
        }

        let is_complete = {
            let snapshot = state.read().await;
            snapshot.tasks.iter().all(|task| is_terminal(&task.status))
        };
        if is_complete {
            break;
        }

        tokio::select! {
            changed = stop_rx.changed() => {
                if changed.is_ok() && *stop_rx.borrow() {
                    break;
                }
            }
            _ = tokio::time::sleep(MONITOR_INTERVAL) => {}
        }
    }
}

async fn refresh_swarm_state(app: &App, state: &Arc<RwLock<SwarmSnapshot>>) -> Result<()> {
    let snapshot = state.read().await.clone();
    let mut updated = snapshot.clone();

    for task in &mut updated.tasks {
        if matches!(task.status, SwarmTaskStatus::Queued) {
            continue;
        }
        *task = reconcile_task(app, &updated.base_branch, task.clone()).await?;
    }

    launch_queued_tasks(app, &mut updated).await?;
    save_snapshot(&updated)?;
    *state.write().await = updated;
    Ok(())
}

async fn launch_queued_tasks(app: &App, snapshot: &mut SwarmSnapshot) -> Result<()> {
    let active = snapshot
        .tasks
        .iter()
        .filter(|task| matches!(task.status, SwarmTaskStatus::InProgress { .. }))
        .count();
    let available = snapshot.config.max_agents.saturating_sub(active);
    if available == 0 {
        return Ok(());
    }

    for task in snapshot
        .tasks
        .iter_mut()
        .filter(|task| matches!(task.status, SwarmTaskStatus::Queued))
        .take(available)
    {
        let (worktree_path, branch) =
            create_worktree(Path::new(&snapshot.repo_path), task.issue.number)?;
        let pane = spawn_agent(&worktree_path, &task.issue, &snapshot.config.agent_provider)?;
        let mission = issue_mission(&task.issue);
        if let Err(err) = app
            .session_controller
            .start(
                pane.clone(),
                mission,
                provider_agent_type(&snapshot.config.agent_provider),
            )
            .await
        {
            tracing::warn!(pane = %pane, error = %err, "failed to start session controller");
        }
        task.branch = branch;
        task.worktree_path = worktree_path.to_string_lossy().to_string();
        task.pane = Some(pane.clone());
        task.status = SwarmTaskStatus::InProgress {
            pane,
            context_pct: 0,
        };
    }

    Ok(())
}

async fn reconcile_task(
    app: &App,
    base_branch: &str,
    mut task: SwarmTaskRecord,
) -> Result<SwarmTaskRecord> {
    let pane = task
        .pane
        .clone()
        .ok_or_else(|| anyhow!("missing tmux pane for issue #{}", task.issue.number))?;
    let watcher = app.session_controller.status(&pane).await;
    let context_pct = watcher
        .as_ref()
        .and_then(|watcher| watcher.context_pct)
        .unwrap_or(0);
    task.status = SwarmTaskStatus::InProgress {
        pane: pane.clone(),
        context_pct,
    };

    let output = tmux::capture_output(&pane);
    if let Some(reason) = detect_failure_marker(&output, task.issue.number) {
        task.status = SwarmTaskStatus::Failed { reason };
        return Ok(task);
    }

    let has_commits = !collect_commits(Path::new(&task.worktree_path), base_branch)?.is_empty();
    let pane_done = !tmux::pane_exists(&pane) || tmux::check_done(&pane);
    let watcher_done = watcher_is_done(watcher.as_ref());
    let ready_marker = detect_done_marker(&output, task.issue.number);

    if ready_marker || watcher_done || (pane_done && has_commits) {
        finalize_task(&mut task, base_branch)?;
        return Ok(task);
    }

    if pane_done && !has_commits {
        task.status = SwarmTaskStatus::Failed {
            reason: "agent exited without producing a commit".to_string(),
        };
    }

    Ok(task)
}

fn finalize_task(task: &mut SwarmTaskRecord, base_branch: &str) -> Result<()> {
    task.commits = collect_commits(Path::new(&task.worktree_path), base_branch)?;
    if task.commits.is_empty() {
        let dirty = git_status(Path::new(&task.worktree_path))?;
        let reason = if dirty.trim().is_empty() {
            "agent reported completion without code changes".to_string()
        } else {
            "agent reported completion but left uncommitted changes".to_string()
        };
        task.status = SwarmTaskStatus::Failed { reason };
        return Ok(());
    }

    match create_pr(Path::new(&task.worktree_path), &task.issue) {
        Ok(pr_url) => {
            task.pr_url = Some(pr_url.clone());
            task.status = SwarmTaskStatus::Complete { pr_url };
        }
        Err(err) => {
            task.status = SwarmTaskStatus::Failed {
                reason: err.to_string(),
            };
        }
    }
    Ok(())
}

fn normalize_config(mut config: SwarmConfig) -> Result<SwarmConfig> {
    config.repo = config.repo.trim().to_string();
    if config.repo.is_empty() {
        bail!("repo is required");
    }
    config.max_agents = match config.max_agents {
        0 => DEFAULT_MAX_AGENTS,
        value => value.min(20),
    };
    config.issue_labels = config
        .issue_labels
        .into_iter()
        .map(|label| label.trim().to_string())
        .filter(|label| !label.is_empty())
        .collect();
    config.agent_provider = match runtime_broker::normalize_provider_id(&config.agent_provider) {
        "claude" => "claude".to_string(),
        "codex" => "codex".to_string(),
        "gemini" => "gemini".to_string(),
        "opencode" => "opencode".to_string(),
        other => other.to_string(),
    };
    Ok(config)
}

fn resolve_repo_path(expected_repo: &str) -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("read current directory")?;
    let root = git_root(&cwd)?;
    let remote = git_remote_url(&root).ok_or_else(|| anyhow!("origin remote is not configured"))?;
    let actual = parse_github_repo_slug(&remote).ok_or_else(|| {
        anyhow!(
            "could not parse GitHub repo from origin remote '{}'",
            remote
        )
    })?;
    if actual != expected_repo {
        bail!(
            "current repository is '{}', but swarm was asked to operate on '{}'",
            actual,
            expected_repo
        );
    }
    Ok(root)
}

fn snapshot_to_report(snapshot: &SwarmSnapshot) -> SwarmStatusReport {
    SwarmStatusReport {
        repo: snapshot.config.repo.clone(),
        repo_path: snapshot.repo_path.clone(),
        provider: snapshot.config.agent_provider.clone(),
        max_agents: snapshot.config.max_agents,
        active: snapshot.tasks.iter().any(|task| !is_terminal(&task.status)),
        started_at: snapshot.started_at.clone(),
        labels: snapshot.config.issue_labels.clone(),
        results: snapshot
            .tasks
            .iter()
            .map(|task| SwarmResult {
                issue_number: task.issue.number,
                branch: task.branch.clone(),
                status: task.status.clone(),
                commits: task.commits.clone(),
                pr_url: task.pr_url.clone(),
            })
            .collect(),
    }
}

fn issue_fetch_limit(max_agents: usize) -> usize {
    max_agents
        .max(1)
        .saturating_mul(FETCH_MULTIPLIER)
        .min(FETCH_LIMIT_CAP)
}

fn issue_matches_labels(issue: &SwarmIssue, requested: &[String]) -> bool {
    let issue_labels = issue
        .labels
        .iter()
        .map(|label| label.to_ascii_lowercase())
        .collect::<Vec<_>>();
    requested.iter().all(|label| {
        let label = label.to_ascii_lowercase();
        issue_labels.iter().any(|candidate| candidate == &label)
    })
}

fn issue_mission(issue: &SwarmIssue) -> String {
    format!("Fix issue #{}: {}", issue.number, issue.title)
}

fn build_issue_prompt(worktree_path: &Path, issue: &SwarmIssue) -> String {
    let mut prompt = vec![
        format!("Issue: {}", issue_mission(issue)),
        format!("Worktree: {}", worktree_path.display()),
        format!("Issue URL: {}", issue.url),
        "Requirements: inspect the repository state first, implement the fix, run the relevant tests/build steps, and commit your changes to the current branch.".to_string(),
        format!(
            "When the branch is ready for PR creation, print '{}' on a final line.",
            done_marker(issue.number)
        ),
        format!(
            "If you are blocked, print '{}: <reason>' on a final line.",
            fail_marker(issue.number)
        ),
    ];
    if !issue.labels.is_empty() {
        prompt.push(format!("Labels: {}", issue.labels.join(", ")));
    }
    if !issue.body.trim().is_empty() {
        prompt.push(format!("Issue body:\n{}", issue.body.trim()));
    }
    prompt.join("\n")
}

fn provider_agent_type(provider: &str) -> AgentType {
    match runtime_broker::normalize_provider_id(provider) {
        "codex" => AgentType::CodexCli,
        "gemini" => AgentType::GeminiCli,
        "opencode" => AgentType::OpenCode,
        "claude" => AgentType::ClaudeCode,
        _ => AgentType::Unknown,
    }
}

fn watcher_is_done(watcher: Option<&PaneWatcher>) -> bool {
    matches!(
        watcher.map(|watcher| &watcher.state),
        Some(WatcherState::Active(
            AgentState::Done { .. } | AgentState::Exited
        ))
    )
}

fn detect_done_marker(output: &str, issue: u32) -> bool {
    output.contains(&done_marker(issue))
}

fn detect_failure_marker(output: &str, issue: u32) -> Option<String> {
    let marker = fail_marker(issue);
    output
        .lines()
        .rev()
        .find(|line| line.contains(&marker))
        .map(|line| {
            line.split_once(':')
                .map(|(_, reason)| reason.trim().to_string())
                .filter(|reason| !reason.is_empty())
                .unwrap_or_else(|| "agent reported failure".to_string())
        })
}

fn branch_name(issue: u32) -> String {
    format!("fix/issue-{}", issue)
}

fn done_marker(issue: u32) -> String {
    format!("{SWARM_DONE_MARKER} issue-{issue}")
}

fn fail_marker(issue: u32) -> String {
    format!("{SWARM_FAIL_MARKER} issue-{issue}")
}

fn git_root(start: &Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .current_dir(start)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("determine git root")?;
    if !output.status.success() {
        bail!("current directory is not inside a git repository");
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
    ))
}

fn git_remote_url(root: &Path) -> Option<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let remote = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if remote.is_empty() {
        None
    } else {
        Some(remote)
    }
}

fn parse_github_repo_slug(remote: &str) -> Option<String> {
    let cleaned = remote.trim().trim_end_matches(".git");
    let marker = if let Some(index) = cleaned.find("github.com/") {
        &cleaned[index + "github.com/".len()..]
    } else if let Some(index) = cleaned.find("github.com:") {
        &cleaned[index + "github.com:".len()..]
    } else {
        return None;
    };

    let slug = marker.trim_matches('/');
    let mut parts = slug.split('/').filter(|part| !part.is_empty());
    let owner = parts.next()?;
    let repo = parts.next()?;
    Some(format!("{owner}/{repo}"))
}

fn current_branch(repo_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .with_context(|| format!("read current branch for {}", repo_path.display()))?;
    if !output.status.success() {
        bail!(
            "could not determine current branch for {}",
            repo_path.display()
        );
    }
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch.is_empty() {
        bail!("git returned an empty branch name");
    }
    Ok(branch)
}

fn collect_commits(worktree_path: &Path, base_branch: &str) -> Result<Vec<String>> {
    let output = Command::new("git")
        .current_dir(worktree_path)
        .args(["log", "--format=%H", &format!("{base_branch}..HEAD")])
        .output()
        .with_context(|| format!("collect commits in {}", worktree_path.display()))?;
    if !output.status.success() {
        bail!("git log failed in {}", worktree_path.display());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect())
}

fn git_status(worktree_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .current_dir(worktree_path)
        .args(["status", "--short"])
        .output()
        .with_context(|| format!("read git status in {}", worktree_path.display()))?;
    if !output.status.success() {
        bail!("git status failed in {}", worktree_path.display());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn existing_pr_url(worktree_path: &Path, branch: &str) -> Result<Option<String>> {
    let output = Command::new("gh")
        .current_dir(worktree_path)
        .args(["pr", "view", "--head", branch, "--json", "url"])
        .output()
        .with_context(|| format!("check existing PR for branch {}", branch))?;
    if !output.status.success() {
        return Ok(None);
    }
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("parse gh pr view output")?;
    Ok(value
        .get("url")
        .and_then(|url| url.as_str())
        .map(|url| url.to_string()))
}

fn save_snapshot(snapshot: &SwarmSnapshot) -> Result<()> {
    let path = snapshot_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(snapshot)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

fn load_snapshot() -> Result<Option<SwarmSnapshot>> {
    let path = snapshot_path();
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    let snapshot =
        serde_json::from_slice::<SwarmSnapshot>(&contents).context("parse swarm snapshot")?;
    Ok(Some(snapshot))
}

fn snapshot_path() -> PathBuf {
    crate::config::dx_root().join("swarm_state.json")
}

fn active_swarm() -> &'static Mutex<Option<ActiveSwarm>> {
    ACTIVE_SWARM.get_or_init(|| Mutex::new(None))
}

fn is_terminal(status: &SwarmTaskStatus) -> bool {
    matches!(
        status,
        SwarmTaskStatus::Complete { .. } | SwarmTaskStatus::Failed { .. }
    )
}

#[derive(Debug, Deserialize)]
struct GitHubIssue {
    number: u32,
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    labels: Vec<GitHubLabel>,
}

#[derive(Debug, Deserialize)]
struct GitHubLabel {
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_slug_from_https_remote() {
        assert_eq!(
            parse_github_repo_slug("https://github.com/pdaxt/dx-terminal.git"),
            Some("pdaxt/dx-terminal".to_string())
        );
    }

    #[test]
    fn matches_labels_case_insensitively() {
        let issue = SwarmIssue {
            number: 7,
            title: "Fix".to_string(),
            body: String::new(),
            labels: vec!["Bug".to_string(), "Urgent".to_string()],
            url: "https://github.com/pdaxt/dx-terminal/issues/7".to_string(),
        };
        assert!(issue_matches_labels(
            &issue,
            &["bug".to_string(), "URGENT".to_string()]
        ));
        assert!(!issue_matches_labels(&issue, &["docs".to_string()]));
    }

    #[test]
    fn issue_prompt_includes_swarm_markers() {
        let prompt = build_issue_prompt(
            Path::new("/tmp/repo/.worktrees/issue-12"),
            &SwarmIssue {
                number: 12,
                title: "Fix swarm".to_string(),
                body: "Need better queueing.".to_string(),
                labels: vec!["enhancement".to_string()],
                url: "https://github.com/pdaxt/dx-terminal/issues/12".to_string(),
            },
        );
        assert!(prompt.contains("SWARM_COMPLETE issue-12"));
        assert!(prompt.contains("SWARM_FAILED issue-12"));
        assert!(prompt.contains("Need better queueing."));
    }

    #[test]
    fn creates_and_cleans_up_worktree() {
        let temp = tempfile::tempdir().unwrap();
        run_git(temp.path(), &["init"]).unwrap();
        run_git(temp.path(), &["config", "user.email", "test@example.com"]).unwrap();
        run_git(temp.path(), &["config", "user.name", "Test User"]).unwrap();
        std::fs::write(temp.path().join("README.md"), "hello\n").unwrap();
        run_git(temp.path(), &["add", "README.md"]).unwrap();
        run_git(temp.path(), &["commit", "-m", "init"]).unwrap();

        let (worktree, branch) = create_worktree(temp.path(), 42).unwrap();
        assert_eq!(branch, "fix/issue-42");
        assert!(worktree.exists());

        cleanup_worktree(&worktree).unwrap();
        assert!(!worktree.exists());
    }

    fn run_git(repo: &Path, args: &[&str]) -> Result<()> {
        let output = Command::new("git").current_dir(repo).args(args).output()?;
        if output.status.success() {
            Ok(())
        } else {
            bail!(
                "{}",
                String::from_utf8_lossy(&output.stderr).trim().to_string()
            )
        }
    }
}

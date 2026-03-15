use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use serde::Deserialize;
use serde_json::json;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use crate::agents::AgentType;
use crate::app::App;
use crate::{engine, queue, runtime_broker, scanner, web};

#[derive(Debug, Clone, Args)]
pub struct GoArgs {
    /// Maximum number of open GitHub issues to pull into the run
    #[arg(long = "issues", default_value_t = 5)]
    pub max_issues: usize,
    /// Number of agent panes to launch
    #[arg(long = "agents", default_value_t = 3)]
    pub agents: usize,
    /// Print the detected plan without starting tmux or the dashboard
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct GoProject {
    pub name: String,
    pub path: PathBuf,
    pub remote_url: Option<String>,
    pub repo_slug: Option<String>,
    pub has_agents_md: bool,
    pub has_claude_md: bool,
    pub has_codex_md: bool,
    pub tech: Vec<String>,
    pub provider: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubIssue {
    pub number: u64,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub labels: Vec<GitHubLabel>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubLabel {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct GoSession {
    pub session_name: String,
    pub window_name: String,
    pub pane_targets: Vec<String>,
    pub created: bool,
}

pub async fn go(app: Arc<App>, args: GoArgs) -> Result<()> {
    let agents = args.agents.clamp(1, 9);
    let max_issues = args.max_issues.max(1);
    let project = detect_project()?;
    let session = ensure_tmux_session(&project, agents)?;
    let issues = fetch_github_issues(&project, max_issues)?;
    let planned_count = issues.len().min(agents);

    if args.dry_run {
        let preview = json!({
            "project": go_project_value(&project),
            "session": {
                "name": session.session_name,
                "window": session.window_name,
                "created": session.created,
                "pane_targets": session.pane_targets,
            },
            "issues": issues.iter().map(issue_value).collect::<Vec<_>>(),
            "spawn_count": planned_count,
        });
        println!("{}", serde_json::to_string_pretty(&preview)?);
        return Ok(());
    }

    engine::start_background_tasks(Some(Arc::clone(&app.state))).await;
    crate::dxos_scheduler::start(Arc::clone(&app));
    crate::dxos_supervisor::start(Arc::clone(&app));

    let port = find_free_port(3001)?;
    let web_app = Arc::clone(&app);
    let dashboard_handle = tokio::spawn(async move { web::run_web_server(web_app, port).await });

    let mut launched = Vec::new();
    for (pane_target, issue) in session
        .pane_targets
        .iter()
        .zip(issues.iter())
        .take(planned_count)
    {
        spawn_agent_for_issue(app.as_ref(), &project, pane_target, issue)?;
        launched.push((pane_target.clone(), issue.clone()));
    }

    println!(
        "DX Terminal running — {} agents on {} issues",
        launched.len(),
        issues.len()
    );
    println!("Dashboard: http://localhost:{port}");
    println!("Tmux: tmux attach -t {}", session.session_name);

    tokio::select! {
        result = dashboard_handle => {
            match result {
                Ok(Ok(())) => Ok(()),
                Ok(Err(error)) => Err(error),
                Err(join_error) => Err(anyhow!("dashboard task failed: {}", join_error)),
            }
        }
        signal = tokio::signal::ctrl_c() => {
            signal.context("waiting for ctrl-c")?;
            Ok(())
        }
    }
}

pub fn detect_project() -> Result<GoProject> {
    let root = git_root().unwrap_or(std::env::current_dir().context("read current directory")?);
    let scan_info = scanner::scan_single(&root);
    let name = scan_info
        .as_ref()
        .map(|info| info.name.clone())
        .or_else(|| {
            root.file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "project".to_string());
    let remote_url = git_remote_url(&root);
    let repo_slug = remote_url
        .as_deref()
        .and_then(parse_github_repo_slug)
        .or_else(|| {
            root.file_name()
                .map(|name| format!("unknown/{}", name.to_string_lossy()))
        });

    Ok(GoProject {
        name,
        path: root.clone(),
        remote_url,
        repo_slug,
        has_agents_md: root.join("AGENTS.md").exists(),
        has_claude_md: root.join("CLAUDE.md").exists(),
        has_codex_md: root.join("CODEX.md").exists(),
        tech: scan_info.map(|info| info.tech).unwrap_or_default(),
        provider: detect_provider_hint(&root),
    })
}

pub fn ensure_tmux_session(project: &GoProject, panes: usize) -> Result<GoSession> {
    let pane_count = panes.clamp(1, 9);
    let session_name = format!("dx-go-{}", slugify(&project.name));
    let window_name = "issues".to_string();
    let created = if tmux_session_exists(&session_name) {
        false
    } else {
        create_tmux_session(&session_name, &window_name, &project.path)?;
        true
    };

    ensure_tmux_window(&session_name, &window_name, &project.path, pane_count)?;
    let pane_targets = list_pane_targets(&session_name, &window_name)?;
    if pane_targets.len() < pane_count {
        bail!(
            "tmux session '{}' has {} panes, expected at least {}",
            session_name,
            pane_targets.len(),
            pane_count
        );
    }

    Ok(GoSession {
        session_name,
        window_name,
        pane_targets,
        created,
    })
}

pub fn fetch_github_issues(project: &GoProject, limit: usize) -> Result<Vec<GitHubIssue>> {
    let repo_slug = project
        .repo_slug
        .clone()
        .ok_or_else(|| anyhow!("could not determine GitHub repo slug from git remote"))?;

    let output = Command::new("gh")
        .current_dir(&project.path)
        .args([
            "issue",
            "list",
            "--repo",
            &repo_slug,
            "--state",
            "open",
            "--limit",
            &limit.to_string(),
            "--json",
            "number,title,url,labels",
        ])
        .output()
        .with_context(|| format!("run gh issue list for {}", repo_slug))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!(
            "gh issue list failed for {}: {}",
            repo_slug,
            if stderr.is_empty() {
                "unknown error"
            } else {
                stderr.as_str()
            }
        );
    }

    let mut issues: Vec<GitHubIssue> =
        serde_json::from_slice(&output.stdout).context("parse gh issue list output")?;
    if issues.is_empty() {
        issues.push(GitHubIssue {
            number: 0,
            title: "Triage repository and identify the highest-value next issue.".to_string(),
            url: format!(
                "https://github.com/{}",
                project.repo_slug.as_deref().unwrap_or("unknown/unknown")
            ),
            labels: Vec::new(),
        });
    }
    Ok(issues)
}

pub fn find_free_port(start: u16) -> Result<u16> {
    for port in start..=start.saturating_add(200) {
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return Ok(port);
        }
    }
    bail!("no free port available starting at {}", start);
}

pub fn spawn_agent_for_issue(
    app: &App,
    session: &GoProject,
    pane_target: &str,
    issue: &GitHubIssue,
) -> Result<()> {
    let mission = issue_mission(issue);
    let prompt = build_issue_prompt(session, issue);
    let plan = runtime_broker::plan_tmux_launch(
        &session.provider,
        &format!("go-{}", slugify(&session.name)),
        &session.path.to_string_lossy(),
        &prompt,
        true,
        None,
    )?;

    reset_pane(pane_target)?;
    send_literal_command(
        pane_target,
        &format!("cd {}", shell_escape(&session.path.to_string_lossy())),
    )?;
    send_literal_command(
        pane_target,
        &format!(
            "export DX_PROJECT={} DX_GO_ISSUE={} DX_GO_URL={} DX_PROVIDER={}",
            shell_escape(&session.name),
            issue.number,
            shell_escape(&issue.url),
            shell_escape(&session.provider),
        ),
    )?;
    send_literal_command(pane_target, &plan.command)?;

    let queue_task = queue::add_task(&session.name, "developer", &mission, &prompt, 1, Vec::new())?;
    queue::mark_running(&queue_task.id, 0)?;
    queue::set_tmux_target(&queue_task.id, pane_target)?;

    let controller = Arc::clone(&app.session_controller);
    let pane = pane_target.to_string();
    let mission_for_controller = mission;
    let agent_type = provider_agent_type(&session.provider);
    tokio::spawn(async move {
        if let Err(error) = controller
            .start(pane.clone(), mission_for_controller, agent_type)
            .await
        {
            tracing::warn!(pane = %pane, error = %error, "failed to start session controller");
        }
    });

    Ok(())
}

fn git_root() -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
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

fn detect_provider_hint(root: &Path) -> String {
    if root.join("CLAUDE.md").exists() {
        "claude".to_string()
    } else if root.join("CODEX.md").exists() {
        "codex".to_string()
    } else {
        "claude".to_string()
    }
}

fn tmux_session_exists(session: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", session])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn create_tmux_session(session: &str, window: &str, cwd: &Path) -> Result<()> {
    let status = Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            session,
            "-n",
            window,
            "-c",
            &cwd.to_string_lossy(),
        ])
        .status()
        .with_context(|| format!("create tmux session '{}'", session))?;
    if !status.success() {
        bail!("tmux new-session failed for '{}'", session);
    }
    Ok(())
}

fn ensure_tmux_window(session: &str, window: &str, cwd: &Path, panes: usize) -> Result<()> {
    if !tmux_window_exists(session, window) {
        let status = Command::new("tmux")
            .args([
                "new-window",
                "-t",
                session,
                "-n",
                window,
                "-c",
                &cwd.to_string_lossy(),
            ])
            .status()
            .with_context(|| format!("create tmux window '{}:{}'", session, window))?;
        if !status.success() {
            bail!("tmux new-window failed for '{}:{}'", session, window);
        }
    }

    let target = format!("{session}:{window}");
    let current_count = list_pane_targets(session, window)?.len();
    if current_count > panes {
        bail!(
            "existing tmux window '{}:{}' has {} panes, expected {} or fewer",
            session,
            window,
            current_count,
            panes
        );
    }
    for _ in current_count..panes {
        let status = Command::new("tmux")
            .args([
                "split-window",
                "-h",
                "-t",
                &target,
                "-c",
                &cwd.to_string_lossy(),
            ])
            .status()
            .with_context(|| format!("split tmux window '{}'", target))?;
        if !status.success() {
            bail!("tmux split-window failed for '{}'", target);
        }
    }

    let status = Command::new("tmux")
        .args(["select-layout", "-t", &target, "even-horizontal"])
        .status()
        .with_context(|| format!("layout tmux window '{}'", target))?;
    if !status.success() {
        bail!("tmux select-layout failed for '{}'", target);
    }

    Ok(())
}

fn tmux_window_exists(session: &str, window: &str) -> bool {
    let output = match Command::new("tmux")
        .args(["list-windows", "-t", session, "-F", "#{window_name}"])
        .output()
    {
        Ok(output) => output,
        Err(_) => return false,
    };

    if !output.status.success() {
        return false;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| line.trim() == window)
}

fn list_pane_targets(session: &str, window: &str) -> Result<Vec<String>> {
    let output = Command::new("tmux")
        .args([
            "list-panes",
            "-t",
            &format!("{session}:{window}"),
            "-F",
            "#{session_name}:#{window_name}.#{pane_index}",
        ])
        .output()
        .with_context(|| format!("list panes for '{}:{}'", session, window))?;
    if !output.status.success() {
        bail!("tmux list-panes failed for '{}:{}'", session, window);
    }

    let panes = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    Ok(panes)
}

fn reset_pane(pane_target: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["send-keys", "-t", pane_target, "C-c"])
        .status()
        .with_context(|| format!("reset pane '{}'", pane_target))?;
    if !status.success() {
        bail!("tmux send-keys C-c failed for '{}'", pane_target);
    }
    send_literal_command(pane_target, "clear")
}

fn send_literal_command(pane_target: &str, command: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["send-keys", "-t", pane_target, "-l", command])
        .status()
        .with_context(|| format!("type command into '{}'", pane_target))?;
    if !status.success() {
        bail!("tmux send-keys -l failed for '{}'", pane_target);
    }

    let enter = Command::new("tmux")
        .args(["send-keys", "-t", pane_target, "Enter"])
        .status()
        .with_context(|| format!("submit command in '{}'", pane_target))?;
    if !enter.success() {
        bail!("tmux send-keys Enter failed for '{}'", pane_target);
    }
    Ok(())
}

fn shell_escape(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn issue_mission(issue: &GitHubIssue) -> String {
    if issue.number == 0 {
        issue.title.clone()
    } else {
        format!("Fix issue #{}: {}", issue.number, issue.title)
    }
}

fn build_issue_prompt(project: &GoProject, issue: &GitHubIssue) -> String {
    let mut prompt = vec![
        format!("Project: {}", project.name),
        format!("Repository root: {}", project.path.display()),
        format!("Issue URL: {}", issue.url),
        format!("Task: {}", issue_mission(issue)),
        "Requirements: inspect the repository state first, implement the fix, run the relevant tests, and summarize the result.".to_string(),
    ];
    if !issue.labels.is_empty() {
        let labels = issue
            .labels
            .iter()
            .map(|label| label.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        prompt.push(format!("Labels: {}", labels));
    }
    if project.has_agents_md {
        prompt.push("Project guidance file: AGENTS.md".to_string());
    }
    if project.has_claude_md {
        prompt.push("Provider overlay: CLAUDE.md".to_string());
    }
    if project.has_codex_md {
        prompt.push("Provider overlay: CODEX.md".to_string());
    }
    prompt.join("\n")
}

fn provider_agent_type(provider: &str) -> AgentType {
    match runtime_broker::normalize_provider_id(provider) {
        "claude" => AgentType::ClaudeCode,
        "codex" => AgentType::CodexCli,
        "gemini" => AgentType::GeminiCli,
        "opencode" => AgentType::OpenCode,
        _ => AgentType::Unknown,
    }
}

fn slugify(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    let mut last_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

fn issue_value(issue: &GitHubIssue) -> serde_json::Value {
    json!({
        "number": issue.number,
        "title": issue.title,
        "url": issue.url,
        "labels": issue.labels.iter().map(|label| label.name.clone()).collect::<Vec<_>>(),
    })
}

fn go_project_value(project: &GoProject) -> serde_json::Value {
    json!({
        "name": project.name,
        "path": project.path,
        "remote_url": project.remote_url,
        "repo_slug": project.repo_slug,
        "has_agents_md": project.has_agents_md,
        "has_claude_md": project.has_claude_md,
        "has_codex_md": project.has_codex_md,
        "tech": project.tech,
        "provider": project.provider,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_https_remote() {
        assert_eq!(
            parse_github_repo_slug("https://github.com/pdaxt/dx-terminal.git"),
            Some("pdaxt/dx-terminal".to_string())
        );
    }

    #[test]
    fn parses_ssh_remote() {
        assert_eq!(
            parse_github_repo_slug("git@github.com:pdaxt/dx-terminal.git"),
            Some("pdaxt/dx-terminal".to_string())
        );
    }

    #[test]
    fn slugifies_names() {
        assert_eq!(slugify("DX Terminal"), "dx-terminal");
        assert_eq!(slugify("repo_name"), "repo-name");
    }

    #[test]
    fn issue_prompt_includes_labels() {
        let project = GoProject {
            name: "dx-terminal".to_string(),
            path: PathBuf::from("/tmp/dx-terminal"),
            remote_url: None,
            repo_slug: Some("pdaxt/dx-terminal".to_string()),
            has_agents_md: true,
            has_claude_md: true,
            has_codex_md: false,
            tech: vec!["rust".to_string()],
            provider: "claude".to_string(),
        };
        let issue = GitHubIssue {
            number: 12,
            title: "Add dx go".to_string(),
            url: "https://github.com/pdaxt/dx-terminal/issues/12".to_string(),
            labels: vec![GitHubLabel {
                name: "enhancement".to_string(),
            }],
        };
        let prompt = build_issue_prompt(&project, &issue);
        assert!(prompt.contains("Issue URL"));
        assert!(prompt.contains("enhancement"));
        assert!(prompt.contains("AGENTS.md"));
    }
}

use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{Context, Result};

use crate::config;

pub struct WorkspaceInfo {
    pub worktree_path: String,
    pub branch_name: String,
    pub base_branch: String,
}

/// Check if a directory is inside a git repository
pub fn is_git_repo(project_path: &str) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(project_path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get the current branch name (or "main" as fallback)
fn current_branch(project_path: &str) -> String {
    Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(project_path)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "main".into())
}

/// Convert a task description into a branch-safe slug
fn task_slug(task: &str) -> String {
    let slug: String = task
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect();
    // Collapse multiple dashes, strip leading/trailing
    let mut result = String::new();
    let mut prev_dash = false;
    for c in slug.chars() {
        if c == '-' {
            if !prev_dash && !result.is_empty() {
                result.push('-');
            }
            prev_dash = true;
        } else {
            result.push(c);
            prev_dash = false;
        }
    }
    let trimmed = result.trim_end_matches('-');
    trimmed.chars().take(40).collect::<String>().trim_end_matches('-').to_string()
}

/// Workspace root directory
fn workspaces_root() -> PathBuf {
    config::agentos_root().join("workspaces")
}

/// Create a git worktree for a pane working on a project
pub fn create_worktree(project_path: &str, pane_num: u8, task: &str) -> Result<WorkspaceInfo> {
    let project_name = Path::new(project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".into());

    let slug = task_slug(task);
    let branch_name = format!("pane-{}/{}", pane_num, if slug.is_empty() { "work" } else { &slug });

    let worktree_dir = workspaces_root()
        .join(format!("pane-{}", pane_num))
        .join(&project_name);

    // Ensure parent exists
    if let Some(parent) = worktree_dir.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Remove existing worktree at this path if it exists
    if worktree_dir.exists() {
        let _ = Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(&worktree_dir)
            .current_dir(project_path)
            .output();
        // If git worktree remove fails, force-remove the directory
        if worktree_dir.exists() {
            let _ = std::fs::remove_dir_all(&worktree_dir);
        }
    }

    // Delete the branch if it already exists (stale from previous run)
    let _ = Command::new("git")
        .args(["branch", "-D", &branch_name])
        .current_dir(project_path)
        .output();

    let base_branch = current_branch(project_path);

    // Create worktree with new branch
    let output = Command::new("git")
        .args(["worktree", "add", "-b", &branch_name])
        .arg(&worktree_dir)
        .arg(&base_branch)
        .current_dir(project_path)
        .output()
        .context("Failed to run git worktree add")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git worktree add failed: {}", stderr.trim());
    }

    Ok(WorkspaceInfo {
        worktree_path: worktree_dir.to_string_lossy().to_string(),
        branch_name,
        base_branch,
    })
}

/// Remove a git worktree and optionally prune
pub fn remove_worktree(project_path: &str, worktree_path: &str) -> Result<()> {
    let wp = Path::new(worktree_path);
    if !wp.exists() {
        return Ok(());
    }

    let output = Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(worktree_path)
        .current_dir(project_path)
        .output()
        .context("Failed to run git worktree remove")?;

    if !output.status.success() {
        // Force-remove directory as fallback
        let _ = std::fs::remove_dir_all(worktree_path);
    }

    // Prune stale worktree entries
    let _ = Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(project_path)
        .output();

    Ok(())
}

/// Stage all changes and commit in a worktree
pub fn commit_all(worktree_path: &str, message: &str) -> Result<String> {
    // Stage everything
    let _ = Command::new("git")
        .args(["add", "-A"])
        .current_dir(worktree_path)
        .output();

    // Check if there's anything to commit
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(worktree_path)
        .output()?;
    let status_text = String::from_utf8_lossy(&status.stdout).trim().to_string();
    if status_text.is_empty() {
        return Ok("nothing to commit".into());
    }

    let msg = if message.is_empty() { "AgentOS: work in progress" } else { message };
    let output = Command::new("git")
        .args(["commit", "-m", msg])
        .current_dir(worktree_path)
        .output()
        .context("Failed to run git commit")?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git commit failed: {}", stderr.trim());
    }
}

/// Push a branch to remote
pub fn push_branch(worktree_path: &str, branch: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["push", "-u", "origin", branch])
        .current_dir(worktree_path)
        .output()
        .context("Failed to run git push")?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stderr).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Not fatal — remote might not exist
        Ok(format!("push failed (non-fatal): {}", stderr.trim()))
    }
}

/// Create a pull request using gh CLI
pub fn create_pr(worktree_path: &str, title: &str, body: &str) -> Result<String> {
    let output = Command::new("gh")
        .args(["pr", "create", "--title", title, "--body", body])
        .current_dir(worktree_path)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            Ok(format!("PR creation failed (non-fatal): {}", stderr.trim()))
        }
        Err(_) => Ok("gh CLI not available — PR not created".into()),
    }
}

/// Enable auto-merge on a PR (squash). pr_url is the URL from create_pr().
pub fn auto_merge_pr(worktree_path: &str, pr_url: &str) -> Result<String> {
    if pr_url.is_empty() || !pr_url.contains("github.com") {
        return Ok("no PR to auto-merge".into());
    }
    let output = Command::new("gh")
        .args(["pr", "merge", "--auto", "--squash", pr_url])
        .current_dir(worktree_path)
        .output();
    match output {
        Ok(o) if o.status.success() => Ok("auto-merge enabled".into()),
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            Ok(format!("auto-merge failed (non-fatal): {}", stderr.trim()))
        }
        Err(_) => Ok("gh CLI not available — auto-merge skipped".into()),
    }
}

/// Get git status for a worktree
pub fn git_status(worktree_path: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["status", "--short"])
        .current_dir(worktree_path)
        .output()
        .context("Failed to run git status")?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Get git diff for a worktree
pub fn git_diff(worktree_path: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["diff", "--stat"])
        .current_dir(worktree_path)
        .output()
        .context("Failed to run git diff")?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Sync worktree with latest from base branch (rebase)
pub fn sync_from_main(worktree_path: &str, base_branch: &str) -> Result<String> {
    // Fetch latest
    let _ = Command::new("git")
        .args(["fetch", "origin", base_branch])
        .current_dir(worktree_path)
        .output();

    // Rebase onto latest base
    let output = Command::new("git")
        .args(["rebase", &format!("origin/{}", base_branch)])
        .current_dir(worktree_path)
        .output()
        .context("Failed to run git rebase")?;

    if output.status.success() {
        Ok("synced".into())
    } else {
        // Abort failed rebase
        let _ = Command::new("git")
            .args(["rebase", "--abort"])
            .current_dir(worktree_path)
            .output();
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(format!("rebase failed (aborted): {}", stderr.trim()))
    }
}

/// Merge a branch into base using rebase + fast-forward merge
pub fn merge_branch(project_path: &str, branch: &str, base_branch: &str) -> Result<String> {
    // Fetch latest
    let _ = Command::new("git")
        .args(["fetch", "origin"])
        .current_dir(project_path)
        .output();

    // Checkout base branch and pull latest
    let checkout = Command::new("git")
        .args(["checkout", base_branch])
        .current_dir(project_path)
        .output()
        .context("Failed to checkout base branch")?;
    if !checkout.status.success() {
        let stderr = String::from_utf8_lossy(&checkout.stderr);
        anyhow::bail!("checkout {} failed: {}", base_branch, stderr.trim());
    }

    let _ = Command::new("git")
        .args(["pull", "--ff-only", "origin", base_branch])
        .current_dir(project_path)
        .output();

    // Merge the feature branch (--no-ff to preserve history)
    let merge = Command::new("git")
        .args(["merge", "--no-ff", branch, "-m", &format!("Merge branch '{}'", branch)])
        .current_dir(project_path)
        .output()
        .context("Failed to merge branch")?;

    if !merge.status.success() {
        // Abort failed merge
        let _ = Command::new("git")
            .args(["merge", "--abort"])
            .current_dir(project_path)
            .output();
        let stderr = String::from_utf8_lossy(&merge.stderr);
        anyhow::bail!("merge failed (aborted): {}", stderr.trim());
    }

    // Push merged base
    let push = Command::new("git")
        .args(["push", "origin", base_branch])
        .current_dir(project_path)
        .output()
        .context("Failed to push merged base")?;

    let push_status = if push.status.success() {
        "pushed".to_string()
    } else {
        format!("push failed: {}", String::from_utf8_lossy(&push.stderr).trim())
    };

    // Delete the merged branch locally and remotely
    let _ = Command::new("git")
        .args(["branch", "-d", branch])
        .current_dir(project_path)
        .output();
    let _ = Command::new("git")
        .args(["push", "origin", "--delete", branch])
        .current_dir(project_path)
        .output();

    Ok(format!("merged {} into {}, {}", branch, base_branch, push_status))
}

/// Clean up stale worktrees from crashed sessions
pub fn cleanup_stale_worktrees() -> Result<Vec<String>> {
    let root = workspaces_root();
    if !root.exists() {
        return Ok(vec![]);
    }

    let mut cleaned = Vec::new();

    // Walk pane-N directories
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        let pane_dir = entry.path();
        if !pane_dir.is_dir() {
            continue;
        }

        let dir_name = pane_dir.file_name().unwrap_or_default().to_string_lossy().to_string();
        if !dir_name.starts_with("pane-") {
            continue;
        }

        // Check each project worktree inside
        if let Ok(projects) = std::fs::read_dir(&pane_dir) {
            for project_entry in projects.flatten() {
                let ws_path = project_entry.path();
                if ws_path.is_dir() {
                    // If it's a valid git worktree, try to find its main repo and remove
                    let git_dir = ws_path.join(".git");
                    if git_dir.exists() {
                        // Read the .git file to find the main repo
                        if let Ok(content) = std::fs::read_to_string(&git_dir) {
                            if let Some(main_git) = content.strip_prefix("gitdir: ") {
                                let main_git = main_git.trim();
                                // Navigate up from .git/worktrees/xxx to the repo root
                                if let Some(repo_root) = Path::new(main_git)
                                    .parent() // worktrees/xxx
                                    .and_then(|p| p.parent()) // worktrees
                                    .and_then(|p| p.parent()) // .git
                                {
                                    let _ = Command::new("git")
                                        .args(["worktree", "remove", "--force"])
                                        .arg(&ws_path)
                                        .current_dir(repo_root)
                                        .output();
                                }
                            }
                        }
                    }
                    // Force-remove if still exists
                    if ws_path.exists() {
                        let _ = std::fs::remove_dir_all(&ws_path);
                    }
                    cleaned.push(ws_path.to_string_lossy().to_string());
                }
            }
        }

        // Remove empty pane directory
        if pane_dir.read_dir().map(|mut d| d.next().is_none()).unwrap_or(true) {
            let _ = std::fs::remove_dir(&pane_dir);
        }
    }

    Ok(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_slug() {
        assert_eq!(task_slug("Build login page"), "build-login-page");
        assert_eq!(task_slug("fix  bug #123"), "fix-bug-123");
        assert_eq!(task_slug(""), "");
        assert_eq!(task_slug("---hello---world---"), "hello-world");
        assert_eq!(task_slug("A"), "a");
    }

    #[test]
    fn test_task_slug_truncation() {
        let long = "a".repeat(60);
        let slug = task_slug(&long);
        assert!(slug.len() <= 40);
    }
}

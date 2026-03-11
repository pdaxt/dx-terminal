use std::path::{Path, PathBuf};
use std::process::Command;

/// Git status summary
pub struct GitStatus {
    pub branch: String,
    pub dirty_count: usize,
    pub ahead: usize,
    pub behind: usize,
    pub has_remote: bool,
}

/// Get git status for a repository
pub fn status(root: &Path) -> anyhow::Result<GitStatus> {
    let branch = current_branch(root).unwrap_or_else(|| "detached".into());

    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output()?;
    let dirty_count = String::from_utf8_lossy(&dirty.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .count();

    let has_remote = Command::new("git")
        .args(["remote"])
        .current_dir(root)
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);

    let (ahead, behind) = if has_remote {
        get_ahead_behind(root, &branch)
    } else {
        (0, 0)
    };

    Ok(GitStatus {
        branch,
        dirty_count,
        ahead,
        behind,
        has_remote,
    })
}

/// Get current branch name
pub fn current_branch(root: &Path) -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(root)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
}

/// Get ahead/behind counts relative to remote tracking branch
fn get_ahead_behind(root: &Path, branch: &str) -> (usize, usize) {
    let output = Command::new("git")
        .args(["rev-list", "--left-right", "--count", &format!("{}...origin/{}", branch, branch)])
        .current_dir(root)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            let parts: Vec<&str> = text.trim().split('\t').collect();
            if parts.len() == 2 {
                let ahead = parts[0].parse().unwrap_or(0);
                let behind = parts[1].parse().unwrap_or(0);
                (ahead, behind)
            } else {
                (0, 0)
            }
        }
        _ => (0, 0),
    }
}

/// Auto-commit changed files with a smart commit message.
/// Returns Some((sha, message, file_count)) on success, None if nothing to commit.
pub fn auto_commit(
    root: &Path,
    changed_files: &[PathBuf],
) -> anyhow::Result<Option<(String, String, usize)>> {
    // Check if there are actually uncommitted changes
    let status_out = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output()?;

    let dirty_files: Vec<String> = String::from_utf8_lossy(&status_out.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l[3..].to_string())
        .collect();

    if dirty_files.is_empty() {
        return Ok(None);
    }

    // Stage all changes
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(root)
        .output()?;

    // Generate smart commit message
    let message = generate_commit_message(&dirty_files, changed_files);
    let file_count = dirty_files.len();

    // Commit
    let commit = Command::new("git")
        .args(["commit", "-m", &message])
        .current_dir(root)
        .output()?;

    if !commit.status.success() {
        let err = String::from_utf8_lossy(&commit.stderr);
        if err.contains("nothing to commit") {
            return Ok(None);
        }
        anyhow::bail!("git commit failed: {}", err);
    }

    // Get the commit SHA
    let sha = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(root)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    Ok(Some((sha, message, file_count)))
}

/// Push to remote (blocking)
pub fn push(root: &Path) -> anyhow::Result<()> {
    let output = Command::new("git")
        .args(["push"])
        .current_dir(root)
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git push failed: {}", err)
    }
}

/// Generate a smart commit message based on changed file paths
fn generate_commit_message(dirty_files: &[String], _trigger_files: &[PathBuf]) -> String {
    let has_vision = dirty_files.iter().any(|f| f.contains(".vision/"));
    let has_src = dirty_files.iter().any(|f| f.contains("src/"));
    let has_assets = dirty_files.iter().any(|f| f.contains("assets/"));
    let has_config = dirty_files.iter().any(|f| {
        f.contains("Cargo.toml") || f.contains("package.json") || f.contains("CLAUDE.md")
    });
    let has_hooks = dirty_files.iter().any(|f| f.contains(".claude/"));

    let mut parts = Vec::new();
    if has_vision { parts.push("vision"); }
    if has_src { parts.push("src"); }
    if has_assets { parts.push("assets"); }
    if has_config { parts.push("config"); }
    if has_hooks { parts.push("hooks"); }

    let scope = if parts.is_empty() {
        "sync".to_string()
    } else {
        parts[..std::cmp::min(parts.len(), 3)].join(",")
    };

    let count = dirty_files.len();
    format!("chore({}): auto-sync {} file{}", scope, count, if count == 1 { "" } else { "s" })
}

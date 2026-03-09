use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config;

/// Registry of all discovered local projects
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectRegistry {
    pub projects: Vec<ProjectInfo>,
    pub last_scan: String,
}

/// Metadata for a single discovered project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: String,
    pub tech: Vec<String>,
    #[serde(default)]
    pub build_cmd: Option<String>,
    #[serde(default)]
    pub test_cmd: Option<String>,
    #[serde(default)]
    pub lint_cmd: Option<String>,
    pub has_ci: bool,
    #[serde(default)]
    pub git_remote: Option<String>,
    #[serde(default)]
    pub default_branch: Option<String>,
    pub git_dirty: bool,
    pub git_ahead: i32,
    pub git_behind: i32,
    #[serde(default)]
    pub last_commit_ts: Option<String>,
    #[serde(default)]
    pub last_commit_msg: Option<String>,
    #[serde(default)]
    pub readme_summary: Option<String>,
    pub loc: u64,
    #[serde(default)]
    pub deps: Vec<String>,
    pub last_scanned: String,
}

fn registry_path() -> PathBuf {
    config::dx_root().join("projects.json")
}

/// Load registry from disk (returns empty if file missing/corrupt)
pub fn load_registry() -> ProjectRegistry {
    let path = registry_path();
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(reg) = serde_json::from_str(&content) {
                return reg;
            }
        }
    }
    ProjectRegistry::default()
}

/// Save registry to disk (tmp + rename)
pub fn save_registry(reg: &ProjectRegistry) {
    let path = registry_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("tmp");
    if let Ok(json) = serde_json::to_string_pretty(reg) {
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

/// Scan all configured directories for git repos
pub fn scan_all() -> ProjectRegistry {
    let dirs = scan_dirs();
    let mut projects = Vec::new();

    for dir in &dirs {
        if !dir.exists() { continue; }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if !p.is_dir() { continue; }
                // Skip hidden directories
                if entry.file_name().to_string_lossy().starts_with('.') { continue; }
                // Must be a git repo
                if !p.join(".git").exists() { continue; }
                if let Some(info) = scan_single(&p) {
                    projects.push(info);
                }
            }
        }
    }

    projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    let reg = ProjectRegistry {
        projects,
        last_scan: crate::multi_agent::now_iso(),
    };
    save_registry(&reg);
    reg
}

/// Scan a single project directory
pub fn scan_single(path: &Path) -> Option<ProjectInfo> {
    let dir_name = path.file_name()?.to_string_lossy().to_string();
    // Prefer package name from manifest over directory name
    let name = package_name(path).unwrap_or(dir_name);
    let abs_path = path.to_string_lossy().to_string();

    let (tech, build_cmd, test_cmd, lint_cmd) = detect_tech(path);
    let has_ci = path.join(".github").join("workflows").exists()
        || path.join(".gitlab-ci.yml").exists();

    let git = git_info(path);
    let readme = readme_summary(path);
    let loc = estimate_loc(path);

    Some(ProjectInfo {
        name,
        path: abs_path,
        tech,
        build_cmd,
        test_cmd,
        lint_cmd,
        has_ci,
        git_remote: git.remote,
        default_branch: git.branch,
        git_dirty: git.dirty,
        git_ahead: git.ahead,
        git_behind: git.behind,
        last_commit_ts: git.last_commit_ts,
        last_commit_msg: git.last_commit_msg,
        readme_summary: readme,
        loc,
        deps: Vec::new(),
        last_scanned: crate::multi_agent::now_iso(),
    })
}

/// Look up a project by name (case-insensitive)
pub fn project_by_name(name: &str) -> Option<ProjectInfo> {
    let reg = load_registry();
    let lower = name.to_lowercase();
    reg.projects.into_iter().find(|p| {
        p.name.to_lowercase() == lower || p.name.to_lowercase().contains(&lower)
    })
}

/// Full project detail with cross-references to quality, tracker, agents
pub fn project_detail(name: &str) -> Value {
    let info = match project_by_name(name) {
        Some(i) => i,
        None => return json!({"error": format!("Project '{}' not found in registry", name)}),
    };

    let health = crate::quality::project_health(&info.name);
    let gate = crate::quality::quality_gate(&info.name);

    // Count open issues from tracker
    let issues = crate::tracker::load_issues(&info.name);
    let open_issues = issues.iter().filter(|i| {
        let status = i.get("status").and_then(|v| v.as_str()).unwrap_or("");
        status != "done" && status != "closed"
    }).count();
    let total_issues = issues.len();

    // Count active agents
    let agents = crate::multi_agent::agent_list(Some(&info.name));
    let active_agents = agents.get("count").and_then(|v| v.as_i64()).unwrap_or(0);

    json!({
        "name": info.name,
        "path": info.path,
        "tech": info.tech,
        "build_cmd": info.build_cmd,
        "test_cmd": info.test_cmd,
        "lint_cmd": info.lint_cmd,
        "has_ci": info.has_ci,
        "git": {
            "remote": info.git_remote,
            "branch": info.default_branch,
            "dirty": info.git_dirty,
            "ahead": info.git_ahead,
            "behind": info.git_behind,
            "last_commit_ts": info.last_commit_ts,
            "last_commit_msg": info.last_commit_msg,
        },
        "health": health,
        "quality_gate": gate,
        "issues": { "open": open_issues, "total": total_issues },
        "active_agents": active_agents,
        "readme_summary": info.readme_summary,
        "loc": info.loc,
        "deps": info.deps,
        "last_scanned": info.last_scanned,
    })
}

// --- Detection ---

/// Extract package name from Cargo.toml or package.json
fn package_name(path: &Path) -> Option<String> {
    // Rust: Cargo.toml [package] name
    let cargo = path.join("Cargo.toml");
    if cargo.exists() {
        if let Ok(content) = std::fs::read_to_string(&cargo) {
            if let Ok(parsed) = content.parse::<toml::Table>() {
                if let Some(pkg) = parsed.get("package").and_then(|p| p.as_table()) {
                    if let Some(name) = pkg.get("name").and_then(|n| n.as_str()) {
                        return Some(name.to_string());
                    }
                }
            }
        }
    }
    // Node: package.json name
    let pkg_json = path.join("package.json");
    if pkg_json.exists() {
        if let Ok(content) = std::fs::read_to_string(&pkg_json) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(name) = parsed.get("name").and_then(|n| n.as_str()) {
                    return Some(name.to_string());
                }
            }
        }
    }
    None
}

fn detect_tech(path: &Path) -> (Vec<String>, Option<String>, Option<String>, Option<String>) {
    let mut tech = Vec::new();
    let mut build_cmd = None;
    let mut test_cmd = None;
    let mut lint_cmd = None;

    // Rust
    if path.join("Cargo.toml").exists() {
        tech.push("rust".to_string());
        build_cmd = Some("cargo build --release".to_string());
        test_cmd = Some("cargo test".to_string());
        lint_cmd = Some("cargo clippy -- -D warnings".to_string());
    }

    // Node / TypeScript
    if path.join("package.json").exists() {
        tech.push("node".to_string());
        if path.join("tsconfig.json").exists() {
            tech.push("typescript".to_string());
        }
        // Try to parse package.json for scripts
        if let Ok(content) = std::fs::read_to_string(path.join("package.json")) {
            if let Ok(pkg) = serde_json::from_str::<Value>(&content) {
                if let Some(scripts) = pkg.get("scripts").and_then(|v| v.as_object()) {
                    if let Some(b) = scripts.get("build").and_then(|v| v.as_str()) {
                        build_cmd = build_cmd.or(Some(format!("npm run build")));
                        // If it's next.js
                        if b.contains("next") { tech.push("nextjs".to_string()); }
                    }
                    if let Some(t) = scripts.get("test").and_then(|v| v.as_str()) {
                        if t != "echo \"Error: no test specified\" && exit 1" {
                            test_cmd = test_cmd.or(Some("npm test".to_string()));
                        }
                    }
                    if scripts.contains_key("lint") {
                        lint_cmd = lint_cmd.or(Some("npm run lint".to_string()));
                    }
                }
            }
        }
    }

    // Python
    if path.join("pyproject.toml").exists() || path.join("setup.py").exists()
        || path.join("requirements.txt").exists()
    {
        tech.push("python".to_string());
        if path.join("tests").exists() || path.join("test").exists() {
            test_cmd = test_cmd.or(Some("pytest".to_string()));
        }
    }

    // Go
    if path.join("go.mod").exists() {
        tech.push("go".to_string());
        build_cmd = build_cmd.or(Some("go build ./...".to_string()));
        test_cmd = test_cmd.or(Some("go test ./...".to_string()));
    }

    // Makefile (fallback)
    if path.join("Makefile").exists() && build_cmd.is_none() {
        build_cmd = Some("make".to_string());
    }

    (tech, build_cmd, test_cmd, lint_cmd)
}

struct GitStatus {
    remote: Option<String>,
    branch: Option<String>,
    dirty: bool,
    ahead: i32,
    behind: i32,
    last_commit_ts: Option<String>,
    last_commit_msg: Option<String>,
}

fn git_info(path: &Path) -> GitStatus {
    let run = |args: &[&str]| -> Option<String> {
        Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    };

    let remote = run(&["remote", "get-url", "origin"]);
    let branch = run(&["rev-parse", "--abbrev-ref", "HEAD"]);

    let dirty = run(&["status", "--porcelain"])
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    let (ahead, behind) = run(&["rev-list", "--left-right", "--count", "HEAD...@{u}"])
        .and_then(|s| {
            let parts: Vec<&str> = s.split_whitespace().collect();
            if parts.len() == 2 {
                Some((parts[0].parse().unwrap_or(0), parts[1].parse().unwrap_or(0)))
            } else {
                None
            }
        })
        .unwrap_or((0, 0));

    let (last_commit_ts, last_commit_msg) = run(&["log", "-1", "--format=%cI|%s"])
        .map(|s| {
            let parts: Vec<&str> = s.splitn(2, '|').collect();
            (
                parts.first().map(|s| s.to_string()),
                parts.get(1).map(|s| s.to_string()),
            )
        })
        .unwrap_or((None, None));

    GitStatus { remote, branch, dirty, ahead, behind, last_commit_ts, last_commit_msg }
}

fn readme_summary(path: &Path) -> Option<String> {
    for name in &["README.md", "README", "readme.md"] {
        let readme_path = path.join(name);
        if readme_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&readme_path) {
                let summary: String = content.lines()
                    .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" ");
                if !summary.is_empty() {
                    return Some(if summary.len() > 200 {
                        let end = summary.char_indices()
                            .take_while(|&(i, _)| i <= 197)
                            .last()
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        format!("{}...", &summary[..end])
                    } else {
                        summary
                    });
                }
            }
        }
    }
    None
}

fn estimate_loc(path: &Path) -> u64 {
    // Quick estimate: count files in src/ or root, multiply by avg lines
    let mut count: u64 = 0;
    let src_dir = if path.join("src").exists() { path.join("src") } else { path.to_path_buf() };

    fn count_files(dir: &Path, count: &mut u64, depth: u8) {
        if depth > 4 { return; } // limit depth
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name == "node_modules" || name == "target"
                    || name == "__pycache__" || name == "dist" || name == "build" { continue; }
                if p.is_dir() {
                    count_files(&p, count, depth + 1);
                } else {
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if matches!(ext, "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "java" | "rb" | "swift") {
                        // Estimate ~50 lines per file on average
                        *count += 50;
                    }
                }
            }
        }
    }

    count_files(&src_dir, &mut count, 0);
    count
}

/// Get configurable scan directories
fn scan_dirs() -> Vec<PathBuf> {
    let cfg = config::get();
    if cfg.scan_dirs.is_empty() {
        vec![config::projects_dir()]
    } else {
        cfg.scan_dirs.iter().map(|d| {
            let expanded = d.replace("~", &config::home_dir().to_string_lossy());
            PathBuf::from(expanded)
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_roundtrip() {
        let reg = ProjectRegistry {
            projects: vec![ProjectInfo {
                name: "test".into(), path: "/tmp/test".into(), tech: vec!["rust".into()],
                build_cmd: Some("cargo build".into()), test_cmd: Some("cargo test".into()),
                lint_cmd: None, has_ci: false, git_remote: None, default_branch: None,
                git_dirty: false, git_ahead: 0, git_behind: 0, last_commit_ts: None,
                last_commit_msg: None, readme_summary: None, loc: 100, deps: vec![],
                last_scanned: "2026-01-01T00:00:00Z".into(),
            }],
            last_scan: "2026-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&reg).unwrap();
        let parsed: ProjectRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.projects.len(), 1);
        assert_eq!(parsed.projects[0].name, "test");
    }

    #[test]
    fn test_scan_dx_terminal_itself() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"));
        if let Some(info) = scan_single(path) {
            assert_eq!(info.name, "dx-terminal");
            assert!(info.tech.contains(&"rust".to_string()));
            assert_eq!(info.test_cmd.as_deref(), Some("cargo test"));
            assert_eq!(info.build_cmd.as_deref(), Some("cargo build --release"));
        }
    }
}

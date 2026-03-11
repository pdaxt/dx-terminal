//! Vision-Driven Development Hook (Rust)
//!
//! Replaces vision-driven.py. Every prompt goes through this binary.
//! It classifies intent against all known visions and injects context.
//!
//! Events handled:
//! - UserPromptSubmit: classify prompt → inject VDD context
//! - PreToolUse (Edit/Write): flag untracked edits in vision projects
//! - PostToolUse (Bash): after git commit → flag task status updates
//! - Stop: session summary

use dx_terminal::config::RuntimeConfig;
use dx_terminal::vision;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SESSION_FILE: &str = "/tmp/vdd_session_edits.json";
const VISIONS_CACHE: &str = "/tmp/vdd_visions_cache.json";
const CACHE_TTL: u64 = 120;

static NOISE_WORDS: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "from", "will", "have", "are", "was", "been",
    "can", "system", "new", "add", "use", "all", "get", "set", "make", "our", "more", "also",
    "into", "like", "well",
];

static WORK_INDICATORS: &[&str] = &[
    "add",
    "build",
    "create",
    "implement",
    "fix",
    "update",
    "refactor",
    "change",
    "modify",
    "improve",
    "make",
    "write",
    "design",
    "develop",
];

// ── Data types ──

#[derive(Serialize, Deserialize, Default)]
struct SessionEdits {
    files: Vec<String>,
    commits: Vec<CommitRecord>,
    project: Option<String>,
    has_vision: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct CommitRecord {
    branch: Option<String>,
    command: String,
}

#[derive(Serialize, Deserialize)]
struct VisionCache {
    ts: f64,
    visions: Vec<Value>,
}

#[derive(Debug)]
enum Classification {
    NewVision {
        prompt: String,
        suggested_project: Option<String>,
    },
    ExistingGoal {
        project: String,
        project_path: String,
        goal: Value,
        features: Vec<Value>,
        score: i32,
        vision: Value,
    },
    ExistingFeature {
        project: String,
        project_path: String,
        goal: Value,
        feature: Value,
        features: Vec<Value>,
        score: i32,
        vision: Value,
    },
    UnmatchedWork {
        project: String,
        project_path: String,
        vision: Value,
        prompt: String,
    },
}

// ── Session persistence ──

fn load_session() -> SessionEdits {
    fs::read_to_string(SESSION_FILE)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_session(session: &SessionEdits) {
    let _ = fs::write(
        SESSION_FILE,
        serde_json::to_string_pretty(session).unwrap_or_default(),
    );
}

// ── Vision scanning ──

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn scan_all_visions() -> Vec<Value> {
    // Check cache
    if let Ok(data) = fs::read_to_string(VISIONS_CACHE) {
        if let Ok(cache) = serde_json::from_str::<VisionCache>(&data) {
            if now_secs() - cache.ts < CACHE_TTL as f64 {
                return cache.visions;
            }
        }
    }

    let home = dirs_home();
    let projects_dir = home.join("Projects");
    if !projects_dir.exists() {
        return vec![];
    }

    let mut visions = Vec::new();
    if let Ok(entries) = fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let vf = path.join(".vision").join("vision.json");
                if vf.exists() {
                    if let Ok(content) = fs::read_to_string(&vf) {
                        if let Ok(mut v) = serde_json::from_str::<Value>(&content) {
                            v["_path"] = json!(path.to_string_lossy());
                            visions.push(v);
                        }
                    }
                }
            }
        }
    }

    // Write cache
    let cache = VisionCache {
        ts: now_secs(),
        visions: visions.clone(),
    };
    let _ = fs::write(
        VISIONS_CACHE,
        serde_json::to_string(&cache).unwrap_or_default(),
    );

    visions
}

fn dirs_home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/Users/pran".into()))
}

// ── Git helpers ──

fn get_current_branch(cwd: Option<&str>) -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(cwd.unwrap_or("."))
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn get_current_project() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    // Walk up to find .vision/
    let mut p = Some(cwd.as_path());
    while let Some(dir) = p {
        if dir.join(".vision").join("vision.json").exists() {
            return Some(dir.to_string_lossy().into());
        }
        p = dir.parent();
    }
    // Check if inside ~/Projects/X
    let projects = dirs_home().join("Projects");
    if let Ok(rel) = cwd.strip_prefix(&projects) {
        if let Some(top) = rel.components().next() {
            let top_path = projects.join(top.as_os_str());
            if top_path.join(".vision").join("vision.json").exists() {
                return Some(top_path.to_string_lossy().into());
            }
        }
    }
    None
}

fn find_vision_root(file_path: &str) -> Option<String> {
    let p = PathBuf::from(file_path);
    let resolved = fs::canonicalize(&p).unwrap_or(p);
    let mut dir = Some(resolved.as_path());
    while let Some(d) = dir {
        if d.join(".vision").join("vision.json").exists() {
            return Some(d.to_string_lossy().into());
        }
        dir = d.parent();
    }
    None
}

fn load_vision(project_path: &str) -> Option<Value> {
    let vf = PathBuf::from(project_path)
        .join(".vision")
        .join("vision.json");
    fs::read_to_string(vf)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn find_task_by_branch<'a>(vision: &'a Value, branch: &str) -> Option<(&'a Value, &'a Value)> {
    for feature in vision.get("features")?.as_array()? {
        for task in feature.get("tasks")?.as_array()? {
            if task.get("branch").and_then(|b| b.as_str()) == Some(branch) {
                return Some((feature, task));
            }
        }
    }
    None
}

fn feature_phase(feature: &Value) -> &str {
    feature
        .get("phase")
        .or_else(|| feature.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("planned")
}

fn feature_readiness_blockers<'a>(feature: &'a Value, phase: &str) -> Vec<&'a str> {
    feature
        .get("readiness")
        .and_then(|r| r.get("blockers"))
        .and_then(|b| b.get(phase))
        .and_then(|v| v.as_array())
        .map(|items| items.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default()
}

fn is_doc_like_edit(file_path: &str) -> bool {
    file_path.contains("/.vision/") || file_path.ends_with(".md") || file_path.contains("/docs/")
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CommandKind {
    Build,
    Test,
    Lint,
    Commit,
    Other,
}

fn classify_command(command: &str) -> CommandKind {
    let cmd = command.trim().to_lowercase();
    let test_patterns = [
        "cargo test",
        "pytest",
        "npm test",
        "pnpm test",
        "yarn test",
        "bun test",
        "vitest",
        "jest",
        "playwright test",
        "cypress run",
    ];
    let lint_patterns = [
        "cargo clippy",
        "cargo fmt",
        "ruff",
        "eslint",
        "tsc --noemit",
        "biome",
    ];
    let build_patterns = [
        "cargo build",
        "cargo check",
        "npm run build",
        "pnpm build",
        "yarn build",
        "next build",
        "vite build",
        "turbo build",
    ];

    if cmd.contains("git commit") {
        return CommandKind::Commit;
    }
    if test_patterns.iter().any(|pat| cmd.contains(pat)) {
        return CommandKind::Test;
    }
    if lint_patterns.iter().any(|pat| cmd.contains(pat)) {
        return CommandKind::Lint;
    }
    if build_patterns.iter().any(|pat| cmd.contains(pat)) {
        return CommandKind::Build;
    }
    CommandKind::Other
}

fn extract_command_success(event: &Value) -> Option<bool> {
    let exit_code = event
        .get("tool_response")
        .and_then(|v| v.get("exit_code"))
        .or_else(|| event.get("tool_output").and_then(|v| v.get("exit_code")))
        .or_else(|| event.get("tool_result").and_then(|v| v.get("exit_code")))
        .or_else(|| event.get("exit_code"))
        .and_then(|v| v.as_i64());

    if let Some(code) = exit_code {
        return Some(code == 0);
    }

    event
        .get("tool_response")
        .and_then(|v| v.get("success"))
        .or_else(|| event.get("tool_result").and_then(|v| v.get("success")))
        .or_else(|| event.get("success"))
        .and_then(|v| v.as_bool())
}

fn extract_actor(event: &Value) -> String {
    event
        .get("agent")
        .or_else(|| event.get("actor"))
        .or_else(|| event.get("session_id"))
        .or_else(|| event.get("user"))
        .and_then(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .or_else(|| v.as_i64().map(|n| n.to_string()))
        })
        .unwrap_or_else(|| "vision-hook".to_string())
}

fn auto_verify_acceptance_items(
    project: &str,
    feature: &Value,
    command: &str,
    actor: &str,
) -> Vec<String> {
    let feature_id = match feature.get("id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return vec![],
    };

    let items = match feature.get("acceptance_items").and_then(|v| v.as_array()) {
        Some(items) => items,
        None => return vec![],
    };

    let mut verified = Vec::new();
    for item in items {
        let criterion_id = match item.get("id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => continue,
        };
        let status = item
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("draft");
        if status == "verified" || status == "failed" {
            continue;
        }

        let method = item
            .get("verification_method")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let method_lower = method.to_lowercase();
        let method_is_test = !method_lower.is_empty()
            && (method_lower.contains("test")
                || method_lower.contains("integration")
                || method_lower.contains("unit")
                || method_lower.contains("e2e"));
        if !method_is_test {
            continue;
        }

        let evidence = vec![format!(
            "command: {}",
            command.chars().take(240).collect::<String>()
        )];
        let result = vision::verify_acceptance_criterion(
            project,
            feature_id,
            criterion_id,
            "verified",
            evidence,
            Some(actor),
            Some("hook:test_command"),
        );
        if !result.contains("\"error\"") {
            notify_dashboard_vision_change(project, &result, Some(feature_id));
            verified.push(criterion_id.to_string());
        }
    }

    verified
}

// ── Scoring ──

fn split_words(text: &str) -> Vec<String> {
    let re = Regex::new(r"\W+").unwrap();
    re.split(&text.to_lowercase())
        .filter(|w| w.len() > 2)
        .map(|w| w.to_string())
        .collect()
}

fn score_match(text: &str, keywords: &[String]) -> i32 {
    let text_lower = text.to_lowercase();
    let noise: HashSet<&str> = NOISE_WORDS.iter().copied().collect();
    let mut score = 0i32;
    for kw in keywords {
        if noise.contains(kw.as_str()) {
            continue;
        }
        // Word boundary match
        let pattern = format!(r"\b{}\b", regex::escape(kw));
        if let Ok(re) = Regex::new(&pattern) {
            if re.is_match(&text_lower) {
                score += 1;
            }
        }
    }
    score
}

// ── Prompt classification ──

fn classify_prompt(prompt: &str, visions: &[Value]) -> Option<Classification> {
    let prompt_lower = prompt.to_lowercase();
    let prompt_trimmed = prompt_lower.trim();
    let words = split_words(prompt_trimmed);

    // Skip very short prompts
    if words.len() < 2 {
        return None;
    }

    // Skip tool/system commands
    let skip_patterns = [
        r"^/\w+",
        r"^(yes|no|ok|sure|thanks|done|good|great|go ahead)$",
        r"^(commit|push|deploy|show|list|status)$",
        r"^(fix it|do it|make it)$",
    ];
    for pat in &skip_patterns {
        if let Ok(re) = Regex::new(pat) {
            if re.is_match(prompt_trimmed) {
                return None;
            }
        }
    }

    // Check for explicit vision commands
    if let Ok(re) = Regex::new(r"\b(create|new|init)\b.*\bvision\b") {
        if re.is_match(prompt_trimmed) {
            let proj = Regex::new(r"\bfor\s+(\w+)")
                .ok()
                .and_then(|r| r.captures(prompt_trimmed))
                .map(|c| c[1].to_string());
            return Some(Classification::NewVision {
                prompt: prompt.to_string(),
                suggested_project: proj,
            });
        }
    }

    // Score each vision's goals and features
    let mut best_match: Option<Classification> = None;
    let mut best_score = 0i32;

    for vision in visions {
        let project = vision.get("project").and_then(|p| p.as_str()).unwrap_or("");
        let project_path = vision.get("_path").and_then(|p| p.as_str()).unwrap_or("");
        let project_bonus = if !project.is_empty() && prompt_lower.contains(&project.to_lowercase())
        {
            2
        } else {
            0
        };

        let goals = match vision.get("goals").and_then(|g| g.as_array()) {
            Some(g) => g,
            None => continue,
        };

        let features_all = vision
            .get("features")
            .and_then(|f| f.as_array())
            .cloned()
            .unwrap_or_default();

        for goal in goals {
            let goal_id = goal.get("id").and_then(|i| i.as_str()).unwrap_or("");
            let goal_title = goal.get("title").and_then(|t| t.as_str()).unwrap_or("");
            let goal_desc = goal
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("");
            let goal_metrics = goal
                .get("metrics")
                .and_then(|m| m.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();

            let goal_text = format!("{} {} {}", goal_title, goal_desc, goal_metrics);
            let mut total_score = score_match(&goal_text, &words) + project_bonus;

            // Find features under this goal and add their scores
            let goal_features: Vec<Value> = features_all
                .iter()
                .filter(|f| f.get("goal_id").and_then(|g| g.as_str()) == Some(goal_id))
                .cloned()
                .collect();

            let mut matched_feature: Option<Value> = None;
            for feat in &goal_features {
                let feat_title = feat.get("title").and_then(|t| t.as_str()).unwrap_or("");
                let feat_desc = feat
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("");
                let feat_text = format!("{} {}", feat_title, feat_desc);
                let f_score = score_match(&feat_text, &words);
                if f_score > 0 {
                    matched_feature = Some(feat.clone());
                    total_score += f_score;
                }
            }

            if total_score > best_score {
                best_score = total_score;
                best_match = Some(if matched_feature.is_some() {
                    Classification::ExistingFeature {
                        project: project.to_string(),
                        project_path: project_path.to_string(),
                        goal: goal.clone(),
                        feature: matched_feature.unwrap(),
                        features: goal_features,
                        score: total_score,
                        vision: vision.clone(),
                    }
                } else {
                    Classification::ExistingGoal {
                        project: project.to_string(),
                        project_path: project_path.to_string(),
                        goal: goal.clone(),
                        features: goal_features,
                        score: total_score,
                        vision: vision.clone(),
                    }
                });
            }
        }
    }

    // Threshold: project mentioned → 2, otherwise → 3
    if let Some(ref m) = best_match {
        if best_score >= 2 {
            let proj = match m {
                Classification::ExistingGoal { project, .. }
                | Classification::ExistingFeature { project, .. } => project.to_lowercase(),
                _ => String::new(),
            };
            let project_mentioned = !proj.is_empty() && prompt_lower.contains(&proj);
            if project_mentioned || best_score >= 3 {
                return best_match;
            }
        }
    }

    // No strong match — check if it's work-related
    let is_work = WORK_INDICATORS
        .iter()
        .any(|w| words.iter().any(|word| word == *w));

    if is_work && !visions.is_empty() {
        if let Some(current) = get_current_project() {
            for v in visions {
                if v.get("_path").and_then(|p| p.as_str()) == Some(&current) {
                    return Some(Classification::UnmatchedWork {
                        project: v
                            .get("project")
                            .and_then(|p| p.as_str())
                            .unwrap_or("")
                            .to_string(),
                        project_path: current,
                        vision: v.clone(),
                        prompt: prompt.to_string(),
                    });
                }
            }
        }
    }

    None
}

// ── Context message building ──

fn build_context(classification: &Classification) -> Option<String> {
    match classification {
        Classification::NewVision {
            suggested_project, ..
        } => {
            let proj_hint = suggested_project
                .as_ref()
                .map(|p| format!(" for '{}'", p))
                .unwrap_or_default();
            Some(format!(
                "VDD: User wants to create a new vision{}. Use `vision_init` or `/vision init` \
                 to create it. Ask for: project name, mission statement, GitHub repo.",
                proj_hint
            ))
        }

        Classification::ExistingGoal {
            project,
            goal,
            features,
            ..
        } => {
            let goal_id = goal.get("id").and_then(|i| i.as_str()).unwrap_or("?");
            let goal_title = goal.get("title").and_then(|t| t.as_str()).unwrap_or("?");
            let goal_status = goal.get("status").and_then(|s| s.as_str()).unwrap_or("?");

            let mut parts = vec![
                format!("VDD CONTEXT \u{2014} Project: {}", project),
                format!(
                    "Matched Goal: {} \"{}\" [{}]",
                    goal_id, goal_title, goal_status
                ),
            ];

            if features.is_empty() {
                parts.push(
                    "No features yet under this goal. \
                     Create one with vision_add_feature() before starting work."
                        .into(),
                );
            } else {
                let mut f_lines = Vec::new();
                for f in features {
                    let fid = f.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                    let ftitle = f.get("title").and_then(|t| t.as_str()).unwrap_or("?");
                    let fstatus = feature_phase(f);
                    let open_q = f
                        .get("questions")
                        .and_then(|q| q.as_array())
                        .map(|qs| {
                            qs.iter()
                                .filter(|q| {
                                    q.get("status").and_then(|s| s.as_str()) == Some("open")
                                })
                                .count()
                        })
                        .unwrap_or(0);
                    let tasks = f
                        .get("tasks")
                        .and_then(|t| t.as_array())
                        .map(|ts| ts.len())
                        .unwrap_or(0);
                    let tasks_done = f
                        .get("tasks")
                        .and_then(|t| t.as_array())
                        .map(|ts| {
                            ts.iter()
                                .filter(|t| {
                                    matches!(
                                        t.get("status").and_then(|s| s.as_str()),
                                        Some("done") | Some("verified")
                                    )
                                })
                                .count()
                        })
                        .unwrap_or(0);

                    let mut line = format!(
                        "  {}: {} [{}] \u{2014} {}/{} tasks",
                        fid, ftitle, fstatus, tasks_done, tasks
                    );
                    if open_q > 0 {
                        line += &format!(" \u{2014} {} OPEN QUESTIONS", open_q);
                    }
                    let blockers = match fstatus {
                        "discovery" => feature_readiness_blockers(f, "build"),
                        "build" => feature_readiness_blockers(f, "test"),
                        "test" => feature_readiness_blockers(f, "done"),
                        _ => Vec::new(),
                    };
                    if !blockers.is_empty() {
                        line += &format!(" \u{2014} blockers: {}", blockers.join(", "));
                    }
                    f_lines.push(line);
                }
                parts.push(format!("Features:\n{}", f_lines.join("\n")));

                // Open questions
                let mut open_questions = Vec::new();
                for f in features {
                    let fid = f.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                    if let Some(qs) = f.get("questions").and_then(|q| q.as_array()) {
                        for q in qs {
                            if q.get("status").and_then(|s| s.as_str()) == Some("open") {
                                let qid = q.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                                let qtext = q.get("text").and_then(|t| t.as_str()).unwrap_or("?");
                                open_questions.push(format!("  {}/{}: {}", fid, qid, qtext));
                            }
                        }
                    }
                }
                if !open_questions.is_empty() {
                    parts.push(format!(
                        "OPEN QUESTIONS (answer before building):\n{}",
                        open_questions.join("\n")
                    ));
                }
            }

            parts.push(
                "WORKFLOW: Check features \u{2192} answer open questions \u{2192} create/update tasks \u{2192} \
                 link branch \u{2192} implement \u{2192} update task status"
                    .into(),
            );

            Some(parts.join("\n"))
        }

        Classification::ExistingFeature {
            project,
            goal,
            feature,
            ..
        } => {
            let goal_id = goal.get("id").and_then(|i| i.as_str()).unwrap_or("?");
            let goal_title = goal.get("title").and_then(|t| t.as_str()).unwrap_or("?");
            let feat_id = feature.get("id").and_then(|i| i.as_str()).unwrap_or("?");
            let feat_title = feature.get("title").and_then(|t| t.as_str()).unwrap_or("?");
            let feat_status = feature_phase(feature);

            let mut parts = vec![
                format!("VDD CONTEXT \u{2014} Project: {}", project),
                format!("Goal: {} \"{}\"", goal_id, goal_title),
                format!("Feature: {} \"{}\" [{}]", feat_id, feat_title, feat_status),
            ];

            // Open questions
            let open_q: Vec<&Value> = feature
                .get("questions")
                .and_then(|q| q.as_array())
                .map(|qs| {
                    qs.iter()
                        .filter(|q| q.get("status").and_then(|s| s.as_str()) == Some("open"))
                        .collect()
                })
                .unwrap_or_default();

            if !open_q.is_empty() {
                parts.push("OPEN QUESTIONS (answer these first):".into());
                for q in &open_q {
                    let qid = q.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                    let qtext = q.get("text").and_then(|t| t.as_str()).unwrap_or("?");
                    parts.push(format!("  {}: {}", qid, qtext));
                }
            }

            // Tasks
            if let Some(tasks) = feature.get("tasks").and_then(|t| t.as_array()) {
                if !tasks.is_empty() {
                    parts.push("Tasks:".into());
                    for t in tasks {
                        let tid = t.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                        let ttitle = t.get("title").and_then(|t| t.as_str()).unwrap_or("?");
                        let tstatus = t.get("status").and_then(|s| s.as_str()).unwrap_or("?");
                        let branch = t
                            .get("branch")
                            .and_then(|b| b.as_str())
                            .map(|b| format!(" [{}]", b))
                            .unwrap_or_default();
                        let pr = t
                            .get("pr")
                            .and_then(|p| p.as_str())
                            .map(|p| format!(" PR:{}", p))
                            .unwrap_or_default();
                        parts.push(format!(
                            "  {}: {} [{}]{}{}",
                            tid, ttitle, tstatus, branch, pr
                        ));
                    }
                } else if open_q.is_empty() {
                    parts.push("No tasks yet. Create tasks with vision_add_task().".into());
                }
            }

            let blockers = match feat_status {
                "discovery" => feature_readiness_blockers(feature, "build"),
                "build" => feature_readiness_blockers(feature, "test"),
                "test" => feature_readiness_blockers(feature, "done"),
                _ => Vec::new(),
            };
            if !blockers.is_empty() {
                parts.push(format!("Current blockers: {}", blockers.join(", ")));
            }

            let workflow = match feat_status {
                "discovery" => format!(
                    "WORKFLOW: Stay in discovery for {}. Answer blocking questions, add discovery docs, and define acceptance until build readiness is clear.",
                    feat_id
                ),
                "build" => format!(
                    "WORKFLOW: Continue implementation on {}. Keep task/branch status current; successful evidence will auto-connect to test.",
                    feat_id
                ),
                "test" => format!(
                    "WORKFLOW: Run verification for {}. Successful tests and verified acceptance criteria will auto-connect to done.",
                    feat_id
                ),
                "done" => format!(
                    "WORKFLOW: {} is done. Only reopen if regression or scope change is intentional.",
                    feat_id
                ),
                _ => format!(
                    "WORKFLOW: Continue work on {}. Update task status as you go.",
                    feat_id
                ),
            };
            parts.push(workflow);

            Some(parts.join("\n"))
        }

        Classification::UnmatchedWork {
            project, vision, ..
        } => {
            let goals = vision
                .get("goals")
                .and_then(|g| g.as_array())
                .cloned()
                .unwrap_or_default();

            let goal_lines: Vec<String> = goals
                .iter()
                .filter(|g| g.get("status").and_then(|s| s.as_str()) != Some("dropped"))
                .map(|g| {
                    let id = g.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                    let title = g.get("title").and_then(|t| t.as_str()).unwrap_or("?");
                    let status = g.get("status").and_then(|s| s.as_str()).unwrap_or("?");
                    format!("  {}: {} [{}]", id, title, status)
                })
                .collect();

            Some(
                [
                    format!("VDD CONTEXT \u{2014} Project: {}", project),
                    "This work doesn't match any existing goal/feature.".into(),
                    format!("Existing goals:\n{}", goal_lines.join("\n")),
                    "ACTION: Either link to an existing goal with vision_add_feature(), \
                     or create a new goal with add_goal() if this is new scope."
                        .into(),
                ]
                .join("\n"),
            )
        }
    }
}

// ── Event handlers ──

fn handle_user_prompt(event: &Value) -> Option<Value> {
    let prompt = event
        .get("user_prompt")
        .or_else(|| event.get("prompt"))
        .and_then(|p| p.as_str())
        .unwrap_or("");
    if prompt.trim().is_empty() {
        return None;
    }

    let visions = scan_all_visions();
    if visions.is_empty() {
        return None;
    }

    let classification = classify_prompt(prompt, &visions)?;
    let context = build_context(&classification)?;

    Some(json!({ "decision": "approve", "reason": context }))
}

fn handle_pre_tool_use(event: &Value) -> Option<Value> {
    let tool = event.get("tool_name").and_then(|t| t.as_str())?;
    if tool != "Edit" && tool != "Write" {
        return None;
    }

    let file_path = event
        .get("tool_input")
        .and_then(|i| i.get("file_path"))
        .and_then(|f| f.as_str())?;

    let project = find_vision_root(file_path)?;
    let vision = load_vision(&project)?;

    // Track the edit
    let mut session = load_session();
    if !session.files.contains(&file_path.to_string()) {
        session.files.push(file_path.to_string());
    }
    session.project = Some(project.clone());
    session.has_vision = true;
    save_session(&session);

    // Check if current branch has a tracked task
    let branch = get_current_branch(Some(&project))?;
    let linked = find_task_by_branch(&vision, &branch);
    let has_task = linked.is_some();

    if let Some((feature, task)) = linked {
        let phase = feature_phase(feature);
        if phase == "discovery" && !is_doc_like_edit(file_path) {
            let fid = feature.get("id").and_then(|i| i.as_str()).unwrap_or("?");
            let tid = task.get("id").and_then(|i| i.as_str()).unwrap_or("?");
            let blockers = feature_readiness_blockers(feature, "build");
            let suffix = if blockers.is_empty() {
                String::new()
            } else {
                format!(" Discovery blockers: {}.", blockers.join(", "))
            };
            return Some(json!({
                "decision": "approve",
                "reason": format!(
                    "VDD: {} is still in discovery on task {}. You're editing an implementation file before discovery is closed.{}",
                    fid, tid, suffix
                )
            }));
        }
    }

    if !has_task {
        let features = vision.get("features").and_then(|f| f.as_array())?;
        let active: Vec<&Value> = features
            .iter()
            .filter(|f| f.get("status").and_then(|s| s.as_str()) != Some("done"))
            .collect();

        if !active.is_empty() {
            let feat_list: String = active
                .iter()
                .take(3)
                .map(|a| {
                    let id = a.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                    let title = a.get("title").and_then(|t| t.as_str()).unwrap_or("?");
                    format!("{} ({})", id, title)
                })
                .collect::<Vec<_>>()
                .join(", ");

            return Some(json!({
                "decision": "approve",
                "reason": format!(
                    "VDD: Branch '{}' not linked to a vision task. Active features: {}. \
                     Link with: vision_add_task(feature_id, title, branch='{}')",
                    branch, feat_list, branch
                )
            }));
        }
    }

    None
}

fn dashboard_port() -> u16 {
    std::env::var("DX_WEB_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or_else(|| RuntimeConfig::load().web_port)
}

fn build_dashboard_notify_request(port: u16, body: &str) -> String {
    format!(
        "POST /api/vision/notify HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        port,
        body.as_bytes().len(),
        body,
    )
}

fn try_notify_via_socket(body: &str) -> bool {
    let socket_paths = dx_terminal::ipc::discover_vision_socket_paths();
    let mut delivered = false;

    for socket_path in socket_paths {
        let mut stream = match UnixStream::connect(&socket_path) {
            Ok(stream) => stream,
            Err(_) => {
                let _ = fs::remove_file(&socket_path);
                continue;
            }
        };
        let _ = stream.set_write_timeout(Some(Duration::from_millis(150)));
        let _ = stream.set_read_timeout(Some(Duration::from_millis(150)));
        if stream.write_all(body.as_bytes()).is_err() {
            continue;
        }
        let _ = stream.shutdown(Shutdown::Write);
        let mut response = [0u8; 64];
        let _ = stream.read(&mut response);
        delivered = true;
    }

    delivered
}

fn notify_dashboard_vision_change(project_path: &str, result: &str, feature_id: Option<&str>) {
    if project_path.trim().is_empty() || result.trim().is_empty() {
        return;
    }

    let payload = json!({
        "project_path": project_path,
        "result": result,
        "feature_id": feature_id,
    });
    let body = match dx_terminal::ipc::prepare_outbound_event(payload) {
        Some(body) => body,
        None => return,
    };

    if try_notify_via_socket(&body) {
        return;
    }

    let port = dashboard_port();
    let addr: SocketAddr = match format!("127.0.0.1:{}", port).parse() {
        Ok(addr) => addr,
        Err(_) => return,
    };
    let request = build_dashboard_notify_request(port, &body);

    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_millis(150)) {
        Ok(stream) => stream,
        Err(_) => return,
    };
    let _ = stream.set_write_timeout(Some(Duration::from_millis(150)));
    let _ = stream.set_read_timeout(Some(Duration::from_millis(150)));
    let _ = stream.write_all(request.as_bytes());

    let mut response = [0u8; 64];
    let _ = stream.read(&mut response);
}

fn handle_post_tool_use(event: &Value) -> Option<Value> {
    let tool = event.get("tool_name").and_then(|t| t.as_str())?;
    if tool != "Bash" {
        return None;
    }

    let command = event
        .get("tool_input")
        .and_then(|i| i.get("command"))
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let command_kind = classify_command(command);
    if command_kind == CommandKind::Other {
        return None;
    }

    let project = find_vision_root(&std::env::current_dir().ok()?.to_string_lossy())
        .or_else(|| load_session().project)?;

    let vision = load_vision(&project)?;
    let features = vision.get("features").and_then(|f| f.as_array())?;
    if features.is_empty() {
        return None;
    }

    let branch = get_current_branch(Some(&project))?;
    let actor = extract_actor(event);
    let command_success = extract_command_success(event);

    if let Some((feature, task)) = find_task_by_branch(&vision, &branch) {
        let tid = task.get("id").and_then(|i| i.as_str()).unwrap_or("?");
        let ttitle = task.get("title").and_then(|t| t.as_str()).unwrap_or("?");
        let fid = feature.get("id").and_then(|i| i.as_str()).unwrap_or("?");
        let task_status = task
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("planned");

        let mut notes = Vec::new();

        if matches!(
            command_kind,
            CommandKind::Build | CommandKind::Test | CommandKind::Lint | CommandKind::Commit
        ) && task_status == "planned"
        {
            let result = vision::update_task_status(
                &project,
                fid,
                tid,
                "in_progress",
                Some(&branch),
                None,
                None,
            );
            notify_dashboard_vision_change(&project, &result, Some(fid));
            notes.push(format!("task {} auto-moved to in_progress", tid));
        }

        if command_kind == CommandKind::Test && command_success == Some(true) {
            let current_status = load_vision(&project)
                .and_then(|v| {
                    find_task_by_branch(&v, &branch).map(|(_, t)| {
                        t.get("status")
                            .and_then(|s| s.as_str())
                            .unwrap_or("planned")
                            .to_string()
                    })
                })
                .unwrap_or_else(|| task_status.to_string());

            if current_status == "done" || current_status == "in_progress" {
                let result = vision::update_task_status(
                    &project,
                    fid,
                    tid,
                    "verified",
                    Some(&branch),
                    None,
                    None,
                );
                notify_dashboard_vision_change(&project, &result, Some(fid));
                notes.push(format!(
                    "task {} auto-marked verified after successful test command",
                    tid
                ));
            }

            let refreshed = load_vision(&project).unwrap_or_else(|| vision.clone());
            if let Some((refreshed_feature, _)) = find_task_by_branch(&refreshed, &branch) {
                let verified =
                    auto_verify_acceptance_items(&project, refreshed_feature, command, &actor);
                if !verified.is_empty() {
                    notes.push(format!("acceptance auto-verified: {}", verified.join(", ")));
                }
            }
        }

        if command_kind == CommandKind::Commit && notes.is_empty() {
            return None;
        }

        if !notes.is_empty() {
            return Some(json!({
                "decision": "approve",
                "reason": format!(
                    "VDD: Branch '{}' linked to {} / {}. {}",
                    branch, fid, ttitle, notes.join("; ")
                )
            }));
        }
        return None;
    }

    // No task linked
    let active: Vec<&Value> = features
        .iter()
        .filter(|f| f.get("status").and_then(|s| s.as_str()) != Some("done"))
        .collect();

    if !active.is_empty() {
        let mut session = load_session();
        session.commits.push(CommitRecord {
            branch: Some(branch.clone()),
            command: command.chars().take(100).collect(),
        });
        save_session(&session);

        let feat_ids: String = active
            .iter()
            .take(3)
            .filter_map(|a| a.get("id").and_then(|i| i.as_str()))
            .collect::<Vec<_>>()
            .join(", ");

        if command_kind == CommandKind::Commit {
            return Some(json!({
                "decision": "approve",
                "reason": format!(
                    "VDD: Commit on untracked branch '{}'. Link to a feature: {}",
                    branch, feat_ids
                )
            }));
        }
    }

    None
}

fn handle_stop(_event: &Value) -> Option<Value> {
    let session = load_session();
    if !session.has_vision {
        return None;
    }

    let project = session.project.as_ref()?;
    let vision = load_vision(project)?;
    let features = vision
        .get("features")
        .and_then(|f| f.as_array())
        .cloned()
        .unwrap_or_default();

    let total_tasks: usize = features
        .iter()
        .map(|f| {
            f.get("tasks")
                .and_then(|t| t.as_array())
                .map(|ts| ts.len())
                .unwrap_or(0)
        })
        .sum();

    let done_tasks: usize = features
        .iter()
        .map(|f| {
            f.get("tasks")
                .and_then(|t| t.as_array())
                .map(|ts| {
                    ts.iter()
                        .filter(|t| {
                            matches!(
                                t.get("status").and_then(|s| s.as_str()),
                                Some("done") | Some("verified")
                            )
                        })
                        .count()
                })
                .unwrap_or(0)
        })
        .sum();

    let open_questions: usize = features
        .iter()
        .map(|f| {
            f.get("questions")
                .and_then(|q| q.as_array())
                .map(|qs| {
                    qs.iter()
                        .filter(|q| q.get("status").and_then(|s| s.as_str()) == Some("open"))
                        .count()
                })
                .unwrap_or(0)
        })
        .sum();

    let files_count = session.files.len();
    let untracked = &session.commits;

    if files_count == 0 && untracked.is_empty() {
        return None;
    }

    let mut parts = vec![format!(
        "Vision: {} features, {}/{} tasks done",
        features.len(),
        done_tasks,
        total_tasks
    )];

    if open_questions > 0 {
        parts.push(format!("{} open questions need answers", open_questions));
    }

    let branches: HashSet<String> = untracked.iter().filter_map(|c| c.branch.clone()).collect();
    if !branches.is_empty() {
        parts.push(format!(
            "Untracked commits on: {}",
            branches.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }

    // Reset session
    save_session(&SessionEdits::default());

    Some(json!({
        "decision": "approve",
        "reason": format!("VDD Session Summary:\n  {}", parts.join("\n  "))
    }))
}

// ── Main ──

fn main() {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        return;
    }
    if input.trim().is_empty() {
        return;
    }

    let event: Value = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(_) => return,
    };

    let hook_event = event
        .get("hook_event")
        .or_else(|| event.get("event"))
        .and_then(|e| e.as_str())
        .unwrap_or("");

    let result = match hook_event {
        "UserPromptSubmit" => handle_user_prompt(&event),
        "PreToolUse" => handle_pre_tool_use(&event),
        "PostToolUse" => handle_post_tool_use(&event),
        "Stop" => handle_stop(&event),
        _ => None,
    };

    if let Some(r) = result {
        println!("{}", r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_command_detects_build_test_lint_commit() {
        assert_eq!(classify_command("cargo test -q"), CommandKind::Test);
        assert_eq!(classify_command("pnpm build"), CommandKind::Build);
        assert_eq!(
            classify_command("cargo clippy --all-targets"),
            CommandKind::Lint
        );
        assert_eq!(classify_command("git commit -m 'x'"), CommandKind::Commit);
        assert_eq!(classify_command("echo hi"), CommandKind::Other);
    }

    #[test]
    fn test_extract_command_success_from_exit_code() {
        let ok = json!({"tool_response": {"exit_code": 0}});
        let fail = json!({"tool_result": {"exit_code": 1}});
        let unknown = json!({"tool_response": {"stdout": "ok"}});

        assert_eq!(extract_command_success(&ok), Some(true));
        assert_eq!(extract_command_success(&fail), Some(false));
        assert_eq!(extract_command_success(&unknown), None);
    }

    #[test]
    fn test_doc_like_edit_detection() {
        assert!(is_doc_like_edit("/tmp/project/.vision/discovery/F1.1.md"));
        assert!(is_doc_like_edit("/tmp/project/docs/notes.md"));
        assert!(!is_doc_like_edit("/tmp/project/src/lib.rs"));
    }

    #[test]
    fn test_build_dashboard_notify_request_contains_path_and_length() {
        let body = r#"{"project_path":"/tmp/demo","result":"ok"}"#;
        let request = build_dashboard_notify_request(3100, body);

        assert!(request.starts_with("POST /api/vision/notify HTTP/1.1\r\n"));
        assert!(request.contains("Host: 127.0.0.1:3100\r\n"));
        assert!(request.contains(&format!("Content-Length: {}\r\n", body.len())));
        assert!(request.ends_with(body));
    }
}

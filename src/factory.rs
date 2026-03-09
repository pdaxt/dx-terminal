//! Factory pipeline — single command → multi-agent orchestration.
//!
//! `:go dataxlr8 add OAuth login` → dev → qa + security → review
//! Pipeline state derived from queue tasks via pipeline_id. No separate storage.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::queue;
use crate::scanner;

// ============================================================
// Pipeline data types
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineView {
    pub id: String,
    pub project: String,
    pub description: String,
    pub template: String,
    pub created_at: String,
    pub status: String,
    pub stages: Vec<StageView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageView {
    pub name: String,
    pub role: String,
    pub task_id: String,
    pub status: String,
    pub pane: Option<u8>,
    pub summary: Option<String>,
}

// ============================================================
// Pipeline templates
// ============================================================

struct StageTemplate {
    name: &'static str,
    role: &'static str,
    parallel_with: &'static [&'static str],
    prompt: &'static str,
}

struct PipelineTemplate {
    name: &'static str,
    stages: &'static [StageTemplate],
}

const PROMPT_DEV: &str = r#"You are the DEVELOPER stage of a factory pipeline.

## Task
{{task}}

## Project: {{project}} ({{project_path}})

## Instructions
1. Implement the requested feature/fix completely
2. Write clean, well-structured code
3. Run existing tests to make sure nothing breaks
4. Commit and push your changes with a clear commit message
5. At the end, summarize: what you built, key decisions, files changed

A QA agent and security auditor will verify your work after you finish."#;

const PROMPT_QA: &str = r#"You are the QA stage of a factory pipeline.

## Task
Test and verify: {{task}}

## Project: {{project}} ({{project_path}})

## Predecessor Results
(Auto-injected by the queue system from the developer agent's output)

## Instructions
1. Review the developer's changes (check git log, git diff)
2. Run the full test suite — report pass/fail counts
3. Write new tests for any untested new code
4. Test edge cases and error handling paths
5. Verify the feature works end-to-end
6. Commit any new tests you write
7. Final verdict: PASS or FAIL with details

Use quality tools: log_test, log_build to record results."#;

const PROMPT_SECURITY: &str = r#"You are the SECURITY AUDITOR stage of a factory pipeline.

## Task
Security audit: {{task}}

## Project: {{project}} ({{project_path}})

## Predecessor Results
(Auto-injected by the queue system from the developer agent's output)

## Instructions
1. Review all changed files for security vulnerabilities
2. Check OWASP Top 10: injection, XSS, broken auth, CSRF, SSRF
3. Verify input validation and output sanitization
4. Check for hardcoded secrets, API keys, credentials
5. Review new dependencies for known CVEs
6. Check authentication/authorization patterns
7. Review error handling — no sensitive info leaked in errors
8. Report findings by severity: CRITICAL / HIGH / MEDIUM / LOW
9. If clean: explicitly state "No security issues found"

Create tracker issues for any findings with priority=critical or priority=high."#;

const PROMPT_PENTEST: &str = r#"You are the PENETRATION TESTER stage of a factory pipeline.

## Task
Pentest: {{task}}

## Project: {{project}} ({{project_path}})

## Predecessor Results
(Auto-injected by the queue system from the developer agent's output)

## Instructions
1. Attempt injection attacks (SQL, XSS, command injection) on any new endpoints
2. Test for path traversal and unauthorized file access
3. Check for broken authentication/session management
4. Test rate limiting and resource exhaustion vectors
5. Verify authorization boundaries (horizontal and vertical privilege escalation)
6. Report exploits found with severity and remediation steps"#;

const PROMPT_REVIEW: &str = r#"You are the REVIEWER — the final gate of a factory pipeline.

## Task
Final review: {{task}}

## Project: {{project}} ({{project_path}})

## All Stage Results
(Auto-injected by the queue system from all predecessor stages)

## Instructions
1. Review the developer's code for quality, correctness, architecture
2. Verify QA tests passed — flag if they didn't
3. Review security findings — verify critical/high issues were addressed
4. Check code style, naming conventions, documentation
5. Create a pull request if not already done
6. Merge if everything passes
7. Final verdict: APPROVE or REQUEST_CHANGES with specific items"#;

// Static template definitions
const TMPL_FULL: PipelineTemplate = PipelineTemplate {
    name: "full",
    stages: &[
        StageTemplate { name: "dev", role: "developer", parallel_with: &[], prompt: PROMPT_DEV },
        StageTemplate { name: "qa", role: "qa", parallel_with: &["security"], prompt: PROMPT_QA },
        StageTemplate { name: "security", role: "security", parallel_with: &["qa"], prompt: PROMPT_SECURITY },
        StageTemplate { name: "review", role: "reviewer", parallel_with: &[], prompt: PROMPT_REVIEW },
    ],
};

const TMPL_QUICK: PipelineTemplate = PipelineTemplate {
    name: "quick",
    stages: &[
        StageTemplate { name: "dev", role: "developer", parallel_with: &[], prompt: PROMPT_DEV },
        StageTemplate { name: "qa", role: "qa", parallel_with: &[], prompt: PROMPT_QA },
    ],
};

const TMPL_SECURE: PipelineTemplate = PipelineTemplate {
    name: "secure",
    stages: &[
        StageTemplate { name: "dev", role: "developer", parallel_with: &[], prompt: PROMPT_DEV },
        StageTemplate { name: "qa", role: "qa", parallel_with: &["security", "pentest"], prompt: PROMPT_QA },
        StageTemplate { name: "security", role: "security", parallel_with: &["qa", "pentest"], prompt: PROMPT_SECURITY },
        StageTemplate { name: "pentest", role: "security", parallel_with: &["qa", "security"], prompt: PROMPT_PENTEST },
        StageTemplate { name: "review", role: "reviewer", parallel_with: &[], prompt: PROMPT_REVIEW },
    ],
};

const ALL_TEMPLATES: &[&PipelineTemplate] = &[&TMPL_FULL, &TMPL_QUICK, &TMPL_SECURE];

pub fn template_names() -> Vec<&'static str> {
    ALL_TEMPLATES.iter().map(|t| t.name).collect()
}

pub fn template_info() -> Vec<(&'static str, Vec<&'static str>)> {
    ALL_TEMPLATES.iter().map(|t| {
        let stages: Vec<&str> = t.stages.iter().map(|s| s.name).collect();
        (t.name, stages)
    }).collect()
}

// ============================================================
// Pipeline creation
// ============================================================

/// Create a multi-stage pipeline. Returns (pipeline_id, task_ids).
pub fn create_pipeline(
    project: &str,
    description: &str,
    template_name: &str,
    priority: u8,
) -> Result<(String, Vec<String>)> {
    let template = ALL_TEMPLATES.iter()
        .find(|t| t.name == template_name)
        .ok_or_else(|| anyhow::anyhow!(
            "Unknown template '{}'. Available: {}",
            template_name,
            template_names().join(", ")
        ))?;

    // Resolve project path for prompt enrichment
    let project_path = scanner::project_by_name(project)
        .map(|p| p.path.clone())
        .unwrap_or_default();

    let pipeline_id = gen_pipeline_id();
    let groups = build_stage_groups(template.stages);
    let mut task_ids: Vec<String> = Vec::new();
    let mut prev_group_ids: Vec<String> = Vec::new();

    for group in &groups {
        let mut group_ids: Vec<String> = Vec::new();

        for stage in group {
            let prompt = stage.prompt
                .replace("{{task}}", description)
                .replace("{{project}}", project)
                .replace("{{project_path}}", &project_path);

            let task_label = format!("[{}] {}", stage.name, description);

            let task = queue::add_task_with_pipeline(
                project,
                stage.role,
                &task_label,
                &prompt,
                priority,
                prev_group_ids.clone(),
                Some(pipeline_id.clone()),
            )?;

            group_ids.push(task.id.clone());
            task_ids.push(task.id);
        }

        prev_group_ids = group_ids;
    }

    log_pipeline_event(&pipeline_id, "created",
        &format!("template={}, stages={}, project={}", template_name, task_ids.len(), project));

    Ok((pipeline_id, task_ids))
}

/// Group stages: stages with parallel_with links form a group.
/// Groups execute sequentially; stages within a group run in parallel.
fn build_stage_groups<'a>(stages: &'a [StageTemplate]) -> Vec<Vec<&'a StageTemplate>> {
    let mut groups: Vec<Vec<&'a StageTemplate>> = Vec::new();
    let mut assigned: HashSet<&str> = HashSet::new();

    for stage in stages {
        if assigned.contains(stage.name) {
            continue;
        }

        let mut group = vec![stage];
        assigned.insert(stage.name);

        for parallel_name in stage.parallel_with {
            if assigned.contains(parallel_name) {
                continue;
            }
            if let Some(ps) = stages.iter().find(|s| s.name == *parallel_name) {
                group.push(ps);
                assigned.insert(ps.name);
            }
        }

        groups.push(group);
    }

    groups
}

fn gen_pipeline_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    let hash: u32 = (ts as u32).wrapping_mul(2654435761);
    format!("pipe_{}_{:04x}", ts % 10_000_000, hash % 0xFFFF)
}

// ============================================================
// Pipeline status (derived from queue)
// ============================================================

/// List all pipelines, derived from queue tasks grouped by pipeline_id.
/// Build a PipelineView from a list of tasks sharing a pipeline_id.
fn build_pipeline_view(id: String, tasks: &[&queue::QueueTask]) -> PipelineView {
    let stages: Vec<StageView> = tasks.iter().map(|t| {
        let name = t.task.strip_prefix('[')
            .and_then(|s| s.split(']').next())
            .unwrap_or("?")
            .to_string();
        let summary = t.result.as_ref().map(|r| r.chars().take(60).collect());
        StageView {
            name,
            role: t.role.clone(),
            task_id: t.id.clone(),
            status: format!("{:?}", t.status).to_lowercase(),
            pane: t.pane,
            summary,
        }
    }).collect();

    let project = tasks.first().map(|t| t.project.clone()).unwrap_or_default();
    let description = tasks.first()
        .map(|t| t.task.split(']').last().unwrap_or(&t.task).trim().to_string())
        .unwrap_or_default();
    let created_at = tasks.first().map(|t| t.added_at.clone()).unwrap_or_default();

    let status = if tasks.iter().any(|t| t.status == queue::QueueStatus::Failed) {
        "failed"
    } else if tasks.iter().all(|t| t.status == queue::QueueStatus::Done) {
        "done"
    } else if tasks.iter().any(|t| t.status == queue::QueueStatus::Running) {
        "running"
    } else {
        "pending"
    }.to_string();

    let stage_names: Vec<&str> = stages.iter().map(|s| s.name.as_str()).collect();
    let template = if stage_names.iter().any(|n| *n == "pentest") { "secure" }
        else if stage_names.iter().any(|n| *n == "review") { "full" }
        else if stage_names.len() <= 2 { "quick" }
        else { "custom" }.to_string();

    PipelineView { id, project, description, template, created_at, status, stages }
}

pub fn list_pipelines() -> Vec<PipelineView> {
    let q = queue::load_queue();
    let mut map: HashMap<String, Vec<&queue::QueueTask>> = HashMap::new();

    for task in &q.tasks {
        if let Some(ref pid) = task.pipeline_id {
            map.entry(pid.clone()).or_default().push(task);
        }
    }

    let mut pipelines: Vec<PipelineView> = map.into_iter()
        .map(|(pid, tasks)| build_pipeline_view(pid, &tasks))
        .collect();

    pipelines.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    pipelines
}

/// Get a single pipeline by ID. Direct lookup — only loads tasks for this pipeline.
pub fn get_pipeline(pipeline_id: &str) -> Option<PipelineView> {
    let q = queue::load_queue();
    let tasks: Vec<&queue::QueueTask> = q.tasks.iter()
        .filter(|t| t.pipeline_id.as_deref() == Some(pipeline_id))
        .collect();

    if tasks.is_empty() {
        return None;
    }

    Some(build_pipeline_view(pipeline_id.to_string(), &tasks))
}

/// Cancel a pipeline: mark all pending/blocked stages as failed, return running pane IDs for killing.
pub fn cancel_pipeline(pipeline_id: &str) -> Result<CancelResult> {
    let mut q = queue::load_queue();
    let mut cancelled = 0u32;
    let mut running_panes: Vec<u8> = Vec::new();

    for task in q.tasks.iter_mut() {
        if task.pipeline_id.as_deref() != Some(pipeline_id) {
            continue;
        }
        match task.status {
            queue::QueueStatus::Pending | queue::QueueStatus::Blocked => {
                task.status = queue::QueueStatus::Failed;
                task.last_error = Some("Pipeline cancelled".to_string());
                task.completed_at = Some(chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string());
                cancelled += 1;
            }
            queue::QueueStatus::Running => {
                task.status = queue::QueueStatus::Failed;
                task.last_error = Some("Pipeline cancelled".to_string());
                task.completed_at = Some(chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string());
                if let Some(pane) = task.pane {
                    running_panes.push(pane);
                }
                cancelled += 1;
            }
            _ => {} // Done/Failed already — leave as-is
        }
    }

    queue::save_queue(&q)?;
    log_pipeline_event(pipeline_id, "cancelled",
        &format!("cancelled={}, killed_panes={:?}", cancelled, running_panes));

    Ok(CancelResult {
        pipeline_id: pipeline_id.to_string(),
        cancelled_tasks: cancelled,
        running_panes,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelResult {
    pub pipeline_id: String,
    pub cancelled_tasks: u32,
    pub running_panes: Vec<u8>,
}

/// Retry failed stages in a pipeline: reset failed tasks back to pending.
pub fn retry_pipeline(pipeline_id: &str) -> Result<RetryResult> {
    let mut q = queue::load_queue();
    let mut retried = 0u32;
    let mut task_ids: Vec<String> = Vec::new();

    for task in q.tasks.iter_mut() {
        if task.pipeline_id.as_deref() != Some(pipeline_id) { continue; }
        if task.status == queue::QueueStatus::Failed {
            task.status = queue::QueueStatus::Pending;
            task.retry_count += 1;
            task.last_error = None;
            task.pane = None;
            task.started_at = None;
            task.completed_at = None;
            task.result = None;
            task_ids.push(task.id.clone());
            retried += 1;
        }
    }

    if retried == 0 {
        anyhow::bail!("No failed stages in pipeline '{}' to retry", pipeline_id);
    }

    queue::save_queue(&q)?;
    log_pipeline_event(pipeline_id, "retry", &format!("Retried {} failed stages", retried));

    Ok(RetryResult { pipeline_id: pipeline_id.to_string(), retried_tasks: retried, task_ids })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryResult {
    pub pipeline_id: String,
    pub retried_tasks: u32,
    pub task_ids: Vec<String>,
}

// ============================================================
// Pipeline Events Log
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineEvent {
    pub timestamp: String,
    pub pipeline_id: String,
    pub event: String,
    pub detail: String,
}

/// Append an event to the pipeline events log.
pub fn log_pipeline_event(pipeline_id: &str, event: &str, detail: &str) {
    let entry = PipelineEvent {
        timestamp: chrono::Utc::now().to_rfc3339(),
        pipeline_id: pipeline_id.to_string(),
        event: event.to_string(),
        detail: detail.to_string(),
    };

    let log_dir = crate::config::dx_root().join("pipeline_events");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_file = log_dir.join(format!("{}.jsonl", pipeline_id));

    if let Ok(line) = serde_json::to_string(&entry) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&log_file) {
            let _ = writeln!(f, "{}", line);
        }
    }
}

/// Read all events for a pipeline.
pub fn get_pipeline_events(pipeline_id: &str) -> Vec<PipelineEvent> {
    let log_file = crate::config::dx_root()
        .join("pipeline_events")
        .join(format!("{}.jsonl", pipeline_id));

    match std::fs::read_to_string(&log_file) {
        Ok(content) => content.lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect(),
        Err(_) => Vec::new(),
    }
}

// ============================================================
// Classification (rule-based project detection)
// ============================================================

/// Try to detect which project a request is about from the description text.
/// Returns (project_name, confidence) or None.
pub fn detect_project(description: &str) -> Option<(String, f32)> {
    let registry = scanner::load_registry();
    let lower = description.to_lowercase();
    let mut scores: Vec<(String, u32)> = Vec::new();

    for project in &registry.projects {
        let mut score: u32 = 0;
        let name_lower = project.name.to_lowercase();

        if lower.contains(&name_lower) { score += 100; }
        for word in lower.split_whitespace() {
            if word.len() >= 3 && name_lower.contains(word) { score += 50; }
        }
        for tech in &project.tech {
            if lower.contains(&tech.to_lowercase()) { score += 30; }
        }

        if score > 0 {
            scores.push((project.name.clone(), score));
        }
    }

    scores.sort_by(|a, b| b.1.cmp(&a.1));
    scores.first().map(|(name, score)| (name.clone(), (*score as f32 / 150.0).min(1.0)))
}

// ============================================================
// Quality Gates
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub pipeline_id: String,
    pub project: String,
    pub build: Option<GateCheck>,
    pub test: Option<GateCheck>,
    pub lint: Option<GateCheck>,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateCheck {
    pub command: String,
    pub success: bool,
    pub output: String,
    pub duration_ms: u64,
}

/// Run quality gates for a pipeline's project.
/// Executes build, test, and lint commands (if configured in scanner).
/// Returns results so auto_cycle can decide whether to proceed.
pub fn run_gate(pipeline_id: &str) -> Result<GateResult> {
    let pipeline = get_pipeline(pipeline_id)
        .ok_or_else(|| anyhow::anyhow!("Pipeline '{}' not found", pipeline_id))?;

    let project_info = scanner::project_by_name(&pipeline.project)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found in scanner", pipeline.project))?;

    let project_path = &project_info.path;
    let mut result = GateResult {
        pipeline_id: pipeline_id.to_string(),
        project: pipeline.project.clone(),
        build: None,
        test: None,
        lint: None,
        passed: true,
    };

    // Run build check
    if let Some(ref cmd) = project_info.build_cmd {
        let check = run_check(project_path, cmd);
        if !check.success { result.passed = false; }
        result.build = Some(check);
    }

    // Run test check
    if let Some(ref cmd) = project_info.test_cmd {
        let check = run_check(project_path, cmd);
        if !check.success { result.passed = false; }
        result.test = Some(check);
    }

    // Run lint check
    if let Some(ref cmd) = project_info.lint_cmd {
        let check = run_check(project_path, cmd);
        // Lint failures are warnings, don't fail the gate
        result.lint = Some(check);
    }

    // Save gate result
    let gate_path = std::path::PathBuf::from(
        crate::config::dx_root().join("gates")
    );
    let _ = std::fs::create_dir_all(&gate_path);
    let gate_file = gate_path.join(format!("{}.json", pipeline_id));
    let _ = std::fs::write(&gate_file, serde_json::to_string_pretty(&result).unwrap_or_default());

    log_pipeline_event(pipeline_id, "gate",
        &format!("passed={}, build={}, test={}, lint={}",
            result.passed,
            result.build.as_ref().map(|c| c.success).unwrap_or(true),
            result.test.as_ref().map(|c| c.success).unwrap_or(true),
            result.lint.as_ref().map(|c| c.success).unwrap_or(true)));

    Ok(result)
}

/// Gate command timeout (5 minutes)
const GATE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// Run a single command in a project directory, capturing output and timing.
/// Times out after 5 minutes to prevent hanging the auto_cycle.
fn run_check(project_path: &str, command: &str) -> GateCheck {
    let start = std::time::Instant::now();
    let child = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(project_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            return GateCheck {
                command: command.to_string(),
                success: false,
                output: format!("Failed to spawn: {}", e),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }
    };

    // Poll with timeout
    let deadline = std::time::Instant::now() + GATE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                let stdout = child.stdout.take()
                    .map(|mut s| { let mut b = Vec::new(); std::io::Read::read_to_end(&mut s, &mut b).ok(); b })
                    .unwrap_or_default();
                let stderr = child.stderr.take()
                    .map(|mut s| { let mut b = Vec::new(); std::io::Read::read_to_end(&mut s, &mut b).ok(); b })
                    .unwrap_or_default();
                let combined: String = format!(
                    "{}{}",
                    String::from_utf8_lossy(&stdout),
                    String::from_utf8_lossy(&stderr),
                ).chars().take(2000).collect();

                return GateCheck {
                    command: command.to_string(),
                    success: status.success(),
                    output: combined,
                    duration_ms,
                };
            }
            Ok(None) => {
                if std::time::Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return GateCheck {
                        command: command.to_string(),
                        success: false,
                        output: format!("TIMEOUT: command exceeded {}s limit", GATE_TIMEOUT.as_secs()),
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => {
                return GateCheck {
                    command: command.to_string(),
                    success: false,
                    output: format!("Wait error: {}", e),
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        }
    }
}

/// Get the most recent gate result for a pipeline.
pub fn get_gate_result(pipeline_id: &str) -> Option<GateResult> {
    let gate_file = crate::config::dx_root()
        .join("gates")
        .join(format!("{}.json", pipeline_id));
    let data = std::fs::read_to_string(&gate_file).ok()?;
    serde_json::from_str(&data).ok()
}

// ============================================================
// Pipeline Coordination
// ============================================================

/// Generate coordination instructions for an agent in a pipeline.
/// This tells the agent about other agents working on the same pipeline,
/// what files are locked, and how to communicate.
pub fn coordination_context(pipeline_id: &str, pane: u8, role: &str) -> String {
    let pipeline = match get_pipeline(pipeline_id) {
        Some(p) => p,
        None => return String::new(),
    };

    let mut lines = Vec::new();
    lines.push(format!("## Pipeline Coordination ({})", pipeline_id));
    lines.push(format!("You are the {} agent in pipeline '{}'.", role, pipeline.template));
    lines.push(format!("Project: {}", pipeline.project));
    lines.push(String::new());

    // Show other agents in the pipeline
    let other_stages: Vec<&StageView> = pipeline.stages.iter()
        .filter(|s| s.pane != Some(pane))
        .collect();

    if !other_stages.is_empty() {
        lines.push("### Other Agents in This Pipeline".to_string());
        for stage in &other_stages {
            let pane_str = stage.pane.map(|p| format!(" (pane {})", p)).unwrap_or_default();
            lines.push(format!("- {} [{}]{}: {}", stage.name, stage.status, pane_str, stage.role));
        }
        lines.push(String::new());
    }

    // Coordination rules
    lines.push("### Coordination Rules".to_string());
    match role {
        "developer" => {
            lines.push("- You are the primary builder. QA and security agents will review your work.".to_string());
            lines.push("- Use `lock_acquire` before editing critical files.".to_string());
            lines.push("- Use `kb_add` to document key decisions, API contracts, and architecture choices.".to_string());
            lines.push("- Commit with clear messages — QA/security agents will read your git log.".to_string());
        }
        "qa" => {
            lines.push("- DO NOT modify the developer's code unless fixing a test.".to_string());
            lines.push("- Check `kb_search` for architecture decisions before questioning implementation.".to_string());
            lines.push("- Use `lock_acquire` before creating new test files.".to_string());
            lines.push("- Report findings via `kb_add` category='qa_finding'.".to_string());
            lines.push("- If you find bugs, create tracker issues, don't fix them yourself.".to_string());
        }
        "security" => {
            lines.push("- DO NOT modify any code. Report only.".to_string());
            lines.push("- Check `kb_search` for known decisions before flagging as vulnerability.".to_string());
            lines.push("- Report findings via `kb_add` category='security_finding'.".to_string());
            lines.push("- Create tracker issues for CRITICAL and HIGH severity findings.".to_string());
            lines.push("- Classify severity: CRITICAL / HIGH / MEDIUM / LOW / INFO.".to_string());
        }
        "reviewer" => {
            lines.push("- Review all KB entries from dev, QA, and security agents.".to_string());
            lines.push("- Check git log for all changes in this pipeline.".to_string());
            lines.push("- Create PR if not already done. Merge if all checks pass.".to_string());
            lines.push("- If issues found, create tracker issues and mark pipeline as needs_review.".to_string());
        }
        _ => {}
    }

    // Coordination tools reminder
    lines.push(String::new());
    lines.push("### Available Coordination Tools".to_string());
    lines.push("- `lock_acquire(files=[...])` — Lock files before editing".to_string());
    lines.push("- `lock_release(files=[...])` — Release locks when done".to_string());
    lines.push("- `lock_check(files=[...])` — Check if files are locked".to_string());
    lines.push("- `kb_add(category, title, content)` — Share knowledge with other agents".to_string());
    lines.push("- `kb_search(query)` — Find knowledge from other agents".to_string());
    lines.push("- `msg_send(to_pane, message)` — Direct message another agent".to_string());
    lines.push("- `conflict_scan()` — Check for git conflicts".to_string());

    lines.join("\n")
}

/// Check for git conflicts between pipeline agents' branches.
pub fn conflict_scan(pipeline_id: &str) -> serde_json::Value {
    let pipeline = match get_pipeline(pipeline_id) {
        Some(p) => p,
        None => return serde_json::json!({"error": "pipeline not found"}),
    };

    let project_info = match scanner::project_by_name(&pipeline.project) {
        Some(p) => p,
        None => return serde_json::json!({"error": "project not in scanner"}),
    };

    // Check if any active stages have uncommitted changes that might conflict
    let mut conflicts = Vec::new();
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&project_info.path)
        .output();

    if let Ok(out) = output {
        let status = String::from_utf8_lossy(&out.stdout);
        let modified_files: Vec<&str> = status.lines()
            .filter(|l| l.starts_with(" M") || l.starts_with("M ") || l.starts_with("MM"))
            .map(|l| l[3..].trim())
            .collect();

        if !modified_files.is_empty() {
            conflicts.push(serde_json::json!({
                "type": "uncommitted_changes",
                "files": modified_files,
                "warning": "Multiple agents may be editing these files",
            }));
        }
    }

    serde_json::json!({
        "pipeline_id": pipeline_id,
        "project": pipeline.project,
        "conflicts": conflicts,
        "clean": conflicts.is_empty(),
    })
}

// ============================================================
// Pipeline Pause/Resume
// ============================================================

fn paused_file() -> std::path::PathBuf {
    crate::config::dx_root().join("paused_pipelines.json")
}

fn load_paused() -> HashSet<String> {
    std::fs::read_to_string(paused_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_paused(set: &HashSet<String>) -> Result<()> {
    let data = serde_json::to_string(set)?;
    std::fs::write(paused_file(), data)?;
    Ok(())
}

/// Pause a pipeline — auto_cycle will skip its tasks.
pub fn pause_pipeline(pipeline_id: &str) -> Result<()> {
    let mut paused = load_paused();
    paused.insert(pipeline_id.to_string());
    save_paused(&paused)
}

/// Resume a paused pipeline.
pub fn resume_pipeline(pipeline_id: &str) -> Result<()> {
    let mut paused = load_paused();
    paused.remove(pipeline_id);
    save_paused(&paused)
}

/// Check if a pipeline is paused.
pub fn is_pipeline_paused(pipeline_id: &str) -> bool {
    load_paused().contains(pipeline_id)
}

/// Retry a specific stage in a pipeline by name (e.g., "dev", "qa").
pub fn retry_stage(pipeline_id: &str, stage_name: &str) -> Result<String> {
    let mut q = queue::load_queue();

    // Find the task matching this pipeline + stage
    let task_id = q.tasks.iter()
        .find(|t| {
            t.pipeline_id.as_deref() == Some(pipeline_id) &&
            t.task.starts_with(&format!("[{}]", stage_name))
        })
        .map(|t| t.id.clone())
        .ok_or_else(|| anyhow::anyhow!("Stage '{}' not found in pipeline '{}'", stage_name, pipeline_id))?;

    // Reset this task and all cascade-failed dependents
    let mut reset_ids = vec![task_id.clone()];
    loop {
        let new_ids: Vec<String> = q.tasks.iter()
            .filter(|t| {
                t.status == queue::QueueStatus::Failed &&
                t.depends_on.iter().any(|d| reset_ids.contains(d)) &&
                !reset_ids.contains(&t.id)
            })
            .map(|t| t.id.clone())
            .collect();
        if new_ids.is_empty() { break; }
        reset_ids.extend(new_ids);
    }

    for t in q.tasks.iter_mut() {
        if reset_ids.contains(&t.id) {
            t.status = queue::QueueStatus::Pending;
            t.last_error = None;
            t.result = None;
            t.started_at = None;
            t.completed_at = None;
            t.pane = None;
        }
    }
    queue::save_queue(&q)?;

    Ok(format!("Reset {} tasks (stage '{}' + dependents)", reset_ids.len(), stage_name))
}

// ============================================================
// Factory Inbox — bridge from TUI/hub_mcp to pipeline system
// ============================================================

/// A factory inbox request (as written by hub_mcp POST /api/factory/submit)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxRequest {
    pub id: String,
    pub request: String,
    #[serde(default = "default_inbox_status")]
    pub status: String,
    #[serde(default)]
    pub classification: serde_json::Value,
    #[serde(default)]
    pub tasks: Vec<serde_json::Value>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub pipeline_id: Option<String>,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

fn default_inbox_status() -> String { "pending".into() }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FactoryInbox {
    #[serde(default)]
    pub requests: Vec<InboxRequest>,
}

fn inbox_path() -> std::path::PathBuf {
    crate::config::dx_root().join("factory_inbox.json")
}

pub fn load_inbox() -> FactoryInbox {
    let path = inbox_path();
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(inbox) = serde_json::from_str(&content) {
                return inbox;
            }
        }
    }
    FactoryInbox::default()
}

pub fn save_inbox(inbox: &FactoryInbox) -> Result<()> {
    let path = inbox_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(inbox)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Process the factory inbox: convert pending requests into pipelines,
/// update running requests with pipeline status.
/// Returns a list of actions taken (for auto_cycle logging).
pub fn process_inbox() -> Vec<serde_json::Value> {
    let mut inbox = load_inbox();
    let mut actions = Vec::new();
    let mut changed = false;

    for req in inbox.requests.iter_mut() {
        match req.status.as_str() {
            "pending" => {
                // Classify: detect project from request text
                let (project, confidence) = match detect_project(&req.request) {
                    Some(pc) => pc,
                    None => {
                        req.status = "failed".into();
                        req.error = Some("Could not identify project from request".into());
                        changed = true;
                        actions.push(serde_json::json!({
                            "action": "inbox_classify_fail",
                            "inbox_id": req.id,
                            "request": req.request,
                        }));
                        continue;
                    }
                };

                if confidence < 0.2 {
                    req.status = "failed".into();
                    req.error = Some(format!(
                        "Low confidence ({:.0}%) match to '{}'. Be more specific.",
                        confidence * 100.0, project
                    ));
                    changed = true;
                    actions.push(serde_json::json!({
                        "action": "inbox_low_confidence",
                        "inbox_id": req.id,
                        "project": project,
                        "confidence": format!("{:.0}%", confidence * 100.0),
                    }));
                    continue;
                }

                // Create pipeline
                let template = req.template.as_deref().unwrap_or("full");
                match create_pipeline(&project, &req.request, template, 2) {
                    Ok((pipeline_id, task_ids)) => {
                        req.status = "running".into();
                        req.pipeline_id = Some(pipeline_id.clone());
                        req.classification = serde_json::json!({
                            "project": project,
                            "confidence": format!("{:.0}%", confidence * 100.0),
                        });
                        req.tasks = task_ids.iter().map(|id| {
                            serde_json::json!({"task_id": id})
                        }).collect();
                        changed = true;
                        actions.push(serde_json::json!({
                            "action": "inbox_pipeline_created",
                            "inbox_id": req.id,
                            "pipeline_id": pipeline_id,
                            "project": project,
                            "template": template,
                            "task_count": task_ids.len(),
                        }));
                    }
                    Err(e) => {
                        req.status = "failed".into();
                        req.error = Some(format!("Pipeline creation failed: {}", e));
                        changed = true;
                        actions.push(serde_json::json!({
                            "action": "inbox_pipeline_fail",
                            "inbox_id": req.id,
                            "error": e.to_string(),
                        }));
                    }
                }
            }
            "running" => {
                // Check if the pipeline is done
                if let Some(ref pid) = req.pipeline_id {
                    if let Some(pipeline) = get_pipeline(pid) {
                        match pipeline.status.as_str() {
                            "done" => {
                                req.status = "complete".into();
                                req.tasks = pipeline.stages.iter().map(|s| {
                                    serde_json::json!({
                                        "task_id": s.task_id,
                                        "stage": s.name,
                                        "role": s.role,
                                        "status": s.status,
                                    })
                                }).collect();
                                changed = true;
                                actions.push(serde_json::json!({
                                    "action": "inbox_complete",
                                    "inbox_id": req.id,
                                    "pipeline_id": pid,
                                }));
                            }
                            "failed" => {
                                req.status = "failed".into();
                                req.error = Some("Pipeline failed".into());
                                req.tasks = pipeline.stages.iter().map(|s| {
                                    serde_json::json!({
                                        "task_id": s.task_id,
                                        "stage": s.name,
                                        "role": s.role,
                                        "status": s.status,
                                        "summary": s.summary,
                                    })
                                }).collect();
                                changed = true;
                                actions.push(serde_json::json!({
                                    "action": "inbox_failed",
                                    "inbox_id": req.id,
                                    "pipeline_id": pid,
                                }));
                            }
                            _ => {
                                // Still running — update task statuses for TUI display
                                let new_tasks: Vec<serde_json::Value> = pipeline.stages.iter().map(|s| {
                                    serde_json::json!({
                                        "task_id": s.task_id,
                                        "stage": s.name,
                                        "role": s.role,
                                        "status": s.status,
                                        "pane": s.pane,
                                    })
                                }).collect();
                                if serde_json::to_string(&new_tasks).unwrap_or_default()
                                    != serde_json::to_string(&req.tasks).unwrap_or_default()
                                {
                                    req.tasks = new_tasks;
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
            _ => {} // "complete", "failed" — no action needed
        }
    }

    if changed {
        let _ = save_inbox(&inbox);
    }

    actions
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stage_grouping_full() {
        let groups = build_stage_groups(TMPL_FULL.stages);
        assert_eq!(groups.len(), 3); // dev | qa+security | review
        assert_eq!(groups[0].len(), 1); // dev alone
        assert_eq!(groups[1].len(), 2); // qa + security parallel
        assert_eq!(groups[2].len(), 1); // review alone
    }

    #[test]
    fn test_stage_grouping_quick() {
        let groups = build_stage_groups(TMPL_QUICK.stages);
        assert_eq!(groups.len(), 2); // dev | qa
        assert_eq!(groups[0].len(), 1);
        assert_eq!(groups[1].len(), 1);
    }

    #[test]
    fn test_stage_grouping_secure() {
        let groups = build_stage_groups(TMPL_SECURE.stages);
        assert_eq!(groups.len(), 3); // dev | qa+security+pentest | review
        assert_eq!(groups[0].len(), 1);
        assert_eq!(groups[1].len(), 3); // qa + security + pentest parallel
        assert_eq!(groups[2].len(), 1);
    }

    #[test]
    fn test_template_names() {
        let names = template_names();
        assert!(names.contains(&"full"));
        assert!(names.contains(&"quick"));
        assert!(names.contains(&"secure"));
    }

    #[test]
    fn test_pipeline_id_format() {
        let id = gen_pipeline_id();
        assert!(id.starts_with("pipe_"));
        assert!(id.len() > 10);
    }

    #[test]
    fn test_pipeline_id_uniqueness() {
        let ids: Vec<String> = (0..100).map(|_| {
            std::thread::sleep(std::time::Duration::from_millis(1));
            gen_pipeline_id()
        }).collect();
        let unique: HashSet<&str> = ids.iter().map(|s| s.as_str()).collect();
        // At least 90% unique (timing jitter may cause rare collisions)
        assert!(unique.len() > 90, "Only {} unique out of 100", unique.len());
    }

    #[test]
    fn test_template_info_structure() {
        let info = template_info();
        assert_eq!(info.len(), 3);
        for (name, stages) in &info {
            assert!(!name.is_empty());
            assert!(!stages.is_empty());
            // Every template has "dev" as first stage
            assert_eq!(stages[0], "dev");
        }
    }

    #[test]
    fn test_template_full_has_review() {
        let info = template_info();
        let full = info.iter().find(|(n, _)| *n == "full").unwrap();
        assert!(full.1.contains(&"review"));
        assert!(full.1.contains(&"qa"));
        assert!(full.1.contains(&"security"));
    }

    #[test]
    fn test_template_secure_has_pentest() {
        let info = template_info();
        let secure = info.iter().find(|(n, _)| *n == "secure").unwrap();
        assert!(secure.1.contains(&"pentest"));
        assert!(secure.1.contains(&"review"));
    }

    #[test]
    fn test_gate_result_serialization() {
        let gate = GateResult {
            pipeline_id: "pipe_123_abcd".to_string(),
            project: "test-project".to_string(),
            build: Some(GateCheck {
                command: "cargo build".to_string(),
                success: true,
                output: "Finished".to_string(),
                duration_ms: 1500,
            }),
            test: Some(GateCheck {
                command: "cargo test".to_string(),
                success: false,
                output: "1 test failed".to_string(),
                duration_ms: 3000,
            }),
            lint: None,
            passed: false,
        };
        let json = serde_json::to_string(&gate).unwrap();
        let restored: GateResult = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.pipeline_id, "pipe_123_abcd");
        assert!(!restored.passed);
        assert!(restored.build.unwrap().success);
        assert!(!restored.test.unwrap().success);
        assert!(restored.lint.is_none());
    }

    #[test]
    fn test_cancel_result_serialization() {
        let result = CancelResult {
            pipeline_id: "pipe_999_beef".to_string(),
            cancelled_tasks: 3,
            running_panes: vec![2, 5],
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("pipe_999_beef"));
        assert!(json.contains("\"cancelled_tasks\":3"));
    }

    #[test]
    fn test_stage_view_status_extraction() {
        // Simulate what list_pipelines does to extract stage name from task label
        let task_text = "[dev] Add OAuth login";
        let name = task_text.strip_prefix('[')
            .and_then(|s| s.split(']').next())
            .unwrap_or("?");
        assert_eq!(name, "dev");

        let desc = task_text.split(']').last().unwrap_or(task_text).trim();
        assert_eq!(desc, "Add OAuth login");
    }

    #[test]
    fn test_stage_view_malformed_label() {
        // Edge case: task label without brackets
        let task_text = "plain task";
        let name = task_text.strip_prefix('[')
            .and_then(|s| s.split(']').next())
            .unwrap_or("?");
        assert_eq!(name, "?");
    }

    #[test]
    fn test_coordination_context_empty_pipeline() {
        // coordination_context with non-existent pipeline returns empty
        let ctx = coordination_context("nonexistent_pipe", 1, "developer");
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_prompt_template_placeholders() {
        // Verify all prompt templates have the expected placeholders
        let prompts = [PROMPT_DEV, PROMPT_QA, PROMPT_SECURITY, PROMPT_PENTEST, PROMPT_REVIEW];
        for prompt in &prompts {
            assert!(prompt.contains("{{task}}"), "Missing {{{{task}}}} in prompt");
            assert!(prompt.contains("{{project}}"), "Missing {{{{project}}}} in prompt");
            assert!(prompt.contains("{{project_path}}"), "Missing {{{{project_path}}}} in prompt");
        }
    }

    #[test]
    fn test_all_stages_have_prompts() {
        for tmpl in ALL_TEMPLATES {
            for stage in tmpl.stages {
                assert!(!stage.prompt.is_empty(), "Stage '{}' in template '{}' has empty prompt", stage.name, tmpl.name);
                assert!(!stage.role.is_empty(), "Stage '{}' in template '{}' has empty role", stage.name, tmpl.name);
            }
        }
    }

    #[test]
    fn test_parallel_stages_are_bidirectional() {
        for tmpl in ALL_TEMPLATES {
            for stage in tmpl.stages {
                for parallel_name in stage.parallel_with {
                    let partner = tmpl.stages.iter().find(|s| s.name == *parallel_name);
                    assert!(partner.is_some(),
                        "Stage '{}' references non-existent parallel partner '{}'", stage.name, parallel_name);
                    let partner = partner.unwrap();
                    assert!(partner.parallel_with.contains(&stage.name),
                        "Stage '{}' lists '{}' as parallel, but '{}' doesn't list '{}' back",
                        stage.name, parallel_name, parallel_name, stage.name);
                }
            }
        }
    }

    #[test]
    fn test_retry_result_serialization() {
        let result = RetryResult {
            pipeline_id: "pipe_123".to_string(),
            retried_tasks: 2,
            task_ids: vec!["t1".to_string(), "t2".to_string()],
        };
        let json = serde_json::to_string(&result).unwrap();
        let restored: RetryResult = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.retried_tasks, 2);
        assert_eq!(restored.task_ids.len(), 2);
    }

    #[test]
    fn test_pipeline_event_serialization() {
        let event = PipelineEvent {
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            pipeline_id: "pipe_x".to_string(),
            event: "created".to_string(),
            detail: "template=full".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let restored: PipelineEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.event, "created");
        assert_eq!(restored.pipeline_id, "pipe_x");
    }

    #[test]
    fn test_pipeline_events_log_and_read() {
        let _g = crate::queue::tests::env_lock();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("DX_ROOT", tmp.path());

        // Initially empty
        assert!(get_pipeline_events("test_pipe").is_empty());

        // Log some events
        log_pipeline_event("test_pipe", "created", "template=full, stages=4");
        log_pipeline_event("test_pipe", "gate", "passed=true");

        let events = get_pipeline_events("test_pipe");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event, "created");
        assert_eq!(events[1].event, "gate");

        // Different pipeline has no events
        assert!(get_pipeline_events("other_pipe").is_empty());

        std::env::remove_var("DX_ROOT");
    }

    #[test]
    fn test_retry_pipeline_with_queue() {
        let _g = crate::queue::tests::env_lock();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("DX_ROOT", tmp.path());

        // Create tasks with pipeline_id
        let t1 = queue::add_task_with_pipeline("p", "dev", "[dev] build", "go", 1, vec![], Some("retry_test".into())).unwrap();
        let _t2 = queue::add_task_with_pipeline("p", "qa", "[qa] test", "go", 1, vec![t1.id.clone()], Some("retry_test".into())).unwrap();

        // Fail both
        queue::mark_failed(&t1.id, "build error").unwrap();
        // t2 was cascade-failed by mark_failed

        // Retry
        let result = retry_pipeline("retry_test").unwrap();
        assert_eq!(result.retried_tasks, 2);

        // Verify tasks are pending again
        let q = queue::load_queue();
        for task in &q.tasks {
            assert_eq!(task.status, queue::QueueStatus::Pending);
            assert_eq!(task.retry_count, 1);
        }

        std::env::remove_var("DX_ROOT");
    }

    #[test]
    fn test_retry_no_failed_stages() {
        let _g = crate::queue::tests::env_lock();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("DX_ROOT", tmp.path());

        queue::add_task_with_pipeline("p", "dev", "t", "go", 1, vec![], Some("no_fail".into())).unwrap();

        let result = retry_pipeline("no_fail");
        assert!(result.is_err()); // No failed stages to retry

        std::env::remove_var("DX_ROOT");
    }
}

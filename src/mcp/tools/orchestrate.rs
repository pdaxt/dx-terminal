//! Orchestrate: natural language → project identification → task decomposition → agent spawning.
//!
//! The core "machine that builds machines" engine.
//! Takes a human request, identifies the project, decomposes work into
//! developer + QA + security tasks, spawns agents on free panes, and monitors.

use crate::app::App;
use crate::config;
use crate::queue;
use crate::scanner;
use super::super::types::*;
use super::helpers::*;
use super::panes;

/// Main orchestration entry point
pub async fn orchestrate(app: &App, req: OrchestrateRequest) -> String {
    let concurrent_qa = req.concurrent_qa.unwrap_or(true);
    let concurrent_security = req.concurrent_security.unwrap_or(false);
    let max_panes = req.max_panes.unwrap_or(3).clamp(1, 6) as usize;

    // Step 1: Identify project
    let project = match identify_project(&req.request, req.project.as_deref()) {
        Some(p) => p,
        None => return json_err("Could not identify project. Specify project= explicitly or run project_scan first."),
    };

    // Step 2: Build task plan
    let plan = build_plan(&req.request, &project, concurrent_qa, concurrent_security, max_panes);

    // Step 3: Queue all tasks
    let mut task_ids: Vec<String> = Vec::new();
    let mut spawned: Vec<serde_json::Value> = Vec::new();

    for planned_task in &plan.tasks {
        let deps: Vec<String> = planned_task.depends_on.iter()
            .filter_map(|dep_idx| task_ids.get(*dep_idx).cloned())
            .collect();

        match queue::add_task(
            &project.name,
            &planned_task.role,
            &planned_task.task,
            &planned_task.prompt,
            planned_task.priority,
            deps.clone(),
        ) {
            Ok(task) => {
                task_ids.push(task.id.clone());
            }
            Err(e) => {
                return json_err(&format!("Failed to queue task '{}': {}", planned_task.task, e));
            }
        }
    }

    // Step 4: Auto-spawn immediately on free panes
    let cfg = queue::load_auto_config();
    let mut occupied: Vec<u8> = Vec::new();

    // Collect currently occupied panes
    for i in 1..=config::pane_count() {
        let pd = app.state.get_pane(i).await;
        if pd.status == "active" {
            occupied.push(i);
        }
    }

    let mut spawned_count = 0;
    for (idx, planned_task) in plan.tasks.iter().enumerate() {
        if spawned_count >= max_panes {
            break;
        }

        // Only auto-spawn tasks with no dependencies (or concurrent tasks)
        let can_spawn = planned_task.depends_on.is_empty();
        if !can_spawn {
            continue;
        }

        let free_pane = queue::find_free_pane(&cfg, &occupied);
        if let Some(pane) = free_pane {
            let task_id = &task_ids[idx];
            let _ = queue::mark_running(task_id, pane);
            occupied.push(pane);

            let _result = panes::spawn(app, SpawnRequest {
                pane: pane.to_string(),
                project: project.name.clone(),
                role: Some(planned_task.role.clone()),
                task: Some(planned_task.task.clone()),
                prompt: Some(planned_task.prompt.clone()),
            }).await;

            spawned.push(serde_json::json!({
                "pane": pane,
                "role": planned_task.role,
                "task": truncate(&planned_task.task, 50),
                "task_id": task_id,
            }));
            spawned_count += 1;
        }
    }

    // Step 5: Return orchestration plan
    let pending = plan.tasks.len() - spawned_count;
    serde_json::json!({
        "status": "orchestrated",
        "project": {
            "name": project.name,
            "path": project.path,
            "tech": project.tech,
        },
        "plan": {
            "total_tasks": plan.tasks.len(),
            "tasks": plan.tasks.iter().enumerate().map(|(i, t)| {
                serde_json::json!({
                    "index": i,
                    "role": t.role,
                    "task": truncate(&t.task, 60),
                    "depends_on": t.depends_on,
                    "priority": t.priority,
                })
            }).collect::<Vec<_>>(),
        },
        "spawned": spawned,
        "pending": pending,
        "task_ids": task_ids,
        "auto_cycle": "Tasks in queue. Auto-cycle will spawn remaining when panes free up.",
    }).to_string()
}

// === Internal helpers ===

struct IdentifiedProject {
    name: String,
    path: String,
    tech: Vec<String>,
}

struct OrchestrationPlan {
    tasks: Vec<PlannedTask>,
}

struct PlannedTask {
    role: String,
    task: String,
    prompt: String,
    priority: u8,
    depends_on: Vec<usize>, // indices into tasks vec
}

/// Identify which project the request is about using fuzzy matching against scanner registry
fn identify_project(request: &str, explicit_project: Option<&str>) -> Option<IdentifiedProject> {
    let reg = scanner::load_registry();

    // If explicitly specified, use that
    if let Some(name) = explicit_project {
        if let Some(p) = reg.projects.iter().find(|p| p.name.to_lowercase() == name.to_lowercase()) {
            return Some(IdentifiedProject {
                name: p.name.clone(),
                path: p.path.clone(),
                tech: p.tech.clone(),
            });
        }
        // Try as path
        return Some(IdentifiedProject {
            name: name.to_string(),
            path: name.to_string(),
            tech: vec![],
        });
    }

    // Fuzzy match request text against project names, tech, readme
    let words: Vec<String> = request.to_lowercase()
        .split_whitespace()
        .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|s| !s.is_empty() && s.len() > 2)
        .collect();

    let mut best_score = 0i32;
    let mut best_project: Option<&scanner::ProjectInfo> = None;

    for project in &reg.projects {
        let mut score = 0i32;
        let name_lower = project.name.to_lowercase();
        let tech_lower: Vec<String> = project.tech.iter().map(|t| t.to_lowercase()).collect();
        let readme = project.readme_summary.as_deref().unwrap_or("").to_lowercase();

        for word in &words {
            // Exact name match = strongest signal
            if name_lower == *word {
                score += 100;
            } else if name_lower.contains(word.as_str()) {
                score += 50;
            }

            // Tech stack match
            for tech in &tech_lower {
                if tech.contains(word.as_str()) {
                    score += 20;
                }
            }

            // Readme match
            if readme.contains(word.as_str()) {
                score += 5;
            }
        }

        if score > best_score {
            best_score = score;
            best_project = Some(project);
        }
    }

    // Need at least some confidence
    if best_score >= 20 {
        best_project.map(|p| IdentifiedProject {
            name: p.name.clone(),
            path: p.path.clone(),
            tech: p.tech.clone(),
        })
    } else {
        None
    }
}

/// Build an orchestration plan: developer task + QA task + security task
fn build_plan(
    request: &str,
    project: &IdentifiedProject,
    concurrent_qa: bool,
    concurrent_security: bool,
    max_panes: usize,
) -> OrchestrationPlan {
    let mut tasks = Vec::new();

    // Task 0: Developer — the main work
    let dev_prompt = format!(
        "You are the lead developer for this task.\n\n\
         ## Project\n\
         - Name: {}\n\
         - Path: {}\n\
         - Tech: {}\n\n\
         ## Task\n\
         {}\n\n\
         ## Instructions\n\
         1. Implement the requested changes\n\
         2. Write tests for your implementation\n\
         3. Ensure the build passes\n\
         4. Commit and push your changes\n\
         5. Create a PR if the project has a remote\n\n\
         Work autonomously. Don't ask for clarification — make reasonable decisions and document them.",
        project.name,
        project.path,
        project.tech.join(", "),
        request,
    );

    tasks.push(PlannedTask {
        role: "developer".to_string(),
        task: format!("Implement: {}", truncate(request, 80)),
        prompt: dev_prompt,
        priority: 1,
        depends_on: vec![],
    });

    // Task 1: QA Agent
    if max_panes >= 2 {
        let qa_deps = if concurrent_qa { vec![] } else { vec![0usize] };
        let qa_mode = if concurrent_qa { "concurrent" } else { "sequential" };

        let qa_prompt = format!(
            "You are the QA engineer for this project.\n\n\
             ## Project\n\
             - Name: {}\n\
             - Path: {}\n\
             - Tech: {}\n\n\
             ## Mode: {}\n\n\
             ## Your mission\n\
             {}\n\n\
             ## QA Checklist\n\
             1. Run the existing test suite — report pass/fail\n\
             2. Review the code changes for correctness\n\
             3. Check edge cases and error handling\n\
             4. Verify the build compiles cleanly (no warnings if possible)\n\
             5. Run lint/clippy/eslint if available\n\
             6. Test the feature manually if applicable\n\
             7. Write additional tests if coverage is lacking\n\
             8. Report your findings as a summary\n\n\
             Be thorough. Find bugs before they reach production.",
            project.name,
            project.path,
            project.tech.join(", "),
            qa_mode,
            if concurrent_qa {
                format!("The developer is currently implementing: {}. \
                         Monitor the project, run tests after each commit. \
                         Report issues immediately.", truncate(request, 80))
            } else {
                format!("The developer has completed: {}. \
                         Review their work and verify everything works.", truncate(request, 80))
            },
        );

        tasks.push(PlannedTask {
            role: "qa".to_string(),
            task: format!("QA: {}", truncate(request, 80)),
            prompt: qa_prompt,
            priority: if concurrent_qa { 2 } else { 1 },
            depends_on: qa_deps,
        });
    }

    // Task 2: Security Audit
    if max_panes >= 3 {
        let sec_deps = if concurrent_security { vec![] } else { vec![0usize] };
        let sec_mode = if concurrent_security { "concurrent" } else { "post-implementation" };

        let security_prompt = format!(
            "You are the security auditor for this project.\n\n\
             ## Project\n\
             - Name: {}\n\
             - Path: {}\n\
             - Tech: {}\n\n\
             ## Mode: {}\n\n\
             ## Security Audit Scope\n\
             The developer is working on: {}\n\n\
             ## Audit Checklist\n\
             1. Check for OWASP Top 10 vulnerabilities\n\
             2. Review authentication/authorization code\n\
             3. Check for hardcoded secrets, API keys, tokens\n\
             4. Review input validation and sanitization\n\
             5. Check dependency vulnerabilities (cargo audit / npm audit)\n\
             6. Review file permissions and path traversal risks\n\
             7. Check for SQL injection, XSS, command injection\n\
             8. Review error handling (no sensitive info leaks)\n\
             9. Check CORS, CSP, and security headers\n\
             10. Report all findings with severity ratings\n\n\
             Be paranoid. Assume attackers are smart.",
            project.name,
            project.path,
            project.tech.join(", "),
            sec_mode,
            truncate(request, 80),
        );

        tasks.push(PlannedTask {
            role: "security".to_string(),
            task: format!("Security audit: {}", truncate(request, 80)),
            prompt: security_prompt,
            priority: if concurrent_security { 2 } else { 1 },
            depends_on: sec_deps,
        });
    }

    OrchestrationPlan { tasks }
}

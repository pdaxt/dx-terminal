//! Pane lifecycle: spawn, kill, restart, reassign, assign, assign_adhoc, collect, complete.

use super::super::types::*;
use super::helpers::*;
use crate::app::App;
use crate::capacity;
use crate::claude;
use crate::config;
use crate::machine;
use crate::queue;
use crate::runtime_broker;
use crate::state;
use crate::state::types::PaneState;
use crate::tmux;
use crate::tracker;
use crate::workspace;
use serde_json::json;
use serde_json::Value;
use std::path::PathBuf;

const DX_GENERATED_GUIDANCE_MARKER: &str = "<!-- dx-generated-guidance:";

fn emit_dxos_session_change(app: &App, project_path: &str, result: &str) {
    if let Some(event) = crate::dxos::session_event_from_result(project_path, result) {
        app.state.event_bus.send(event);
    }
}

fn push_env_if_present(env_vars: &mut Vec<(String, String)>, key: &str, value: Option<&str>) {
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
        env_vars.push((key.to_string(), value.to_string()));
    }
}

fn value_string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|value| value.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn effective_task_from_dxos_context(task: &str, context: &Value) -> String {
    if !task.trim().is_empty() {
        return task.trim().to_string();
    }
    context
        .get("session")
        .and_then(|session| session.get("objective"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| "Advance the assigned DXOS work package.".to_string())
}

fn build_dxos_runtime_context_section(context: &Value) -> Option<String> {
    let session = context.get("session")?;
    let session_id = session.get("id").and_then(Value::as_str).unwrap_or("");
    if session_id.is_empty() {
        return None;
    }

    let mut lines = vec![
        "## DXOS Session Contract".to_string(),
        format!(
            "- Session: {} ({})",
            session_id,
            session
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("worker")
        ),
    ];

    if let Some(status) = session.get("status").and_then(Value::as_str) {
        lines.push(format!("- Session status: {}", status));
    }
    if let Some(objective) = session.get("objective").and_then(Value::as_str) {
        if !objective.trim().is_empty() {
            lines.push(format!("- Session objective: {}", objective.trim()));
        }
    }
    if let Some(stage) = session.get("stage").and_then(Value::as_str) {
        if !stage.trim().is_empty() {
            lines.push(format!("- Delivery stage: {}", stage.trim()));
        }
    }
    if let Some(feature_id) = session.get("feature_id").and_then(Value::as_str) {
        if !feature_id.trim().is_empty() {
            lines.push(format!("- Feature: {}", feature_id.trim()));
        }
    }

    if let Some(work_order) = context.get("primary_work_order") {
        let work_order_id = work_order.get("id").and_then(Value::as_str).unwrap_or("");
        if !work_order_id.is_empty() {
            lines.push(String::new());
            lines.push("## DXOS Assigned Work Package".to_string());
            lines.push(format!(
                "- Work order: {} ({})",
                work_order_id,
                work_order
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("assigned")
            ));
            if let Some(title) = work_order.get("title").and_then(Value::as_str) {
                if !title.trim().is_empty() {
                    lines.push(format!("- Title: {}", title.trim()));
                }
            }
            if let Some(objective) = work_order.get("objective").and_then(Value::as_str) {
                if !objective.trim().is_empty() {
                    lines.push(format!("- Objective: {}", objective.trim()));
                }
            }
            if let Some(stage) = work_order.get("stage").and_then(Value::as_str) {
                if !stage.trim().is_empty() {
                    lines.push(format!("- Work stage: {}", stage.trim()));
                }
            }
            if let Some(feature_id) = work_order.get("feature_id").and_then(Value::as_str) {
                if !feature_id.trim().is_empty() {
                    lines.push(format!("- Work feature: {}", feature_id.trim()));
                }
            }

            let required_capabilities = value_string_array(work_order.get("required_capabilities"));
            if !required_capabilities.is_empty() {
                lines.push(format!(
                    "- Required capabilities: {}",
                    required_capabilities.join(", ")
                ));
            }

            let expected_outputs = value_string_array(work_order.get("expected_outputs"));
            if !expected_outputs.is_empty() {
                lines.push("- Expected outputs:".to_string());
                for item in expected_outputs {
                    lines.push(format!("  - {}", item));
                }
            }

            let blockers = value_string_array(work_order.get("blockers"));
            if !blockers.is_empty() {
                lines.push("- Active blockers:".to_string());
                for blocker in blockers {
                    lines.push(format!("  - {}", blocker));
                }
            }

            if let Some(last_resolution) = work_order.get("last_resolution").and_then(Value::as_str)
            {
                if !last_resolution.trim().is_empty() {
                    lines.push(format!(
                        "- Latest lead guidance: {}",
                        last_resolution.trim()
                    ));
                }
            }
        }
    }

    if let Some(workflow_run) = context.get("primary_workflow_run") {
        let workflow_run_id = workflow_run.get("id").and_then(Value::as_str).unwrap_or("");
        if !workflow_run_id.is_empty() {
            lines.push(String::new());
            lines.push("## DXOS Workflow Run".to_string());
            lines.push(format!(
                "- Workflow run: {} ({})",
                workflow_run_id,
                workflow_run
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("planned")
            ));
            if let Some(name) = workflow_run.get("name").and_then(Value::as_str) {
                if !name.trim().is_empty() {
                    lines.push(format!("- Workflow: {}", name.trim()));
                }
            }
            if let Some(summary) = workflow_run.get("summary").and_then(Value::as_str) {
                if !summary.trim().is_empty() {
                    lines.push(format!("- Workflow objective: {}", summary.trim()));
                }
            }
            let steps = workflow_run
                .get("steps")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if !steps.is_empty() {
                lines.push("- Workflow steps:".to_string());
                for step in steps.iter().take(6) {
                    let title = step.get("title").and_then(Value::as_str).unwrap_or("");
                    let status = step
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("planned");
                    if !title.trim().is_empty() {
                        lines.push(format!("  - [{}] {}", status, title.trim()));
                    }
                }
                if let Some(next_step) = steps.iter().find(|step| {
                    !matches!(
                        step.get("status")
                            .and_then(Value::as_str)
                            .unwrap_or("planned"),
                        "completed" | "skipped"
                    )
                }) {
                    let step_id = next_step.get("id").and_then(Value::as_str).unwrap_or("");
                    let title = next_step.get("title").and_then(Value::as_str).unwrap_or("");
                    if !step_id.is_empty() && !title.trim().is_empty() {
                        lines.push(format!(
                            "- Next workflow step: {} {}",
                            step_id,
                            title.trim()
                        ));
                    }
                }
            }
        }
    }

    if let Some(adoption) = context.get("adoption") {
        let adoption_id = adoption.get("id").and_then(Value::as_str).unwrap_or("");
        if !adoption_id.is_empty() {
            lines.push(String::new());
            lines.push("## DXOS Adoption Context".to_string());
            lines.push(format!(
                "- Adoption: {} ({})",
                adoption_id,
                adoption
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("active")
            ));
            if let Some(summary) = adoption.get("summary").and_then(Value::as_str) {
                if !summary.trim().is_empty() {
                    lines.push(format!("- Summary: {}", summary.trim()));
                }
            }
            if let Some(objective) = adoption.get("objective").and_then(Value::as_str) {
                if !objective.trim().is_empty() {
                    lines.push(format!("- Adoption objective: {}", objective.trim()));
                }
            }
        }
    }

    if let Some(debate) = context.get("debate") {
        let debate_id = debate.get("id").and_then(Value::as_str).unwrap_or("");
        if !debate_id.is_empty() {
            lines.push(String::new());
            lines.push("## DXOS Recovery Council".to_string());
            lines.push(format!(
                "- Debate: {} ({})",
                debate_id,
                debate
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("open")
            ));
            if let Some(title) = debate.get("title").and_then(Value::as_str) {
                if !title.trim().is_empty() {
                    lines.push(format!("- Council: {}", title.trim()));
                }
            }
        }
    }

    lines.push(String::new());
    lines.push("## DXOS Operating Rule".to_string());
    lines.push(
        "- Treat the assigned work package as the canonical governed task for this lane. Produce the expected outputs and raise blockers through DXOS instead of going silent."
            .to_string(),
    );
    if context.get("primary_workflow_run").is_some() {
        lines.push(
            "- When you start, block, or finish a workflow step, call dxos_workflow_step so the portal, session contract, and work order stay synchronized."
                .to_string(),
        );
    }
    Some(lines.join("\n"))
}

fn merge_prompt_sections(prompt: &str, sections: &[Option<&str>]) -> String {
    let mut parts = Vec::new();
    let prompt = prompt.trim();
    if !prompt.is_empty() {
        parts.push(prompt.to_string());
    }
    for section in sections {
        if let Some(value) = section.map(str::trim).filter(|value| !value.is_empty()) {
            parts.push(value.to_string());
        }
    }
    parts.join("\n\n")
}

fn guidance_file_provider(file_name: &str) -> Option<&'static str> {
    match file_name {
        "CLAUDE.md" => Some("claude"),
        "CODEX.md" => Some("codex"),
        "GEMINI.md" => Some("gemini"),
        _ => None,
    }
}

fn render_generated_guidance(
    file_name: &str,
    active_provider: &str,
    preamble: &str,
    automation_context: &str,
    shared_guidance_path: &str,
    automation_doc_path: &str,
) -> String {
    let target_provider = guidance_file_provider(file_name).unwrap_or("shared");
    let payload = json!({
        "provider": target_provider,
        "activeProvider": active_provider,
        "file": file_name,
        "sourceOfTruth": "dx_runtime_shell",
    });
    let title = if target_provider == "shared" {
        "DX Shared Guidance".to_string()
    } else {
        format!(
            "DX {} Guidance",
            crate::runtime_broker::provider_short(target_provider)
        )
    };
    let provider_note = if target_provider == "shared" {
        format!(
            "This file is DX-managed shared guidance. Provider-specific runtime notes live beside it. Shared runtime guide: `{}`. Automation guide: `{}`.",
            shared_guidance_path, automation_doc_path
        )
    } else {
        format!(
            "This file is DX-managed provider guidance for {}. Active runtime provider for this lane: {}. Shared runtime guide: `{}`. Automation guide: `{}`.",
            crate::runtime_broker::provider_label(target_provider),
            crate::runtime_broker::provider_label(active_provider),
            shared_guidance_path,
            automation_doc_path
        )
    };
    format!(
        "{} {} -->\n\n# {}\n\n{}\n\n{}\n",
        DX_GENERATED_GUIDANCE_MARKER,
        serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string()),
        title,
        provider_note,
        if target_provider == "shared" {
            preamble.trim()
        } else {
            automation_context.trim()
        }
    )
}

fn is_dx_generated_guidance(path: &str) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| content.lines().next().map(|line| line.to_string()))
        .map(|line| line.starts_with(DX_GENERATED_GUIDANCE_MARKER))
        .unwrap_or(false)
}

fn write_guidance_if_safe(path: &str, content: &str) {
    if std::path::Path::new(path).exists() && !is_dx_generated_guidance(path) {
        return;
    }
    let _ = std::fs::write(path, content);
}

fn inject_dxos_runtime_env(env_vars: &mut Vec<(String, String)>, context: &Value) {
    if let Some(work_order) = context.get("primary_work_order") {
        push_env_if_present(
            env_vars,
            "DX_WORK_ORDER_ID",
            work_order.get("id").and_then(Value::as_str),
        );
        push_env_if_present(
            env_vars,
            "DX_WORK_ORDER_TITLE",
            work_order.get("title").and_then(Value::as_str),
        );
        push_env_if_present(
            env_vars,
            "DX_WORK_ORDER_STATUS",
            work_order.get("status").and_then(Value::as_str),
        );
        let expected_outputs = value_string_array(work_order.get("expected_outputs"));
        if !expected_outputs.is_empty() {
            env_vars.push((
                "DX_WORK_ORDER_EXPECTED_OUTPUTS".to_string(),
                expected_outputs.join(","),
            ));
        }
        let required_capabilities = value_string_array(work_order.get("required_capabilities"));
        if !required_capabilities.is_empty() {
            env_vars.push((
                "DX_WORK_ORDER_REQUIRED_CAPABILITIES".to_string(),
                required_capabilities.join(","),
            ));
        }
    }

    if let Some(adoption) = context.get("adoption") {
        push_env_if_present(
            env_vars,
            "DX_ADOPTION_ID",
            adoption.get("id").and_then(Value::as_str),
        );
    }
    if let Some(debate) = context.get("debate") {
        push_env_if_present(
            env_vars,
            "DX_DEBATE_ID",
            debate.get("id").and_then(Value::as_str),
        );
    }
    if let Some(workflow_run) = context.get("primary_workflow_run") {
        push_env_if_present(
            env_vars,
            "DX_WORKFLOW_RUN_ID",
            workflow_run.get("id").and_then(Value::as_str),
        );
        push_env_if_present(
            env_vars,
            "DX_WORKFLOW_ID",
            workflow_run.get("workflow_id").and_then(Value::as_str),
        );
        push_env_if_present(
            env_vars,
            "DX_WORKFLOW_STATUS",
            workflow_run.get("status").and_then(Value::as_str),
        );
        let workflow_steps = workflow_run
            .get("steps")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.get("title").and_then(Value::as_str))
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !workflow_steps.is_empty() {
            env_vars.push(("DX_WORKFLOW_STEPS".to_string(), workflow_steps.join(",")));
        }
        if let Some(next_step) = workflow_run
            .get("steps")
            .and_then(Value::as_array)
            .and_then(|items| {
                items.iter().find(|item| {
                    !matches!(
                        item.get("status")
                            .and_then(Value::as_str)
                            .unwrap_or("planned"),
                        "completed" | "skipped"
                    )
                })
            })
        {
            push_env_if_present(
                env_vars,
                "DX_WORKFLOW_NEXT_STEP_ID",
                next_step.get("id").and_then(Value::as_str),
            );
            push_env_if_present(
                env_vars,
                "DX_WORKFLOW_NEXT_STEP_TITLE",
                next_step.get("title").and_then(Value::as_str),
            );
        }
    }
}

fn spawn_pty_planned_agent(
    app: &App,
    pane_num: u8,
    plan: &runtime_broker::RuntimeLaunchPlan,
    env_vars: &[(String, String)],
) -> anyhow::Result<Option<String>> {
    let mut pty = app.pty_lock();
    pty.spawn(
        pane_num,
        "/bin/zsh",
        &["-lc", &plan.command],
        &plan.project_path,
        env_vars.to_vec(),
    )?;
    Ok(None)
}

async fn resolve_spawn_pane(app: &App, pane_ref: &str) -> Result<u8, String> {
    let requested = pane_ref.trim();
    let auto_allocate = requested.is_empty()
        || matches!(
            requested.to_ascii_lowercase().as_str(),
            "auto" | "next" | "any" | "free"
        );
    if !auto_allocate {
        return config::resolve_pane(requested).ok_or_else(|| {
            format!(
                "Invalid pane: {}. Use 1-9, theme name, or 'auto'.",
                pane_ref
            )
        });
    }

    let snapshot = app.state.get_state_snapshot().await;
    let reserved = queue::load_auto_config().reserved_panes;
    let pty = app.pty_lock();
    for pane_num in 1..=config::pane_count() {
        if reserved.contains(&pane_num) {
            continue;
        }
        let occupied_by_state = snapshot
            .panes
            .get(&pane_num.to_string())
            .map(|pane| !matches!(pane.status.as_str(), "idle" | "done" | "lost"))
            .unwrap_or(false);
        let occupied_by_pty = pty.is_running(pane_num);
        if !occupied_by_state && !occupied_by_pty {
            return Ok(pane_num);
        }
    }

    Err(format!(
        "No free pane is available right now. {} panes are configured and all non-reserved lanes are occupied.",
        config::pane_count()
    ))
}

/// Execute os_spawn logic — allocates a DX runtime lane through the broker
pub async fn spawn(app: &App, req: SpawnRequest) -> String {
    let client_request_id = req.client_request_id.clone();
    let requested_session_id = req.session_id.clone();
    let pane_num = match resolve_spawn_pane(app, &req.pane).await {
        Ok(n) => n,
        Err(error) => {
            return serde_json::json!({
                "error": error,
                "client_request_id": client_request_id,
                "pane": req.pane,
            })
            .to_string()
        }
    };

    let role = req.role.unwrap_or_else(|| "developer".into());
    let provider =
        runtime_broker::normalize_provider_id(req.provider.as_deref().unwrap_or("claude"))
            .to_string();
    let runtime_adapter =
        runtime_broker::normalize_adapter_id(req.runtime_adapter.as_deref()).to_string();
    let model = req.model.clone();
    let feature_id = req.feature_id.clone();
    let stage = req.stage.clone();
    let supervisor_session_id = req.supervisor_session_id.clone();
    let task = req.task.unwrap_or_default();
    let prompt = req.prompt.unwrap_or_default();
    let theme = config::theme_name(pane_num);

    // Pre-spawn cleanup: kill any stale processes owned by this pane
    cleanup_pane_resources(pane_num);

    // Micro-helpers: workspace setup + MCP selection
    let ws = prepare_workspace(&req.project, pane_num, &task);
    let _mcps = select_mcps(app, &ws.project_name, &ws.project_path, &task, &role).await;

    let project_path = ws.project_path;
    let project_name = ws.project_name;
    let mut spawn_cwd = ws.spawn_cwd;
    let ws_path = ws.ws_path;
    let ws_branch = ws.ws_branch;
    let ws_base = ws.ws_base;
    let browser_port = config::pane_browser_port(pane_num);
    let browser_profile_root = config::pane_browser_profile_root(pane_num);
    let browser_artifacts_root = config::pane_browser_artifacts_root(pane_num);
    let provider_bridge_sync = crate::provider_plugins::convert_provider_plugin(
        None, &provider, false,
    )
    .unwrap_or_else(|error| {
        serde_json::json!({
            "error": error.to_string(),
            "target": provider,
        })
    });
    let provider_bridge_path = provider_bridge_sync
        .get("path")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let provider_bridge_exported = provider_bridge_sync
        .get("exported_servers")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let automation_bridge_sync = crate::provider_asset_plugins::convert_provider_asset_plugin(
        Some(&project_path),
        None,
        &provider,
        false,
    )
    .unwrap_or_else(|error| {
        serde_json::json!({
            "error": error.to_string(),
            "target": provider,
        })
    });
    let automation_project_manifest_path = automation_bridge_sync
        .get("project_manifest_path")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let automation_user_manifest_path = automation_bridge_sync
        .get("user_manifest_path")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let automation_project_assets = automation_bridge_sync
        .get("project")
        .and_then(|value| value.get("assets"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let automation_project_workflow_catalog_path = automation_bridge_sync
        .get("project_workflow_catalog_path")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let automation_user_workflow_catalog_path = automation_bridge_sync
        .get("user_workflow_catalog_path")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let automation_project_workflows = automation_bridge_sync
        .get("project")
        .and_then(|value| value.get("workflows"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let automation_user_workflows = automation_bridge_sync
        .get("user")
        .and_then(|value| value.get("workflows"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let automation_user_assets = automation_bridge_sync
        .get("user")
        .and_then(|value| value.get("assets"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);

    // Validate CWD exists — fall back to project_path to avoid posix_spawn ENOENT
    if !std::path::Path::new(&spawn_cwd).exists() {
        tracing::warn!(
            "spawn_cwd does not exist: {}, falling back to project_path: {}",
            spawn_cwd,
            project_path
        );
        spawn_cwd = project_path.clone();
        // If project_path also doesn't exist, fail early with clear error
        if !std::path::Path::new(&spawn_cwd).exists() {
            return serde_json::json!({
                "error": format!("Neither workspace nor project path exists: {}", spawn_cwd),
                "client_request_id": client_request_id,
                "pane": pane_num,
            })
            .to_string();
        }
    }
    let automation_doc_path = format!("{}/DX_AUTOMATION.md", spawn_cwd);
    let shared_guidance_doc_path = format!("{}/DX_GUIDANCE.md", spawn_cwd);
    let provider_guidance_doc_path =
        format!("{}/DX_{}_GUIDANCE.md", spawn_cwd, provider.to_uppercase());

    // Register machine identity
    let machine_id = machine::register(pane_num);

    // Environment variables for the agent
    let mut env_vars = vec![
        ("P".to_string(), pane_num.to_string()),
        ("DX_PANE".to_string(), pane_num.to_string()),
        ("DX_THEME".to_string(), theme.to_string()),
        ("DX_PROJECT".to_string(), project_name.clone()),
        ("DX_ROLE".to_string(), role.clone()),
        ("DX_PROVIDER".to_string(), provider.clone()),
        ("DX_MODEL".to_string(), model.clone().unwrap_or_default()),
        ("DX_RUNTIME_ADAPTER".to_string(), runtime_adapter.clone()),
        ("DX_PROVIDER_BRIDGE_PROVIDER".to_string(), provider.clone()),
        (
            "DX_PROVIDER_BRIDGE_SOURCE".to_string(),
            "dx_shared_manifest".to_string(),
        ),
        ("DX_BROWSER_PORT".to_string(), browser_port.to_string()),
        ("PLAYWRIGHT_PORT".to_string(), browser_port.to_string()),
        (
            "DX_BROWSER_PROFILE_ROOT".to_string(),
            browser_profile_root.to_string_lossy().to_string(),
        ),
        (
            "DX_BROWSER_ARTIFACTS_ROOT".to_string(),
            browser_artifacts_root.to_string_lossy().to_string(),
        ),
        ("MACHINE_IP".to_string(), machine_id.ip.clone()),
        ("MACHINE_HOSTNAME".to_string(), machine_id.hostname.clone()),
        ("MACHINE_MAC".to_string(), machine_id.mac.clone()),
        (
            "DX_PROVIDER_BRIDGE_EXPORTED_SERVERS".to_string(),
            provider_bridge_exported.to_string(),
        ),
        (
            "DX_AUTOMATION_BRIDGE_PROVIDER".to_string(),
            provider.clone(),
        ),
        (
            "DX_AUTOMATION_BRIDGE_SOURCE".to_string(),
            "dx_shared_automation_manifest".to_string(),
        ),
        (
            "DX_AUTOMATION_BRIDGE_PROJECT_ASSETS".to_string(),
            automation_project_assets.to_string(),
        ),
        (
            "DX_AUTOMATION_BRIDGE_USER_ASSETS".to_string(),
            automation_user_assets.to_string(),
        ),
        (
            "DX_WORKFLOW_CATALOG_PROJECT_COUNT".to_string(),
            automation_project_workflows.to_string(),
        ),
        (
            "DX_WORKFLOW_CATALOG_USER_COUNT".to_string(),
            automation_user_workflows.to_string(),
        ),
        (
            "DX_AUTOMATION_GUIDE_PATH".to_string(),
            automation_doc_path.clone(),
        ),
        (
            "DX_SHARED_GUIDANCE_PATH".to_string(),
            shared_guidance_doc_path.clone(),
        ),
        (
            "DX_PROVIDER_GUIDANCE_PATH".to_string(),
            provider_guidance_doc_path.clone(),
        ),
    ];
    if let Some(path) = provider_bridge_path.as_deref() {
        env_vars.push(("DX_PROVIDER_BRIDGE_PATH".to_string(), path.to_string()));
    }
    if let Some(path) = automation_project_manifest_path.as_deref() {
        env_vars.push((
            "DX_AUTOMATION_BRIDGE_PROJECT_PATH".to_string(),
            path.to_string(),
        ));
    }
    if let Some(path) = automation_user_manifest_path.as_deref() {
        env_vars.push((
            "DX_AUTOMATION_BRIDGE_USER_PATH".to_string(),
            path.to_string(),
        ));
    }
    if let Some(path) = automation_project_workflow_catalog_path.as_deref() {
        env_vars.push((
            "DX_WORKFLOW_CATALOG_PROJECT_PATH".to_string(),
            path.to_string(),
        ));
    }
    if let Some(path) = automation_user_workflow_catalog_path.as_deref() {
        env_vars.push((
            "DX_WORKFLOW_CATALOG_USER_PATH".to_string(),
            path.to_string(),
        ));
    }
    if let Some(error) = provider_bridge_sync
        .get("error")
        .and_then(|value| value.as_str())
    {
        env_vars.push((
            "DX_PROVIDER_BRIDGE_SYNC_ERROR".to_string(),
            error.to_string(),
        ));
    }
    if let Some(error) = automation_bridge_sync
        .get("error")
        .and_then(|value| value.as_str())
    {
        env_vars.push((
            "DX_AUTOMATION_BRIDGE_SYNC_ERROR".to_string(),
            error.to_string(),
        ));
    }

    let autonomous = req.autonomous.unwrap_or(true);

    let initial_session_result = crate::dxos::upsert_session_contract(
        &project_path,
        Some(&project_name),
        requested_session_id.as_deref(),
        &role,
        Some(&provider),
        model.as_deref(),
        Some(if autonomous {
            "high_autonomy"
        } else {
            "guarded_auto"
        }),
        &task,
        vec!["task_result".to_string(), "runtime_handoff".to_string()],
        app.state.get_project_mcps(&project_name).await,
        vec![project_path.clone()],
        vec![spawn_cwd.clone()],
        ws_path.as_deref(),
        ws_branch.as_deref(),
        Some(browser_port),
        Some(pane_num),
        Some(&runtime_adapter),
        None,
        feature_id.as_deref(),
        stage.as_deref(),
        supervisor_session_id.as_deref(),
        Some("lead_then_human"),
        Some("launching"),
    );
    let initial_session_value: serde_json::Value =
        serde_json::from_str(&initial_session_result).unwrap_or_else(|_| serde_json::json!({}));
    emit_dxos_session_change(app, &project_path, &initial_session_result);
    let dxos_session_id = initial_session_value
        .get("session_id")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    if let Some(session_id) = dxos_session_id.as_deref() {
        env_vars.push(("DXOS_SESSION_ID".to_string(), session_id.to_string()));
    }
    if let Some(value) = feature_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        env_vars.push(("DX_FEATURE_ID".to_string(), value.to_string()));
    }
    if let Some(value) = stage.as_deref().filter(|value| !value.trim().is_empty()) {
        env_vars.push(("DX_STAGE".to_string(), value.to_string()));
    }
    if let Some(value) = supervisor_session_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        env_vars.push(("DX_SUPERVISOR_SESSION_ID".to_string(), value.to_string()));
    }
    let initial_policy_violations = initial_session_value
        .get("session")
        .and_then(|value| value.get("policy_violations"))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str().map(|value| value.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let initial_session_status = initial_session_value
        .get("session")
        .and_then(|value| value.get("status"))
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if initial_session_value.get("error").is_some()
        || initial_session_status == "blocked"
        || !initial_policy_violations.is_empty()
    {
        let error_message = initial_session_value
            .get("error")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string())
            .unwrap_or_else(|| initial_policy_violations.join(" ").trim().to_string());
        return serde_json::json!({
            "error": if error_message.is_empty() {
                "DXOS provider policy blocked the launch".to_string()
            } else {
                error_message
            },
            "pane": pane_num,
            "provider": provider,
            "runtime_adapter": runtime_adapter,
            "model": model,
            "dxos_session_id": dxos_session_id,
            "policy_violations": initial_policy_violations,
            "project": project_name,
            "project_path": project_path,
            "workspace": ws_path,
            "branch": ws_branch,
            "browser_port": browser_port,
            "automation_doc_path": serde_json::Value::Null,
            "shared_guidance_path": serde_json::Value::Null,
            "provider_guidance_path": serde_json::Value::Null,
            "provider_bridge": provider_bridge_sync,
            "automation_bridge": automation_bridge_sync,
            "workflow_catalog": serde_json::Value::Null,
        })
        .to_string();
    }

    let dxos_launch_context = dxos_session_id
        .as_deref()
        .map(|session_id| {
            crate::dxos::runtime_launch_context(&project_path, Some(&project_name), session_id)
        })
        .unwrap_or_else(|| serde_json::json!({}));
    inject_dxos_runtime_env(&mut env_vars, &dxos_launch_context);

    let effective_task = effective_task_from_dxos_context(&task, &dxos_launch_context);
    let dxos_context_section = build_dxos_runtime_context_section(&dxos_launch_context);
    let automation_context_section =
        crate::provider_asset_plugins::runtime_guidance_markdown(Some(&project_path), &provider, 8);
    let effective_prompt = merge_prompt_sections(
        &prompt,
        &[
            dxos_context_section.as_deref(),
            Some(automation_context_section.as_str()),
        ],
    );

    // Generate preamble and write a shared guidance bundle in the workspace for
    // whichever provider is active there.
    let preamble = claude::generate_preamble(
        pane_num,
        theme,
        &project_name,
        &role,
        &effective_task,
        &effective_prompt,
    );
    let _ = claude::write_preamble(pane_num, &preamble);
    let _ = std::fs::write(
        &automation_doc_path,
        format!("{}\n", automation_context_section.trim()),
    );
    let shared_guidance = render_generated_guidance(
        "AGENTS.md",
        &provider,
        &preamble,
        &automation_context_section,
        &shared_guidance_doc_path,
        &automation_doc_path,
    );
    let provider_guidance = render_generated_guidance(
        &format!("{}.md", provider.to_uppercase()),
        &provider,
        &preamble,
        &automation_context_section,
        &shared_guidance_doc_path,
        &automation_doc_path,
    );
    let _ = std::fs::write(&shared_guidance_doc_path, &shared_guidance);
    let _ = std::fs::write(&provider_guidance_doc_path, &provider_guidance);
    for guidance_file in ["AGENTS.md", "CLAUDE.md", "CODEX.md", "GEMINI.md"] {
        let guidance_path = format!("{}/{}", spawn_cwd, guidance_file);
        let guidance_content = render_generated_guidance(
            guidance_file,
            &provider,
            &preamble,
            &automation_context_section,
            &shared_guidance_doc_path,
            &automation_doc_path,
        );
        write_guidance_if_safe(&guidance_path, &guidance_content);
    }

    let task_prompt = format!(
        "{}\n\n{}",
        effective_task,
        if effective_prompt.is_empty() {
            ""
        } else {
            &effective_prompt
        }
    );

    let window_name = format!(
        "dx-{}-{}-{}",
        provider,
        pane_num,
        config::theme_name(pane_num).to_lowercase()
    );
    let launch_plan = match runtime_broker::plan_launch(
        Some(&runtime_adapter),
        &provider,
        &window_name,
        &spawn_cwd,
        &task_prompt,
        autonomous,
        model.as_deref(),
    ) {
        Ok(plan) => plan,
        Err(error) => {
            if let Some(session_id) = dxos_session_id.as_deref() {
                let failure_result = crate::dxos::record_session_launch_failure(
                    &project_path,
                    Some(&project_name),
                    session_id,
                    &format!("Runtime broker failed: {}", error),
                );
                emit_dxos_session_change(app, &project_path, &failure_result);
            }
            return serde_json::json!({
                "error": format!("Runtime broker failed: {}", error),
                "pane": pane_num,
                "provider": provider,
                "runtime_adapter": runtime_adapter,
                "model": model,
                "dxos_session_id": dxos_session_id,
                "project": project_name,
                "project_path": project_path,
                "workspace": ws_path,
                "branch": ws_branch,
                "browser_port": browser_port,
                "automation_doc_path": automation_doc_path,
                "shared_guidance_path": shared_guidance_doc_path,
                "provider_guidance_path": provider_guidance_doc_path,
                "provider_bridge": provider_bridge_sync,
                "automation_bridge": automation_bridge_sync,
                "workflow_catalog": {
                    "project_path": automation_project_workflow_catalog_path,
                    "user_path": automation_user_workflow_catalog_path,
                    "project_workflows": automation_project_workflows,
                    "user_workflows": automation_user_workflows,
                },
                "runtime_broker": launch_broker_json_from_error(&window_name, &spawn_cwd),
            })
            .to_string();
        }
    };
    let launch_result = match launch_plan.adapter.as_str() {
        "pty_native_adapter" => spawn_pty_planned_agent(app, pane_num, &launch_plan, &env_vars),
        _ => tmux::spawn_planned_agent(&launch_plan, &env_vars).map(|agent| Some(agent.target)),
    };

    let (launch_status, tmux_target) = match &launch_result {
        Ok(target) => (format!("{}_spawned", launch_plan.adapter), target.clone()),
        Err(e) => (format!("launch_error: {}", e), None),
    };

    if let Err(error) = launch_result {
        if let Some(session_id) = dxos_session_id.as_deref() {
            let failure_result = crate::dxos::record_session_launch_failure(
                &project_path,
                Some(&project_name),
                session_id,
                &format!("Runtime launch failed: {}", error),
            );
            emit_dxos_session_change(app, &project_path, &failure_result);
        }
        return serde_json::json!({
            "error": format!("Runtime launch failed: {}", launch_status),
            "client_request_id": client_request_id,
            "pane": pane_num,
            "provider": provider,
            "runtime_adapter": runtime_adapter,
            "model": model,
            "dxos_session_id": dxos_session_id,
            "project": project_name,
            "project_path": project_path,
            "workspace": ws_path,
            "branch": ws_branch,
            "browser_port": browser_port,
            "automation_doc_path": automation_doc_path,
            "shared_guidance_path": shared_guidance_doc_path,
            "provider_guidance_path": provider_guidance_doc_path,
            "provider_bridge": provider_bridge_sync,
            "automation_bridge": automation_bridge_sync,
            "workflow_catalog": {
                "project_path": automation_project_workflow_catalog_path,
                "user_path": automation_user_workflow_catalog_path,
                "project_workflows": automation_project_workflows,
                "user_workflows": automation_user_workflows,
            },
            "runtime_broker": launch_broker_json(&launch_plan),
        })
        .to_string();
    }

    let pane_state = PaneState {
        theme: theme.to_string(),
        project: project_name.clone(),
        project_path: project_path.clone(),
        role: role.clone(),
        provider: Some(provider.clone()),
        model: model.clone(),
        runtime_adapter: Some(runtime_adapter.clone()),
        dxos_session_id: dxos_session_id.clone(),
        task: task.clone(),
        issue_id: None,
        space: None,
        status: "active".into(),
        started_at: Some(state::now()),
        acu_spent: 0.0,
        workspace_path: ws_path.clone(),
        branch_name: ws_branch.clone(),
        base_branch: ws_base.clone(),
        machine_ip: Some(machine_id.ip.clone()),
        machine_hostname: Some(machine_id.hostname.clone()),
        machine_mac: Some(machine_id.mac.clone()),
        tmux_target: tmux_target.clone(),
        ..Default::default()
    };
    let session_result = crate::dxos::upsert_session_contract(
        &project_path,
        Some(&project_name),
        dxos_session_id.as_deref(),
        &role,
        Some(&provider),
        model.as_deref(),
        Some(if autonomous {
            "high_autonomy"
        } else {
            "guarded_auto"
        }),
        &task,
        vec!["task_result".to_string(), "runtime_handoff".to_string()],
        app.state.get_project_mcps(&project_name).await,
        vec![project_path.clone()],
        vec![spawn_cwd.clone()],
        ws_path.as_deref(),
        ws_branch.as_deref(),
        Some(browser_port),
        Some(pane_num),
        Some(&runtime_adapter),
        tmux_target.as_deref(),
        feature_id.as_deref(),
        stage.as_deref(),
        supervisor_session_id.as_deref(),
        Some("lead_then_human"),
        Some("active"),
    );
    let session_value: serde_json::Value =
        serde_json::from_str(&session_result).unwrap_or_else(|_| serde_json::json!({}));
    app.state.set_pane(pane_num, pane_state).await;
    emit_dxos_session_change(app, &project_path, &session_result);
    app.state
        .event_bus
        .send(crate::state::events::StateEvent::PaneSpawned {
            pane: pane_num,
            project: project_name.clone(),
            role: role.clone(),
        });
    app.state
        .log_activity(
            pane_num,
            "spawn",
            &format!(
                "Spawned {} on {}: {}",
                role,
                project_name,
                truncate(&task, 40)
            ),
        )
        .await;

    update_agents_json(pane_num, &project_name, &task);

    // Auto-register agent with multi_agent coordination system
    let _ = crate::multi_agent::agent_register(
        &pane_id_str(pane_num),
        &project_name,
        &task,
        &[], // files will be claimed via lock_acquire as agent works
    );

    if let Some(ref branch) = ws_branch {
        let _ = crate::multi_agent::git_claim_branch(
            &pane_id_str(pane_num),
            branch,
            &project_name,
            &task,
        );
    }

    serde_json::json!({
        "status": "spawned",
        "client_request_id": client_request_id,
        "pane": pane_num,
        "theme": theme,
        "project": project_name,
        "role": role,
        "provider": provider,
        "runtime_adapter": runtime_adapter,
        "model": model,
        "task": task,
        "project_path": project_path,
        "workspace": ws_path,
        "branch": ws_branch,
        "browser_port": browser_port,
        "automation_doc_path": automation_doc_path,
        "shared_guidance_path": shared_guidance_doc_path,
        "provider_guidance_path": provider_guidance_doc_path,
        "browser_profile_root": browser_profile_root,
        "browser_artifacts_root": browser_artifacts_root,
        "launch": launch_status,
        "tmux_target": tmux_target,
        "runtime_broker": launch_broker_json(&launch_plan),
        "provider_bridge": provider_bridge_sync,
        "automation_bridge": automation_bridge_sync,
        "workflow_catalog": {
            "project_path": automation_project_workflow_catalog_path,
            "user_path": automation_user_workflow_catalog_path,
            "project_workflows": automation_project_workflows,
            "user_workflows": automation_user_workflows,
        },
        "dxos_session_id": session_value.get("session_id").cloned().unwrap_or(serde_json::Value::Null),
        "machine_ip": machine_id.ip,
        "machine_hostname": machine_id.hostname,
        "machine_mac": machine_id.mac,
    })
    .to_string()
}

/// Execute os_kill logic — kills PTY process and cleans up state
pub async fn kill(app: &App, req: KillRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };
    let reason = req.reason.unwrap_or_else(|| "manual".into());

    let pane_data = app.state.get_pane(pane_num).await;
    let ws_path = pane_data.workspace_path.clone();
    let project_path = pane_data.project_path.clone();

    let output_log = save_agent_output(app, pane_num, &reason);

    // Kill via tmux if we have a target, otherwise try PTY fallback
    let kill_status = if let Some(ref target) = pane_data.tmux_target {
        match tmux::kill_window(target) {
            Ok(()) => "tmux_killed",
            Err(_) => "tmux_no_window",
        }
    } else {
        // Fallback: try PTY kill for legacy agents
        let mut pty = app.pty_lock();
        match pty.kill(pane_num) {
            Ok(()) => "pty_killed",
            Err(_) => "no_process",
        }
    };

    let mut git_info = serde_json::Value::Null;
    let branch_name = pane_data.branch_name.clone();
    let project_name = pane_data.project.clone();
    if let Some(ws) = &ws_path {
        let commit_result = workspace::commit_all(ws, &format!("WIP: killed ({})", reason));
        let wt_result = workspace::remove_worktree(&project_path, ws);
        git_info = serde_json::json!({
            "wip_commit": commit_result.unwrap_or_else(|e| e.to_string()),
            "worktree_removed": wt_result.is_ok(),
        });
    }

    if let Some(ref branch) = branch_name {
        let _ =
            crate::multi_agent::git_release_branch(&pane_id_str(pane_num), branch, &project_name);
    }

    // Deregister from coordination system + release all file locks
    let _ = crate::multi_agent::agent_deregister(&pane_id_str(pane_num));
    machine::deregister(pane_num);

    let mut pane_state = pane_data;
    if let Some(session_id) = pane_state.dxos_session_id.clone() {
        let session_result = crate::dxos::update_session_status(
            &project_path,
            Some(&project_name),
            &session_id,
            "idle",
            Some(&format!("Pane killed: {}", reason)),
        );
        emit_dxos_session_change(app, &project_path, &session_result);
    }
    pane_state.status = "idle".into();
    pane_state.task = String::new();
    pane_state.project = "--".into();
    pane_state.project_path = String::new();
    pane_state.role = "--".into();
    pane_state.runtime_adapter = None;
    pane_state.dxos_session_id = None;
    pane_state.started_at = None;
    pane_state.acu_spent = 0.0;
    pane_state.issue_id = None;
    pane_state.space = None;
    pane_state.workspace_path = None;
    pane_state.branch_name = None;
    pane_state.base_branch = None;
    pane_state.machine_ip = None;
    pane_state.machine_hostname = None;
    pane_state.machine_mac = None;
    pane_state.tmux_target = None;
    app.state.set_pane(pane_num, pane_state).await;
    app.state
        .event_bus
        .send(crate::state::events::StateEvent::PaneKilled {
            pane: pane_num,
            reason: reason.clone(),
        });
    app.state
        .log_activity(pane_num, "kill", &format!("Killed: {}", reason))
        .await;

    remove_from_agents_json(pane_num);

    serde_json::json!({
        "status": "killed",
        "pane": pane_num,
        "reason": reason,
        "kill_method": kill_status,
        "git": git_info,
        "output_log": output_log,
    })
    .to_string()
}

/// Execute os_restart logic
pub async fn restart(app: &App, req: RestartRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let pane_data = app.state.get_pane(pane_num).await;
    if pane_data.project == "--" || pane_data.project.is_empty() {
        return json_err(&format!(
            "Pane {} has no previous config to restart",
            pane_num
        ));
    }

    let _ = kill(
        app,
        KillRequest {
            pane: pane_num.to_string(),
            reason: Some("restart".into()),
        },
    )
    .await;

    spawn(
        app,
        SpawnRequest {
            pane: pane_num.to_string(),
            project: if pane_data.project_path.is_empty() {
                pane_data.project
            } else {
                pane_data.project_path
            },
            session_id: None,
            role: Some(pane_data.role),
            provider: pane_data.provider,
            model: pane_data.model,
            runtime_adapter: pane_data.runtime_adapter,
            client_request_id: None,
            feature_id: None,
            stage: None,
            supervisor_session_id: None,
            task: Some(pane_data.task),
            prompt: None,
            autonomous: None,
        },
    )
    .await
}

/// Execute os_reassign logic — sends new task to running agent via PTY
pub async fn reassign(app: &App, req: ReassignRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let mut pane_data = app.state.get_pane(pane_num).await;
    if pane_data.status != "active" {
        return json_err(&format!("Pane {} is not active", pane_num));
    }

    if let Some(project) = &req.project {
        let path = config::resolve_project_path(project);
        pane_data.project = PathBuf::from(&path)
            .file_name()
            .map(|n: &std::ffi::OsStr| n.to_string_lossy().to_string())
            .unwrap_or_else(|| project.clone());
        pane_data.project_path = path;
    }
    if let Some(role) = &req.role {
        pane_data.role = role.clone();
    }
    if let Some(task) = &req.task {
        pane_data.task = task.clone();
    }

    if let Some(task) = &req.task {
        let msg = format!(
            "NEW TASK: {}\nRole: {}\nProject: {}\nPlease acknowledge and begin working on this new task.",
            task, pane_data.role, pane_data.project
        );
        // Send via tmux if available, otherwise PTY fallback
        if let Some(ref target) = pane_data.tmux_target {
            if let Err(e) = tmux::send_command(target, &msg) {
                tracing::warn!(
                    "Failed to send reassign via tmux to pane {}: {}",
                    pane_num,
                    e
                );
            }
        } else {
            let send_result = {
                let mut pty = app.pty_lock();
                pty.send_line(pane_num, &msg)
            };
            if let Err(e) = send_result {
                tracing::warn!(
                    "Failed to send reassign message to pane {}: {}",
                    pane_num,
                    e
                );
            }
        }
    }

    app.state.set_pane(pane_num, pane_data.clone()).await;
    if let Some(session_id) = pane_data.dxos_session_id.clone() {
        let session_result = crate::dxos::upsert_session_contract(
            &pane_data.project_path,
            Some(&pane_data.project),
            Some(&session_id),
            &pane_data.role,
            pane_data.provider.as_deref(),
            pane_data.model.as_deref(),
            Some("guarded_auto"),
            &pane_data.task,
            vec!["task_result".to_string(), "runtime_handoff".to_string()],
            app.state.get_project_mcps(&pane_data.project).await,
            vec![pane_data.project_path.clone()],
            pane_data
                .workspace_path
                .clone()
                .into_iter()
                .collect::<Vec<_>>(),
            pane_data.workspace_path.as_deref(),
            pane_data.branch_name.as_deref(),
            Some(config::pane_browser_port(pane_num)),
            Some(pane_num),
            pane_data.runtime_adapter.as_deref(),
            pane_data.tmux_target.as_deref(),
            None,
            Some("build"),
            None,
            Some("lead_then_human"),
            Some("active"),
        );
        emit_dxos_session_change(app, &pane_data.project_path, &session_result);
    }
    app.state
        .log_activity(
            pane_num,
            "reassign",
            &format!(
                "Reassigned: {}",
                truncate(req.task.as_deref().unwrap_or("config change"), 40)
            ),
        )
        .await;

    update_agents_json(pane_num, &pane_data.project, &pane_data.task);

    serde_json::json!({
        "status": "reassigned",
        "pane": pane_num,
        "updates": {
            "project": pane_data.project,
            "role": pane_data.role,
            "task": pane_data.task,
            "runtime_adapter": pane_data.runtime_adapter,
        }
    })
    .to_string()
}

fn launch_broker_json(plan: &runtime_broker::RuntimeLaunchPlan) -> serde_json::Value {
    serde_json::json!({
        "adapter": plan.adapter,
        "provider": plan.provider,
        "provider_label": plan.provider_label,
        "binary": plan.binary,
        "model": plan.model,
        "bootstrap_mode": plan.bootstrap_mode,
        "bootstrap_files": plan.bootstrap_files,
    })
}

fn launch_broker_json_from_error(window_name: &str, project_path: &str) -> serde_json::Value {
    serde_json::json!({
        "window_name": window_name,
        "project_path": project_path,
    })
}

/// Execute os_assign logic
pub async fn assign(app: &App, req: AssignRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let issue = match tracker::find_issue(&req.space, &req.issue_id) {
        Some(i) => i,
        None => {
            return json_err(&format!(
                "Issue {} not found in space {}",
                req.issue_id, req.space
            ))
        }
    };

    let project_path = app
        .state
        .get_space_project_path(&req.space)
        .await
        .unwrap_or_else(|| format!("{}/Projects/{}", config::home_dir().display(), req.space));

    let state_snap = app.state.get_state_snapshot().await;
    let role = issue
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or(&state_snap.config.default_role)
        .to_string();

    let title = issue.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let task = format!("[{}] {}", req.issue_id, title);
    let description = issue
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let priority = issue
        .get("priority")
        .and_then(|v| v.as_str())
        .unwrap_or("medium");
    let issue_type = issue.get("type").and_then(|v| v.as_str()).unwrap_or("task");
    let est_acu = issue
        .get("estimated_acu")
        .map(|v| v.to_string())
        .unwrap_or("not set".into());

    let prompt = format!(
        "You have been assigned issue {}: {}\n\nPriority: {}\nType: {}\n\nDescription:\n{}\n\nAcceptance criteria: Complete this issue and update its status when done.\nEstimated ACU: {}",
        req.issue_id, title, priority, issue_type, description, est_acu
    );

    let theme = config::theme_name(pane_num);
    let _ = tracker::update_issue(
        &req.space,
        &req.issue_id,
        &serde_json::json!({
            "status": "in_progress",
            "assignee": theme.to_lowercase(),
            "updated_at": state::now(),
        }),
    );

    let _result = spawn(
        app,
        SpawnRequest {
            pane: pane_num.to_string(),
            project: project_path,
            session_id: None,
            role: Some(role.clone()),
            provider: None,
            model: None,
            runtime_adapter: None,
            client_request_id: None,
            feature_id: None,
            stage: None,
            supervisor_session_id: None,
            task: Some(task),
            prompt: Some(prompt),
            autonomous: None,
        },
    )
    .await;

    let mut pane_data = app.state.get_pane(pane_num).await;
    pane_data.issue_id = Some(req.issue_id.clone());
    pane_data.space = Some(req.space.clone());
    app.state.set_pane(pane_num, pane_data).await;

    serde_json::json!({
        "status": "assigned",
        "pane": pane_num,
        "issue": req.issue_id,
        "title": title,
        "role": role,
    })
    .to_string()
}

/// Execute os_assign_adhoc logic
pub async fn assign_adhoc(app: &App, req: AssignAdhocRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let project = match &req.project {
        Some(p) if !p.is_empty() => p.clone(),
        _ => {
            let existing = app.state.get_pane(pane_num).await;
            if !existing.project_path.is_empty() {
                existing.project_path
            } else if existing.project != "--" {
                existing.project
            } else {
                "Projects".into()
            }
        }
    };

    spawn(
        app,
        SpawnRequest {
            pane: pane_num.to_string(),
            project,
            session_id: None,
            role: req.role.or(Some("developer".into())),
            provider: None,
            model: None,
            runtime_adapter: None,
            client_request_id: None,
            feature_id: None,
            stage: None,
            supervisor_session_id: None,
            task: Some(req.task),
            prompt: None,
            autonomous: None,
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_user_owned_guidance_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AGENTS.md");
        std::fs::write(&path, "# User guidance\nkeep this").unwrap();

        write_guidance_if_safe(path.to_str().unwrap(), "# DX guidance");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("keep this"));
    }

    #[test]
    fn updates_dx_owned_guidance_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AGENTS.md");
        std::fs::write(
            &path,
            format!(
                "{} {{\"provider\":\"shared\"}} -->\n\nold",
                DX_GENERATED_GUIDANCE_MARKER
            ),
        )
        .unwrap();

        write_guidance_if_safe(path.to_str().unwrap(), "# DX guidance");

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "# DX guidance");
    }
}

/// Execute os_collect logic — reads tmux output (or PTY fallback)
pub async fn collect(app: &App, req: CollectRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let pane_data = app.state.get_pane(pane_num).await;

    let git_info = if let Some(ws) = &pane_data.workspace_path {
        let status = workspace::git_status(ws).unwrap_or_default();
        let diff = workspace::git_diff(ws).unwrap_or_default();
        serde_json::json!({
            "branch": pane_data.branch_name,
            "status": status,
            "diff_stat": diff,
        })
    } else {
        serde_json::json!(null)
    };

    // Prefer tmux capture if we have a target
    if let Some(ref target) = pane_data.tmux_target {
        let t = target.clone();
        let output = tokio::task::spawn_blocking(move || tmux::capture_output(&t))
            .await
            .unwrap_or_default();
        let t2 = target.clone();
        let done = tokio::task::spawn_blocking(move || tmux::check_done(&t2))
            .await
            .unwrap_or(false);
        let t3 = target.clone();
        let error = tokio::task::spawn_blocking(move || tmux::check_error(&t3))
            .await
            .unwrap_or(None);

        let line_count = output.lines().count();
        let display_output = truncate(&output, 3000);

        if done && pane_data.status == "active" {
            app.state.update_pane_status(pane_num, "done").await;
        }

        return serde_json::json!({
            "pane": pane_num,
            "theme": pane_data.theme,
            "project": pane_data.project,
            "task": truncate(&pane_data.task, 60),
            "status": if done && pane_data.status == "active" { "done" } else { &pane_data.status },
            "branch": pane_data.branch_name,
            "tmux_target": target,
            "running": !done,
            "done": done,
            "error": error,
            "output": display_output,
            "line_count": line_count,
            "git": git_info,
        })
        .to_string();
    }

    // Fallback: try PTY
    let state_snap = app.state.get_state_snapshot().await;
    let markers = state_snap.config.completion_markers.clone();
    let pty_info = {
        let pty = app.pty_lock();
        if pty.has_agent(pane_num) {
            let output = pty.last_output(pane_num, 50).unwrap_or_default();
            let screen = pty.screen_text(pane_num).unwrap_or_default();
            let running = pty.is_running(pane_num);
            let health = pty.check_health(pane_num, &markers);
            let line_count = pty.line_count(pane_num);
            Some((output, screen, running, health, line_count))
        } else {
            None
        }
    };

    if let Some((output, screen, running, health, line_count)) = pty_info {
        let display_output = if !screen.trim().is_empty() {
            truncate(&screen, 3000)
        } else {
            truncate(&output, 3000)
        };

        if health.done && pane_data.status == "active" {
            app.state.update_pane_status(pane_num, "done").await;
        }

        serde_json::json!({
            "pane": pane_num,
            "theme": pane_data.theme,
            "project": pane_data.project,
            "task": truncate(&pane_data.task, 60),
            "status": pane_data.status,
            "branch": pane_data.branch_name,
            "running": running,
            "done": health.done,
            "error": health.error,
            "done_marker": health.done_marker,
            "exit_code": health.exit_code,
            "output": display_output,
            "line_count": line_count,
            "git": git_info,
        })
        .to_string()
    } else {
        let done = pane_data.status == "done" || pane_data.status == "idle";
        serde_json::json!({
            "pane": pane_num,
            "theme": pane_data.theme,
            "project": pane_data.project,
            "task": truncate(&pane_data.task, 60),
            "status": pane_data.status,
            "branch": pane_data.branch_name,
            "running": false,
            "done": done,
            "error": serde_json::Value::Null,
            "output": format!("[No agent] Pane {} - Status: {}", pane_num, pane_data.status),
            "line_count": 0,
            "git": git_info,
        })
        .to_string()
    }
}

/// Execute os_complete logic
pub async fn complete(app: &App, req: CompleteRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let mut pane_data = app.state.get_pane(pane_num).await;
    let summary = req
        .summary
        .clone()
        .unwrap_or_else(|| extract_result(app, pane_num));

    // Micro-helper: calculate ACU spent
    let acu = pane_data
        .started_at
        .as_deref()
        .map(calculate_acu)
        .unwrap_or(0.0);

    if let (Some(issue_id), Some(space)) = (&pane_data.issue_id, &pane_data.space) {
        let _ = tracker::update_issue(
            space,
            issue_id,
            &serde_json::json!({
                "status": "done",
                "actual_acu": acu,
                "updated_at": state::now(),
            }),
        );
    }

    let review_needed = matches!(pane_data.role.as_str(), "frontend" | "backend" | "devops");
    let _ = capacity::log_work_entry(serde_json::json!({
        "issue_id": pane_data.issue_id.as_deref().unwrap_or("adhoc"),
        "space": pane_data.space.as_deref().unwrap_or(""),
        "role": pane_data.role,
        "pane_id": pane_num.to_string(),
        "acu_spent": acu,
        "review_needed": review_needed,
        "logged_at": state::now(),
        "summary": summary,
    }));

    // Micro-helpers: git finalization + feature-to-code bridge
    let mut git_info = serde_json::json!(null);
    if let (Some(ws), Some(branch)) = (&pane_data.workspace_path, &pane_data.branch_name) {
        if let (Some(issue_id), Some(space)) = (&pane_data.issue_id, &pane_data.space) {
            let base = pane_data.base_branch.as_deref().unwrap_or("main");
            let started = pane_data.started_at.as_deref().unwrap_or("");
            attach_code_to_issue(space, issue_id, ws, base, started);
        }
        let result = finalize_git(
            ws,
            branch,
            &pane_data.project_path,
            pane_num,
            &pane_data.task,
            &summary,
            acu,
        );
        git_info = result.info;
    }

    let _output_log = save_agent_output(app, pane_num, "completed");

    // Save handoff context to KB for dependent tasks
    let result_text = extract_result(app, pane_num);
    if let Some(qt) = queue::task_for_pane(pane_num) {
        let pid = pane_id_str(pane_num);
        let handoff_content = format!(
            "Task: {}\nResult: {}\nSummary: {}\nBranch: {}\nPR: {}",
            qt.task,
            result_text,
            summary,
            pane_data.branch_name.as_deref().unwrap_or("none"),
            git_info
                .get("pr")
                .and_then(|v| v.as_str())
                .unwrap_or("none"),
        );
        let _ = crate::multi_agent::kb_add(
            &pid,
            &pane_data.project,
            "agent_handoff",
            &qt.id,
            &handoff_content,
            &[],
        );
    }

    // Kill the agent process (tmux or PTY)
    if let Some(ref target) = pane_data.tmux_target {
        let _ = tmux::kill_window(target);
    } else {
        let mut pty = app.pty_lock();
        let _ = pty.kill(pane_num);
    }

    if let Some(ref branch) = pane_data.branch_name {
        let _ = crate::multi_agent::git_release_branch(
            &pane_id_str(pane_num),
            branch,
            &pane_data.project,
        );
    }

    // Deregister from coordination system + release all file locks
    let _ = crate::multi_agent::agent_deregister(&pane_id_str(pane_num));

    remove_from_agents_json(pane_num);

    let task_display = truncate(&pane_data.task, 30);
    if let Some(session_id) = pane_data.dxos_session_id.clone() {
        let session_result = crate::dxos::update_session_status(
            &pane_data.project_path,
            Some(&pane_data.project),
            &session_id,
            "completed",
            Some(&summary),
        );
        emit_dxos_session_change(app, &pane_data.project_path, &session_result);
    }
    pane_data.status = "idle".into();
    pane_data.acu_spent = acu;
    pane_data.task = String::new();
    pane_data.project = "--".into();
    pane_data.project_path = String::new();
    pane_data.role = "--".into();
    pane_data.dxos_session_id = None;
    pane_data.started_at = None;
    pane_data.issue_id = None;
    pane_data.space = None;
    pane_data.workspace_path = None;
    pane_data.branch_name = None;
    pane_data.base_branch = None;
    pane_data.machine_ip = None;
    pane_data.machine_hostname = None;
    pane_data.machine_mac = None;
    pane_data.tmux_target = None;
    app.state.set_pane(pane_num, pane_data.clone()).await;
    app.state
        .log_activity(
            pane_num,
            "complete",
            &format!("Done: {} ({} ACU)", task_display, acu),
        )
        .await;

    serde_json::json!({
        "status": "completed",
        "pane": pane_num,
        "acu_spent": acu,
        "issue_id": pane_data.issue_id,
        "summary": summary,
        "git": git_info,
    })
    .to_string()
}

//! Factory tools: factory_run, factory_status, factory_list.
//!
//! Thin MCP wrappers over crate::factory pipeline API.

use super::super::types::*;
use super::helpers::*;
use crate::app::App;
use crate::factory;

/// Start a factory pipeline: detect project → create pipeline → trigger auto_cycle.
pub async fn factory_run(app: &App, req: FactoryRequest) -> String {
    // Step 1: Resolve project
    let project_name: String = if let Some(ref explicit) = req.project {
        explicit.clone()
    } else {
        match factory::detect_project(&req.request) {
            Some((name, confidence)) if confidence >= 0.2 => name,
            Some((name, confidence)) => {
                return json_err(&format!(
                    "Low confidence match ({:.0}%) to '{}'. Use project= to override.",
                    confidence * 100.0,
                    name
                ));
            }
            None => {
                return json_err(
                    "Could not identify a project. Be more specific or add project= parameter.",
                );
            }
        }
    };

    // Step 2: Create pipeline (default: "full" template = dev → qa+security → review)
    let template = req.template.as_deref().unwrap_or("full");
    let (pipeline_id, task_ids) = match factory::create_pipeline(
        &project_name,
        &req.request,
        template,
        req.priority.unwrap_or(1),
    ) {
        Ok(result) => result,
        Err(e) => return json_err(&format!("Pipeline creation failed: {}", e)),
    };

    // Step 3: Trigger auto_cycle to spawn the dev task immediately
    let _cycle = super::queue_tools::auto_cycle(app).await;

    // Step 4: Return pipeline info
    let pipeline = factory::get_pipeline(&pipeline_id);
    let stages: Vec<serde_json::Value> = pipeline
        .as_ref()
        .map(|p| {
            p.stages
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "name": s.name,
                        "role": s.role,
                        "task_id": s.task_id,
                        "status": s.status,
                        "pane": s.pane,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    serde_json::json!({
        "status": "pipeline_started",
        "pipeline_id": pipeline_id,
        "project": project_name,
        "template": template,
        "task_ids": task_ids,
        "stages": stages,
        "message": "Pipeline started. Dev agent spawns on next free pane. QA + Security auto-trigger when dev completes.",
    }).to_string()
}

/// Get status of a specific pipeline (or list all if pipeline_id omitted).
pub fn factory_status(req: &FactoryStatusRequest) -> String {
    match &req.pipeline_id {
        Some(pid) => match factory::get_pipeline(pid) {
            Some(pipeline) => {
                serde_json::to_string(&pipeline).unwrap_or_else(|e| json_err(&e.to_string()))
            }
            None => json_err(&format!("Pipeline '{}' not found", pid)),
        },
        None => factory_list(),
    }
}

/// Run quality gates on a pipeline (build, test, lint).
pub fn factory_gate(req: &FactoryStatusRequest) -> String {
    let pid = match &req.pipeline_id {
        Some(pid) => pid.clone(),
        None => return json_err("pipeline_id required for gate check"),
    };
    match factory::run_gate(&pid) {
        Ok(gate) => serde_json::json!({
            "pipeline_id": gate.pipeline_id,
            "project": gate.project,
            "passed": gate.passed,
            "build": gate.build.as_ref().map(|c| serde_json::json!({
                "command": c.command, "success": c.success,
                "duration_ms": c.duration_ms, "output": truncate(&c.output, 200),
            })),
            "test": gate.test.as_ref().map(|c| serde_json::json!({
                "command": c.command, "success": c.success,
                "duration_ms": c.duration_ms, "output": truncate(&c.output, 200),
            })),
            "lint": gate.lint.as_ref().map(|c| serde_json::json!({
                "command": c.command, "success": c.success,
                "duration_ms": c.duration_ms, "output": truncate(&c.output, 200),
            })),
        })
        .to_string(),
        Err(e) => json_err(&e.to_string()),
    }
}

/// List all pipelines.
pub fn factory_list() -> String {
    let pipelines = factory::list_pipelines();
    let active: Vec<&factory::PipelineView> = pipelines
        .iter()
        .filter(|p| p.status != "done" && p.status != "failed")
        .collect();
    let done = pipelines.iter().filter(|p| p.status == "done").count();
    let failed = pipelines.iter().filter(|p| p.status == "failed").count();

    let summaries: Vec<serde_json::Value> = pipelines
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "project": p.project,
                "description": truncate(&p.description, 50),
                "template": p.template,
                "status": p.status,
                "stages": p.stages.len(),
                "created_at": p.created_at,
            })
        })
        .collect();

    serde_json::json!({
        "pipelines": summaries,
        "active": active.len(),
        "done": done,
        "failed": failed,
        "total": pipelines.len(),
        "templates": factory::template_info().iter().map(|(name, stages)| {
            serde_json::json!({"name": name, "stages": stages})
        }).collect::<Vec<_>>(),
    })
    .to_string()
}

/// Cancel a running pipeline — kills pending stages and returns panes to kill.
pub async fn factory_cancel(app: &App, req: &FactoryStatusRequest) -> String {
    let pid = match &req.pipeline_id {
        Some(pid) => pid.clone(),
        None => return json_err("pipeline_id required to cancel a pipeline"),
    };

    match factory::cancel_pipeline(&pid) {
        Ok(result) => {
            // Kill running agents on the returned panes
            for pane in &result.running_panes {
                let _ = super::panes::kill(
                    app,
                    super::super::types::KillRequest {
                        pane: pane.to_string(),
                        reason: Some(format!("Pipeline {} cancelled", pid)),
                    },
                )
                .await;
            }

            serde_json::json!({
                "status": "cancelled",
                "pipeline_id": result.pipeline_id,
                "cancelled_tasks": result.cancelled_tasks,
                "killed_panes": result.running_panes,
            })
            .to_string()
        }
        Err(e) => json_err(&format!("Cancel failed: {}", e)),
    }
}

/// Detect which project a description refers to (standalone diagnostic).
pub fn factory_detect(req: &FactoryDetectRequest) -> String {
    match factory::detect_project(&req.description) {
        Some((name, confidence)) => serde_json::json!({
            "project": name,
            "confidence": format!("{:.0}%", confidence * 100.0),
            "confidence_raw": confidence,
        })
        .to_string(),
        None => json_err("No project matched. Check ~/Projects for git repos or run project_scan."),
    }
}

/// Get saved quality gate results for a pipeline.
pub fn factory_gate_result(req: &FactoryStatusRequest) -> String {
    let pid = match &req.pipeline_id {
        Some(pid) => pid,
        None => return json_err("pipeline_id required"),
    };
    match factory::get_gate_result(pid) {
        Some(gate) => serde_json::json!({
            "pipeline_id": gate.pipeline_id,
            "project": gate.project,
            "passed": gate.passed,
            "build": gate.build.as_ref().map(|c| serde_json::json!({
                "command": c.command, "success": c.success,
                "duration_ms": c.duration_ms, "output": truncate(&c.output, 500),
            })),
            "test": gate.test.as_ref().map(|c| serde_json::json!({
                "command": c.command, "success": c.success,
                "duration_ms": c.duration_ms, "output": truncate(&c.output, 500),
            })),
            "lint": gate.lint.as_ref().map(|c| serde_json::json!({
                "command": c.command, "success": c.success,
                "duration_ms": c.duration_ms, "output": truncate(&c.output, 500),
            })),
        })
        .to_string(),
        None => json_err(&format!("No gate results found for pipeline '{}'", pid)),
    }
}

/// Retry failed stages in a pipeline.
pub fn factory_retry(req: &FactoryStatusRequest) -> String {
    let pid = match &req.pipeline_id {
        Some(pid) => pid,
        None => return json_err("pipeline_id required to retry a pipeline"),
    };
    match factory::retry_pipeline(pid) {
        Ok(result) => serde_json::json!({
            "status": "retried",
            "pipeline_id": result.pipeline_id,
            "retried_tasks": result.retried_tasks,
            "task_ids": result.task_ids,
        })
        .to_string(),
        Err(e) => json_err(&format!("Retry failed: {}", e)),
    }
}

/// Get pipeline events log.
pub fn factory_events(req: &FactoryStatusRequest) -> String {
    let pid = match &req.pipeline_id {
        Some(pid) => pid,
        None => return json_err("pipeline_id required"),
    };
    let events = factory::get_pipeline_events(pid);
    serde_json::json!({
        "pipeline_id": pid,
        "event_count": events.len(),
        "events": events,
    })
    .to_string()
}

/// Pause a pipeline — blocked/pending stages won't spawn until resumed.
pub fn factory_pause(req: &FactoryStatusRequest) -> String {
    let pid = match &req.pipeline_id {
        Some(pid) => pid,
        None => return json_err("pipeline_id required to pause"),
    };
    match factory::pause_pipeline(pid) {
        Ok(()) => serde_json::json!({
            "status": "paused",
            "pipeline_id": pid,
        })
        .to_string(),
        Err(e) => json_err(&format!("Pause failed: {}", e)),
    }
}

/// Resume a paused pipeline.
pub fn factory_resume(req: &FactoryStatusRequest) -> String {
    let pid = match &req.pipeline_id {
        Some(pid) => pid,
        None => return json_err("pipeline_id required to resume"),
    };
    match factory::resume_pipeline(pid) {
        Ok(()) => serde_json::json!({
            "status": "resumed",
            "pipeline_id": pid,
        })
        .to_string(),
        Err(e) => json_err(&format!("Resume failed: {}", e)),
    }
}

/// Retry a specific stage by name within a pipeline.
pub fn factory_retry_stage(req: &FactoryRetryStageRequest) -> String {
    match factory::retry_stage(&req.pipeline_id, &req.stage) {
        Ok(msg) => serde_json::json!({
            "status": "retried",
            "pipeline_id": req.pipeline_id,
            "stage": req.stage,
            "message": msg,
        })
        .to_string(),
        Err(e) => json_err(&format!("Retry stage failed: {}", e)),
    }
}

/// Get factory inbox status — shows TUI-submitted requests and their pipeline mappings.
pub fn factory_inbox() -> String {
    let inbox = factory::load_inbox();
    let summary: Vec<serde_json::Value> = inbox
        .requests
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "request": truncate(&r.request, 60),
                "status": r.status,
                "pipeline_id": r.pipeline_id,
                "classification": r.classification,
                "task_count": r.tasks.len(),
                "created_at": r.created_at,
                "error": r.error,
            })
        })
        .collect();

    let pending = inbox
        .requests
        .iter()
        .filter(|r| r.status == "pending")
        .count();
    let running = inbox
        .requests
        .iter()
        .filter(|r| r.status == "running")
        .count();
    let complete = inbox
        .requests
        .iter()
        .filter(|r| r.status == "complete")
        .count();
    let failed = inbox
        .requests
        .iter()
        .filter(|r| r.status == "failed")
        .count();

    serde_json::json!({
        "requests": summary,
        "summary": {
            "pending": pending,
            "running": running,
            "complete": complete,
            "failed": failed,
            "total": inbox.requests.len(),
        },
    })
    .to_string()
}

/// Scan for conflicts in a pipeline's project.
pub fn conflict_scan(req: &FactoryStatusRequest) -> String {
    match &req.pipeline_id {
        Some(pid) => factory::conflict_scan(pid).to_string(),
        None => json_err("pipeline_id required for conflict scan"),
    }
}

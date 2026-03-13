use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecoveryGap {
    pub kind: String,
    pub severity: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SuggestedSessionPlan {
    pub role: String,
    pub stage: String,
    pub priority: String,
    #[serde(default)]
    pub feature_id: Option<String>,
    pub reason: String,
    pub task_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecoveryPlan {
    pub mode: String,
    pub summary: String,
    pub feature_total: usize,
    pub active_delivery_features: usize,
    pub runtime_count: usize,
    pub worktree_count: usize,
    #[serde(default)]
    pub gaps: Vec<RecoveryGap>,
    #[serde(default)]
    pub suggested_sessions: Vec<SuggestedSessionPlan>,
    pub next_action: SuggestedSessionPlan,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdoptionDefaults {
    pub summary: String,
    pub objective: String,
    #[serde(default)]
    pub feature_id: Option<String>,
    pub stage: String,
}

pub fn assess(
    project: &str,
    phase_counts: &HashMap<String, usize>,
    documentation_health: &Value,
    blocking_features: &[Value],
    ready_features: &[Value],
    client_review_features: &[Value],
    runtime_count: usize,
    worktree_count: usize,
    features: &[Value],
) -> RecoveryPlan {
    let missing_feature_docs = documentation_health
        .get("missing_feature_docs")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let missing_acceptance = documentation_health
        .get("missing_acceptance")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let dirty_docs = documentation_health
        .get("dirty_docs")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let shared_guidance_present = documentation_health
        .get("shared_guidance_present")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let missing_provider_guidance = documentation_health
        .get("missing_provider_guidance")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    let feature_total = features.len();
    let active_delivery = phase_counts
        .iter()
        .filter(|(phase, _)| !matches!(phase.as_str(), "planned" | "done"))
        .map(|(_, count)| *count)
        .sum::<usize>();

    let mode = if feature_total == 0 {
        "unscoped".to_string()
    } else if active_delivery > 0 || runtime_count > 0 || !blocking_features.is_empty() {
        "adopt_in_progress".to_string()
    } else {
        "structured_planning".to_string()
    };

    let summary = match mode.as_str() {
        "unscoped" => format!(
            "{} has code and runtime surfaces, but DXOS does not have a structured feature map yet.",
            project
        ),
        "adopt_in_progress" if !client_review_features.is_empty() => {
            "The project is already moving, but design or client approval still blocks trusted build execution."
                .to_string()
        }
        "adopt_in_progress" if !blocking_features.is_empty() => {
            "The project is already in flight, but it needs a recovery pass to clear blockers, fill documentation gaps, and restart the right specialist lanes."
                .to_string()
        }
        "adopt_in_progress" => {
            "DXOS can adopt this in-progress project, verify its missing artifacts, and push it through the remaining delivery stages."
                .to_string()
        }
        _ => {
            "The project has a structured plan, but it still needs the right discovery, build, and QA lanes before delivery can accelerate."
                .to_string()
        }
    };

    let primary_missing_docs = missing_feature_docs
        .first()
        .cloned()
        .unwrap_or_else(|| json!({}));
    let primary_client_review = client_review_features
        .first()
        .cloned()
        .unwrap_or_else(|| json!({}));
    let primary_ready = ready_features.first().cloned().unwrap_or_else(|| json!({}));
    let primary_missing_acceptance = missing_acceptance
        .first()
        .cloned()
        .unwrap_or_else(|| json!({}));
    let primary_blocked = blocking_features
        .first()
        .cloned()
        .unwrap_or_else(|| json!({}));

    let mut gaps = Vec::new();
    if !shared_guidance_present {
        gaps.push(RecoveryGap {
            kind: "shared_guidance".to_string(),
            severity: "high".to_string(),
            summary: "AGENTS.md is missing, so shared operating guidance is not aligned yet."
                .to_string(),
        });
    }
    if !missing_provider_guidance.is_empty() {
        gaps.push(RecoveryGap {
            kind: "provider_guidance".to_string(),
            severity: "medium".to_string(),
            summary: format!(
                "Active provider guidance is missing for {}.",
                missing_provider_guidance
                    .iter()
                    .filter_map(|value| value.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    }
    if !missing_feature_docs.is_empty() {
        gaps.push(RecoveryGap {
            kind: "feature_docs".to_string(),
            severity: "high".to_string(),
            summary: format!(
                "{} features have advanced without attached discovery or research docs.",
                missing_feature_docs.len()
            ),
        });
    }
    if !missing_acceptance.is_empty() {
        gaps.push(RecoveryGap {
            kind: "acceptance".to_string(),
            severity: "high".to_string(),
            summary: format!(
                "{} delivery-phase features still lack acceptance coverage.",
                missing_acceptance.len()
            ),
        });
    }
    if !client_review_features.is_empty() {
        gaps.push(RecoveryGap {
            kind: "client_approval".to_string(),
            severity: "high".to_string(),
            summary: format!(
                "{} features still need client or design approval before build should continue.",
                client_review_features.len()
            ),
        });
    }
    if !blocking_features.is_empty() {
        gaps.push(RecoveryGap {
            kind: "blocked_features".to_string(),
            severity: "medium".to_string(),
            summary: format!(
                "{} features are blocked at their next stage gate.",
                blocking_features.len()
            ),
        });
    }
    if !dirty_docs.is_empty() {
        gaps.push(RecoveryGap {
            kind: "doc_drift".to_string(),
            severity: "medium".to_string(),
            summary: format!(
                "{} docs have drift or uncommitted changes that should be reconciled.",
                dirty_docs.len()
            ),
        });
    }
    if runtime_count == 0 && (!ready_features.is_empty() || active_delivery > 0) {
        gaps.push(RecoveryGap {
            kind: "no_active_lanes".to_string(),
            severity: "medium".to_string(),
            summary:
                "No active runtime lanes are attached even though the project has work ready to move."
                    .to_string(),
        });
    }

    let mut suggested_sessions = Vec::new();
    if mode == "unscoped" {
        suggested_sessions.push(SuggestedSessionPlan {
            role: "lead".to_string(),
            stage: "discovery".to_string(),
            priority: "high".to_string(),
            feature_id: None,
            reason: "Map the current project into a structured DXOS feature tree before more implementation work happens."
                .to_string(),
            task_prompt: format!(
                "Adopt {} into DXOS. Inventory the existing code, reconstruct missing features and stages, and produce the first recovery plan with evidence gaps.",
                project
            ),
        });
    }
    if !missing_feature_docs.is_empty() || !shared_guidance_present {
        let feature_id = primary_missing_docs
            .get("feature_id")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());
        suggested_sessions.push(SuggestedSessionPlan {
            role: "discovery".to_string(),
            stage: "discovery".to_string(),
            priority: "high".to_string(),
            feature_id: feature_id.clone(),
            reason: "Reconstruct missing discovery context, attach research notes, and align shared guidance before more execution happens."
                .to_string(),
            task_prompt: feature_id.as_ref().map(|feature_id| {
                format!(
                    "Reconstruct discovery context for {}. Attach missing research or discovery docs, list unresolved assumptions, and align shared guidance before more build work continues.",
                    feature_id
                )
            }).unwrap_or_else(|| {
                format!(
                    "Reconstruct missing discovery context for {}. Attach research notes, clarify assumptions, and align AGENTS.md plus provider guidance before more execution continues.",
                    project
                )
            }),
        });
    }
    if !client_review_features.is_empty() {
        let feature_id = primary_client_review
            .get("feature_id")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());
        suggested_sessions.push(SuggestedSessionPlan {
            role: "design".to_string(),
            stage: "design".to_string(),
            priority: "high".to_string(),
            feature_id: feature_id.clone(),
            reason: "Prepare design options or client-review artifacts so the blocked feature set can move into build with approval."
                .to_string(),
            task_prompt: feature_id.as_ref().map(|feature_id| {
                format!(
                    "Prepare approval-ready design options for {}. Generate alternatives, explain the tradeoffs, and package the best direction for client review.",
                    feature_id
                )
            }).unwrap_or_else(|| {
                "Prepare design options and approval-ready mockups for the client-blocked feature set so build can continue with trusted direction."
                    .to_string()
            }),
        });
    }
    if !ready_features.is_empty() && runtime_count == 0 {
        let feature_id = primary_ready
            .get("feature_id")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());
        let next_gate = primary_ready
            .get("next_gate")
            .and_then(|value| value.as_str())
            .unwrap_or("build");
        let role = match next_gate {
            "test" => "qa",
            "done" => "release",
            _ => "frontend",
        };
        let stage = match next_gate {
            "test" => "test",
            "done" => "done",
            _ => "build",
        };
        suggested_sessions.push(SuggestedSessionPlan {
            role: role.to_string(),
            stage: stage.to_string(),
            priority: "medium".to_string(),
            feature_id: feature_id.clone(),
            reason: "There is work ready to build, but no live implementation lane is running."
                .to_string(),
            task_prompt: feature_id.as_ref().map(|feature_id| {
                format!(
                    "Start a {} lane for {} and move it through {} with evidence and linked artifacts.",
                    role, feature_id, stage
                )
            }).unwrap_or_else(|| {
                format!(
                    "Start the next {} lane for {} and move the ready work forward with evidence.",
                    role, project
                )
            }),
        });
    }
    if !missing_acceptance.is_empty() {
        let feature_id = primary_missing_acceptance
            .get("feature_id")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());
        suggested_sessions.push(SuggestedSessionPlan {
            role: "qa".to_string(),
            stage: "test".to_string(),
            priority: "high".to_string(),
            feature_id: feature_id.clone(),
            reason: "Backfill acceptance criteria and verification evidence before calling work complete."
                .to_string(),
            task_prompt: feature_id.as_ref().map(|feature_id| {
                format!(
                    "Backfill acceptance criteria and verification evidence for {} before the project treats it as complete.",
                    feature_id
                )
            }).unwrap_or_else(|| {
                "Backfill missing acceptance criteria, verification evidence, and test coverage for the current delivery-stage work."
                    .to_string()
            }),
        });
    }
    if !dirty_docs.is_empty() {
        suggested_sessions.push(SuggestedSessionPlan {
            role: "docs".to_string(),
            stage: "build".to_string(),
            priority: "medium".to_string(),
            feature_id: None,
            reason: "Clean up documentation drift so the portal, Git, and runtime state tell the same story."
                .to_string(),
            task_prompt: "Reconcile documentation drift, align handbook pages with the current Git/runtime state, and leave the project in a clean sync state."
                .to_string(),
        });
    }
    if !blocking_features.is_empty() {
        let feature_id = primary_blocked
            .get("feature_id")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());
        suggested_sessions.push(SuggestedSessionPlan {
            role: "lead".to_string(),
            stage: "build".to_string(),
            priority: "high".to_string(),
            feature_id: feature_id.clone(),
            reason: "Review blockers, route missing approvals, and re-sequence worker lanes to restart stalled work."
                .to_string(),
            task_prompt: feature_id.as_ref().map(|feature_id| {
                format!(
                    "Review blockers on {}. Route missing approvals, answer open questions, and re-sequence the next worker lane so execution can restart.",
                    feature_id
                )
            }).unwrap_or_else(|| {
                "Review the blocked feature set, route missing approvals or evidence, and re-sequence worker lanes so stalled work can restart."
                    .to_string()
            }),
        });
    }

    let next_action = suggested_sessions.first().cloned().unwrap_or_else(|| SuggestedSessionPlan {
        role: "lead".to_string(),
        stage: "build".to_string(),
        priority: "medium".to_string(),
        feature_id: None,
        reason: "Review the current project state and choose the next governed lane.".to_string(),
        task_prompt: format!(
            "Review {} and identify the next governed lane that should be launched through DXOS.",
            project
        ),
    });

    RecoveryPlan {
        mode,
        summary,
        feature_total,
        active_delivery_features: active_delivery,
        runtime_count,
        worktree_count,
        gaps,
        suggested_sessions,
        next_action,
    }
}

pub fn derive_adoption_defaults(
    project: &str,
    recovery: &RecoveryPlan,
    focus_feature_id: Option<&str>,
    focus_stage: Option<&str>,
) -> AdoptionDefaults {
    let summary = if recovery.summary.trim().is_empty() {
        format!(
            "Adopt {} into DXOS, reconstruct the current truth, and build the first governed recovery plan.",
            project
        )
    } else {
        recovery.summary.trim().to_string()
    };
    let objective = if recovery.next_action.task_prompt.trim().is_empty() {
        format!(
            "Inventory {}, reconstruct the active features and stages, and seed the first governed recovery council.",
            project
        )
    } else {
        recovery.next_action.task_prompt.trim().to_string()
    };
    let feature_id = recovery
        .next_action
        .feature_id
        .clone()
        .or_else(|| focus_feature_id.map(|value| value.trim().to_string()))
        .filter(|value| !value.is_empty());
    let stage = if recovery.next_action.stage.trim().is_empty() {
        focus_stage
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("discovery")
            .to_string()
    } else {
        recovery.next_action.stage.trim().to_string()
    };
    AdoptionDefaults {
        summary,
        objective,
        feature_id,
        stage,
    }
}

pub fn follow_on_suggestions(recovery: &RecoveryPlan) -> Vec<SuggestedSessionPlan> {
    recovery
        .suggested_sessions
        .iter()
        .skip(1)
        .cloned()
        .collect()
}

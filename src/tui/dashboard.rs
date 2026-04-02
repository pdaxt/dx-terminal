use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use super::widgets;
use super::ViewMode;
use crate::app::App;
use crate::audit;
use crate::capacity;
use crate::config;
use crate::multi_agent;
use crate::quality;
use crate::queue;
use crate::runtime_panes;
use crate::scanner;
use crate::tracker;

const GRID_CAPTURE_LINES: u32 = 160;
const FOCUSED_CAPTURE_LINES: u32 = 500;
/// Lines shown per pane card in the multiplexer grid (live output preview).
const LIVE_PREVIEW_LINES: usize = 5;

/// Snapshot of pane data for rendering (no locks held during draw)
pub struct PaneSnapshot {
    pub pane: u8,
    pub theme: String,
    pub theme_fg: String,
    pub project: String,
    pub role: String,
    pub provider: String,
    pub task: String,
    pub status: String,
    pub branch: Option<String>,
    pub pty_running: bool,
    pub line_count: usize,
    pub health: String,  // "error", "done", "stuck", "ok", ""
    pub runtime: String, // "3m", "1h22m", "" for non-active
    pub tmux_target: Option<String>,
    pub output: String,
    pub state_backed: bool,
}

/// Snapshot of a feature and its micro-features
pub struct FeatureSnapshot {
    pub id: String,
    pub title: String,
    pub status: String,
    pub space: String,
    pub children: Vec<MicroFeatureSnapshot>,
    pub done: usize,
    pub total: usize,
}

pub struct MicroFeatureSnapshot {
    pub id: String,
    pub title: String,
    pub status: String,
    pub queue_status: Option<String>,
    pub pane: Option<u8>,
}

/// Project health snapshot for project view
#[allow(dead_code)]
pub struct ProjectSnapshot {
    pub name: String,
    pub tech: String,
    pub health_grade: String,
    pub health_score: i64,
    pub last_test: Option<(bool, String)>, // (passed, relative_time)
    pub last_build: Option<(bool, String)>,
    pub open_issues: usize,
    pub active_agents: usize,
    pub git_dirty: bool,
    pub git_ahead: i32,
    pub git_behind: i32,
    pub last_commit: Option<String>,
    pub readme: Option<String>,
}

/// Board column for kanban view
pub struct BoardColumn {
    pub name: String,
    pub cards: Vec<BoardCard>,
}

pub struct BoardCard {
    pub id: String,
    pub title: String,
    pub priority: String,
    pub role: String,
}

/// Coordination snapshot (locks, agents, KB)
pub struct CoordSnapshot {
    pub agents: Vec<(String, String, String)>, // (pane_id, project, task)
    pub locks: Vec<(String, String)>,          // (pane_id, file_path)
    pub kb_recent: Vec<(String, String, String)>, // (category, title, pane_id)
    pub branches: Vec<(String, String, String)>, // (pane_id, branch, project)
    pub ports: Vec<(i64, String, String)>,     // (port, service, pane_id)
    pub claims: Vec<(String, u32, String, String)>, // (repo, issue, agent_id, claimed_at)
}

/// Infrastructure snapshot (ports, builds, messages, sessions)
pub struct InfraSnapshot {
    pub ports: Vec<(i64, String, String)>, // (port, service, pane_id)
    pub builds: Vec<(String, bool, String)>, // (project, success, time_ago)
    pub messages: Vec<(String, String, String, String)>, // (from, to, message, priority)
    pub sessions: Vec<(String, String, String)>, // (pane_id, project, status)
}

/// Intelligence snapshot (kgraph, facts, replay, analytics)
pub struct IntelSnapshot {
    pub kgraph_entities: i64,
    pub kgraph_edges: i64,
    pub kgraph_top: Vec<(String, i64)>, // (entity_name, edge_count)
    pub facts: Vec<(String, String, bool)>, // (key, value, verified)
    pub fact_count: i64,
    pub replay_sessions: i64,
    pub replay_tool_calls: i64,
    pub replay_errors: i64,
    pub top_tools: Vec<(String, f64)>, // (tool_name, weight)
}

/// Audit snapshot for a single project
pub struct AuditProjectSummary {
    pub name: String,
    pub grade: String,
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    pub info: usize,
    pub total: usize,
    pub last_audit: String,
    pub top_findings: Vec<(String, String, String, usize)>, // (severity, category, file, line)
}

/// Aggregate audit snapshot
pub struct AuditSnapshot {
    pub projects: Vec<AuditProjectSummary>,
    pub total_critical: usize,
    pub total_high: usize,
    pub worst_grade: String,
}

/// Action log entry for TUI command history
#[derive(Clone)]
pub struct ActionLogEntry {
    pub timestamp: String,
    pub tool: String,
    pub success: bool,
    pub summary: String,
}

/// Pipeline snapshot for factory view
pub struct PipelineSnapshot {
    pub id: String,
    pub project: String,
    pub description: String,
    pub template: String,
    pub status: String,
    #[allow(dead_code)]
    pub paused: bool,
    pub stages: Vec<PipelineStageSnapshot>,
    pub gate_passed: Option<bool>,
    pub signal_count: usize,
}

#[allow(dead_code)]
pub struct PipelineStageSnapshot {
    pub name: String,
    pub role: String,
    pub status: String,
    pub pane: Option<u8>,
    pub pty_snippet: Option<String>,
}

/// Full dashboard snapshot
#[allow(dead_code)]
pub struct DashboardData {
    pub panes: Vec<PaneSnapshot>,
    pub selected: u8,
    pub pane_focus: bool,
    pub acu_used: f64,
    pub acu_total: f64,
    pub reviews_used: usize,
    pub reviews_total: usize,
    pub active_count: usize,
    pub pty_count: usize,
    pub selected_output: String,
    pub selected_screen: String,
    pub log_lines: Vec<String>,
    pub queue_lines: Vec<(String, String, String, String, String, Option<String>)>, // (status, priority, project, task, id, issue_id)
    pub queue_pending: usize,
    pub queue_running: usize,
    pub queue_done: usize,
    pub queue_failed: usize,
    pub features: Vec<FeatureSnapshot>,
    pub view_mode: ViewMode,
    pub alerts: Vec<(u8, String)>, // (pane, message)
    pub roles: Vec<(String, f64)>, // (name, utilization_pct)
    pub board: Vec<BoardColumn>,
    pub coord: CoordSnapshot,
    pub started_at: Vec<(u8, String)>, // (pane, started_at timestamp)
    pub projects: Vec<ProjectSnapshot>,
    pub feature_cursor: usize,
    pub infra: InfraSnapshot,
    pub intel: IntelSnapshot,
    pub audit: AuditSnapshot,
    pub action_log: Vec<ActionLogEntry>,
    pub pipelines: Vec<PipelineSnapshot>,
    pub signal_count: usize,
    pub pane_signals: std::collections::HashMap<u8, Vec<(String, String)>>,
    pub screen_count: usize,
    pub screen_names: Vec<String>,
    pub builds: Vec<crate::build::BuildInfo>,
}

/// Collect all data in one pass (lock once, release)
pub fn collect_data(
    app: &App,
    selected: u8,
    view_mode: ViewMode,
    feature_cursor: usize,
    pane_focus: bool,
) -> DashboardData {
    let state = app.state.blocking_read().clone();
    let live_panes = crate::tmux::discover_live_panes();
    let resolved_panes = runtime_panes::resolve_runtime_panes(&state, &live_panes, None);

    let log_lines: Vec<String> = state
        .activity_log
        .iter()
        .take(15)
        .map(|e| {
            let ts = e.ts.get(11..16).unwrap_or(&e.ts);
            format!("{} P{} {}", ts, e.pane, &e.summary)
        })
        .collect();

    let markers = state.config.completion_markers.clone();

    // Tmux-first health checks (PTY fallback)
    let mut panes = Vec::new();
    let mut alerts = Vec::new();
    let mut pty_count = 0;
    let mut active_count = 0;
    {
        let pty = app.pty_lock();

        for resolved in resolved_panes {
            let pane = resolved.pane;
            let focused_selected_pane = pane_focus && pane == selected;
            let pane_state = resolved.pane_state.clone();
            let tmux_target = pane_state.tmux_target.clone();
            let provider = resolved
                .live
                .as_ref()
                .map(|live| {
                    crate::tmux::infer_provider(
                        &live.command,
                        &live.window_name,
                        live.jsonl_path.as_deref(),
                    )
                    .to_string()
                })
                .or_else(|| pane_state.provider.clone())
                .unwrap_or_else(|| "unknown".to_string());
            let role = if !pane_state.role.trim().is_empty() {
                config::role_short(&pane_state.role).to_string()
            } else {
                crate::tmux::provider_short(&provider).to_string()
            };
            let project = if !pane_state.project.trim().is_empty() && pane_state.project != "--" {
                pane_state.project.clone()
            } else if let Some(ref live) = resolved.live {
                runtime_panes::project_from_cwd(
                    live.jsonl_path
                        .as_deref()
                        .and_then(crate::tmux::read_jsonl_cwd)
                        .as_deref()
                        .unwrap_or(&live.cwd),
                )
            } else {
                "--".to_string()
            };
            let task = if !pane_state.task.trim().is_empty() {
                pane_state.task.clone()
            } else if let Some(ref target) = tmux_target {
                format!("{} in {}", crate::tmux::provider_label(&provider), target)
            } else {
                "--".to_string()
            };
            let runtime = if pane_state.status == "active" {
                pane_state
                    .started_at
                    .as_deref()
                    .map(format_runtime)
                    .unwrap_or_default()
            } else {
                String::new()
            };

            let mut snapshot = PaneSnapshot {
                pane,
                theme: config::theme_name(pane).to_string(),
                theme_fg: config::theme_fg(pane).to_string(),
                project,
                role,
                provider,
                task,
                status: pane_state.status.clone(),
                branch: pane_state.branch_name.clone(),
                pty_running: false,
                line_count: 0,
                health: String::new(),
                runtime,
                tmux_target: tmux_target.clone(),
                output: String::new(),
                state_backed: resolved.state_backed,
            };

            if let Some(ref target) = tmux_target {
                if resolved.live.is_some() || crate::tmux::pane_exists(target) {
                    let capture_lines = if focused_selected_pane {
                        FOCUSED_CAPTURE_LINES
                    } else {
                        GRID_CAPTURE_LINES
                    };
                    snapshot.pty_running = true;
                    pty_count += 1;
                    snapshot.output = crate::tmux::capture_output_extended(target, capture_lines);
                    snapshot.line_count = snapshot.output.lines().count();

                    if snapshot.status == "active" {
                        if tmux_done_from_output(&snapshot.output) {
                            snapshot.health = "done".to_string();
                        } else if let Some(err) = tmux_error_from_output(&snapshot.output) {
                            snapshot.health = "error".to_string();
                            alerts.push((snapshot.pane, err));
                        } else {
                            snapshot.health = "ok".to_string();
                        }
                    }
                }
            } else {
                let capture_lines = if focused_selected_pane {
                    FOCUSED_CAPTURE_LINES as usize
                } else {
                    GRID_CAPTURE_LINES as usize
                };
                snapshot.pty_running = pty.is_running(pane);
                snapshot.line_count = pty.line_count(pane);
                if snapshot.pty_running {
                    pty_count += 1;
                }
                if pty.has_agent(pane) {
                    let history = pty.last_output(pane, capture_lines).unwrap_or_default();
                    snapshot.output = if focused_selected_pane {
                        if history.trim().is_empty() {
                            pty.screen_text(pane).unwrap_or_default()
                        } else {
                            history
                        }
                    } else {
                        pty.screen_text(pane)
                            .filter(|screen| !screen.trim().is_empty())
                            .unwrap_or(history)
                    };
                }
                if snapshot.status == "active" && pty.has_agent(pane) {
                    let h = pty.check_health(pane, &markers);
                    if let Some(ref err) = h.error {
                        snapshot.health = "error".to_string();
                        alerts.push((snapshot.pane, err.clone()));
                    } else if h.done {
                        snapshot.health = "done".to_string();
                    } else {
                        snapshot.health = "ok".to_string();
                    }
                }
            }

            if snapshot.status == "active" || snapshot.pty_running || has_pane_identity(&snapshot) {
                if snapshot.status == "active" {
                    active_count += 1;
                }
                panes.push(snapshot);
            }
        }
    }

    panes.sort_by_key(|pane| pane.pane);

    let (selected_output, selected_screen) = panes
        .iter()
        .find(|pane| pane.pane == selected)
        .map(|pane| {
            let lines: Vec<&str> = pane.output.lines().collect();
            let tail = lines
                .iter()
                .rev()
                .take(40)
                .rev()
                .copied()
                .collect::<Vec<&str>>()
                .join("\n");
            (tail, pane.output.clone())
        })
        .unwrap_or_else(|| (String::new(), String::new()));

    let cap = capacity::load_capacity();

    // Role utilization
    let roles_data = capacity::cap_roles();
    let mut roles: Vec<(String, f64)> = Vec::new();
    if let Some(roles_obj) = roles_data.get("roles").and_then(|v| v.as_object()) {
        for (name, info) in roles_obj {
            let util = info
                .get("utilization_pct")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let display = info
                .get("display_name")
                .and_then(|v| v.as_str())
                .unwrap_or(name);
            roles.push((display.to_string(), util));
        }
    }
    roles.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Queue data — include ALL tasks, sorted: running > pending > blocked > failed > done
    let q = queue::load_queue();
    let mut queue_pending = 0usize;
    let mut queue_running = 0usize;
    let mut queue_done = 0usize;
    let mut queue_failed = 0usize;

    let mut sorted_tasks: Vec<&queue::QueueTask> = q.tasks.iter().collect();
    sorted_tasks.sort_by_key(|t| match t.status {
        queue::QueueStatus::Running => 0,
        queue::QueueStatus::Pending => 1,
        queue::QueueStatus::Blocked => 2,
        queue::QueueStatus::Failed => 3,
        queue::QueueStatus::Done => 4,
    });

    let queue_lines: Vec<(String, String, String, String, String, Option<String>)> = sorted_tasks
        .iter()
        .map(|t| {
            match t.status {
                queue::QueueStatus::Pending => queue_pending += 1,
                queue::QueueStatus::Running => queue_running += 1,
                queue::QueueStatus::Done => queue_done += 1,
                queue::QueueStatus::Failed => queue_failed += 1,
                queue::QueueStatus::Blocked => {}
            }
            let status = match t.status {
                queue::QueueStatus::Pending => "PEND",
                queue::QueueStatus::Running => "RUN ",
                queue::QueueStatus::Failed => "FAIL",
                queue::QueueStatus::Blocked => "BLOK",
                queue::QueueStatus::Done => "DONE",
            };
            let proj = t
                .project
                .split('/')
                .last()
                .unwrap_or(&t.project)
                .to_string();
            (
                status.to_string(),
                format!("P{}", t.priority),
                proj,
                t.task.clone(),
                t.id.clone(),
                t.issue_id.clone(),
            )
        })
        .collect();

    // Board data
    let board = if view_mode == ViewMode::Board {
        collect_board()
    } else {
        Vec::new()
    };

    // Feature data — always collect for Normal view summary + Features view
    let features = collect_features(&q);

    // Project data
    let projects = if view_mode == ViewMode::Projects {
        collect_projects()
    } else {
        Vec::new()
    };

    // Coordination data
    let coord = if view_mode == ViewMode::Coord {
        collect_coord(&q)
    } else {
        CoordSnapshot {
            agents: Vec::new(),
            locks: Vec::new(),
            kb_recent: Vec::new(),
            branches: Vec::new(),
            ports: Vec::new(),
            claims: Vec::new(),
        }
    };

    // Infrastructure data
    let infra = if view_mode == ViewMode::Infra {
        collect_infra()
    } else {
        InfraSnapshot {
            ports: Vec::new(),
            builds: Vec::new(),
            messages: Vec::new(),
            sessions: Vec::new(),
        }
    };

    // Intelligence data
    let intel = if view_mode == ViewMode::Intel {
        collect_intel()
    } else {
        IntelSnapshot {
            kgraph_entities: 0,
            kgraph_edges: 0,
            kgraph_top: Vec::new(),
            facts: Vec::new(),
            fact_count: 0,
            replay_sessions: 0,
            replay_tool_calls: 0,
            replay_errors: 0,
            top_tools: Vec::new(),
        }
    };

    // Pipeline data
    let pipelines = if view_mode == ViewMode::Pipeline {
        collect_pipelines(app)
    } else {
        Vec::new()
    };

    // Audit data — always collect (header badge needs it)
    let audit_data = collect_audit();

    // Started timestamps from state
    let started_at: Vec<(u8, String)> = panes
        .iter()
        .filter(|p| p.status == "active")
        .filter_map(|p| {
            let task = queue::task_for_pane(p.pane);
            task.and_then(|t| t.started_at.map(|s| (p.pane, s)))
        })
        .collect();

    DashboardData {
        panes,
        selected,
        pane_focus,
        acu_used: cap.acu_used,
        acu_total: cap.acu_total,
        reviews_used: cap.reviews_used,
        reviews_total: cap.reviews_total,
        active_count,
        pty_count,
        selected_output,
        selected_screen,
        log_lines,
        queue_lines,
        queue_pending,
        queue_running,
        queue_done,
        queue_failed,
        features,
        view_mode,
        alerts,
        roles,
        board,
        coord,
        started_at,
        projects,
        feature_cursor,
        infra,
        intel,
        audit: audit_data,
        action_log: Vec::new(),
        pipelines,
        signal_count: crate::multi_agent::signal_count_unack(),
        pane_signals: crate::multi_agent::signal_by_pane(),
        screen_count: {
            let mgr = app.screens.read().unwrap();
            mgr.list_screens().len()
        },
        screen_names: {
            let mgr = app.screens.read().unwrap();
            mgr.list_screens().iter().map(|s| s.name.clone()).collect()
        },
        builds: if view_mode == ViewMode::Dashboard {
            crate::build::build_status()
        } else {
            Vec::new()
        },
    }
}

/// Collect kanban board from tracker spaces
fn collect_board() -> Vec<BoardColumn> {
    let statuses = ["backlog", "todo", "in_progress", "review", "done"];
    let display = ["Backlog", "To Do", "In Progress", "Review", "Done"];
    let spaces_dir = config::collab_root().join("spaces");
    if !spaces_dir.exists() {
        return statuses
            .iter()
            .zip(display.iter())
            .map(|(_, d)| BoardColumn {
                name: d.to_string(),
                cards: Vec::new(),
            })
            .collect();
    }

    let mut columns: Vec<(String, Vec<BoardCard>)> = statuses
        .iter()
        .zip(display.iter())
        .map(|(_, d)| (d.to_string(), Vec::new()))
        .collect();

    if let Ok(entries) = std::fs::read_dir(&spaces_dir) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let space = entry.file_name().to_string_lossy().to_string();
            let issues = tracker::load_issues(&space);
            for issue in &issues {
                let status = issue
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("backlog");
                let idx = statuses.iter().position(|s| *s == status);
                if let Some(i) = idx {
                    let id = issue
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let title = issue
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let priority = issue
                        .get("priority")
                        .and_then(|v| v.as_str())
                        .unwrap_or("medium")
                        .to_string();
                    let role = issue
                        .get("role")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    columns[i].1.push(BoardCard {
                        id,
                        title,
                        priority,
                        role,
                    });
                }
            }
        }
    }

    columns
        .into_iter()
        .map(|(name, cards)| BoardColumn { name, cards })
        .collect()
}

/// Collect features from all tracker spaces
fn collect_features(q: &queue::TaskQueue) -> Vec<FeatureSnapshot> {
    let spaces_dir = config::collab_root().join("spaces");
    if !spaces_dir.exists() {
        return Vec::new();
    }

    let mut features = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&spaces_dir) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let space = entry.file_name().to_string_lossy().to_string();
            let issues = tracker::load_issues(&space);

            for issue in &issues {
                let itype = issue.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if itype != "feature" && itype != "epic" {
                    continue;
                }
                let status = issue
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("todo");
                if status == "closed" {
                    continue;
                }

                let feature_id = issue.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let title = issue.get("title").and_then(|v| v.as_str()).unwrap_or("");

                let children: Vec<MicroFeatureSnapshot> = issues
                    .iter()
                    .filter(|i| i.get("parent").and_then(|v| v.as_str()) == Some(feature_id))
                    .map(|child| {
                        let child_id = child.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let child_status = child
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("todo");
                        let child_title = child.get("title").and_then(|v| v.as_str()).unwrap_or("");
                        let qt = q
                            .tasks
                            .iter()
                            .find(|t| t.issue_id.as_deref() == Some(child_id));
                        let queue_status = qt.map(|t| format!("{:?}", t.status));
                        let pane = qt.and_then(|t| t.pane);
                        MicroFeatureSnapshot {
                            id: child_id.to_string(),
                            title: child_title.to_string(),
                            status: child_status.to_string(),
                            queue_status,
                            pane,
                        }
                    })
                    .collect();

                let done = children
                    .iter()
                    .filter(|c| c.status == "done" || c.status == "closed")
                    .count();
                let total = children.len();

                features.push(FeatureSnapshot {
                    id: feature_id.to_string(),
                    title: title.to_string(),
                    status: status.to_string(),
                    space: space.clone(),
                    children,
                    done,
                    total,
                });
            }
        }
    }

    features.sort_by(|a, b| {
        let a_active = if a.status == "in_progress" { 0 } else { 1 };
        let b_active = if b.status == "in_progress" { 0 } else { 1 };
        a_active.cmp(&b_active).then(a.id.cmp(&b.id))
    });

    features
}

/// Collect coordination snapshot (agents, locks, KB, branches, dep graph)
fn collect_coord(_q: &queue::TaskQueue) -> CoordSnapshot {
    // Agents
    let agents_json = multi_agent::agent_list(None);
    let agents: Vec<(String, String, String)> = agents_json
        .get("agents")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    let pane = a.get("pane_id").and_then(|v| v.as_str())?.to_string();
                    let proj = a
                        .get("project")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let task = a
                        .get("task")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Some((pane, proj, task))
                })
                .collect()
        })
        .unwrap_or_default();

    // Locks from overview
    let overview = multi_agent::status_overview(None);
    let lock_count = overview.get("locks").and_then(|v| v.as_i64()).unwrap_or(0);
    let locks: Vec<(String, String)> = if lock_count > 0 {
        // Check all known files for locks
        multi_agent::lock_check(&[])
            .get("locked")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|l| {
                        let pane = l.get("locked_by").and_then(|v| v.as_str())?.to_string();
                        let file = l.get("file").and_then(|v| v.as_str())?.to_string();
                        Some((pane, file))
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // KB recent entries
    let kb_json = multi_agent::kb_list(None, 10);
    let kb_recent: Vec<(String, String, String)> = kb_json
        .get("entries")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    let cat = e
                        .get("category")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let title = e
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let pane = e
                        .get("pane_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Some((cat, title, pane))
                })
                .collect()
        })
        .unwrap_or_default();

    // Git branches
    let branches_json = multi_agent::git_list_branches(None);
    let branches: Vec<(String, String, String)> = branches_json
        .get("branches")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|b| {
                    let pane = b.get("pane_id").and_then(|v| v.as_str())?.to_string();
                    let branch = b
                        .get("branch")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let proj = b
                        .get("repo")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Some((pane, branch, proj))
                })
                .collect()
        })
        .unwrap_or_default();

    // Port allocations
    let ports_json = multi_agent::port_list();
    let ports: Vec<(i64, String, String)> = ports_json
        .get("ports")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|p| {
                    let port = p.get("port").and_then(|v| v.as_i64())?;
                    let svc = p
                        .get("service")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let pane = p
                        .get("pane_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Some((port, svc, pane))
                })
                .collect()
        })
        .unwrap_or_default();

    // Issue claims
    let claims: Vec<(String, u32, String, String)> = crate::claims::list(None, true)
        .unwrap_or_default()
        .into_iter()
        .map(|c| (c.repo, c.issue, c.agent_id, c.claimed_at))
        .collect();

    CoordSnapshot {
        agents,
        locks,
        kb_recent,
        branches,
        ports,
        claims,
    }
}

/// Collect project snapshots from scanner registry + quality data
fn collect_projects() -> Vec<ProjectSnapshot> {
    let reg = scanner::load_registry();
    let mut snapshots = Vec::new();

    for proj in &reg.projects {
        let health = quality::project_health(&proj.name);
        let gate = quality::quality_gate(&proj.name);

        let grade = health
            .get("grade")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string();
        let score = health
            .get("health_score")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let last_test = gate.get("tests").and_then(|v| {
            let pass = v.get("pass").and_then(|p| p.as_bool())?;
            let ts = v.get("last_run").and_then(|t| t.as_str())?;
            Some((pass, format_relative_time(ts)))
        });

        let last_build = gate.get("build").and_then(|v| {
            let pass = v.get("pass").and_then(|p| p.as_bool())?;
            let ts = v.get("last_run").and_then(|t| t.as_str())?;
            Some((pass, format_relative_time(ts)))
        });

        // Count open issues
        let issues = tracker::load_issues(&proj.name);
        let open_issues = issues
            .iter()
            .filter(|i| {
                let s = i.get("status").and_then(|v| v.as_str()).unwrap_or("");
                s != "done" && s != "closed"
            })
            .count();

        // Count active agents
        let agents = multi_agent::agent_list(Some(&proj.name));
        let active_agents = agents.get("count").and_then(|v| v.as_i64()).unwrap_or(0) as usize;

        let last_commit = proj
            .last_commit_ts
            .as_ref()
            .map(|ts| format_relative_time(ts));

        snapshots.push(ProjectSnapshot {
            name: proj.name.clone(),
            tech: proj.tech.join(", "),
            health_grade: grade,
            health_score: score,
            last_test,
            last_build,
            open_issues,
            active_agents,
            git_dirty: proj.git_dirty,
            git_ahead: proj.git_ahead,
            git_behind: proj.git_behind,
            last_commit,
            readme: proj.readme_summary.clone(),
        });
    }

    // Sort: highest health score first, then alphabetically
    snapshots.sort_by(|a, b| {
        b.health_score
            .cmp(&a.health_score)
            .then(a.name.cmp(&b.name))
    });
    snapshots
}

/// Collect infrastructure snapshot: ports, builds, messages, sessions
fn collect_infra() -> InfraSnapshot {
    // Ports
    let ports_json = multi_agent::port_list();
    let ports: Vec<(i64, String, String)> = ports_json
        .get("ports")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|p| {
                    let port = p.get("port").and_then(|v| v.as_i64())?;
                    let svc = p
                        .get("service")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let pane = p
                        .get("pane_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Some((port, svc, pane))
                })
                .collect()
        })
        .unwrap_or_default();

    // Builds — get overview which includes build counts
    let overview = multi_agent::status_overview(None);
    let builds: Vec<(String, bool, String)> = overview
        .get("builds")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(proj, info)| {
                    let success = info
                        .get("last_success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let ts = info.get("last_at").and_then(|v| v.as_str()).unwrap_or("");
                    let ago = if ts.is_empty() {
                        "never".to_string()
                    } else {
                        format_relative_time(ts)
                    };
                    Some((proj.clone(), success, ago))
                })
                .collect()
        })
        .unwrap_or_default();

    // Messages — get for all panes
    let msg_json = multi_agent::msg_get("*", false);
    let messages: Vec<(String, String, String, String)> = msg_json
        .get("messages")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .rev()
                .take(10)
                .filter_map(|m| {
                    let from = m
                        .get("from_pane")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?")
                        .to_string();
                    let to = m
                        .get("to_pane")
                        .and_then(|v| v.as_str())
                        .unwrap_or("*")
                        .to_string();
                    let msg = m
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let pri = m
                        .get("priority")
                        .and_then(|v| v.as_str())
                        .unwrap_or("info")
                        .to_string();
                    Some((from, to, msg, pri))
                })
                .collect()
        })
        .unwrap_or_default();

    // Sessions from who()
    let who = multi_agent::who();
    let sessions: Vec<(String, String, String)> = who
        .get("agents")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    let pane = a.get("pane_id").and_then(|v| v.as_str())?.to_string();
                    let proj = a
                        .get("project")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let status = a
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("active")
                        .to_string();
                    Some((pane, proj, status))
                })
                .collect()
        })
        .unwrap_or_default();

    InfraSnapshot {
        ports,
        builds,
        messages,
        sessions,
    }
}

/// Collect intelligence snapshot: kgraph, facts, replay, analytics
fn collect_intel() -> IntelSnapshot {
    use crate::knowledge;

    // Knowledge graph stats
    let stats = knowledge::kgraph_stats();
    let kgraph_entities = stats
        .get("entity_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let kgraph_edges = stats
        .get("edge_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let kgraph_top: Vec<(String, i64)> = stats
        .get("top_entities")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .take(8)
                .filter_map(|e| {
                    let name = e.get("name").and_then(|v| v.as_str())?.to_string();
                    let count = e.get("edge_count").and_then(|v| v.as_i64()).unwrap_or(0);
                    Some((name, count))
                })
                .collect()
        })
        .unwrap_or_default();

    // Facts
    let facts_json = knowledge::fact_search("", "", 0.0, 20);
    let fact_count = facts_json
        .get("count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let facts: Vec<(String, String, bool)> = facts_json
        .get("facts")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .take(10)
                .filter_map(|f| {
                    let key = f
                        .get("key")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let val = f
                        .get("value")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let verified = f.get("verified").and_then(|v| v.as_bool()).unwrap_or(false);
                    Some((key, val, verified))
                })
                .collect()
        })
        .unwrap_or_default();

    // Replay status
    let replay = knowledge::replay_status();
    let replay_sessions = replay
        .get("total_sessions")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let replay_tool_calls = replay
        .get("total_tool_calls")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let replay_errors = replay
        .get("total_errors")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    // Top tools from analytics (use tool ranking if available)
    let top_tools: Vec<(String, f64)> = Vec::new(); // Populated from MEMORY.md or analytics

    IntelSnapshot {
        kgraph_entities,
        kgraph_edges,
        kgraph_top,
        facts,
        fact_count,
        replay_sessions,
        replay_tool_calls,
        replay_errors,
        top_tools,
    }
}

/// Collect audit snapshots from stored results
fn collect_audit() -> AuditSnapshot {
    let project_names = audit::list_audited_projects();
    let mut projects = Vec::new();
    let mut total_critical = 0usize;
    let mut total_high = 0usize;
    let mut worst_grade = "A".to_string();

    let grade_order = |g: &str| -> u8 {
        match g {
            "F" => 0,
            "D" => 1,
            "C" => 2,
            "B" => 3,
            "A" => 4,
            _ => 5,
        }
    };

    for name in &project_names {
        if let Some(report) = audit::load_latest_audit(name) {
            let grade = report
                .get("grade")
                .and_then(|v| v.as_str())
                .unwrap_or("?")
                .to_string();
            let empty_obj = serde_json::json!({});
            let by_sev = report.get("by_severity").unwrap_or(&empty_obj);
            let critical = by_sev.get("critical").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let high = by_sev.get("high").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let medium = by_sev.get("medium").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let low = by_sev.get("low").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let info = by_sev.get("info").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let total = report
                .get("total_findings")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;

            total_critical += critical;
            total_high += high;
            if grade_order(&grade) < grade_order(&worst_grade) {
                worst_grade = grade.clone();
            }

            // Extract top findings (critical/high/medium only, max 5)
            let mut top_findings: Vec<(String, String, String, usize)> = Vec::new();
            for audit_type in &["code", "security", "intent", "deps"] {
                if let Some(sub) = report.get(*audit_type) {
                    if let Some(findings) = sub.get("findings").and_then(|f| f.as_array()) {
                        for f in findings {
                            let sev = f.get("severity").and_then(|v| v.as_str()).unwrap_or("info");
                            if sev == "critical" || sev == "high" || sev == "medium" {
                                top_findings.push((
                                    sev.to_string(),
                                    f.get("category")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                    f.get("file")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                    f.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                                ));
                            }
                        }
                    }
                }
            }
            top_findings.sort_by_key(|(sev, _, _, _)| match sev.as_str() {
                "critical" => 0,
                "high" => 1,
                "medium" => 2,
                _ => 3,
            });
            top_findings.truncate(5);

            // Last audit time from file modification
            let audit_path = config::dx_root()
                .join("audits")
                .join(name)
                .join("latest.json");
            let last_audit = std::fs::metadata(&audit_path)
                .and_then(|m| m.modified())
                .ok()
                .map(|t| {
                    let dt: chrono::DateTime<chrono::Utc> = t.into();
                    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
                })
                .map(|ts| format_relative_time(&ts))
                .unwrap_or_else(|| "?".to_string());

            projects.push(AuditProjectSummary {
                name: name.clone(),
                grade,
                critical,
                high,
                medium,
                low,
                info,
                total,
                last_audit,
                top_findings,
            });
        }
    }

    projects.sort_by(|a, b| {
        grade_order(&a.grade)
            .cmp(&grade_order(&b.grade))
            .then(b.total.cmp(&a.total))
    });

    AuditSnapshot {
        projects,
        total_critical,
        total_high,
        worst_grade,
    }
}

fn collect_pipelines(app: &App) -> Vec<PipelineSnapshot> {
    use crate::factory;
    let state = app.state.blocking_read();
    let pane_sigs = crate::multi_agent::signal_by_pane();
    factory::list_pipelines()
        .into_iter()
        .map(|p| {
            let gate_passed = factory::get_gate_result(&p.id).map(|g| g.passed);
            let paused = factory::is_pipeline_paused(&p.id);
            // Count signals for panes in this pipeline
            let sig_count: usize = p
                .stages
                .iter()
                .filter_map(|s| s.pane)
                .map(|pn| pane_sigs.get(&pn).map_or(0, |v| v.len()))
                .sum();
            PipelineSnapshot {
                id: p.id,
                project: p.project,
                description: p.description,
                template: p.template,
                status: if paused { "paused".into() } else { p.status },
                paused,
                stages: p
                    .stages
                    .into_iter()
                    .map(|s| {
                        let snippet = if s.status == "running" {
                            s.pane.and_then(|pn| {
                                // Tmux-first: get output from tmux target
                                let pd = state.panes.get(&pn.to_string());
                                if let Some(target) = pd.and_then(|p| p.tmux_target.as_deref()) {
                                    let output = crate::tmux::capture_output(target);
                                    let lines: Vec<&str> = output.lines().collect();
                                    let tail: String = lines
                                        .iter()
                                        .rev()
                                        .take(2)
                                        .rev()
                                        .copied()
                                        .collect::<Vec<&str>>()
                                        .join("\n");
                                    if tail.is_empty() {
                                        None
                                    } else {
                                        Some(tail)
                                    }
                                } else {
                                    // PTY fallback
                                    let pty = app.pty_lock();
                                    pty.last_output(pn, 2)
                                }
                            })
                        } else {
                            None
                        };
                        PipelineStageSnapshot {
                            name: s.name,
                            role: s.role,
                            status: s.status,
                            pane: s.pane,
                            pty_snippet: snippet,
                        }
                    })
                    .collect(),
                gate_passed,
                signal_count: sig_count,
            }
        })
        .collect()
}

/// Format ISO timestamp to relative time ("3m ago", "2h ago", "1d ago")
fn format_relative_time(ts: &str) -> String {
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S%.fZ")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%SZ"))
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S"))
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S%z"))
    {
        let now = chrono::Utc::now().naive_utc();
        let elapsed = now.signed_duration_since(dt);
        let mins = elapsed.num_minutes();
        if mins < 1 {
            return "<1m".to_string();
        }
        if mins < 60 {
            return format!("{}m", mins);
        }
        let hours = mins / 60;
        if hours < 24 {
            return format!("{}h", hours);
        }
        return format!("{}d", hours / 24);
    }
    ts.get(..16).unwrap_or(ts).to_string()
}

// ========== RENDERING ==========

pub fn render(f: &mut Frame, data: &DashboardData) {
    let pane_table_height = data.panes.len() as u16 + 3;
    let alert_height = if data.alerts.is_empty() { 0 } else { 3 };

    match data.view_mode {
        ViewMode::Board => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),            // Header
                    Constraint::Length(alert_height), // Alerts
                    Constraint::Min(12),              // Board
                    Constraint::Length(8),            // Queue + Activity
                    Constraint::Length(1),            // Help
                ])
                .split(f.area());

            let bottom = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(chunks[3]);

            render_header(f, chunks[0], data);
            if alert_height > 0 {
                render_alert_bar(f, chunks[1], data);
            }
            render_board(f, chunks[2], data);
            render_queue(f, bottom[0], data);
            render_activity_log(f, bottom[1], data);
            render_help_bar(f, chunks[4], data);
        }
        ViewMode::Features => {
            let feature_height = (data
                .features
                .iter()
                .map(|ft| 1 + ft.children.len())
                .sum::<usize>() as u16)
                .max(3)
                .min(14)
                + 2;

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(alert_height),
                    Constraint::Length(pane_table_height),
                    Constraint::Length(feature_height),
                    Constraint::Min(6),
                    Constraint::Length(6),
                    Constraint::Length(1),
                ])
                .split(f.area());

            render_header(f, chunks[0], data);
            if alert_height > 0 {
                render_alert_bar(f, chunks[1], data);
            }
            render_pane_table(f, chunks[2], data);
            render_features(f, chunks[3], data);
            render_pty_output(f, chunks[4], data);
            render_queue(f, chunks[5], data);
            render_help_bar(f, chunks[6], data);
        }
        ViewMode::Coord => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),                 // Header
                    Constraint::Length(alert_height),      // Alerts
                    Constraint::Length(pane_table_height), // Pane table
                    Constraint::Min(10),                   // Coordination panels (split)
                    Constraint::Length(1),                 // Help
                ])
                .split(f.area());

            let coord_panels = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(35), // Agents + Locks
                    Constraint::Percentage(35), // KB + Branches
                    Constraint::Percentage(30), // Dep Graph + Queue summary
                ])
                .split(chunks[3]);

            let left_split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(coord_panels[0]);

            let mid_split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(coord_panels[1]);

            let right_split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(35),
                    Constraint::Percentage(35),
                    Constraint::Percentage(30),
                ])
                .split(coord_panels[2]);

            render_header(f, chunks[0], data);
            if alert_height > 0 {
                render_alert_bar(f, chunks[1], data);
            }
            render_pane_table(f, chunks[2], data);
            render_coord_agents(f, left_split[0], data);
            render_coord_locks(f, left_split[1], data);
            render_coord_kb(f, mid_split[0], data);
            render_coord_branches(f, mid_split[1], data);
            render_coord_ports(f, right_split[0], data);
            render_coord_claims(f, right_split[1], data);
            render_queue(f, right_split[2], data);
            render_help_bar(f, chunks[4], data);
        }
        ViewMode::Projects => {
            let project_height = (data.projects.len() as u16 + 3).max(5).min(25);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),            // Header
                    Constraint::Length(alert_height), // Alerts
                    Constraint::Min(project_height),  // Projects table
                    Constraint::Length(8),            // Queue
                    Constraint::Length(1),            // Help
                ])
                .split(f.area());

            render_header(f, chunks[0], data);
            if alert_height > 0 {
                render_alert_bar(f, chunks[1], data);
            }
            render_projects(f, chunks[2], data);
            render_queue(f, chunks[3], data);
            render_help_bar(f, chunks[4], data);
        }
        ViewMode::Infra => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(alert_height),
                    Constraint::Min(10),
                    Constraint::Length(1),
                ])
                .split(f.area());

            let panels = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(33),
                    Constraint::Percentage(34),
                    Constraint::Percentage(33),
                ])
                .split(chunks[2]);

            let left = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(panels[0]);

            let right = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(panels[2]);

            render_header(f, chunks[0], data);
            if alert_height > 0 {
                render_alert_bar(f, chunks[1], data);
            }
            render_infra_ports(f, left[0], data);
            render_infra_sessions(f, left[1], data);
            render_infra_builds(f, panels[1], data);
            render_infra_messages(f, right[0], data);
            render_queue(f, right[1], data);
            render_help_bar(f, chunks[3], data);
        }
        ViewMode::Intel => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(alert_height),
                    Constraint::Min(10),
                    Constraint::Length(1),
                ])
                .split(f.area());

            let panels = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(33),
                    Constraint::Percentage(34),
                    Constraint::Percentage(33),
                ])
                .split(chunks[2]);

            let left = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(panels[0]);

            let right = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(panels[2]);

            render_header(f, chunks[0], data);
            if alert_height > 0 {
                render_alert_bar(f, chunks[1], data);
            }
            render_intel_kgraph(f, left[0], data);
            render_intel_replay(f, left[1], data);
            render_intel_facts(f, panels[1], data);
            render_intel_analytics(f, right[0], data);
            render_queue(f, right[1], data);
            render_help_bar(f, chunks[3], data);
        }
        ViewMode::Audit => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(alert_height),
                    Constraint::Min(10),
                    Constraint::Length(8),
                    Constraint::Length(1),
                ])
                .split(f.area());

            let panels = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(chunks[2]);

            render_header(f, chunks[0], data);
            if alert_height > 0 {
                render_alert_bar(f, chunks[1], data);
            }
            render_audit_summary(f, panels[0], data);
            render_audit_findings(f, panels[1], data);
            render_queue(f, chunks[3], data);
            render_help_bar(f, chunks[4], data);
        }
        ViewMode::Log => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(alert_height),
                    Constraint::Min(10),
                    Constraint::Length(1),
                ])
                .split(f.area());

            render_header(f, chunks[0], data);
            if alert_height > 0 {
                render_alert_bar(f, chunks[1], data);
            }
            render_action_log_view(f, chunks[2], data);
            render_help_bar(f, chunks[3], data);
        }
        ViewMode::Pipeline => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(alert_height),
                    Constraint::Min(10),
                    Constraint::Length(8),
                    Constraint::Length(1),
                ])
                .split(f.area());

            render_header(f, chunks[0], data);
            if alert_height > 0 {
                render_alert_bar(f, chunks[1], data);
            }
            render_pipeline_view(f, chunks[2], data);
            render_queue(f, chunks[3], data);
            render_help_bar(f, chunks[4], data);
        }
        ViewMode::Dashboard => {
            let build_height = if data.builds.is_empty() {
                3
            } else {
                (data.builds.len() as u16 * 3 + 3).min(18)
            };
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),                 // Header
                    Constraint::Length(alert_height),      // Alerts
                    Constraint::Length(5),                 // Gauges row
                    Constraint::Length(pane_table_height), // Pane table
                    Constraint::Length(build_height),      // Build environments
                    Constraint::Min(6),                    // Queue + Roles
                    Constraint::Length(1),                 // Help
                ])
                .split(f.area());

            let bottom = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(chunks[5]);

            render_header(f, chunks[0], data);
            if alert_height > 0 {
                render_alert_bar(f, chunks[1], data);
            }
            render_gauges_row(f, chunks[2], data);
            render_pane_table(f, chunks[3], data);
            render_builds_panel(f, chunks[4], data);
            render_queue(f, bottom[0], data);
            render_activity_log(f, bottom[1], data);
            render_help_bar(f, chunks[6], data);
        }
        ViewMode::Normal => {
            if data.pane_focus {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),            // Header
                        Constraint::Length(alert_height), // Alerts (conditional)
                        Constraint::Min(12),              // Focused terminal
                        Constraint::Length(1),            // Help
                    ])
                    .split(f.area());

                render_header(f, chunks[0], data);
                if alert_height > 0 {
                    render_alert_bar(f, chunks[1], data);
                }
                render_pty_output(f, chunks[2], data);
                render_help_bar(f, chunks[3], data);
            } else {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),            // Header
                        Constraint::Length(alert_height), // Alerts (conditional)
                        Constraint::Min(10),              // Live pane grid
                        Constraint::Length(4),            // Selected pane status
                        Constraint::Length(1),            // Help
                    ])
                    .split(f.area());

                render_header(f, chunks[0], data);
                if alert_height > 0 {
                    render_alert_bar(f, chunks[1], data);
                }
                render_multiplexer_grid(f, chunks[2], data);
                render_selected_pane_status(f, chunks[3], data);
                render_help_bar(f, chunks[4], data);
            }
        }
    }
}

fn render_header(f: &mut Frame, area: Rect, data: &DashboardData) {
    let (acu_bar, acu_color) = widgets::gauge_bar(data.acu_used, data.acu_total, 8);
    let status_label = if !data.alerts.is_empty() {
        Span::styled(
            " ALERT ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        )
    } else if data.active_count > 0 {
        Span::styled(
            " LIVE ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            " IDLE ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
    };

    let view_label = match data.view_mode {
        ViewMode::Normal => Span::raw(""),
        ViewMode::Dashboard => Span::styled(
            " DASH ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        ViewMode::Features => Span::styled(
            " FEAT ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        ViewMode::Board => Span::styled(
            " BOARD ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        ViewMode::Coord => Span::styled(
            " COORD ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        ViewMode::Projects => Span::styled(
            " PROJ ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        ViewMode::Infra => Span::styled(
            " INFRA ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        ViewMode::Intel => Span::styled(
            " INTEL ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ),
        ViewMode::Audit => Span::styled(
            " AUDIT ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        ViewMode::Log => Span::styled(
            " LOG ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        ViewMode::Pipeline => Span::styled(
            " PIPE ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
        ),
    };

    let header = Line::from(vec![
        Span::styled(
            " DX Terminal ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        status_label,
        view_label,
        Span::styled(" │ ACU ", Style::default().fg(Color::DarkGray)),
        Span::styled(acu_bar, Style::default().fg(acu_color)),
        Span::styled(
            format!(" {:.1}/{:.0}", data.acu_used, data.acu_total),
            Style::default().fg(acu_color),
        ),
        Span::styled(" │ Rev ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}/{}", data.reviews_used, data.reviews_total),
            Style::default().fg(Color::White),
        ),
        Span::styled(" │ Agents ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}/{}", data.active_count, data.panes.len()),
            Style::default().fg(if data.active_count > 0 {
                Color::Green
            } else {
                Color::DarkGray
            }),
        ),
        Span::styled(
            format!(" ({}▶)", data.pty_count),
            Style::default().fg(Color::Green),
        ),
        Span::styled(" │ Q ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}p", data.queue_pending),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled("·", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}r", data.queue_running),
            Style::default().fg(Color::Green),
        ),
        Span::styled("·", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}d", data.queue_done),
            Style::default().fg(Color::Blue),
        ),
        if data.queue_failed > 0 {
            Span::styled(
                format!("·{}f", data.queue_failed),
                Style::default().fg(Color::Red),
            )
        } else {
            Span::raw("")
        },
        if data.signal_count > 0 {
            Span::styled(
                format!(" [!{}]", data.signal_count),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::raw("")
        },
        if !data.audit.projects.is_empty() {
            let gc = match data.audit.worst_grade.as_str() {
                "A" => Color::Green,
                "B" => Color::Cyan,
                "C" => Color::Yellow,
                "D" | "F" => Color::Red,
                _ => Color::DarkGray,
            };
            Span::styled(
                format!(" │ Aud {}", data.audit.worst_grade),
                Style::default().fg(gc).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::raw("")
        },
        Span::styled(" │ Screens ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}", data.screen_count),
            Style::default().fg(Color::Cyan),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let p = Paragraph::new(header).block(block);
    f.render_widget(p, area);
}

fn render_alert_bar(f: &mut Frame, area: Rect, data: &DashboardData) {
    let spans: Vec<Span> = data
        .alerts
        .iter()
        .take(4)
        .flat_map(|(pane, msg)| {
            vec![
                Span::styled(
                    " ⚠ ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("P{}: ", pane),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    widgets::truncate_pub(msg, 25),
                    Style::default().fg(Color::Red),
                ),
                Span::styled(" │", Style::default().fg(Color::DarkGray)),
            ]
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    let p = Paragraph::new(Line::from(spans)).block(block);
    f.render_widget(p, area);
}

fn render_pane_table(f: &mut Frame, area: Rect, data: &DashboardData) {
    let mut lines = vec![Line::from(vec![
        Span::styled("  # ", Style::default().fg(Color::DarkGray)),
        Span::styled("Theme   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Project     ", Style::default().fg(Color::DarkGray)),
        Span::styled("Role ", Style::default().fg(Color::DarkGray)),
        Span::styled("Status  ", Style::default().fg(Color::DarkGray)),
        Span::styled("▶ ", Style::default().fg(Color::DarkGray)),
        Span::styled("H   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Time   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Branch/Task", Style::default().fg(Color::DarkGray)),
    ])];

    for ps in &data.panes {
        let mut line = widgets::pane_line(
            ps.pane,
            &ps.theme_fg,
            &ps.theme,
            &ps.project,
            &ps.role,
            &ps.task,
            &ps.status,
            ps.branch.as_deref(),
            ps.pty_running,
            ps.pane == data.selected,
            &ps.health,
            &ps.runtime,
        );
        // Append signal badge if this pane has unack'd signals
        if let Some(signals) = data.pane_signals.get(&ps.pane) {
            if let Some((sig_type, _msg)) = signals.first() {
                let (icon, color) = match sig_type.as_str() {
                    "need_help" | "failed" => ("!", Color::Red),
                    "blocked" => ("~", Color::Yellow),
                    "found_issue" => ("?", Color::Magenta),
                    _ => ("*", Color::Cyan),
                };
                let badge = if signals.len() > 1 {
                    format!(" [{}{}]", icon, signals.len())
                } else {
                    format!(" [{}]", icon)
                };
                line.spans.push(Span::styled(
                    badge,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ));
            }
        }
        lines.push(line);
    }

    let block = Block::default()
        .title(" Panes ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn render_pty_output(f: &mut Frame, area: Rect, data: &DashboardData) {
    let Some(sel) = data
        .panes
        .iter()
        .find(|pane| pane.pane == data.selected)
        .or_else(|| data.panes.first())
    else {
        let block = Block::default()
            .title(" Pane Output ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        let paragraph = Paragraph::new(Line::from(Span::styled(
            "  No live pane selected",
            Style::default().fg(Color::DarkGray),
        )))
        .block(block);
        f.render_widget(paragraph, area);
        return;
    };
    let branch_display = sel.branch.as_deref().unwrap_or("");
    let project_name = if sel.project.is_empty() || sel.project == "--" {
        "idle"
    } else {
        sel.project.as_str()
    };
    let agent_type = if sel.role.trim().is_empty() {
        crate::tmux::provider_short(&sel.provider).to_ascii_uppercase()
    } else {
        sel.role.to_ascii_uppercase()
    };
    let state = if sel.health.is_empty() {
        sel.status.clone()
    } else {
        format!("{} {}", sel.status, sel.health)
    };
    let title = if !branch_display.is_empty() {
        format!(
            " P{} {} {} {} [{}] ",
            sel.pane,
            widgets::truncate_pub(project_name, 20),
            widgets::truncate_pub(&agent_type, 10),
            widgets::truncate_pub(&state, 14),
            branch_display
        )
    } else {
        format!(
            " P{} {} {} {} ",
            sel.pane,
            widgets::truncate_pub(project_name, 22),
            widgets::truncate_pub(&agent_type, 10),
            widgets::truncate_pub(&state, 14)
        )
    };

    let tc = widgets::theme_color(&sel.theme_fg);

    let output = if !data.selected_screen.trim().is_empty() {
        &data.selected_screen
    } else if !data.selected_output.trim().is_empty() {
        &data.selected_output
    } else {
        "[No output — agent not running or no data yet]"
    };

    let available_height = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = output
        .lines()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .take(available_height)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect();

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(tc));

    let p = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn render_multiplexer_grid(f: &mut Frame, area: Rect, data: &DashboardData) {
    if data.panes.is_empty() {
        let block = Block::default()
            .title(" Agent Panes ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        let paragraph = Paragraph::new(vec![
            Line::from(Span::styled(
                "  No live tmux or PTY agent panes detected.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "  Spawn with [s] or let the TUI auto-discover running claude/codex/node panes.",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .block(block);
        f.render_widget(paragraph, area);
        return;
    }

    let pane_count = data.panes.len();
    let width_limited_cols = usize::from((area.width / 32).max(1));
    let desired_cols = match pane_count {
        0 | 1 => 1,
        2..=4 => 2,
        5..=9 => 3,
        _ => 4,
    };
    let cols = desired_cols.min(width_limited_cols).max(1);
    let rows = pane_count.div_ceil(cols);

    let row_constraints = vec![Constraint::Ratio(1, rows as u32); rows];
    let row_areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_constraints)
        .split(area);

    for row_idx in 0..rows {
        let start = row_idx * cols;
        let end = (start + cols).min(pane_count);
        let row_panes = &data.panes[start..end];
        let col_constraints = vec![Constraint::Ratio(1, row_panes.len() as u32); row_panes.len()];
        let col_areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(col_constraints)
            .split(row_areas[row_idx]);

        for (pane, pane_area) in row_panes.iter().zip(col_areas.iter()) {
            render_mux_pane(f, *pane_area, pane, pane.pane == data.selected);
        }
    }
}

/// Map agent status + health to a color for live output streaming.
fn status_color(status: &str, health: &str) -> Color {
    match (status, health) {
        (_, "error") | (_, "dead") => Color::Red,
        ("active", "ok" | "") => Color::Green,
        ("active", "stuck") => Color::Yellow,
        ("idle", _) | ("", _) => Color::Yellow,
        ("done", _) => Color::Cyan,
        _ => Color::DarkGray,
    }
}

fn render_mux_pane(f: &mut Frame, area: Rect, pane: &PaneSnapshot, selected: bool) {
    let agent_color = status_color(&pane.status, &pane.health);
    let border_color = if selected {
        widgets::theme_color(&pane.theme_fg)
    } else {
        agent_color
    };
    let provider = crate::tmux::provider_short(&pane.provider);
    let target = pane
        .tmux_target
        .as_deref()
        .map(|target| widgets::truncate_pub(target, 18))
        .unwrap_or_else(|| format!("pty:{}", pane.pane));
    let state = if pane.health.is_empty() {
        pane.status.clone()
    } else {
        format!("{} {}", pane.status, pane.health)
    };
    let project_name = if pane.project.is_empty() || pane.project == "--" {
        "idle"
    } else {
        pane.project.as_str()
    };
    let agent_type = if pane.role.trim().is_empty() {
        provider.to_ascii_uppercase()
    } else {
        pane.role.to_ascii_uppercase()
    };
    let title = format!(
        " P{} {} {} {} ",
        pane.pane,
        widgets::truncate_pub(project_name, 14),
        widgets::truncate_pub(&agent_type, 10),
        widgets::truncate_pub(&state, 12)
    );

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color).add_modifier(if selected {
            Modifier::BOLD
        } else {
            Modifier::empty()
        }));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    let lines_capacity = inner.height as usize;
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(
            widgets::truncate_pub(&provider.to_ascii_uppercase(), 8),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default().fg(Color::DarkGray)),
        Span::styled(target, Style::default().fg(Color::DarkGray)),
        if !pane.runtime.is_empty() {
            Span::styled(
                format!("  {}", pane.runtime),
                Style::default().fg(Color::Yellow),
            )
        } else {
            Span::raw("")
        },
    ]));

    let body_capacity = lines_capacity.saturating_sub(1);
    // Show last 5 lines of live output, color-coded by agent status
    let preview_lines = body_capacity.min(LIVE_PREVIEW_LINES).max(1);
    let output_color = status_color(&pane.status, &pane.health);
    let body_lines: Vec<Line> = if pane.output.trim().is_empty() {
        vec![Line::from(Span::styled(
            "waiting for terminal output",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        pane.output
            .lines()
            .rev()
            .take(preview_lines)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|line| {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(output_color),
                ))
            })
            .collect()
    };
    lines.extend(body_lines);

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(paragraph, inner);
}

fn render_selected_pane_status(f: &mut Frame, area: Rect, data: &DashboardData) {
    let Some(selected) = data.panes.iter().find(|pane| pane.pane == data.selected) else {
        let block = Block::default()
            .title(" Selected ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        let paragraph = Paragraph::new(Line::from(Span::styled(
            " No pane selected",
            Style::default().fg(Color::DarkGray),
        )))
        .block(block);
        f.render_widget(paragraph, area);
        return;
    };

    let block = Block::default()
        .title(" Selected ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(widgets::theme_color(&selected.theme_fg)));
    let project_name = if selected.project.is_empty() || selected.project == "--" {
        "idle"
    } else {
        selected.project.as_str()
    };
    let state = if selected.health.is_empty() {
        selected.status.clone()
    } else {
        format!("{} {}", selected.status, selected.health)
    };
    let details = vec![
        Line::from(vec![
            Span::styled(
                format!(" P{} ", selected.pane),
                Style::default()
                    .fg(Color::Black)
                    .bg(widgets::theme_color(&selected.theme_fg))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {} ", widgets::truncate_pub(project_name, 22)),
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!(
                    "{} ",
                    widgets::truncate_pub(
                        if selected.role.trim().is_empty() {
                            &selected.provider
                        } else {
                            &selected.role
                        },
                        10
                    )
                ),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!("{} ", state),
                Style::default().fg(widgets::status_color(&selected.status)),
            ),
            if !selected.runtime.is_empty() {
                Span::styled(
                    format!("{} ", selected.runtime),
                    Style::default().fg(Color::Yellow),
                )
            } else {
                Span::raw("")
            },
            Span::styled(
                format!("lines:{} ", selected.line_count),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                if selected.state_backed {
                    "managed"
                } else {
                    "discovered"
                },
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::styled(" target ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                selected
                    .tmux_target
                    .clone()
                    .unwrap_or_else(|| format!("pty:{}", selected.pane)),
                Style::default().fg(Color::White),
            ),
        ]),
    ];
    let paragraph = Paragraph::new(details).block(block);
    f.render_widget(paragraph, area);
}

#[allow(dead_code)]
fn render_feature_summary(f: &mut Frame, area: Rect, data: &DashboardData) {
    let available = area.height.saturating_sub(2) as usize;

    let active_features: Vec<&FeatureSnapshot> = data
        .features
        .iter()
        .filter(|f| f.status != "done" && f.status != "closed")
        .collect();

    let lines: Vec<Line> = if active_features.is_empty() {
        vec![Line::from(Span::styled(
            "  No active features",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        active_features
            .iter()
            .take(available)
            .map(|feat| {
                let pct = if feat.total > 0 {
                    feat.done * 100 / feat.total
                } else {
                    0
                };
                let bar = progress_bar(feat.done, feat.total, 8);
                let pct_color = if pct == 100 {
                    Color::Green
                } else if pct > 50 {
                    Color::Yellow
                } else {
                    Color::White
                };
                Line::from(vec![
                    Span::styled(
                        format!(" {:<6}", widgets::truncate_pub(&feat.id, 6)),
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!("{} ", bar), Style::default().fg(pct_color)),
                    Span::styled(format!("{:>3}% ", pct), Style::default().fg(pct_color)),
                    Span::styled(
                        widgets::truncate_pub(&feat.title, 18),
                        Style::default().fg(Color::White),
                    ),
                ])
            })
            .collect()
    };

    let title = format!(" Features ({} active) ", active_features.len());
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn render_queue(f: &mut Frame, area: Rect, data: &DashboardData) {
    let title = format!(
        " Queue ({}p {}r {}d{})",
        data.queue_pending,
        data.queue_running,
        data.queue_done,
        if data.queue_failed > 0 {
            format!(" {}f", data.queue_failed)
        } else {
            String::new()
        },
    );
    let available = area.height.saturating_sub(2) as usize;

    let lines: Vec<Line> = if data.queue_lines.is_empty() {
        vec![Line::from(Span::styled(
            "  No queued tasks",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        data.queue_lines
            .iter()
            .take(available)
            .map(|(status, pri, proj, task, id, issue_id)| {
                let sc = match status.trim() {
                    "RUN" => Color::Green,
                    "PEND" => Color::Yellow,
                    "FAIL" => Color::Red,
                    "BLOK" => Color::Magenta,
                    "DONE" => Color::Blue,
                    _ => Color::DarkGray,
                };
                let issue_tag = match issue_id {
                    Some(iid) => format!(" [{}]", iid),
                    None => String::new(),
                };
                Line::from(vec![
                    Span::styled(
                        format!(" {} ", status),
                        Style::default().fg(sc).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!("{} ", pri), Style::default().fg(Color::Cyan)),
                    Span::styled(
                        format!("{:<8}", widgets::truncate_pub(id, 8)),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!("{:<10}", widgets::truncate_pub(proj, 10)),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(
                        widgets::truncate_pub(task, 22),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(issue_tag, Style::default().fg(Color::Magenta)),
                ])
            })
            .collect()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn render_activity_log(f: &mut Frame, area: Rect, data: &DashboardData) {
    let available = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = data
        .log_lines
        .iter()
        .take(available)
        .map(|l| {
            let color = if l.contains("Spawned") {
                Color::Green
            } else if l.contains("Killed") || l.contains("Error") {
                Color::Red
            } else if l.contains("Done") || l.contains("Complete") {
                Color::Blue
            } else if l.contains("Assigned") || l.contains("Started") {
                Color::Cyan
            } else {
                Color::DarkGray
            };
            Line::from(Span::styled(
                l.as_str().to_string(),
                Style::default().fg(color),
            ))
        })
        .collect();

    let block = Block::default()
        .title(" Activity ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn render_board(f: &mut Frame, area: Rect, data: &DashboardData) {
    if data.board.is_empty() {
        let block = Block::default()
            .title(" Board ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta));
        let p = Paragraph::new(Line::from(Span::styled(
            "  No issues. Create with os_issue_create.",
            Style::default().fg(Color::DarkGray),
        )))
        .block(block);
        f.render_widget(p, area);
        return;
    }

    let col_count = data.board.len() as u32;
    let constraints: Vec<Constraint> = (0..col_count)
        .map(|_| Constraint::Ratio(1, col_count))
        .collect();

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    for (i, col) in data.board.iter().enumerate() {
        if i >= cols.len() {
            break;
        }
        let available = cols[i].height.saturating_sub(2) as usize;
        let count = col.cards.len();

        let title_color = match col.name.as_str() {
            "In Progress" => Color::Green,
            "Review" => Color::Yellow,
            "Done" => Color::Blue,
            "Backlog" => Color::DarkGray,
            _ => Color::White,
        };

        let mut lines: Vec<Line> = Vec::new();
        for card in col.cards.iter().take(available) {
            let pc = widgets::priority_color(&card.priority);
            let mut spans = vec![
                Span::styled(
                    format!(" {}", widgets::truncate_pub(&card.id, 8)),
                    Style::default().fg(pc).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {}", widgets::truncate_pub(&card.title, 14)),
                    Style::default().fg(Color::White),
                ),
            ];
            if !card.role.is_empty() {
                spans.push(Span::styled(
                    format!(" {}", widgets::truncate_pub(&card.role, 4)),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            lines.push(Line::from(spans));
        }
        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "  (empty)",
                Style::default().fg(Color::DarkGray),
            )));
        }

        let block = Block::default()
            .title(format!(" {} ({}) ", col.name, count))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(title_color));
        let p = Paragraph::new(lines).block(block);
        f.render_widget(p, cols[i]);
    }
}

fn render_features(f: &mut Frame, area: Rect, data: &DashboardData) {
    let available = area.height.saturating_sub(2) as usize;

    let lines: Vec<Line> = if data.features.is_empty() {
        vec![Line::from(Span::styled(
            "  No features tracked. Create with issue_create(type=\"feature\") then feature_decompose()",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        let mut result = Vec::new();
        let mut flat_idx: usize = 0;
        for feat in &data.features {
            if result.len() >= available {
                break;
            }

            let pct = if feat.total > 0 {
                feat.done * 100 / feat.total
            } else {
                0
            };
            let bar = progress_bar(feat.done, feat.total, 10);
            let status_color = match feat.status.as_str() {
                "in_progress" => Color::Green,
                "done" => Color::Blue,
                "blocked" => Color::Red,
                _ => Color::Yellow,
            };

            let is_selected = flat_idx == data.feature_cursor;
            let sel_marker = if is_selected { "▸" } else { " " };
            let base_style = if is_selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };

            result.push(Line::from(vec![
                Span::styled(sel_marker, base_style.fg(Color::White)),
                Span::styled(
                    format!("{} ", feat.id),
                    base_style.fg(Color::Magenta).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{} ", bar), base_style.fg(status_color)),
                Span::styled(
                    format!("{}% ", pct),
                    base_style.fg(if pct == 100 {
                        Color::Green
                    } else {
                        Color::White
                    }),
                ),
                Span::styled(
                    widgets::truncate_pub(&feat.title, 28),
                    base_style.fg(Color::White),
                ),
                Span::styled(
                    format!("  {}/{} ", feat.done, feat.total),
                    base_style.fg(Color::DarkGray),
                ),
                Span::styled(format!("({})", feat.space), base_style.fg(Color::DarkGray)),
            ]));
            flat_idx += 1;

            for child in &feat.children {
                if result.len() >= available {
                    break;
                }
                let child_selected = flat_idx == data.feature_cursor;
                let cs_marker = if child_selected { " ▸" } else { "  " };
                let cs = if child_selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };

                let icon = match child.status.as_str() {
                    "done" | "closed" => "[x]",
                    "in_progress" => "[>]",
                    "blocked" => "[!]",
                    _ => "[ ]",
                };
                let child_color = match child.status.as_str() {
                    "done" | "closed" => Color::Green,
                    "in_progress" => Color::Cyan,
                    "blocked" => Color::Red,
                    _ => Color::DarkGray,
                };
                let mut spans = vec![
                    Span::styled(cs_marker.to_string(), cs.fg(Color::White)),
                    Span::styled(icon.to_string(), cs.fg(child_color)),
                    Span::styled(format!(" {} ", child.id), cs.fg(Color::DarkGray)),
                    Span::styled(widgets::truncate_pub(&child.title, 35), cs.fg(child_color)),
                ];
                if let Some(qs) = &child.queue_status {
                    let qc = match qs.as_str() {
                        "Running" => Color::Green,
                        "Pending" => Color::Yellow,
                        "Failed" => Color::Red,
                        _ => Color::DarkGray,
                    };
                    spans.push(Span::styled(format!(" Q:{}", qs), cs.fg(qc)));
                }
                if let Some(p) = child.pane {
                    spans.push(Span::styled(format!(" P{}", p), cs.fg(Color::Cyan)));
                }
                result.push(Line::from(spans));
                flat_idx += 1;
            }
        }
        result
    };

    let feat_count = data.features.len();
    let total_children: usize = data.features.iter().map(|ft| ft.total).sum();
    let total_done: usize = data.features.iter().map(|ft| ft.done).sum();
    let title = format!(
        " Features ({} features, {}/{} tasks) ",
        feat_count, total_done, total_children
    );

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn progress_bar(done: usize, total: usize, width: usize) -> String {
    if total == 0 {
        return format!("[{}]", " ".repeat(width));
    }
    let filled = (done * width) / total;
    let empty = width - filled;
    format!("[{}{}]", "#".repeat(filled), ".".repeat(empty))
}

fn has_pane_identity(pane: &PaneSnapshot) -> bool {
    pane.tmux_target.is_some()
        || pane.project != "--"
        || !pane.task.trim().is_empty()
        || !pane.output.trim().is_empty()
}

fn tmux_done_from_output(output: &str) -> bool {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return false;
    }

    trimmed
        .lines()
        .last()
        .map(|line| {
            let line = line.trim();
            line.ends_with('$')
                || line.ends_with("$ ")
                || line.ends_with('%')
                || line.ends_with("% ")
                || line.contains("Claude exited")
        })
        .unwrap_or(false)
}

fn tmux_error_from_output(output: &str) -> Option<String> {
    let patterns = [
        "Error:",
        "FATAL:",
        "panic:",
        "Traceback",
        "rate limit",
        "hit your limit",
        "SIGTERM",
    ];

    patterns
        .iter()
        .find(|pattern| output.contains(**pattern))
        .map(|pattern| (*pattern).to_string())
}

fn render_coord_agents(f: &mut Frame, area: Rect, data: &DashboardData) {
    let available = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = if data.coord.agents.is_empty() {
        vec![Line::from(Span::styled(
            "  No registered agents",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        data.coord
            .agents
            .iter()
            .take(available)
            .map(|(pane, proj, task)| {
                // Match pane_id format "screen:window.pane" to pane number
                let runtime = data
                    .started_at
                    .iter()
                    .find(|(p, _)| {
                        // Extract pane number from pane_id like "claude6:0.0" -> compare with p
                        pane.ends_with(&format!(".{}", p.saturating_sub(1)))
                            || pane.ends_with(&format!(":{}", p))
                            || pane == &p.to_string()
                    })
                    .map(|(_, ts)| format_runtime(ts))
                    .unwrap_or_default();
                Line::from(vec![
                    Span::styled(
                        format!(" {:<10}", widgets::truncate_pub(pane, 10)),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{:<10}", widgets::truncate_pub(proj, 10)),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(
                        widgets::truncate_pub(task, 20),
                        Style::default().fg(Color::DarkGray),
                    ),
                    if !runtime.is_empty() {
                        Span::styled(format!(" {}", runtime), Style::default().fg(Color::Yellow))
                    } else {
                        Span::raw("")
                    },
                ])
            })
            .collect()
    };

    let block = Block::default()
        .title(format!(" Agents ({}) ", data.coord.agents.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn render_coord_locks(f: &mut Frame, area: Rect, data: &DashboardData) {
    let available = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = if data.coord.locks.is_empty() {
        vec![Line::from(Span::styled(
            "  No active locks",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        data.coord
            .locks
            .iter()
            .take(available)
            .map(|(pane, file)| {
                let short_file = file.split('/').last().unwrap_or(file);
                Line::from(vec![
                    Span::styled(
                        " LK ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{:<10}", widgets::truncate_pub(pane, 10)),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(
                        widgets::truncate_pub(short_file, 20),
                        Style::default().fg(Color::Yellow),
                    ),
                ])
            })
            .collect()
    };

    let block = Block::default()
        .title(format!(" Locks ({}) ", data.coord.locks.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn render_coord_kb(f: &mut Frame, area: Rect, data: &DashboardData) {
    let available = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = if data.coord.kb_recent.is_empty() {
        vec![Line::from(Span::styled(
            "  No KB entries",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        data.coord
            .kb_recent
            .iter()
            .take(available)
            .map(|(cat, title, pane)| {
                let cat_color = match cat.as_str() {
                    "gotcha" => Color::Red,
                    "pattern" => Color::Green,
                    "code_location" => Color::Cyan,
                    "decision" => Color::Yellow,
                    "handoff" => Color::Magenta,
                    _ => Color::DarkGray,
                };
                Line::from(vec![
                    Span::styled(
                        format!(" {:<10}", widgets::truncate_pub(cat, 10)),
                        Style::default().fg(cat_color),
                    ),
                    Span::styled(
                        widgets::truncate_pub(title, 22),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(
                        format!(" {}", widgets::truncate_pub(pane, 8)),
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            })
            .collect()
    };

    let block = Block::default()
        .title(format!(" Knowledge Base ({}) ", data.coord.kb_recent.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn render_coord_branches(f: &mut Frame, area: Rect, data: &DashboardData) {
    let available = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = if data.coord.branches.is_empty() {
        vec![Line::from(Span::styled(
            "  No claimed branches",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        data.coord
            .branches
            .iter()
            .take(available)
            .map(|(pane, branch, proj)| {
                Line::from(vec![
                    Span::styled(
                        format!(" {:<10}", widgets::truncate_pub(pane, 10)),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(
                        format!("{:<18}", widgets::truncate_pub(branch, 18)),
                        Style::default().fg(Color::Green),
                    ),
                    Span::styled(
                        widgets::truncate_pub(proj, 12),
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            })
            .collect()
    };

    let block = Block::default()
        .title(format!(" Branches ({}) ", data.coord.branches.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn render_coord_ports(f: &mut Frame, area: Rect, data: &DashboardData) {
    let available = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = if data.coord.ports.is_empty() {
        vec![Line::from(Span::styled(
            "  No ports allocated",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        data.coord
            .ports
            .iter()
            .take(available)
            .map(|(port, svc, pane)| {
                Line::from(vec![
                    Span::styled(
                        format!(" {:<6}", port),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{:<16}", widgets::truncate_pub(svc, 16)),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(
                        widgets::truncate_pub(pane, 10),
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            })
            .collect()
    };

    let block = Block::default()
        .title(format!(" Ports ({}) ", data.coord.ports.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}
fn render_coord_claims(f: &mut Frame, area: Rect, data: &DashboardData) {
    let available = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = if data.coord.claims.is_empty() {
        vec![Line::from(Span::styled(
            "  No active claims",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        data.coord
            .claims
            .iter()
            .take(available)
            .map(|(repo, issue, agent, claimed_at)| {
                let time = claimed_at.get(11..16).unwrap_or(claimed_at);
                Line::from(vec![
                    Span::styled(
                        format!(" #{:<5}", issue),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{:<18}", widgets::truncate_pub(repo, 18)),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(
                        format!("{:<12}", widgets::truncate_pub(agent, 12)),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(time.to_string(), Style::default().fg(Color::DarkGray)),
                ])
            })
            .collect()
    };

    let block = Block::default()
        .title(format!(" Claims ({}) ", data.coord.claims.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn format_runtime(started: &str) -> String {
    // Parse ISO timestamp and compute elapsed
    if let Ok(start) = chrono::NaiveDateTime::parse_from_str(started, "%Y-%m-%dT%H:%M:%S%.fZ")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(started, "%Y-%m-%dT%H:%M:%SZ"))
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(started, "%Y-%m-%dT%H:%M:%S"))
    {
        let now = chrono::Utc::now().naive_utc();
        let elapsed = now.signed_duration_since(start);
        let mins = elapsed.num_minutes();
        if mins < 1 {
            "<1m".to_string()
        } else if mins < 60 {
            format!("{}m", mins)
        } else {
            format!("{}h{}m", mins / 60, mins % 60)
        }
    } else {
        String::new()
    }
}

fn render_projects(f: &mut Frame, area: Rect, data: &DashboardData) {
    let available = area.height.saturating_sub(2) as usize;

    let mut lines = vec![Line::from(vec![
        Span::styled(" Project          ", Style::default().fg(Color::DarkGray)),
        Span::styled("Tech        ", Style::default().fg(Color::DarkGray)),
        Span::styled("Health  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Test          ", Style::default().fg(Color::DarkGray)),
        Span::styled("Build  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Issues ", Style::default().fg(Color::DarkGray)),
        Span::styled("Agents ", Style::default().fg(Color::DarkGray)),
        Span::styled("Git", Style::default().fg(Color::DarkGray)),
    ])];

    if data.projects.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No projects discovered. Run project_scan or wait for auto-scan.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for proj in data.projects.iter().take(available.saturating_sub(1)) {
            let grade_color = match proj.health_grade.as_str() {
                "A" => Color::Green,
                "B" => Color::Green,
                "C" => Color::Yellow,
                "D" => Color::Red,
                "F" => Color::Red,
                _ => Color::DarkGray,
            };

            let test_spans = match &proj.last_test {
                Some((true, ts)) => vec![
                    Span::styled("PASS ", Style::default().fg(Color::Green)),
                    Span::styled(
                        format!("{:<8}", widgets::truncate_pub(ts, 8)),
                        Style::default().fg(Color::DarkGray),
                    ),
                ],
                Some((false, ts)) => vec![
                    Span::styled(
                        "FAIL ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{:<8}", widgets::truncate_pub(ts, 8)),
                        Style::default().fg(Color::DarkGray),
                    ),
                ],
                None => vec![Span::styled(
                    "--            ",
                    Style::default().fg(Color::DarkGray),
                )],
            };

            let build_spans = match &proj.last_build {
                Some((true, _)) => vec![Span::styled("PASS   ", Style::default().fg(Color::Green))],
                Some((false, _)) => vec![Span::styled("FAIL   ", Style::default().fg(Color::Red))],
                None => vec![Span::styled(
                    "--     ",
                    Style::default().fg(Color::DarkGray),
                )],
            };

            let dirty_indicator = if proj.git_dirty { "*" } else { "" };
            let git_info = if proj.git_ahead > 0 || proj.git_behind > 0 {
                format!(
                    "{}{} +{}-{}",
                    dirty_indicator,
                    if proj.git_dirty { "" } else { "" },
                    proj.git_ahead,
                    proj.git_behind
                )
            } else if proj.git_dirty {
                "dirty".to_string()
            } else {
                "clean".to_string()
            };

            let mut spans = vec![
                Span::styled(
                    format!(
                        " {:<16}{}",
                        widgets::truncate_pub(&proj.name, 16),
                        dirty_indicator
                    ),
                    Style::default().fg(if proj.active_agents > 0 {
                        Color::White
                    } else {
                        Color::Gray
                    }),
                ),
                Span::styled(
                    format!("{:<12}", widgets::truncate_pub(&proj.tech, 12)),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("{} ({:>2}) ", proj.health_grade, proj.health_score),
                    Style::default().fg(grade_color),
                ),
            ];
            spans.extend(test_spans);
            spans.extend(build_spans);
            spans.push(Span::styled(
                format!("{:<7}", proj.open_issues),
                Style::default().fg(if proj.open_issues > 0 {
                    Color::Yellow
                } else {
                    Color::DarkGray
                }),
            ));
            spans.push(Span::styled(
                format!("{:<7}", proj.active_agents),
                Style::default().fg(if proj.active_agents > 0 {
                    Color::Cyan
                } else {
                    Color::DarkGray
                }),
            ));
            spans.push(Span::styled(
                git_info,
                Style::default().fg(if proj.git_dirty {
                    Color::Yellow
                } else {
                    Color::DarkGray
                }),
            ));

            lines.push(Line::from(spans));
        }
    }

    let title = format!(" Projects ({} repos) ", data.projects.len());
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

// ========== INFRA VIEW PANELS ==========

fn render_infra_ports(f: &mut Frame, area: Rect, data: &DashboardData) {
    let available = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = if data.infra.ports.is_empty() {
        vec![Line::from(Span::styled(
            "  No ports allocated",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        data.infra
            .ports
            .iter()
            .take(available)
            .map(|(port, svc, pane)| {
                Line::from(vec![
                    Span::styled(
                        format!(" {:<6}", port),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{:<18}", widgets::truncate_pub(svc, 18)),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(
                        widgets::truncate_pub(pane, 10),
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            })
            .collect()
    };
    let block = Block::default()
        .title(format!(" Ports ({}) ", data.infra.ports.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_infra_builds(f: &mut Frame, area: Rect, data: &DashboardData) {
    let available = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = if data.infra.builds.is_empty() {
        vec![Line::from(Span::styled(
            "  No build data",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        data.infra
            .builds
            .iter()
            .take(available)
            .map(|(proj, success, ago)| {
                let icon = if *success { "✓" } else { "✗" };
                let color = if *success { Color::Green } else { Color::Red };
                Line::from(vec![
                    Span::styled(
                        format!(" {} ", icon),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{:<16}", widgets::truncate_pub(proj, 16)),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(ago.clone(), Style::default().fg(Color::DarkGray)),
                ])
            })
            .collect()
    };
    let block = Block::default()
        .title(format!(" Builds ({}) ", data.infra.builds.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_infra_messages(f: &mut Frame, area: Rect, data: &DashboardData) {
    let available = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = if data.infra.messages.is_empty() {
        vec![Line::from(Span::styled(
            "  No messages",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        data.infra
            .messages
            .iter()
            .take(available)
            .map(|(from, to, msg, pri)| {
                let pri_color = match pri.as_str() {
                    "urgent" => Color::Red,
                    "warn" => Color::Yellow,
                    _ => Color::DarkGray,
                };
                Line::from(vec![
                    Span::styled(
                        format!(" {}", widgets::truncate_pub(from, 6)),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled("→", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{:<6} ", widgets::truncate_pub(to, 6)),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(
                        widgets::truncate_pub(msg, 22),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(format!(" {}", pri), Style::default().fg(pri_color)),
                ])
            })
            .collect()
    };
    let block = Block::default()
        .title(format!(" Messages ({}) ", data.infra.messages.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_infra_sessions(f: &mut Frame, area: Rect, data: &DashboardData) {
    let available = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = if data.infra.sessions.is_empty() {
        vec![Line::from(Span::styled(
            "  No active sessions",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        data.infra
            .sessions
            .iter()
            .take(available)
            .map(|(pane, proj, status)| {
                let sc = widgets::status_color(status);
                Line::from(vec![
                    Span::styled(
                        format!(" {:<10}", widgets::truncate_pub(pane, 10)),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(
                        format!("{:<14}", widgets::truncate_pub(proj, 14)),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(status.clone(), Style::default().fg(sc)),
                ])
            })
            .collect()
    };
    let block = Block::default()
        .title(format!(" Sessions ({}) ", data.infra.sessions.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

// ========== INTEL VIEW PANELS ==========

fn render_intel_kgraph(f: &mut Frame, area: Rect, data: &DashboardData) {
    let mut lines = vec![Line::from(vec![
        Span::styled(
            format!(" {} entities", data.intel.kgraph_entities),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} edges", data.intel.kgraph_edges),
            Style::default().fg(Color::Green),
        ),
    ])];
    if !data.intel.kgraph_top.is_empty() {
        lines.push(Line::from(Span::styled(
            " ─── Top Entities ───",
            Style::default().fg(Color::DarkGray),
        )));
        let available = area.height.saturating_sub(4) as usize;
        for (name, count) in data.intel.kgraph_top.iter().take(available) {
            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {:<18}", widgets::truncate_pub(name, 18)),
                    Style::default().fg(Color::White),
                ),
                Span::styled(format!("({})", count), Style::default().fg(Color::DarkGray)),
            ]));
        }
    }
    let block = Block::default()
        .title(" Knowledge Graph ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_intel_facts(f: &mut Frame, area: Rect, data: &DashboardData) {
    let available = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = if data.intel.facts.is_empty() {
        vec![Line::from(Span::styled(
            "  No facts registered",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        data.intel
            .facts
            .iter()
            .take(available)
            .map(|(key, _val, verified)| {
                let icon = if *verified { "✓" } else { "?" };
                let color = if *verified {
                    Color::Green
                } else {
                    Color::Yellow
                };
                Line::from(vec![
                    Span::styled(format!(" {} ", icon), Style::default().fg(color)),
                    Span::styled(
                        widgets::truncate_pub(key, 30),
                        Style::default().fg(Color::White),
                    ),
                ])
            })
            .collect()
    };
    let block = Block::default()
        .title(format!(" Facts ({}) ", data.intel.fact_count))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_intel_analytics(f: &mut Frame, area: Rect, data: &DashboardData) {
    let available = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = if data.intel.top_tools.is_empty() {
        vec![Line::from(Span::styled(
            "  No analytics data",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        data.intel
            .top_tools
            .iter()
            .take(available)
            .map(|(tool, weight)| {
                Line::from(vec![
                    Span::styled(
                        format!(" {:<16}", widgets::truncate_pub(tool, 16)),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(format!("{:.1}", weight), Style::default().fg(Color::Cyan)),
                ])
            })
            .collect()
    };
    let block = Block::default()
        .title(" Analytics ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_intel_replay(f: &mut Frame, area: Rect, data: &DashboardData) {
    let lines = vec![
        Line::from(vec![
            Span::styled(" Sessions: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", data.intel.replay_sessions),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Tool calls: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", data.intel.replay_tool_calls),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Errors: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", data.intel.replay_errors),
                Style::default().fg(if data.intel.replay_errors > 0 {
                    Color::Red
                } else {
                    Color::Green
                }),
            ),
        ]),
    ];
    let block = Block::default()
        .title(" Session Replay ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_audit_summary(f: &mut Frame, area: Rect, data: &DashboardData) {
    let available = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = if data.audit.projects.is_empty() {
        vec![Line::from(Span::styled(
            "  No audits yet. Run audit_full or wait for background cycle.",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        let mut result = vec![Line::from(vec![
            Span::styled("  Project       ", Style::default().fg(Color::DarkGray)),
            Span::styled("Grade ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "C  H  M  L   I   Total ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled("Ago", Style::default().fg(Color::DarkGray)),
        ])];
        for proj in data.audit.projects.iter().take(available.saturating_sub(1)) {
            let gc = match proj.grade.as_str() {
                "A" => Color::Green,
                "B" => Color::Cyan,
                "C" => Color::Yellow,
                "D" | "F" => Color::Red,
                _ => Color::DarkGray,
            };
            result.push(Line::from(vec![
                Span::styled(
                    format!("  {:<14}", widgets::truncate_pub(&proj.name, 14)),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("  {}   ", proj.grade),
                    Style::default().fg(gc).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}  ", proj.critical),
                    Style::default().fg(if proj.critical > 0 {
                        Color::Red
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::styled(
                    format!("{}  ", proj.high),
                    Style::default().fg(if proj.high > 0 {
                        Color::Yellow
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::styled(
                    format!("{}  ", proj.medium),
                    Style::default().fg(if proj.medium > 0 {
                        Color::White
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::styled(
                    format!("{:<3}", proj.low),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{:<4}", proj.info),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{:>5} ", proj.total),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    proj.last_audit.clone(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
        result
    };
    let block = Block::default()
        .title(" Audit Summary ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_audit_findings(f: &mut Frame, area: Rect, data: &DashboardData) {
    let available = area.height.saturating_sub(2) as usize;
    let mut lines: Vec<Line> = Vec::new();

    if data.audit.projects.is_empty() {
        lines.push(Line::from(Span::styled(
            "  Waiting for audit results...",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for proj in &data.audit.projects {
            if lines.len() >= available {
                break;
            }
            if proj.top_findings.is_empty() {
                continue;
            }
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", proj.name),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("(grade {})", proj.grade),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            for (sev, cat, file, line_num) in &proj.top_findings {
                if lines.len() >= available {
                    break;
                }
                let sc = match sev.as_str() {
                    "critical" => Color::Red,
                    "high" => Color::Yellow,
                    "medium" => Color::White,
                    _ => Color::DarkGray,
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("    {:>8} ", sev),
                        Style::default().fg(sc).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{:<15} ", widgets::truncate_pub(cat, 15)),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(
                        format!("{}:{}", widgets::truncate_pub(file, 30), line_num),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
        }
    }

    let title = format!(
        " Top Findings ({} crit, {} high) ",
        data.audit.total_critical, data.audit.total_high
    );
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_help_bar(f: &mut Frame, area: Rect, data: &DashboardData) {
    let help = if data.view_mode == ViewMode::Features {
        Line::from(vec![
            Span::styled(
                " [j/k]",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("nav ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[n]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("ew ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[Enter]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("queue ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[u]",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("pdate ", Style::default().fg(Color::DarkGray)),
            Span::styled("│ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[s]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("pawn ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[a]",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("uto ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[:]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("cmd ", Style::default().fg(Color::DarkGray)),
            Span::styled("│ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[f]",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("eat ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[b]",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("oard ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[q]",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("uit", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        let focus_label = if data.pane_focus { "grid " } else { "focus " };
        Line::from(vec![
            Span::styled(
                " [s]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("pawn ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[t]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("alk ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[a]",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("uto ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[k]",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled("ill ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[d]",
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("one ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[x]",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled("kill ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[r]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("estart ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[o]",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("rch ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[:]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("cmd ", Style::default().fg(Color::DarkGray)),
            Span::styled("│ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[f]",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("eat ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[b]",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("oard ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[c]",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("oord ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[p]",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("roj ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[i]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("nfra ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[g]",
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("intel ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[h]",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled("ealth ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[l]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("og ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[w]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("ork ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[y]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("pipe ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[0]",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("dash ", Style::default().fg(Color::DarkGray)),
            Span::styled("│ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[+]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("scr ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[-]",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled("rm ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[←↑↓→]",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" sel ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[Enter]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(focus_label, Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[Tab]",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" next ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[q]",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("uit", Style::default().fg(Color::DarkGray)),
        ])
    };
    let p = Paragraph::new(help);
    f.render_widget(p, area);
}

fn render_action_log_view(f: &mut Frame, area: Rect, data: &DashboardData) {
    let block = Block::default()
        .title(" Action Log ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if data.action_log.is_empty() {
        let empty = Paragraph::new("  No actions yet. Press [:] to run MCP tools.")
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(empty, inner);
        return;
    }

    let rows: Vec<Line> = data
        .action_log
        .iter()
        .take(inner.height as usize)
        .map(|entry| {
            let icon = if entry.success { "+" } else { "x" };
            let icon_color = if entry.success {
                Color::Green
            } else {
                Color::Red
            };
            Line::from(vec![
                Span::styled(&entry.timestamp, Style::default().fg(Color::DarkGray)),
                Span::raw("  "),
                Span::styled(
                    icon,
                    Style::default().fg(icon_color).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    format!(
                        "{:<24}",
                        if entry.tool.len() > 24 {
                            &entry.tool[..24]
                        } else {
                            &entry.tool
                        }
                    ),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    if entry.summary.len() > 60 {
                        format!("{}...", &entry.summary[..57])
                    } else {
                        entry.summary.clone()
                    },
                    Style::default().fg(Color::White),
                ),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(rows);
    f.render_widget(paragraph, inner);
}

fn render_pipeline_view(f: &mut Frame, area: Rect, data: &DashboardData) {
    let block = Block::default()
        .title(" Factory Pipelines ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if data.pipelines.is_empty() {
        let empty = Paragraph::new("  No pipelines. Use :go <project> <request> to start one.")
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(empty, inner);
        return;
    }

    // Reserve last line for controls hint
    let content_area = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(1),
    );
    let hint_area = Rect::new(
        inner.x,
        inner.y + inner.height.saturating_sub(1),
        inner.width,
        1,
    );

    let mut lines: Vec<Line> = Vec::new();
    for pipe in &data.pipelines {
        // Header line: id  project  [template]  STATUS  [!N signals]  [PAUSED]
        let status_color = match pipe.status.as_str() {
            "running" => Color::Green,
            "done" => Color::DarkGray,
            "failed" => Color::Red,
            "paused" => Color::Yellow,
            _ => Color::Yellow,
        };
        let mut header_spans = vec![
            Span::styled(
                &pipe.id,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(&pipe.project, Style::default().fg(Color::White)),
            Span::raw("  "),
            Span::styled(
                format!("[{}]", pipe.template),
                Style::default().fg(Color::Magenta),
            ),
            Span::raw("  "),
            Span::styled(
                pipe.status.to_uppercase(),
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        if pipe.signal_count > 0 {
            header_spans.push(Span::raw("  "));
            header_spans.push(Span::styled(
                format!("[!{}]", pipe.signal_count),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ));
        }
        lines.push(Line::from(header_spans));

        // Stage flow
        let mut stage_spans: Vec<Span> = vec![Span::raw("  ")];
        for (i, stage) in pipe.stages.iter().enumerate() {
            if i > 0 {
                stage_spans.push(Span::styled(" -> ", Style::default().fg(Color::DarkGray)));
            }
            let (icon, color) = match stage.status.as_str() {
                "done" => ("+", Color::Green),
                "running" => ("*", Color::Cyan),
                "failed" => ("x", Color::Red),
                "blocked" => (".", Color::DarkGray),
                _ => (".", Color::DarkGray),
            };
            stage_spans.push(Span::styled(icon, Style::default().fg(color)));
            stage_spans.push(Span::styled(&stage.name, Style::default().fg(color)));
            if let Some(pane) = stage.pane {
                stage_spans.push(Span::styled(
                    format!("(P{})", pane),
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }
        lines.push(Line::from(stage_spans));

        // Live PTY output for running stages
        for stage in &pipe.stages {
            if let Some(ref snippet) = stage.pty_snippet {
                for line in snippet.lines().take(2) {
                    let trimmed = if line.len() > 78 { &line[..78] } else { line };
                    lines.push(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(
                            format!("{}| ", stage.name),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(trimmed, Style::default().fg(Color::White)),
                    ]));
                }
            }
        }

        // Gate status
        if let Some(passed) = pipe.gate_passed {
            let (gate_icon, gate_color, gate_text) = if passed {
                ("PASS", Color::Green, "build+test passed")
            } else {
                ("FAIL", Color::Red, "build or test failed")
            };
            lines.push(Line::from(vec![
                Span::raw("  Gate: "),
                Span::styled(
                    gate_icon,
                    Style::default().fg(gate_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" ({})", gate_text),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        // Description
        let desc = if pipe.description.len() > 80 {
            format!("  {}...", &pipe.description[..77])
        } else {
            format!("  {}", pipe.description)
        };
        lines.push(Line::from(Span::styled(
            desc,
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::raw("")));
    }

    let paragraph = Paragraph::new(lines).scroll((0, 0));
    f.render_widget(paragraph, content_area);

    // Controls hint bar
    let hint = Line::from(vec![
        Span::styled(" :pause ", Style::default().fg(Color::Yellow)),
        Span::styled(":resume ", Style::default().fg(Color::Green)),
        Span::styled(":retry ", Style::default().fg(Color::Cyan)),
        Span::styled(":abort ", Style::default().fg(Color::Red)),
        Span::styled(":talk N msg ", Style::default().fg(Color::Magenta)),
        Span::styled(":go proj desc ", Style::default().fg(Color::White)),
    ]);
    f.render_widget(Paragraph::new(hint), hint_area);
}

// ==================== Dashboard View Components ====================

fn render_gauges_row(f: &mut Frame, area: Rect, data: &DashboardData) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    // Agents gauge
    let total = data.panes.len();
    let active = data.active_count;
    let agent_pct = if total > 0 {
        (active as f64 / total as f64 * 100.0) as u16
    } else {
        0
    };
    let agent_bar = widgets::mini_bar(agent_pct, 12, Color::Blue);
    let agents = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            " AGENTS ",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(
                format!(" {} ", active),
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("/ {} ", total),
                Style::default().fg(Color::DarkGray),
            ),
            agent_bar,
        ]),
        Line::from(vec![Span::styled(
            format!(" {}pty {}err", data.pty_count, data.alerts.len()),
            Style::default().fg(Color::DarkGray),
        )]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(26, 38, 54))),
    );
    f.render_widget(agents, cols[0]);

    // Queue gauge
    let _q_total = data.queue_pending + data.queue_running + data.queue_done + data.queue_failed;
    let queue = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            " QUEUE ",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(
                format!(" {} ", data.queue_pending),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("pend ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} ", data.queue_running),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled("run", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![Span::styled(
            format!(" {} done {} fail", data.queue_done, data.queue_failed),
            Style::default().fg(Color::DarkGray),
        )]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(26, 38, 54))),
    );
    f.render_widget(queue, cols[1]);

    // ACU gauge
    let acu_pct = if data.acu_total > 0.0 {
        (data.acu_used / data.acu_total * 100.0).min(100.0) as u16
    } else {
        0
    };
    let acu_bar = widgets::mini_bar(acu_pct, 12, Color::Magenta);
    let acu = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            " ACU TODAY ",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(
                format!(" {:.0} ", data.acu_used),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("/ {:.0} ", data.acu_total),
                Style::default().fg(Color::DarkGray),
            ),
            acu_bar,
        ]),
        Line::from(vec![Span::styled(
            format!(" {}%", acu_pct),
            Style::default().fg(Color::DarkGray),
        )]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(26, 38, 54))),
    );
    f.render_widget(acu, cols[2]);

    // Reviews gauge
    let rev_pct = if data.reviews_total > 0 {
        (data.reviews_used as f64 / data.reviews_total as f64 * 100.0).min(100.0) as u16
    } else {
        0
    };
    let rev_bar = widgets::mini_bar(rev_pct, 12, Color::Yellow);
    let reviews = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            " REVIEWS ",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(
                format!(" {} ", data.reviews_used),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("/ {} ", data.reviews_total),
                Style::default().fg(Color::DarkGray),
            ),
            rev_bar,
        ]),
        Line::from(vec![Span::styled(
            format!(" {}%", rev_pct),
            Style::default().fg(Color::DarkGray),
        )]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(26, 38, 54))),
    );
    f.render_widget(reviews, cols[3]);
}

fn render_builds_panel(f: &mut Frame, area: Rect, data: &DashboardData) {
    let block = Block::default()
        .title(Span::styled(
            format!(" Build Environments ({}/5) ", data.builds.len()),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(26, 38, 54)));

    if data.builds.is_empty() {
        let p = Paragraph::new(Span::styled(
            "  No build environments found",
            Style::default().fg(Color::DarkGray),
        ))
        .block(block);
        f.render_widget(p, area);
        return;
    }

    let inner = block.inner(area);
    f.render_widget(block, area);

    let build_colors: [Color; 5] = [
        Color::Rgb(232, 64, 64),   // Bloodstream - red
        Color::Rgb(61, 220, 132),  // Matrix - green
        Color::Rgb(92, 154, 255),  // Ghost Protocol - blue
        Color::Rgb(167, 139, 250), // Neon Noir - purple
        Color::Rgb(251, 191, 36),  // Molten - gold
    ];

    let mut lines: Vec<Line> = Vec::new();
    for build in &data.builds {
        let color = if build.number >= 1 && build.number <= 5 {
            build_colors[(build.number - 1) as usize]
        } else {
            Color::DarkGray
        };

        // Build header line
        let mut spans = vec![
            Span::styled(
                format!(" B{} ", build.number),
                Style::default()
                    .fg(Color::Black)
                    .bg(color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {} ", build.theme),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled("│ ", Style::default().fg(Color::Rgb(26, 38, 54))),
        ];

        // Pane dots
        for pane in &build.panes {
            let pane_fg = Color::Rgb(
                u8::from_str_radix(&pane.colors.fg[1..3], 16).unwrap_or(128),
                u8::from_str_radix(&pane.colors.fg[3..5], 16).unwrap_or(128),
                u8::from_str_radix(&pane.colors.fg[5..7], 16).unwrap_or(128),
            );
            let pane_bg = Color::Rgb(
                u8::from_str_radix(&pane.colors.bg[1..3], 16).unwrap_or(8),
                u8::from_str_radix(&pane.colors.bg[3..5], 16).unwrap_or(8),
                u8::from_str_radix(&pane.colors.bg[5..7], 16).unwrap_or(8),
            );
            spans.push(Span::styled(
                format!(" {} ", pane.pane_index),
                Style::default()
                    .fg(pane_fg)
                    .bg(pane_bg)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(" "));
        }

        spans.push(Span::styled(
            "│ ",
            Style::default().fg(Color::Rgb(26, 38, 54)),
        ));
        // Show what's running in first pane's cwd
        let cwd_short = build
            .panes
            .first()
            .map(|p| p.cwd.split('/').last().unwrap_or(&p.cwd).to_string())
            .unwrap_or_default();
        spans.push(Span::styled(
            cwd_short,
            Style::default().fg(Color::DarkGray),
        ));

        lines.push(Line::from(spans));

        // Description line
        lines.push(Line::from(vec![Span::styled(
            format!(
                "     {} · {} panes",
                crate::build::theme_desc(build.number),
                build.pane_count
            ),
            Style::default().fg(Color::Rgb(61, 79, 99)),
        )]));
    }

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_color_active_ok_is_green() {
        assert_eq!(status_color("active", "ok"), Color::Green);
        assert_eq!(status_color("active", ""), Color::Green);
    }

    #[test]
    fn status_color_error_is_red() {
        assert_eq!(status_color("active", "error"), Color::Red);
        assert_eq!(status_color("idle", "error"), Color::Red);
        assert_eq!(status_color("", "dead"), Color::Red);
    }

    #[test]
    fn status_color_idle_is_yellow() {
        assert_eq!(status_color("idle", ""), Color::Yellow);
        assert_eq!(status_color("idle", "ok"), Color::Yellow);
    }

    #[test]
    fn status_color_stuck_is_yellow() {
        assert_eq!(status_color("active", "stuck"), Color::Yellow);
    }

    #[test]
    fn status_color_done_is_cyan() {
        assert_eq!(status_color("done", ""), Color::Cyan);
        assert_eq!(status_color("done", "ok"), Color::Cyan);
    }

    #[test]
    fn status_color_unknown_is_dark_gray() {
        assert_eq!(status_color("unknown", ""), Color::DarkGray);
    }

    #[test]
    fn live_preview_lines_is_five() {
        assert_eq!(LIVE_PREVIEW_LINES, 5);
    }
}

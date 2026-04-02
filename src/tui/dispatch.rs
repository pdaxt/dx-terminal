//! Universal MCP dispatch — routes any tool name to its implementation.

use crate::app::App;
use crate::mcp::{tools, types::*};
use serde_json::Value;

macro_rules! deser {
    ($args:expr, $ty:ty) => {
        match serde_json::from_value::<$ty>($args.clone()) {
            Ok(r) => r,
            Err(e) => return format!("{{\"error\":\"Bad args for {}: {}\"}}", stringify!($ty), e),
        }
    };
}

/// Dispatch any MCP tool by name + JSON args. Returns JSON string result.
pub async fn dispatch_mcp_tool(app: &App, tool: &str, args: Value) -> String {
    match tool {
        // === PANE LIFECYCLE ===
        "spawn" | "os_spawn" => tools::spawn(app, deser!(args, SpawnRequest)).await,
        "kill" | "os_kill" => tools::kill(app, deser!(args, KillRequest)).await,
        "restart" | "os_restart" => tools::restart(app, deser!(args, RestartRequest)).await,
        "reassign" | "os_reassign" => tools::reassign(app, deser!(args, ReassignRequest)).await,
        "assign" | "os_assign" => tools::assign(app, deser!(args, AssignRequest)).await,
        "assign_adhoc" | "os_assign_adhoc" => {
            tools::assign_adhoc(app, deser!(args, AssignAdhocRequest)).await
        }
        "collect" | "os_collect" => tools::collect(app, deser!(args, CollectRequest)).await,
        "complete" | "os_complete" => tools::complete(app, deser!(args, CompleteRequest)).await,

        // === CONFIGURATION ===
        "set_mcps" | "os_set_mcps" => tools::set_mcps(app, deser!(args, SetMcpsRequest)).await,
        "set_preamble" | "os_set_preamble" => {
            tools::set_preamble(app, deser!(args, SetPreambleRequest)).await
        }
        "config_show" | "os_config_show" => {
            tools::config_show(app, deser!(args, ConfigShowRequest)).await
        }

        // === MONITORING ===
        "status" | "os_status" => tools::status(app).await,
        "dashboard" | "os_dashboard" => tools::dashboard(app, deser!(args, DashboardRequest)).await,
        "logs" | "os_logs" => tools::logs(app, deser!(args, LogsRequest)).await,
        "health" | "os_health" => tools::health(app).await,
        "monitor" | "os_monitor" => tools::monitor(app, deser!(args, MonitorRequest)).await,
        "project_status" | "os_project_status" => {
            tools::project_status(app, deser!(args, ProjectStatusRequest)).await
        }
        "digest" | "os_digest" => tools::digest(app, deser!(args, DigestRequest)).await,
        "watch" | "os_watch" => tools::watch(app, deser!(args, WatchRequest)).await,

        // === MCP ROUTING ===
        "mcp_list" | "os_mcp_list" => tools::mcp_list(app, deser!(args, McpListRequest)).await,
        "mcp_route" | "os_mcp_route" => tools::mcp_route(app, deser!(args, McpRouteRequest)).await,
        "mcp_search" | "os_mcp_search" => {
            tools::mcp_search(app, deser!(args, McpSearchRequest)).await
        }

        // === GIT ISOLATION ===
        "git_sync" | "os_git_sync" => tools::git_sync(app, deser!(args, GitSyncRequest)).await,
        "git_status" | "os_git_status" => {
            tools::git_status_tool(app, deser!(args, GitStatusRequest)).await
        }
        "git_push" | "os_git_push" => tools::git_push(app, deser!(args, GitPushRequest)).await,
        "git_pr" | "os_git_pr" => tools::git_pr(app, deser!(args, GitPrRequest)).await,
        "git_merge" | "os_git_merge" => tools::git_merge(app, deser!(args, GitMergeRequest)).await,

        // === QUEUE / AUTO-CYCLE ===
        "queue_add" | "os_queue_add" => tools::queue_add(app, deser!(args, QueueAddRequest)).await,
        "queue_decompose" | "os_queue_decompose" => {
            tools::queue_decompose(app, deser!(args, DecomposeRequest)).await
        }
        "queue_list" | "os_queue_list" => {
            tools::queue_list(app, deser!(args, QueueListRequest)).await
        }
        "queue_done" | "os_queue_done" => {
            tools::queue_done(app, deser!(args, QueueDoneRequest)).await
        }
        "auto" | "os_auto" | "auto_cycle" => tools::auto_cycle(app).await,
        "auto_config" | "os_auto_config" => {
            tools::auto_config(app, deser!(args, AutoConfigRequest)).await
        }
        "queue_cancel" | "os_queue_cancel" => {
            tools::queue_cancel(app, deser!(args, QueueCancelRequest)).await
        }
        "queue_retry" | "os_queue_retry" => {
            tools::queue_retry(app, deser!(args, QueueRetryRequest)).await
        }
        "queue_clear" | "os_queue_clear" => {
            tools::queue_clear(app, deser!(args, QueueClearRequest)).await
        }

        // === MULTI-AGENT: PORTS ===
        "port_allocate" => {
            let r = deser!(args, PortAllocateRequest);
            crate::multi_agent::port_allocate(
                &r.service,
                &r.pane_id,
                r.preferred,
                &r.description.unwrap_or_default(),
            )
            .to_string()
        }
        "port_release" => {
            let r = deser!(args, PortReleaseRequest);
            crate::multi_agent::port_release(r.port).to_string()
        }
        "port_list" => crate::multi_agent::port_list().to_string(),
        "port_get" => {
            let r = deser!(args, PortGetRequest);
            crate::multi_agent::port_get(&r.service).to_string()
        }

        // === MULTI-AGENT: AGENTS ===
        "agent_register" => {
            let r = deser!(args, AgentRegisterRequest);
            crate::multi_agent::agent_register(
                &r.pane_id,
                &r.project,
                &r.task,
                &r.files.unwrap_or_default(),
            )
            .to_string()
        }
        "agent_update" => {
            let r = deser!(args, AgentUpdateRequest);
            crate::multi_agent::agent_update(&r.pane_id, &r.task, r.files.as_deref()).to_string()
        }
        "agent_list" => {
            let r = deser!(args, AgentListRequest);
            crate::multi_agent::agent_list(r.project.as_deref()).to_string()
        }
        "agent_deregister" => {
            let r = deser!(args, AgentDeregisterRequest);
            crate::multi_agent::agent_deregister(&r.pane_id).to_string()
        }

        // === MULTI-AGENT: LOCKS ===
        "lock_acquire" => {
            let r = deser!(args, LockAcquireRequest);
            crate::multi_agent::lock_acquire(&r.pane_id, &r.files, &r.reason.unwrap_or_default())
                .to_string()
        }
        "lock_release" => {
            let r = deser!(args, LockReleaseRequest);
            crate::multi_agent::lock_release(&r.pane_id, &r.files.unwrap_or_default()).to_string()
        }
        "lock_check" => {
            let r = deser!(args, LockCheckRequest);
            crate::multi_agent::lock_check(&r.files).to_string()
        }

        // === MULTI-AGENT: GIT BRANCHES ===
        "git_claim_branch" => {
            let r = deser!(args, GitClaimBranchRequest);
            crate::multi_agent::git_claim_branch(
                &r.pane_id,
                &r.branch,
                &r.repo,
                &r.purpose.unwrap_or_default(),
            )
            .to_string()
        }
        "git_release_branch" => {
            let r = deser!(args, GitReleaseBranchRequest);
            crate::multi_agent::git_release_branch(&r.pane_id, &r.branch, &r.repo).to_string()
        }
        "git_list_branches" => {
            let r = deser!(args, GitListBranchesRequest);
            crate::multi_agent::git_list_branches(r.repo.as_deref()).to_string()
        }
        "git_pre_commit_check" => {
            let r = deser!(args, GitPreCommitCheckRequest);
            crate::multi_agent::git_pre_commit_check(&r.pane_id, &r.repo, &r.files).to_string()
        }

        // === MULTI-AGENT: BUILDS ===
        "build_claim" => {
            let r = deser!(args, BuildClaimRequest);
            crate::multi_agent::build_claim(
                &r.pane_id,
                &r.project,
                &r.build_type.unwrap_or_else(|| "default".into()),
            )
            .to_string()
        }
        "build_release" => {
            let r = deser!(args, BuildReleaseRequest);
            crate::multi_agent::build_release(
                &r.pane_id,
                &r.project,
                r.success,
                &r.output.unwrap_or_default(),
            )
            .to_string()
        }
        "build_status" => {
            let r = deser!(args, BuildStatusRequest);
            crate::multi_agent::build_status(&r.project).to_string()
        }
        "build_get_last" => {
            let r = deser!(args, BuildGetLastRequest);
            crate::multi_agent::build_get_last(&r.project).to_string()
        }

        // === MULTI-AGENT: TASKS ===
        "ma_task_add" => {
            let r = deser!(args, MaTaskAddRequest);
            crate::multi_agent::task_add(
                &r.project,
                &r.title,
                &r.description.unwrap_or_default(),
                &r.priority.unwrap_or_else(|| "medium".into()),
                &r.added_by,
            )
            .to_string()
        }
        "ma_task_claim" => {
            let r = deser!(args, MaTaskClaimRequest);
            crate::multi_agent::task_claim(&r.pane_id, r.project.as_deref()).to_string()
        }
        "ma_task_complete" => {
            let r = deser!(args, MaTaskCompleteRequest);
            crate::multi_agent::task_complete(&r.task_id, &r.pane_id, &r.result.unwrap_or_default())
                .to_string()
        }
        "ma_task_list" => {
            let r = deser!(args, MaTaskListRequest);
            crate::multi_agent::task_list(r.status.as_deref(), r.project.as_deref()).to_string()
        }

        // === MULTI-AGENT: KB ===
        "kb_add" => {
            let r = deser!(args, KbAddRequest);
            crate::multi_agent::kb_add(
                &r.pane_id,
                &r.project,
                &r.category,
                &r.title,
                &r.content,
                &r.files.unwrap_or_default(),
            )
            .to_string()
        }
        "kb_search" => {
            let r = deser!(args, KbSearchRequest);
            crate::multi_agent::kb_search(&r.query, r.project.as_deref(), r.category.as_deref())
                .to_string()
        }
        "kb_list" => {
            let r = deser!(args, KbListRequest);
            crate::multi_agent::kb_list(r.project.as_deref(), r.limit.unwrap_or(20)).to_string()
        }

        // === MULTI-AGENT: MESSAGING ===
        "msg_broadcast" => {
            let r = deser!(args, MsgBroadcastRequest);
            let result = crate::multi_agent::msg_broadcast(
                &r.from_pane,
                &r.message,
                &r.priority.unwrap_or_else(|| "info".into()),
            );
            // Push to all active panes for real-time delivery
            let formatted = format!("[BROADCAST from {}]: {}", r.from_pane, r.message);
            {
                let state = app.state.blocking_read();
                for i in 1..=crate::config::pane_count() {
                    let pd = state.panes.get(&i.to_string());
                    if let Some(target) = pd.and_then(|p| p.tmux_target.as_deref()) {
                        let _ = crate::tmux::send_command(target, &formatted);
                    } else {
                        let mut pty = app.pty_lock();
                        if pty.is_running(i) {
                            let _ = pty.send_line(i, &formatted);
                        }
                    }
                }
            }
            result.to_string()
        }
        "msg_send" => {
            let r = deser!(args, MsgSendRequest);
            let result = crate::multi_agent::msg_send(&r.from_pane, &r.to_pane, &r.message);
            // Push to target agent via tmux (PTY fallback)
            if let Ok(pane_num) = r.to_pane.parse::<u8>() {
                let formatted = format!("[MSG from {}]: {}", r.from_pane, r.message);
                let state = app.state.blocking_read();
                let tmux_target = state
                    .panes
                    .get(&pane_num.to_string())
                    .and_then(|p| p.tmux_target.clone());
                drop(state);
                if let Some(target) = tmux_target {
                    let _ = crate::tmux::send_command(&target, &formatted);
                } else {
                    let mut pty = app.pty_lock();
                    let _ = pty.send_line(pane_num, &formatted);
                }
            }
            result.to_string()
        }
        "msg_get" => {
            let r = deser!(args, MsgGetRequest);
            crate::multi_agent::msg_get(&r.pane_id, r.mark_read.unwrap_or(true)).to_string()
        }

        // === AGENT SIGNALS ===
        "os_signal" | "signal" => {
            let r = deser!(args, SignalRequest);
            crate::multi_agent::signal_send(
                &r.pane_id,
                &r.signal_type,
                &r.message,
                r.pipeline_id.as_deref(),
            )
            .to_string()
        }
        "signal_list" | "signals" | "os_signal_list" => {
            crate::multi_agent::signal_list(true).to_string()
        }
        "signal_ack" | "os_signal_ack" => {
            let r = deser!(args, SignalAckRequest);
            crate::multi_agent::signal_acknowledge(r.signal_id).to_string()
        }

        // === MULTI-AGENT: MISC ===
        "cleanup_all" => crate::multi_agent::cleanup_all().to_string(),
        "status_overview" => {
            let r = deser!(args, StatusOverviewRequest);
            crate::multi_agent::status_overview(r.project.as_deref()).to_string()
        }

        // === TRACKER ===
        "issue_create" => tools::tracker_tools::issue_create(&deser!(args, IssueCreateRequest)),
        "issue_update_full" | "issue_update" => {
            tools::tracker_tools::issue_update_full(&deser!(args, IssueUpdateFullRequest))
        }
        "issue_list_filtered" | "issue_list" => {
            tools::tracker_tools::issue_list_filtered(&deser!(args, IssueListFilteredRequest))
        }
        "issue_view" => {
            let r = deser!(args, IssueViewRequest);
            tools::tracker_tools::issue_view(&r.space, &r.issue_id)
        }
        "issue_comment" => {
            let r = deser!(args, IssueCommentRequest);
            tools::tracker_tools::issue_comment(
                &r.space,
                &r.issue_id,
                &r.text,
                &r.author.unwrap_or_else(|| "agent".into()),
            )
        }
        "issue_link" => {
            let r = deser!(args, IssueLinkRequest);
            tools::tracker_tools::issue_link(&r.space, &r.issue_id, &r.link_type, &r.reference)
        }
        "issue_close" => {
            let r = deser!(args, IssueCloseRequest);
            tools::tracker_tools::issue_close(
                &r.space,
                &r.issue_id,
                r.resolution.as_deref().unwrap_or(""),
            )
        }
        "milestone_create" => {
            tools::tracker_tools::milestone_create(&deser!(args, MilestoneCreateRequest))
        }
        "milestone_list" => {
            let r = deser!(args, MilestoneListRequest);
            tools::tracker_tools::milestone_list(&r.space)
        }
        "timeline_generate" => {
            let r = deser!(args, TimelineGenerateRequest);
            tools::tracker_tools::timeline_generate(&r.space, &r.milestone.unwrap_or_default())
        }
        "board_view" => {
            let r = deser!(args, BoardViewRequest);
            tools::tracker_tools::board_view(&r.space)
        }
        "feature_to_queue" => {
            tools::tracker_tools::feature_to_queue(&deser!(args, FeatureToQueueRequest))
        }

        // === TRACKER: FEATURES ===
        "issue_children" => {
            let r = deser!(args, IssueChildrenRequest);
            crate::tracker::issue_children(&r.space, &r.parent_id).to_string()
        }
        "feature_decompose" => {
            let r = deser!(args, FeatureDecomposeRequest);
            crate::tracker::feature_decompose(&r.space, &r.parent_id, &r.children).to_string()
        }
        "feature_status" => {
            let r = deser!(args, FeatureStatusRequest);
            crate::tracker::feature_status(&r.space, &r.feature_id).to_string()
        }

        // === TRACKER: PROCESSES ===
        "process_start" => {
            let r = deser!(args, ProcessStartRequest);
            crate::tracker::process_start(
                &r.space,
                &r.template_name,
                &r.context.unwrap_or(serde_json::json!({})),
            )
            .to_string()
        }
        "process_update" => {
            let r = deser!(args, ProcessUpdateRequest);
            crate::tracker::process_update(
                &r.space,
                &r.process_id,
                r.step_index,
                r.done.unwrap_or(true),
            )
            .to_string()
        }
        "process_list" => {
            let r = deser!(args, ProcessListRequest);
            crate::tracker::process_list(&r.space).to_string()
        }
        "process_template_create" => {
            let r = deser!(args, ProcessTemplateCreateRequest);
            crate::tracker::process_template_create(&r.name, &r.content).to_string()
        }

        // === CAPACITY ===
        "cap_configure" => {
            let r = deser!(args, CapConfigureRequest);
            crate::capacity::cap_configure(
                r.pane_count,
                r.hours_per_day,
                r.availability_factor,
                r.review_bandwidth,
                r.build_slots,
            )
            .to_string()
        }
        "cap_estimate" => {
            let r = deser!(args, CapEstimateRequest);
            crate::capacity::cap_estimate(
                &r.description,
                &r.complexity.unwrap_or_default(),
                &r.task_type.unwrap_or_default(),
                &r.role.unwrap_or_default(),
            )
            .to_string()
        }
        "cap_log_work" => {
            let r = deser!(args, CapLogWorkRequest);
            crate::capacity::cap_log_work_full(
                &r.issue_id,
                &r.space,
                &r.role,
                &r.pane_id.unwrap_or_default(),
                r.acu_spent,
                r.review_needed.unwrap_or(false),
                &r.notes.unwrap_or_default(),
            )
            .to_string()
        }
        "cap_plan_sprint" => {
            let r = deser!(args, CapPlanSprintRequest);
            crate::capacity::cap_plan_sprint(
                &r.space,
                &r.name.unwrap_or_default(),
                &r.start_date.unwrap_or_default(),
                r.days.unwrap_or(5),
                &r.issue_ids.unwrap_or_default(),
            )
            .to_string()
        }
        "cap_dashboard" => {
            let r = deser!(args, CapDashboardRequest);
            crate::capacity::cap_dashboard(
                &r.space.unwrap_or_default(),
                &r.sprint_id.unwrap_or_default(),
            )
            .to_string()
        }
        "cap_burndown" => {
            let r = deser!(args, CapBurndownRequest);
            crate::capacity::cap_burndown(&r.sprint_id.unwrap_or_default()).to_string()
        }
        "cap_velocity" => {
            let r = deser!(args, CapVelocityRequest);
            crate::capacity::cap_velocity(&r.space.unwrap_or_default(), r.count.unwrap_or(5))
                .to_string()
        }
        "cap_roles" => crate::capacity::cap_roles().to_string(),

        // === COLLAB ===
        "space_list" => crate::collab::space_list().to_string(),
        "space_create" => {
            let r = deser!(args, SpaceCreateRequest);
            crate::collab::space_create(&r.name).to_string()
        }
        "doc_list" => {
            let r = deser!(args, DocListRequest);
            crate::collab::doc_list(&r.space.unwrap_or_default(), &r.status.unwrap_or_default())
                .to_string()
        }
        "doc_read" => {
            let r = deser!(args, DocReadRequest);
            crate::collab::doc_read(&r.space, &r.name, r.include_meta.unwrap_or(true)).to_string()
        }
        "doc_create" => {
            let r = deser!(args, DocCreateRequest);
            crate::collab::doc_create(
                &r.space,
                &r.name,
                &r.content.unwrap_or_default(),
                &r.status.unwrap_or_default(),
                &r.tags.unwrap_or_default(),
            )
            .to_string()
        }
        "doc_edit" => {
            let r = deser!(args, DocEditRequest);
            crate::collab::doc_edit(
                &r.space,
                &r.name,
                &r.content,
                &r.agent_id.unwrap_or_default(),
            )
            .to_string()
        }
        "doc_propose" => {
            let r = deser!(args, DocProposeRequest);
            crate::collab::doc_propose(
                &r.space,
                &r.name,
                &r.content,
                &r.summary.unwrap_or_default(),
                &r.agent_id.unwrap_or_default(),
            )
            .to_string()
        }
        "doc_approve" => {
            let r = deser!(args, DocApproveRequest);
            crate::collab::doc_approve(
                &r.space,
                &r.name,
                &r.proposal_id.unwrap_or_else(|| "latest".into()),
            )
            .to_string()
        }
        "doc_reject" => {
            let r = deser!(args, DocRejectRequest);
            crate::collab::doc_reject(
                &r.space,
                &r.name,
                &r.proposal_id,
                &r.reason.unwrap_or_default(),
            )
            .to_string()
        }
        "doc_lock" => {
            let r = deser!(args, DocLockRequest);
            crate::collab::doc_lock(&r.space, &r.name, &r.locked_by.unwrap_or_default()).to_string()
        }
        "doc_unlock" => {
            let r = deser!(args, DocUnlockRequest);
            crate::collab::doc_unlock(&r.space, &r.name).to_string()
        }
        "doc_comment" => {
            let r = deser!(args, DocCommentRequest);
            crate::collab::doc_comment(
                &r.space,
                &r.name,
                &r.text,
                &r.author.unwrap_or_default(),
                r.line.unwrap_or(0),
            )
            .to_string()
        }
        "doc_comments" => {
            let r = deser!(args, DocCommentsRequest);
            crate::collab::doc_comments(&r.space, &r.name).to_string()
        }
        "doc_status" => {
            let r = deser!(args, DocStatusRequest);
            crate::collab::doc_status(&r.space, &r.name, &r.status).to_string()
        }
        "doc_search" => {
            let r = deser!(args, DocSearchRequest);
            crate::collab::doc_search(&r.query, &r.space.unwrap_or_default()).to_string()
        }
        "doc_directives" => {
            let r = deser!(args, DocDirectivesRequest);
            crate::collab::doc_directives(&r.space.unwrap_or_default()).to_string()
        }
        "doc_history" => {
            let r = deser!(args, DocHistoryRequest);
            crate::collab::doc_history(&r.space, &r.name, r.limit.unwrap_or(10)).to_string()
        }
        "doc_delete" => {
            let r = deser!(args, DocDeleteRequest);
            crate::collab::doc_delete(&r.space, &r.name, r.confirm.unwrap_or(false)).to_string()
        }
        "collab_init" => crate::collab::collab_init().to_string(),

        // === KNOWLEDGE GRAPH ===
        "kgraph_add_entity" => {
            let r = deser!(args, KgraphAddEntityRequest);
            crate::knowledge::kgraph_add_entity(
                &r.name,
                &r.entity_type,
                &r.properties.unwrap_or_else(|| "{}".into()),
                &r.id.unwrap_or_default(),
            )
            .to_string()
        }
        "kgraph_add_edge" => {
            let r = deser!(args, KgraphAddEdgeRequest);
            crate::knowledge::kgraph_add_edge(
                &r.source,
                &r.target,
                &r.relation,
                r.weight.unwrap_or(1.0),
                &r.properties.unwrap_or_else(|| "{}".into()),
            )
            .to_string()
        }
        "kgraph_observe" => {
            let r = deser!(args, KgraphObserveRequest);
            crate::knowledge::kgraph_observe(
                &r.source,
                &r.target,
                &r.relation,
                &r.observation,
                r.impact.unwrap_or(0.1),
                &r.session_id.unwrap_or_default(),
            )
            .to_string()
        }
        "kgraph_query_neighbors" => {
            let r = deser!(args, KgraphQueryNeighborsRequest);
            crate::knowledge::kgraph_query_neighbors(
                &r.entity,
                &r.relation.unwrap_or_default(),
                &r.direction.unwrap_or_else(|| "both".into()),
                r.depth.unwrap_or(1),
                r.limit.unwrap_or(50),
            )
            .to_string()
        }
        "kgraph_query_path" => {
            let r = deser!(args, KgraphQueryPathRequest);
            crate::knowledge::kgraph_query_path(&r.source, &r.target, r.max_depth.unwrap_or(4))
                .to_string()
        }
        "kgraph_search" => {
            let r = deser!(args, KgraphSearchRequest);
            crate::knowledge::kgraph_search(
                &r.query,
                &r.entity_type.unwrap_or_default(),
                r.limit.unwrap_or(20),
            )
            .to_string()
        }
        "kgraph_delete" => {
            let r = deser!(args, KgraphDeleteRequest);
            crate::knowledge::kgraph_delete(
                &r.entity_id.unwrap_or_default(),
                &r.edge_source.unwrap_or_default(),
                &r.edge_target.unwrap_or_default(),
                &r.edge_relation.unwrap_or_default(),
            )
            .to_string()
        }
        "kgraph_stats" => crate::knowledge::kgraph_stats().to_string(),

        // === SESSION REPLAY ===
        "replay_index" => {
            let r = deser!(args, ReplayIndexRequest);
            crate::knowledge::replay_index(r.force.unwrap_or(false), &r.project.unwrap_or_default())
                .to_string()
        }
        "replay_search" => {
            let r = deser!(args, ReplaySearchRequest);
            crate::knowledge::replay_search(
                &r.query,
                &r.project.unwrap_or_default(),
                &r.tool.unwrap_or_default(),
                r.limit.unwrap_or(20),
                r.days.unwrap_or(0),
            )
            .to_string()
        }
        "replay_session" => {
            let r = deser!(args, ReplaySessionRequest);
            crate::knowledge::replay_session(
                &r.session_id,
                r.include_tools.unwrap_or(true),
                r.include_errors.unwrap_or(true),
                r.max_messages.unwrap_or(100),
            )
            .to_string()
        }
        "replay_list_sessions" => {
            let r = deser!(args, ReplayListSessionsRequest);
            crate::knowledge::replay_list_sessions(
                &r.project.unwrap_or_default(),
                r.days.unwrap_or(30),
                r.limit.unwrap_or(50),
            )
            .to_string()
        }
        "replay_tool_history" => {
            let r = deser!(args, ReplayToolHistoryRequest);
            crate::knowledge::replay_tool_history(
                &r.tool_name,
                r.limit.unwrap_or(20),
                r.days.unwrap_or(0),
            )
            .to_string()
        }
        "replay_errors" => {
            let r = deser!(args, ReplayErrorsRequest);
            crate::knowledge::replay_errors(
                &r.project.unwrap_or_default(),
                r.days.unwrap_or(7),
                r.limit.unwrap_or(50),
            )
            .to_string()
        }
        "replay_status" => crate::knowledge::replay_status().to_string(),

        // === TRUTHGUARD ===
        "fact_add" => {
            let r = deser!(args, FactAddRequest);
            crate::knowledge::fact_add(
                &r.category,
                &r.key,
                &r.value,
                r.confidence.unwrap_or(1.0),
                &r.source.unwrap_or_default(),
                &r.aliases.unwrap_or_default(),
                &r.tags.unwrap_or_default(),
            )
            .to_string()
        }
        "fact_get" => {
            let r = deser!(args, FactGetRequest);
            crate::knowledge::fact_get(
                &r.fact_id.unwrap_or_default(),
                &r.key.unwrap_or_default(),
                &r.category.unwrap_or_default(),
            )
            .to_string()
        }
        "fact_search" => {
            let r = deser!(args, FactSearchRequest);
            crate::knowledge::fact_search(
                &r.query.unwrap_or_default(),
                &r.category.unwrap_or_default(),
                r.min_confidence.unwrap_or(0.0),
                r.limit.unwrap_or(20),
            )
            .to_string()
        }
        "fact_check" => {
            let r = deser!(args, FactCheckRequest);
            crate::knowledge::fact_check(&r.claim).to_string()
        }
        "fact_check_response" => {
            let r = deser!(args, FactCheckResponseRequest);
            crate::knowledge::fact_check_response(&r.response_text).to_string()
        }
        "fact_update" => {
            let r = deser!(args, FactUpdateRequest);
            crate::knowledge::fact_update(
                &r.fact_id.unwrap_or_default(),
                &r.category.unwrap_or_default(),
                &r.key.unwrap_or_default(),
                &r.value.unwrap_or_default(),
                r.confidence.unwrap_or(-1.0),
                &r.aliases.unwrap_or_default(),
                &r.source.unwrap_or_default(),
                &r.tags.unwrap_or_default(),
            )
            .to_string()
        }
        "fact_delete" => {
            let r = deser!(args, FactDeleteRequest);
            crate::knowledge::fact_delete(&r.fact_id, &r.reason.unwrap_or_default()).to_string()
        }
        "truthguard_status" => crate::knowledge::truthguard_status().to_string(),

        // === ANALYTICS ===
        "log_tool_call" => {
            let r = deser!(args, LogToolCallRequest);
            crate::analytics::log_tool_call(
                &r.pane_id,
                &r.tool_name,
                r.input_size.unwrap_or(0),
                r.output_size.unwrap_or(0),
                r.latency_ms,
                r.success.unwrap_or(true),
                r.error_preview.as_deref(),
            )
            .to_string()
        }
        "log_file_op" => {
            let r = deser!(args, LogFileOpRequest);
            crate::analytics::log_file_op(&r.pane_id, &r.file_path, &r.operation, r.lines_changed)
                .to_string()
        }
        "log_tokens" => {
            let r = deser!(args, LogTokensRequest);
            crate::analytics::log_tokens(
                &r.pane_id,
                &r.model,
                r.input_tokens,
                r.output_tokens,
                r.cache_read.unwrap_or(0),
                r.cache_write.unwrap_or(0),
            )
            .to_string()
        }
        "log_git_commit" => {
            let r = deser!(args, LogGitCommitRequest);
            crate::analytics::log_git_commit(
                &r.pane_id,
                &r.project,
                &r.repo_path.unwrap_or_default(),
                &r.commit_hash,
                &r.branch.unwrap_or_default(),
                &r.message,
                r.files_changed.unwrap_or(0),
                r.insertions.unwrap_or(0),
                r.deletions.unwrap_or(0),
            )
            .to_string()
        }
        "usage_report" => {
            let r = deser!(args, UsageReportRequest);
            crate::analytics::usage_report(
                r.pane_id.as_deref(),
                r.project.as_deref(),
                r.days.unwrap_or(7),
            )
            .to_string()
        }
        "tool_ranking" => {
            let r = deser!(args, ToolRankingRequest);
            crate::analytics::tool_ranking(
                r.project.as_deref(),
                r.days.unwrap_or(7),
                r.limit.unwrap_or(20),
            )
            .to_string()
        }
        "mcp_health" => {
            let r = deser!(args, McpHealthRequest);
            crate::analytics::mcp_health(r.days.unwrap_or(7)).to_string()
        }
        "agent_activity" => {
            let r = deser!(args, AgentActivityRequest);
            crate::analytics::agent_activity(&r.pane_id, r.limit.unwrap_or(50)).to_string()
        }
        "cost_report" => {
            let r = deser!(args, CostReportRequest);
            crate::analytics::cost_report(r.project.as_deref(), r.days.unwrap_or(30)).to_string()
        }
        "trends" => {
            let r = deser!(args, TrendsRequest);
            crate::analytics::trends(
                &r.metric,
                r.project.as_deref(),
                &r.granularity.unwrap_or_else(|| "daily".into()),
                r.periods.unwrap_or(30),
            )
            .to_string()
        }

        // === QUALITY ===
        "log_test" => {
            let r = deser!(args, LogTestRequest);
            crate::quality::log_test(
                &r.pane_id,
                &r.project,
                r.command.as_deref(),
                r.success,
                r.total,
                r.passed,
                r.failed,
                r.skipped,
                r.duration_ms,
                r.output.as_deref(),
            )
            .to_string()
        }
        "log_build" => {
            let r = deser!(args, LogBuildRequest);
            crate::quality::log_build(
                &r.pane_id,
                &r.project,
                r.command.as_deref(),
                r.success,
                r.duration_ms,
                r.output.as_deref(),
            )
            .to_string()
        }
        "log_lint" => {
            let r = deser!(args, LogLintRequest);
            crate::quality::log_lint(
                &r.pane_id,
                &r.project,
                r.command.as_deref(),
                r.success,
                r.total,
                r.errors,
                r.warnings,
                r.output.as_deref(),
            )
            .to_string()
        }
        "log_deploy" => {
            let r = deser!(args, LogDeployRequest);
            crate::quality::log_deploy(
                &r.pane_id,
                &r.project,
                r.target.as_deref(),
                r.success,
                r.duration_ms,
                r.output.as_deref(),
            )
            .to_string()
        }
        "quality_report" => {
            let r = deser!(args, QualityReportRequest);
            crate::quality::quality_report(&r.project, r.days.unwrap_or(7)).to_string()
        }
        "quality_gate" => {
            let r = deser!(args, QualityGateRequest);
            crate::quality::quality_gate(&r.project).to_string()
        }
        "regressions" => {
            let r = deser!(args, RegressionsRequest);
            crate::quality::regressions(&r.project, r.days.unwrap_or(14)).to_string()
        }
        "project_health" => {
            let r = deser!(args, ProjectHealthRequest);
            crate::quality::project_health(&r.project).to_string()
        }

        // === DASHBOARD (crate::dashboard) ===
        "dash_overview" => {
            let r = deser!(args, DashOverviewRequest);
            crate::dashboard::dash_overview(r.project.as_deref()).to_string()
        }
        "dash_agent_detail" => {
            let r = deser!(args, DashAgentDetailRequest);
            crate::dashboard::dash_agent_detail(&r.pane_id).to_string()
        }
        "dash_project" => {
            let r = deser!(args, DashProjectRequest);
            crate::dashboard::dash_project(&r.project).to_string()
        }
        "dash_leaderboard" => {
            let r = deser!(args, DashLeaderboardRequest);
            crate::dashboard::dash_leaderboard(r.days.unwrap_or(7), r.project.as_deref())
                .to_string()
        }
        "dash_timeline" => {
            let r = deser!(args, DashTimelineRequest);
            crate::dashboard::dash_timeline(
                r.project.as_deref(),
                r.pane_id.as_deref(),
                r.limit.unwrap_or(50),
            )
            .to_string()
        }
        "dash_alerts" => {
            let r = deser!(args, DashAlertsRequest);
            crate::dashboard::dash_alerts(r.project.as_deref()).to_string()
        }
        "dash_daily_digest" => {
            let r = deser!(args, DashDailyDigestRequest);
            crate::dashboard::dash_daily_digest(r.project.as_deref()).to_string()
        }
        "dash_export" => {
            let r = deser!(args, DashExportRequest);
            crate::dashboard::dash_export(&r.report, r.project.as_deref(), r.days.unwrap_or(30))
                .to_string()
        }

        // === LIFECYCLE ===
        "heartbeat" => {
            let r = deser!(args, HeartbeatRequest);
            crate::multi_agent::heartbeat(&r.pane_id, r.task.as_deref(), r.status.as_deref())
                .to_string()
        }
        "session_start" => {
            let r = deser!(args, SessionStartRequest);
            crate::multi_agent::session_start(&r.pane_id, &r.project).to_string()
        }
        "session_end" => {
            let r = deser!(args, SessionEndRequest);
            crate::multi_agent::session_end(&r.session_id, &r.summary.unwrap_or_default())
                .to_string()
        }
        "who" => crate::multi_agent::who().to_string(),
        "lock_steal" => {
            let r = deser!(args, LockStealRequest);
            crate::multi_agent::lock_steal(&r.pane_id, &r.file_path, &r.reason).to_string()
        }
        "conflict_scan" => {
            let r = deser!(args, ConflictScanRequest);
            crate::multi_agent::conflict_scan(r.project.as_deref()).to_string()
        }

        // === MACHINE ===
        "machine_info" | "os_machine_info" => {
            tools::machine_info_tool(&deser!(args, MachineInfoRequest))
        }
        "machine_list" | "os_machine_list" => tools::machine_list_tool(),

        // === DATA RETENTION ===
        "prune_data" => crate::engine::retention::prune_manual().to_string(),

        // === PROJECT INTELLIGENCE ===
        "project_scan" => {
            let reg = crate::scanner::scan_all();
            let summary: Vec<Value> = reg.projects.iter().map(|p| serde_json::json!({"name": p.name, "tech": p.tech, "git_dirty": p.git_dirty})).collect();
            serde_json::json!({"count": reg.projects.len(), "projects": summary}).to_string()
        }
        "project_list" => {
            let r = deser!(args, ProjectListRequest);
            let reg = crate::scanner::load_registry();
            let projects: Vec<Value> = reg
                .projects
                .iter()
                .filter(|p| {
                    r.tech
                        .as_ref()
                        .is_none_or(|t| p.tech.iter().any(|pt| pt.contains(t)))
                })
                .map(|p| serde_json::json!({"name": p.name, "path": p.path, "tech": p.tech}))
                .collect();
            serde_json::json!({"count": projects.len(), "projects": projects}).to_string()
        }
        "project_detail" => {
            let r = deser!(args, ProjectDetailRequest);
            crate::scanner::project_detail(&r.project).to_string()
        }
        "project_test" => {
            let r = deser!(args, ProjectTestRequest);
            match crate::scanner::project_by_name(&r.project) {
                Some(info) => match crate::engine::health::run_tests(&info).await {
                    Some(result) => serde_json::json!({"project": info.name, "success": result.success, "total": result.total, "passed": result.passed, "failed": result.failed}).to_string(),
                    None => r#"{"error":"No test command available"}"#.to_string(),
                },
                None => format!("{{\"error\":\"Project '{}' not found\"}}", r.project),
            }
        }
        "project_deps" => {
            let r = deser!(args, ProjectDepsRequest);
            let reg = crate::scanner::load_registry();
            if let Some(name) = r.project {
                if let Some(p) = reg
                    .projects
                    .iter()
                    .find(|p| p.name.to_lowercase() == name.to_lowercase())
                {
                    serde_json::json!({"project": p.name, "depends_on": p.deps}).to_string()
                } else {
                    format!("{{\"error\":\"Project '{}' not found\"}}", name)
                }
            } else {
                let graph: Vec<Value> = reg
                    .projects
                    .iter()
                    .filter(|p| !p.deps.is_empty())
                    .map(|p| serde_json::json!({"project": p.name, "depends_on": p.deps}))
                    .collect();
                serde_json::json!({"dependencies": graph}).to_string()
            }
        }

        // === AUDIT ===
        "audit_code" => {
            let r = deser!(args, AuditCodeRequest);
            crate::audit::audit_code(&r.project).to_string()
        }
        "audit_security" => {
            let r = deser!(args, AuditSecurityRequest);
            crate::audit::audit_security(&r.project).to_string()
        }
        "audit_intent" => {
            let r = deser!(args, AuditIntentRequest);
            crate::audit::audit_intent(&r.project, r.description.as_deref().unwrap_or(""))
                .to_string()
        }
        "audit_deps" => {
            let r = deser!(args, AuditDepsRequest);
            crate::audit::audit_deps(&r.project).to_string()
        }
        "audit_full" => {
            let r = deser!(args, AuditFullRequest);
            crate::audit::audit_full(&r.project).to_string()
        }

        // === FACTORY ===
        "factory" | "factory_run" | "factory_go" | "go" | "work" => {
            tools::factory_tools::factory_run(app, deser!(args, FactoryRequest)).await
        }
        "factory_status" | "pipeline" | "pipe" => {
            tools::factory_tools::factory_status(&deser!(args, FactoryStatusRequest))
        }
        "factory_list" | "pipelines" => tools::factory_tools::factory_list(),
        "factory_gate" | "gate" => {
            tools::factory_tools::factory_gate(&deser!(args, FactoryStatusRequest))
        }
        "pipeline_conflicts" => {
            tools::factory_tools::conflict_scan(&deser!(args, FactoryStatusRequest))
        }
        "factory_cancel" | "pipeline_cancel" | "cancel_pipeline" => {
            tools::factory_tools::factory_cancel(app, &deser!(args, FactoryStatusRequest)).await
        }
        "factory_detect" | "detect_project" => {
            tools::factory_tools::factory_detect(&deser!(args, FactoryDetectRequest))
        }
        "factory_gate_result" | "gate_result" => {
            tools::factory_tools::factory_gate_result(&deser!(args, FactoryStatusRequest))
        }
        "factory_retry" | "pipeline_retry" | "retry_pipeline" => {
            tools::factory_tools::factory_retry(&deser!(args, FactoryStatusRequest))
        }
        "factory_events" | "pipeline_events" => {
            tools::factory_tools::factory_events(&deser!(args, FactoryStatusRequest))
        }
        "factory_pause" | "pipeline_pause" | "pause" => {
            tools::factory_tools::factory_pause(&deser!(args, FactoryStatusRequest))
        }
        "factory_resume" | "pipeline_resume" | "resume" => {
            tools::factory_tools::factory_resume(&deser!(args, FactoryStatusRequest))
        }
        "factory_retry_stage" | "retry_stage" => {
            tools::factory_tools::factory_retry_stage(&deser!(args, FactoryRetryStageRequest))
        }

        // === ORCHESTRATION ===
        "orchestrate" => {
            tools::orchestrate::orchestrate(app, deser!(args, OrchestrateRequest)).await
        }

        // === GATEWAY (MICRO MCP) ===
        "mcp_discover" | "gateway_discover" => {
            tools::gateway_tools::gateway_discover(app, deser!(args, GatewayDiscoverRequest)).await
        }
        "mcp_call" | "gateway_call" => {
            tools::gateway_tools::gateway_call(app, deser!(args, GatewayCallRequest)).await
        }
        "mcp_gateway_list" | "gateway_list" => {
            tools::gateway_tools::gateway_list(app, deser!(args, GatewayListRequest)).await
        }
        "mcp_gateway_tools" | "gateway_tools" => {
            tools::gateway_tools::gateway_tools(app, deser!(args, GatewayToolsRequest)).await
        }

        _ => format!("{{\"error\":\"Unknown tool: {}\"}}", tool),
    }
}

/// All tool names with short descriptions for autocomplete
pub const MCP_TOOLS: &[(&str, &str)] = &[
    // Pane lifecycle
    ("spawn", "Spawn agent in pane"),
    ("kill", "Stop agent"),
    ("restart", "Kill and re-spawn"),
    ("reassign", "Update task/project/role"),
    ("assign", "Assign tracker issue to pane"),
    ("assign_adhoc", "Assign ad-hoc task"),
    ("collect", "Capture agent output"),
    ("complete", "Mark task complete"),
    // Config
    ("set_mcps", "Set project MCPs"),
    ("set_preamble", "Write agent preamble"),
    ("config_show", "Show pane config"),
    // Monitoring
    ("status", "All pane status"),
    ("dashboard", "Rich dashboard"),
    ("logs", "Activity log"),
    ("health", "Health check"),
    ("monitor", "Full overview"),
    ("project_status", "Project deep-dive"),
    ("digest", "Daily/weekly digest"),
    ("watch", "Live pane output"),
    // MCP routing
    ("mcp_list", "List available MCPs"),
    ("mcp_route", "Auto-route MCPs for task"),
    ("mcp_search", "Search MCPs"),
    // Git
    ("git_sync", "Sync worktree from base"),
    ("git_status", "Git status/diff"),
    ("git_push", "Commit and push"),
    ("git_pr", "Create pull request"),
    ("git_merge", "Merge branch to base"),
    // Queue
    ("queue_add", "Add task to queue"),
    ("queue_decompose", "Decompose goal to tasks"),
    ("queue_list", "List queued tasks"),
    ("queue_done", "Mark task done"),
    ("auto", "Run auto-cycle"),
    ("auto_config", "Configure auto-cycle"),
    ("queue_cancel", "Cancel a queue task"),
    ("queue_retry", "Retry a failed task"),
    ("queue_clear", "Clear done/failed tasks"),
    // Ports
    ("port_allocate", "Allocate a port"),
    ("port_release", "Release a port"),
    ("port_list", "List allocated ports"),
    ("port_get", "Get port for service"),
    // Agents
    ("agent_register", "Register agent"),
    ("agent_update", "Update agent task"),
    ("agent_list", "List agents"),
    ("agent_deregister", "Deregister agent"),
    // Locks
    ("lock_acquire", "Acquire file locks"),
    ("lock_release", "Release file locks"),
    ("lock_check", "Check lock status"),
    ("lock_steal", "Force-steal a lock"),
    // Git branches
    ("git_claim_branch", "Claim branch"),
    ("git_release_branch", "Release branch"),
    ("git_list_branches", "List claimed branches"),
    ("git_pre_commit_check", "Pre-commit conflict check"),
    // Builds
    ("build_claim", "Claim build access"),
    ("build_release", "Release build"),
    ("build_status", "Check build status"),
    ("build_get_last", "Last build result"),
    // Inter-agent tasks
    ("ma_task_add", "Add inter-agent task"),
    ("ma_task_claim", "Claim next task"),
    ("ma_task_complete", "Complete task"),
    ("ma_task_list", "List tasks"),
    // KB
    ("kb_add", "Add knowledge"),
    ("kb_search", "Search knowledge base"),
    ("kb_list", "List KB entries"),
    // Messaging
    ("msg_broadcast", "Broadcast message"),
    ("msg_send", "Send direct message"),
    ("msg_get", "Get messages"),
    // Signals
    (
        "signal",
        "Send agent signal (need_help/blocked/found_issue)",
    ),
    ("signals", "List unack'd agent signals"),
    ("signal_ack", "Acknowledge signal"),
    // Multi-agent misc
    ("cleanup_all", "Clean stale entries"),
    ("status_overview", "Full status overview"),
    ("who", "List active agents"),
    ("conflict_scan", "Detect concurrent edits"),
    // Tracker
    ("issue_create", "Create issue"),
    ("issue_update", "Update issue"),
    ("issue_list", "List issues"),
    ("issue_view", "View issue details"),
    ("issue_comment", "Comment on issue"),
    ("issue_link", "Link to issue"),
    ("issue_close", "Close issue"),
    ("issue_children", "List child issues"),
    ("milestone_create", "Create milestone"),
    ("milestone_list", "List milestones"),
    ("timeline_generate", "Generate Gantt"),
    ("board_view", "Kanban board"),
    ("feature_decompose", "Decompose feature"),
    ("feature_to_queue", "Push to queue"),
    ("feature_status", "Feature progress"),
    // Processes
    ("process_start", "Start process"),
    ("process_update", "Update process step"),
    ("process_list", "List processes"),
    ("process_template_create", "Create template"),
    // Capacity
    ("cap_configure", "Configure capacity"),
    ("cap_estimate", "Estimate ACU"),
    ("cap_log_work", "Log work done"),
    ("cap_plan_sprint", "Plan sprint"),
    ("cap_dashboard", "Capacity dashboard"),
    ("cap_burndown", "Sprint burndown"),
    ("cap_velocity", "Sprint velocity"),
    ("cap_roles", "List roles"),
    // Collab
    ("space_list", "List spaces"),
    ("space_create", "Create space"),
    ("doc_list", "List documents"),
    ("doc_read", "Read document"),
    ("doc_create", "Create document"),
    ("doc_edit", "Edit document"),
    ("doc_propose", "Propose changes"),
    ("doc_approve", "Approve proposal"),
    ("doc_reject", "Reject proposal"),
    ("doc_lock", "Lock document"),
    ("doc_unlock", "Unlock document"),
    ("doc_comment", "Comment on doc"),
    ("doc_comments", "Read comments"),
    ("doc_status", "Update doc status"),
    ("doc_search", "Search documents"),
    ("doc_directives", "Find directives"),
    ("doc_history", "Doc version history"),
    ("doc_delete", "Delete document"),
    ("collab_init", "Init collab workspace"),
    // Knowledge graph
    ("kgraph_add_entity", "Add entity"),
    ("kgraph_add_edge", "Add edge"),
    ("kgraph_observe", "Record observation"),
    ("kgraph_query_neighbors", "Query neighbors"),
    ("kgraph_query_path", "Find path"),
    ("kgraph_search", "Search entities"),
    ("kgraph_delete", "Delete entity/edge"),
    ("kgraph_stats", "Graph statistics"),
    // Session replay
    ("replay_index", "Index sessions"),
    ("replay_search", "Search sessions"),
    ("replay_session", "View session"),
    ("replay_list_sessions", "List sessions"),
    ("replay_tool_history", "Tool usage history"),
    ("replay_errors", "Recent errors"),
    ("replay_status", "Replay status"),
    // TruthGuard
    ("fact_add", "Add fact"),
    ("fact_get", "Get fact"),
    ("fact_search", "Search facts"),
    ("fact_check", "Check claim"),
    ("fact_check_response", "Check response"),
    ("fact_update", "Update fact"),
    ("fact_delete", "Delete fact"),
    ("truthguard_status", "TruthGuard status"),
    // Analytics
    ("log_tool_call", "Log tool call"),
    ("log_file_op", "Log file operation"),
    ("log_tokens", "Log token usage"),
    ("log_git_commit", "Log git commit"),
    ("usage_report", "Usage report"),
    ("tool_ranking", "Tool rankings"),
    ("mcp_health", "MCP error rates"),
    ("agent_activity", "Agent activity"),
    ("cost_report", "Token cost report"),
    ("trends", "Time-series metrics"),
    // Quality
    ("log_test", "Log test results"),
    ("log_build", "Log build result"),
    ("log_lint", "Log lint results"),
    ("log_deploy", "Log deployment"),
    ("quality_report", "Quality report"),
    ("quality_gate", "Quality gate check"),
    ("regressions", "Detect regressions"),
    ("project_health", "Project health score"),
    // Dashboard
    ("dash_overview", "God view"),
    ("dash_agent_detail", "Agent deep-dive"),
    ("dash_project", "Project view"),
    ("dash_leaderboard", "Agent leaderboard"),
    ("dash_timeline", "Event stream"),
    ("dash_alerts", "Active alerts"),
    ("dash_daily_digest", "24h summary"),
    ("dash_export", "Export data"),
    // Lifecycle
    ("heartbeat", "Send heartbeat"),
    ("session_start", "Start session"),
    ("session_end", "End session"),
    // Machine
    ("machine_info", "Machine identity"),
    ("machine_list", "List machines"),
    // Data
    ("prune_data", "Prune old data"),
    // Projects
    ("project_scan", "Scan for projects"),
    ("project_list", "List projects"),
    ("project_detail", "Project details"),
    ("project_test", "Run project tests"),
    ("project_deps", "Dependency graph"),
    // Audit
    ("audit_code", "Code quality audit"),
    ("audit_security", "Security audit"),
    ("audit_intent", "Intent verification"),
    ("audit_deps", "Dependency audit"),
    ("audit_full", "Full audit"),
    // Factory
    ("go", "Factory: NL → dev+qa+sec pipeline"),
    ("factory", "Factory: NL → pipeline"),
    ("pipeline", "Pipeline status"),
    ("pipelines", "List all pipelines"),
    ("gate", "Run quality gate on pipeline"),
    ("pipeline_conflicts", "Scan pipeline file conflicts"),
    ("factory_cancel", "Cancel a pipeline"),
    ("factory_pause", "Pause a pipeline"),
    ("factory_resume", "Resume a paused pipeline"),
    ("factory_retry_stage", "Retry a specific stage"),
    ("factory_detect", "Detect project from text"),
    ("gate_result", "View saved gate results"),
    // Orchestration
    ("orchestrate", "Auto-build: NL → agents"),
    // Gateway (micro MCPs)
    ("mcp_discover", "Find micro MCPs"),
    ("mcp_call", "Call micro MCP tool"),
    ("mcp_gateway_list", "List micro MCPs"),
    ("mcp_gateway_tools", "Inspect MCP tool schemas"),
];

/// Get completions for a prefix
pub fn completions_for(prefix: &str) -> Vec<(&'static str, &'static str)> {
    if prefix.is_empty() {
        return Vec::new();
    }
    let lower = prefix.to_lowercase();
    MCP_TOOLS
        .iter()
        .filter(|(name, _)| name.starts_with(&lower))
        .take(8)
        .copied()
        .collect()
}

pub mod api;
pub mod replicator;
pub mod sse;
pub mod ws;

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use crate::app::App;

/// Build the axum router with all API endpoints
pub fn build_router(app: Arc<App>) -> Router {
    Router::new()
        // Dashboard
        .route("/", get(api::index))
        // DX Terminal state endpoints (real-time from memory + PTY)
        .route("/api/status", get(api::get_status))
        .route("/api/pane/{id}", get(api::get_pane))
        .route("/api/pane/{id}/output", get(api::get_pane_output))
        .route("/api/pane/{id}/context", get(api::get_pane_context))
        .route("/api/health", get(api::get_health))
        .route("/api/logs", get(api::get_logs))
        // Backward-compatible hub_mcp endpoints
        .route("/api/spaces", get(api::get_spaces))
        .route("/api/agents", get(api::get_agents))
        .route("/api/capacity/dashboard", get(api::get_capacity_dashboard))
        .route("/api/board", get(api::get_board))
        .route("/api/issues", get(api::get_issues))
        .route("/api/sprints", get(api::get_sprints))
        .route("/api/burndown", get(api::get_burndown))
        .route("/api/roles", get(api::get_roles))
        .route("/api/mcps", get(api::get_mcps))
        .route("/api/mcps/route", get(api::get_mcp_route))
        .route("/api/gateway/list", get(api::get_gateway_list))
        .route("/api/gateway/tools", get(api::get_gateway_tools))
        .route("/api/gateway/call", post(api::post_gateway_call))
        .route("/api/queue", get(api::get_queue))
        .route("/api/queue/add", post(api::post_queue_add))
        .route("/api/queue/done", post(api::post_queue_done))
        .route("/api/queue/delete", post(api::post_queue_delete))
        .route("/api/queue/retry", post(api::post_queue_retry))
        // Enhanced monitoring endpoints
        .route("/api/monitor", get(api::get_monitor))
        .route("/api/pane/{id}/watch", get(api::get_watch))
        // Analytics endpoints (FORGE data for TUI)
        .route("/api/analytics/digest", get(api::get_analytics_digest))
        .route("/api/analytics/alerts", get(api::get_analytics_alerts))
        .route("/api/analytics/quality", get(api::get_analytics_quality))
        .route(
            "/api/analytics/leaderboard",
            get(api::get_analytics_leaderboard),
        )
        .route("/api/analytics/overview", get(api::get_analytics_overview))
        // Build environments
        .route("/api/builds", get(api::get_builds))
        .route("/api/builds/create", post(api::post_build_create))
        .route("/api/builds/restyle", post(api::post_build_restyle))
        .route("/api/builds/send", post(api::post_build_send))
        .route("/api/builds/rename", post(api::post_build_rename))
        .route("/api/project/brief", get(api::get_project_brief))
        // Vision
        .route("/api/vision", get(api::get_vision))
        .route("/api/vision/summary", get(api::get_vision_summary))
        .route("/api/vision/diff", get(api::get_vision_diff))
        .route("/api/vision/list", get(api::list_visions))
        .route("/api/vision/init", post(api::init_vision))
        .route("/api/vision/sync", post(api::sync_vision))
        // VDD: Vision-Driven Development
        .route("/api/vision/tree", get(api::get_vision_tree))
        .route("/api/vision/drill", get(api::get_vision_drill))
        .route(
            "/api/vision/feature/readiness",
            get(api::get_vision_feature_readiness),
        )
        .route(
            "/api/vision/discovery/readiness",
            get(api::get_vision_discovery_readiness),
        )
        .route(
            "/api/vision/discovery/start",
            post(api::start_vision_discovery),
        )
        .route(
            "/api/vision/discovery/complete",
            post(api::complete_vision_discovery),
        )
        .route("/api/vision/feature", post(api::add_vision_feature))
        .route("/api/vision/acceptance", post(api::add_vision_acceptance))
        .route(
            "/api/vision/acceptance/update",
            post(api::update_vision_acceptance),
        )
        .route(
            "/api/vision/acceptance/verify",
            post(api::verify_vision_acceptance),
        )
        .route("/api/vision/question", post(api::add_vision_question))
        .route("/api/vision/answer", post(api::answer_vision_question))
        .route("/api/vision/task", post(api::add_vision_task))
        .route("/api/vision/task/status", post(api::update_vision_task))
        .route(
            "/api/vision/feature/status",
            post(api::update_vision_feature_status),
        )
        .route("/api/vision/git-sync", post(api::git_sync_vision))
        .route("/api/vision/work", post(api::assess_vision_work))
        // VDD Research & Discovery Docs
        .route("/api/vision/docs", get(api::list_vision_docs))
        .route(
            "/api/vision/focus",
            get(api::get_vision_focus).post(api::set_vision_focus),
        )
        .route(
            "/api/vision/doc",
            get(api::get_vision_doc).post(api::upsert_vision_doc),
        )
        .route(
            "/api/vision/design/mockups/seed",
            post(api::seed_vision_mockups),
        )
        .route("/api/vision/design/review", post(api::review_vision_design))
        .route("/vision/mockup", get(api::get_vision_mockup))
        .route("/api/vision/notify", post(api::notify_vision_change))
        // Confluence-style Wiki page
        .route("/wiki", get(api::wiki_page))
        // UI/UX Audit
        .route("/api/audit/ui", get(api::get_audit_ui))
        .route("/api/audit/ux", get(api::get_audit_ux))
        .route("/api/audit/frontend", get(api::get_audit_frontend))
        .route("/api/design-tokens", get(api::get_design_tokens))
        .route("/api/contrast", get(api::get_contrast))
        // Sync status
        .route("/api/sync", get(api::get_sync_status))
        // SSE events
        .route("/api/events", get(sse::event_stream))
        // WebSocket — real-time bidirectional
        .route("/ws", get(ws::ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(app)
}

/// Start the web server
pub async fn run_web_server(app: Arc<App>, port: u16) -> anyhow::Result<()> {
    // Start the RuntimeReplicator — single server-side polling task
    replicator::start(Arc::clone(&app));

    let router = build_router(app);
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    tracing::info!("DX Terminal web dashboard: http://localhost:{}", port);
    eprintln!("DX Terminal web dashboard: http://localhost:{}", port);
    axum::serve(listener, router).await?;
    Ok(())
}

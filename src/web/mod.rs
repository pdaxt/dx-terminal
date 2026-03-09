pub mod api;
pub mod sse;

use std::sync::Arc;
use axum::{
    Router,
    routing::{get, post},
};
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
        .route("/api/analytics/leaderboard", get(api::get_analytics_leaderboard))
        .route("/api/analytics/overview", get(api::get_analytics_overview))
        // SSE events
        .route("/api/events", get(sse::event_stream))
        .layer(CorsLayer::permissive())
        .with_state(app)
}

/// Start the web server
pub async fn run_web_server(app: Arc<App>, port: u16) -> anyhow::Result<()> {
    let router = build_router(app);
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    tracing::info!("DX Terminal web dashboard: http://localhost:{}", port);
    eprintln!("DX Terminal web dashboard: http://localhost:{}", port);
    axum::serve(listener, router).await?;
    Ok(())
}

pub mod health;
pub mod reaper;
pub mod reconcile;
pub mod retention;

/// Spawn background maintenance tasks alongside the MCP server.
/// Uses coordination_db() per-call (no shared Db struct needed).
pub async fn start_background_tasks(state: Option<std::sync::Arc<crate::state::StateManager>>) {
    // Startup reconciliation: clear stale "active" panes
    if let Some(ref s) = state {
        reconcile::reconcile_on_startup(s).await;
    }

    // Tmux reconciler: check active agents every 10s
    if let Some(s) = state.clone() {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                reconcile::reconcile_active_panes(&s).await;
            }
        });
    }

    // Reaper: detect dead agents every 120s
    tokio::spawn(async {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(120)).await;
            if let Err(e) = reaper::reap_dead_agents() {
                tracing::warn!("Reaper error: {e}");
            }
        }
    });

    // Lock expiry: clean expired locks every 60s
    tokio::spawn(async {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            if let Err(e) = reaper::expire_locks() {
                tracing::warn!("Lock expiry error: {e}");
            }
        }
    });

    // Retention: prune old data every 6 hours
    tokio::spawn(async {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(6 * 3600)).await;
            if let Err(e) = retention::prune() {
                tracing::warn!("Retention error: {e}");
            }
        }
    });

    // Project scanner: discover repos every 5 minutes
    tokio::spawn(async {
        // Initial scan at startup
        let reg = crate::scanner::scan_all();
        tracing::info!("Project scanner: discovered {} repos", reg.projects.len());
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(300)).await;
            let reg = crate::scanner::scan_all();
            tracing::info!("Project scanner: {} repos", reg.projects.len());
        }
    });

    // Health monitor: run tests/builds every 15 minutes for changed projects
    tokio::spawn(async {
        // Delay to let scanner populate registry first
        tokio::time::sleep(std::time::Duration::from_secs(120)).await;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(15 * 60)).await;
            health::health_cycle().await;
        }
    });

    // Audit cycle: run audit_full on registered projects every 30 minutes
    tokio::spawn(async {
        // Delay to let scanner populate registry first
        tokio::time::sleep(std::time::Duration::from_secs(180)).await;
        loop {
            audit_cycle().await;
            tokio::time::sleep(std::time::Duration::from_secs(30 * 60)).await;
        }
    });
}

/// Run audit_full on registered projects (max 5 per cycle).
async fn audit_cycle() {
    let reg = crate::scanner::load_registry();
    let mut audited = 0;

    for proj in &reg.projects {
        if audited >= 5 {
            break;
        }

        // Run audit in a blocking task since it does filesystem I/O
        let path = proj.path.clone();
        let name = proj.name.clone();
        let result = tokio::task::spawn_blocking(move || crate::audit::audit_full(&path)).await;

        match result {
            Ok(report) => {
                let grade = report.get("grade").and_then(|v| v.as_str()).unwrap_or("?");
                let total = report
                    .get("total_findings")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                tracing::info!("Audit cycle: {} grade={} findings={}", name, grade, total);
            }
            Err(e) => {
                tracing::warn!("Audit cycle: {} failed: {}", name, e);
            }
        }
        audited += 1;
    }
}

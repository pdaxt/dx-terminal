//! State reconciliation: ensure persisted state matches actual tmux/process reality.
//!
//! On startup: any pane marked "active" that has no running tmux window gets reset to "idle".
//! Periodically: check active panes for completion/error, update state with live data.

use crate::config;
use crate::state::StateManager;
use crate::tmux;
use std::sync::Arc;

/// Run once at startup: clear stale "active" states that have no backing process.
pub async fn reconcile_on_startup(state: &Arc<StateManager>) {
    let snap = state.get_state_snapshot().await;
    let mut reconciled = 0;

    for (key, pane) in &snap.panes {
        if pane.status != "active" && pane.status != "error" {
            continue;
        }

        let is_alive = if let Some(ref target) = pane.tmux_target {
            // Check if tmux target actually exists
            tmux::pane_exists(target)
        } else {
            // No tmux target — definitely stale
            false
        };

        if !is_alive {
            if let Ok(pane_num) = key.parse::<u8>() {
                tracing::info!(
                    "Reconcile: pane {} ({}) was '{}' but has no running agent — resetting to idle",
                    pane_num, pane.project, pane.status
                );
                let mut reset = pane.clone();
                reset.status = "idle".into();
                reset.task = String::new();
                reset.project = "--".into();
                reset.project_path = String::new();
                reset.role = String::new();
                reset.started_at = None;
                reset.tmux_target = None;
                reset.machine_ip = None;
                reset.machine_hostname = None;
                reset.machine_mac = None;
                state.set_pane(pane_num, reset).await;
                state.log_activity(pane_num, "reconcile", "Cleared stale active state on startup").await;
                reconciled += 1;
            }
        }
    }

    if reconciled > 0 {
        tracing::info!("Reconciled {} stale panes on startup", reconciled);
    }
}

/// Periodic reconciler: check all active tmux agents for done/error/progress.
/// Called every 10s by the engine. Updates state with live data from tmux.
pub async fn reconcile_active_panes(state: &Arc<StateManager>) {
    for i in 1..=config::pane_count() {
        let pd = state.get_pane(i).await;
        if pd.status != "active" {
            continue;
        }

        let target = match &pd.tmux_target {
            Some(t) => t.clone(),
            None => continue,
        };

        // Verify the tmux pane still exists
        if !tmux::pane_exists(&target) {
            tracing::warn!("Reconciler: pane {} tmux target {} no longer exists — marking lost", i, target);
            state.update_pane_status(i, "lost").await;
            state.event_bus.send(crate::state::events::StateEvent::PaneRemoved {
                pane: i,
                reason: format!("tmux pane {} disappeared without clean completion", target),
            });
            state.log_activity(i, "auto_lost", &format!("Tmux pane disappeared (no clean exit): {}", &pd.task)).await;
            continue;
        }

        // Capture current output
        let output = tmux::capture_output(&target);

        // Check if agent finished (shell prompt visible = claude exited)
        if tmux::check_done(&target) {
            tracing::info!("Reconciler: pane {} agent finished (shell prompt detected)", i);
            state.update_pane_status(i, "done").await;
            state.log_activity(i, "auto_done", &format!("Agent finished: {}", &pd.task)).await;
            continue;
        }

        // Check for errors (but only if not done)
        if let Some(error) = tmux::check_error(&target) {
            // Only flag as error if it's a fatal pattern, not just output containing "Error:"
            let fatal_patterns = ["rate limit", "hit your limit", "SIGTERM", "panic:", "FATAL:"];
            if fatal_patterns.iter().any(|p| output.contains(p)) {
                tracing::warn!("Reconciler: pane {} has fatal error: {}", i, error);
                state.update_pane_status(i, "error").await;
                state.log_activity(i, "auto_error", &format!("Fatal error: {}", error)).await;
            }
        }
    }
}

pub mod events;
pub mod persistence;
pub mod types;

use chrono::Local;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use self::events::{EventBus, StateEvent};
use self::persistence::{load_state, save_state};
use self::types::{DxTerminalState, LogEntry, PaneState};
use crate::config;

pub struct StateManager {
    state: Arc<RwLock<DxTerminalState>>,
    state_file: PathBuf,
    pub event_bus: Arc<EventBus>,
}

impl Default for StateManager {
    fn default() -> Self {
        Self::new()
    }
}

impl StateManager {
    pub fn new() -> Self {
        let state_file = config::state_file();
        let state = load_state(&state_file);
        Self {
            state: Arc::new(RwLock::new(state)),
            state_file,
            event_bus: Arc::new(EventBus::new(256)),
        }
    }

    pub async fn get_pane(&self, pane: u8) -> PaneState {
        let state = self.state.read().await;
        state
            .panes
            .get(&pane.to_string())
            .cloned()
            .unwrap_or_default()
    }

    pub async fn set_pane(&self, pane: u8, pane_state: PaneState) {
        let mut state = self.state.write().await;
        state.panes.insert(pane.to_string(), pane_state.clone());
        let _ = save_state(&self.state_file, &state);
        // Authoritative pane upsert — clients replace their view of this pane
        self.event_bus.send(StateEvent::PaneUpsert {
            pane,
            data: serde_json::to_value(&pane_state).unwrap_or_default(),
        });
    }

    pub async fn update_pane_status(&self, pane: u8, status: &str) {
        let mut state = self.state.write().await;
        if let Some(ps) = state.panes.get_mut(&pane.to_string()) {
            ps.status = status.to_string();
        }
        let _ = save_state(&self.state_file, &state);
        self.event_bus.send(StateEvent::PaneStatusChanged {
            pane,
            status: status.to_string(),
        });
    }

    pub async fn log_activity(&self, pane: u8, event: &str, summary: &str) {
        let mut state = self.state.write().await;
        let entry = LogEntry {
            ts: now(),
            pane,
            event: event.to_string(),
            summary: summary.to_string(),
        };
        state.activity_log.push_front(entry);
        while state.activity_log.len() > 100 {
            state.activity_log.pop_back();
        }
        let _ = save_state(&self.state_file, &state);
        self.event_bus.send(StateEvent::LogAppended {
            pane,
            event: event.to_string(),
            summary: summary.to_string(),
        });
    }

    pub async fn get_state_snapshot(&self) -> DxTerminalState {
        self.state.read().await.clone()
    }

    /// Blocking read for non-async contexts (TUI thread)
    pub fn blocking_read(&self) -> tokio::sync::RwLockReadGuard<'_, DxTerminalState> {
        self.state.blocking_read()
    }

    pub async fn get_project_mcps(&self, project: &str) -> Vec<String> {
        let state = self.state.read().await;
        state.project_mcps.get(project).cloned().unwrap_or_default()
    }

    pub async fn set_project_mcps(&self, project: &str, mcps: Vec<String>) {
        let mut state = self.state.write().await;
        state.project_mcps.insert(project.to_string(), mcps);
        let _ = save_state(&self.state_file, &state);
    }

    pub async fn get_space_project_path(&self, space: &str) -> Option<String> {
        let state = self.state.read().await;
        state.space_project_map.get(space).cloned()
    }

    /// Update health status for a pane (called by health monitor every 2s).
    /// Only persists if health actually changed to avoid disk thrashing.
    pub async fn update_pane_health(
        &self,
        pane: u8,
        health: types::PaneHealthStatus,
        output_hash: u64,
    ) {
        let mut state = self.state.write().await;
        let key = pane.to_string();
        if let Some(ps) = state.panes.get_mut(&key) {
            let health_changed = ps.health != health;
            let output_changed = ps.last_output_hash != output_hash;

            if output_changed {
                ps.last_output_hash = output_hash;
                ps.last_output_changed_at = Some(now());
            }

            if health_changed {
                let old = ps.health;
                ps.health = health;
                let _ = save_state(&self.state_file, &state);
                drop(state);
                self.event_bus.send(events::StateEvent::PaneHealthChanged {
                    pane,
                    old_health: old,
                    new_health: health,
                });
            }
        }
    }

    /// Get health status for all panes (for MCP tool / dashboard).
    pub async fn get_all_health(&self) -> Vec<(u8, types::PaneHealthStatus, Option<String>)> {
        let state = self.state.read().await;
        let mut result = Vec::new();
        for (key, ps) in &state.panes {
            if let Ok(pane) = key.parse::<u8>() {
                result.push((pane, ps.health, ps.last_output_changed_at.clone()));
            }
        }
        result.sort_by_key(|(p, _, _)| *p);
        result
    }
}

pub fn now() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

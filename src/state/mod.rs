pub mod types;
pub mod persistence;
pub mod events;

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::Local;

use self::types::{AgentOSState, LogEntry, PaneState};
use self::persistence::{load_state, save_state};
use self::events::{EventBus, StateEvent};
use crate::config;

pub struct StateManager {
    state: Arc<RwLock<AgentOSState>>,
    state_file: PathBuf,
    pub event_bus: Arc<EventBus>,
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

    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, AgentOSState> {
        self.state.read().await
    }

    pub async fn get_pane(&self, pane: u8) -> PaneState {
        let state = self.state.read().await;
        state.panes.get(&pane.to_string()).cloned().unwrap_or_default()
    }

    pub async fn set_pane(&self, pane: u8, pane_state: PaneState) {
        let mut state = self.state.write().await;
        state.panes.insert(pane.to_string(), pane_state);
        let _ = save_state(&self.state_file, &state);
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

    pub async fn get_state_snapshot(&self) -> AgentOSState {
        self.state.read().await.clone()
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

    pub async fn flush(&self) {
        let state = self.state.read().await;
        let _ = save_state(&self.state_file, &state);
    }
}

pub fn now() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

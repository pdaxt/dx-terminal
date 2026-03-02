use std::sync::{Arc, Mutex, MutexGuard};
use crate::state::StateManager;
use crate::pty::PtyManager;
use crate::config;
use agentos_gateway::MCPRegistry;

pub struct App {
    pub state: Arc<StateManager>,
    pub pty: Arc<Mutex<PtyManager>>,
    pub gateway: Arc<tokio::sync::Mutex<MCPRegistry>>,
}

impl App {
    pub fn new() -> Self {
        let descriptors_dir = config::agentos_root().join("mcps");
        Self {
            state: Arc::new(StateManager::new()),
            pty: Arc::new(Mutex::new(PtyManager::new())),
            gateway: Arc::new(tokio::sync::Mutex::new(MCPRegistry::new(descriptors_dir))),
        }
    }

    /// Poison-safe PTY lock — recovers from panicked threads instead of cascading
    pub fn pty_lock(&self) -> MutexGuard<'_, PtyManager> {
        self.pty.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("PTY mutex was poisoned, recovering");
            poisoned.into_inner()
        })
    }
}

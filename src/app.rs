use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use crate::state::StateManager;
use crate::pty::PtyManager;
use crate::screen::ScreenManager;
use crate::config;
use dx_gateway::MCPRegistry;

pub struct App {
    pub state: Arc<StateManager>,
    pub pty: Arc<Mutex<PtyManager>>,
    pub gateway: Arc<tokio::sync::Mutex<MCPRegistry>>,
    pub screens: Arc<RwLock<ScreenManager>>,
}

impl App {
    pub fn new() -> Self {
        let descriptors_dir = config::dx_root().join("mcps");
        let screen_mgr = ScreenManager::new(config::dx_root());
        screen_mgr.init_default(&config::session_name());
        Self {
            state: Arc::new(StateManager::new()),
            pty: Arc::new(Mutex::new(PtyManager::new())),
            gateway: Arc::new(tokio::sync::Mutex::new(MCPRegistry::new(descriptors_dir))),
            screens: Arc::new(RwLock::new(screen_mgr)),
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

use std::sync::{Arc, Mutex, MutexGuard};
use crate::state::StateManager;
use crate::pty::PtyManager;

pub struct App {
    pub state: Arc<StateManager>,
    pub pty: Arc<Mutex<PtyManager>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: Arc::new(StateManager::new()),
            pty: Arc::new(Mutex::new(PtyManager::new())),
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

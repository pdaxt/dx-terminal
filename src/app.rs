use std::sync::Arc;
use crate::state::StateManager;

pub struct App {
    pub state: Arc<StateManager>,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: Arc::new(StateManager::new()),
        }
    }
}

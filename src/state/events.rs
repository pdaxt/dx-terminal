use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub enum StateEvent {
    PaneSpawned { pane: u8, project: String, role: String },
    PaneKilled { pane: u8, reason: String },
    PaneStatusChanged { pane: u8, status: String },
    LogAppended { pane: u8, event: String, summary: String },
    QueueChanged { action: String, task_id: String, task: String },
    StateRefreshed,
}

pub struct EventBus {
    tx: broadcast::Sender<StateEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    pub fn send(&self, event: StateEvent) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<StateEvent> {
        self.tx.subscribe()
    }
}

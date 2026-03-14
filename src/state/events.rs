use serde::Serialize;
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::broadcast;

/// Monotonic sequence counter for all events.
/// Clients detect gaps and request full resync.
static GLOBAL_SEQ: AtomicU64 = AtomicU64::new(1);

pub fn next_seq() -> u64 {
    GLOBAL_SEQ.fetch_add(1, Ordering::Relaxed)
}

/// Sequenced envelope — every WS message gets a monotonic seq.
#[derive(Debug, Clone, Serialize)]
pub struct SeqEnvelope {
    pub seq: u64,
    #[serde(flatten)]
    pub event: StateEvent,
}

/// All event types that flow through the event bus.
/// Typed deltas — not invalidation messages.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
pub enum StateEvent {
    // --- Pane lifecycle ---
    /// Full pane state upsert (authoritative — replaces client's view of this pane)
    PaneUpsert {
        pane: u8,
        #[serde(flatten)]
        data: Value,
    },
    /// Pane removed from active set
    PaneRemoved {
        pane: u8,
        reason: String,
    },

    // --- Legacy lifecycle (still emitted for SSE compat) ---
    PaneSpawned {
        pane: u8,
        project: String,
        role: String,
    },
    PaneKilled {
        pane: u8,
        reason: String,
    },
    PaneStatusChanged {
        pane: u8,
        status: String,
    },

    // --- Terminal output ---
    /// Incremental terminal output chunk for a pane
    OutputChunk {
        pane: u8,
        output: String,
        full_lines: usize,
        tmux_target: Option<String>,
    },

    // --- Session events ---
    /// New JSONL session events for a pane (cursor-based, no duplicates)
    SessionEventChunk {
        pane: u8,
        events: Value,
    },

    // --- Queue ---
    QueueUpsert {
        task_id: String,
        task: Value,
    },
    QueueRemoved {
        task_id: String,
    },
    /// Legacy compat
    QueueChanged {
        action: String,
        task_id: String,
        task: String,
    },

    // --- Activity log ---
    LogAppended {
        pane: u8,
        event: String,
        summary: String,
    },

    // --- Vision ---
    VisionChanged {
        project: String,
        summary: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        feature_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        feature_title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        phase: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        state: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        readiness: Option<Value>,
    },

    // --- Debate / governance ---
    DebateChanged {
        project: String,
        debate_id: String,
        title: String,
        status: String,
        action: String,
    },

    SessionContractChanged {
        project: String,
        session_id: String,
        role: String,
        status: String,
        action: String,
    },

    WorkflowRunChanged {
        project: String,
        workflow_run_id: String,
        workflow_id: String,
        status: String,
        action: String,
    },

    AuditLogged {
        project: String,
        action_id: String,
        kind: String,
        target: String,
        outcome: String,
    },

    // --- Sync ---
    SyncStatusChanged {
        project: String,
        #[serde(flatten)]
        data: Value,
    },

    // --- Control ---
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

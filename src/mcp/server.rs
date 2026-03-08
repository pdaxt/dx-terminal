//! MCP server implementation — exposes DX Terminal as an MCP.
//!
//! Uses the micro MCP router to compose tool definitions from
//! independent domain modules (sessions, pty, analytics, git).

use serde_json::Value;
use tokio::sync::mpsc;

use super::router::McpRouter;

/// Commands the MCP server sends to the main app loop.
#[derive(Debug)]
pub enum McpCommand {
    // --- Sessions ---
    ListSessions { filter: String },
    SpawnAgent {
        pane_num: u8,
        project: String,
        role: String,
        task: String,
        agent: String,
        autonomous: bool,
    },
    KillAgent { pane_num: u8 },
    GetStatus { pane_num: u8 },

    // --- PTY Control ---
    SendInput {
        pane_num: u8,
        input: String,
        enter: bool,
    },
    GetContent {
        pane_num: u8,
        lines: usize,
        from_bottom: bool,
    },
    SendApproval {
        pane_num: u8,
        approve: bool,
    },
    SendChoice {
        pane_num: u8,
        choice: u8,
    },
    ResizePane {
        pane_num: u8,
        cols: u16,
        rows: u16,
    },

    // --- Analytics ---
    GetUsage { period: String, group_by: String },
    GetCostBreakdown { days: u32 },
    GetSystemStats,

    // --- Git ---
    GetBranch { pane_num: u8 },
    GetDiff { pane_num: u8, staged: bool },
    GetLog { pane_num: u8, count: usize },
}

/// Response from the main app back to MCP.
#[derive(Debug, Clone)]
pub struct McpResponse {
    pub data: Value,
}

/// Handle for the MCP server — used by the main app to receive commands.
pub struct McpServerHandle {
    _command_tx: mpsc::Sender<(McpCommand, tokio::sync::oneshot::Sender<McpResponse>)>,
    router: McpRouter,
}

impl McpServerHandle {
    /// Create a new MCP server handle with command channel and micro MCP router.
    pub fn new() -> (
        Self,
        mpsc::Receiver<(McpCommand, tokio::sync::oneshot::Sender<McpResponse>)>,
    ) {
        let (tx, rx) = mpsc::channel(32);
        let router = McpRouter::new();

        tracing::info!(
            "MCP server initialized: {} tools across {} modules ({:?})",
            router.tool_count(),
            router.namespaces().len(),
            router.namespaces()
        );

        (
            Self {
                _command_tx: tx,
                router,
            },
            rx,
        )
    }

    /// Get all tool definitions from the micro MCP router.
    pub fn tool_definitions(&self) -> Vec<Value> {
        self.router.all_tools()
    }

    /// Get the total number of MCP tools.
    pub fn tool_count(&self) -> usize {
        self.router.tool_count()
    }

    /// Get module namespaces for display.
    pub fn namespaces(&self) -> Vec<&str> {
        self.router.namespaces()
    }
}

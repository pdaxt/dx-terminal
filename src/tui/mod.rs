pub mod dashboard;
pub mod widgets;
pub mod input;
pub mod overlays;

use std::io;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::App;
use crate::mcp::{tools, types};

const TICK_MS: u64 = 250;
const RESULT_DISMISS_SECS: u64 = 4;

// ========== View Modes ==========

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewMode {
    Normal,
    Features,
    Board,
    Coord,
    Projects,
    Infra,
    Intel,
}

// ========== TUI Mode (State Machine) ==========

#[derive(Clone)]
pub enum TuiMode {
    Navigate,
    Command { input: String, cursor: usize },
    Input { form: input::FormState },
    Confirm { action: PendingAction, message: String },
    Executing { description: String, started: Instant },
    Result { message: String, is_error: bool, shown_at: Instant },
}

#[derive(Clone)]
pub enum PendingAction {
    Kill { pane: u8 },
    Complete { pane: u8 },
    FeatureToQueue { space: String, issue_ids: Vec<String> },
}

// ========== Command / Result Channel Types ==========

pub enum TuiCommand {
    Spawn {
        pane: String,
        project: String,
        role: Option<String>,
        task: Option<String>,
    },
    Kill {
        pane: String,
        reason: Option<String>,
    },
    Complete {
        pane: String,
        summary: Option<String>,
    },
    AutoCycle,
    QueueAdd {
        project: String,
        task: String,
        role: Option<String>,
        priority: Option<u8>,
    },
    FeatureCreate {
        space: String,
        title: String,
        issue_type: String,
        priority: Option<String>,
    },
    FeatureToQueue {
        space: String,
        issue_ids: Vec<String>,
    },
    IssueUpdateStatus {
        space: String,
        issue_id: String,
        status: String,
    },
}

pub struct TuiResult {
    pub description: String,
    pub success: bool,
    pub message: String,
}

// ========== Entry Point ==========

pub fn run_tui(app: Arc<App>) -> anyhow::Result<()> {
    // Create a small tokio runtime for async tool calls
    // (TUI thread has no parent runtime — main.rs spawns it on bare OS thread)
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;

    // Channels: TUI (sync) → Executor (async) → TUI (sync)
    let (cmd_tx, cmd_rx) = mpsc::channel::<TuiCommand>();
    let (result_tx, result_rx) = mpsc::channel::<TuiResult>();

    // Spawn executor loop on the runtime
    let exec_app = Arc::clone(&app);
    rt.spawn(async move {
        executor_loop(exec_app, cmd_rx, result_tx).await;
    });

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &app, &cmd_tx, &result_rx);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    drop(rt);
    result
}

// ========== Executor (async side) ==========

async fn executor_loop(
    app: Arc<App>,
    cmd_rx: mpsc::Receiver<TuiCommand>,
    result_tx: mpsc::Sender<TuiResult>,
) {
    while let Ok(cmd) = cmd_rx.recv() {
        let result = execute_command(&app, cmd).await;
        let _ = result_tx.send(result);
    }
}

async fn execute_command(app: &App, cmd: TuiCommand) -> TuiResult {
    match cmd {
        TuiCommand::Spawn { pane, project, role, task } => {
            let desc = format!("Spawn P{} {}", pane, project);
            let result = tools::spawn(app, types::SpawnRequest {
                pane, project, role, task, prompt: None,
            }).await;
            let success = !result.contains("\"error\"") && !result.contains("Error");
            TuiResult { description: desc, success, message: result }
        }
        TuiCommand::Kill { pane, reason } => {
            let desc = format!("Kill P{}", pane);
            let result = tools::kill(app, types::KillRequest { pane, reason }).await;
            TuiResult { description: desc, success: true, message: result }
        }
        TuiCommand::Complete { pane, summary } => {
            let desc = format!("Complete P{}", pane);
            let result = tools::complete(app, types::CompleteRequest { pane, summary }).await;
            TuiResult { description: desc, success: true, message: result }
        }
        TuiCommand::AutoCycle => {
            let result = tools::auto_cycle(app).await;
            TuiResult { description: "Auto-cycle".into(), success: true, message: result }
        }
        TuiCommand::QueueAdd { project, task, role, priority } => {
            let desc = format!("Queue: {}", &task);
            let result = tools::queue_add(app, types::QueueAddRequest {
                project, task, role, priority, prompt: None, depends_on: None, max_retries: None,
            }).await;
            let success = !result.contains("\"error\"");
            TuiResult { description: desc, success, message: result }
        }
        TuiCommand::FeatureCreate { space, title, issue_type, priority } => {
            let desc = format!("Create: {}", &title);
            let pri = priority.as_deref().unwrap_or("medium");
            let result = crate::tracker::issue_create(
                &space, &title, &issue_type, pri,
                "", "", "", &[], 0.0, "", "", "",
            );
            let id = result.get("created").and_then(|v| v.as_str()).unwrap_or("?");
            let message = format!("Created {} in {}", id, space);
            let success = !result.get("error").is_some();
            TuiResult { description: desc, success, message }
        }
        TuiCommand::FeatureToQueue { space, issue_ids } => {
            let desc = format!("Queue {} issues", issue_ids.len());
            let result = crate::tracker::feature_to_queue(&space, &issue_ids, false);
            let message = serde_json::to_string(&result).unwrap_or_default();
            let success = !message.contains("\"error\"");
            TuiResult { description: desc, success, message }
        }
        TuiCommand::IssueUpdateStatus { space, issue_id, status } => {
            let desc = format!("{} → {}", issue_id, status);
            let result = crate::tracker::issue_update_full(
                &space, &issue_id, &status,
                "", "", "", "", "", "", "", 0.0, 0.0, "", "",
            );
            let message = serde_json::to_string(&result).unwrap_or_default();
            TuiResult { description: desc, success: true, message }
        }
    }
}

// ========== Main Loop ==========

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &App,
    cmd_tx: &mpsc::Sender<TuiCommand>,
    result_rx: &mpsc::Receiver<TuiResult>,
) -> anyhow::Result<()> {
    let mut selected: u8 = 1;
    let mut view_mode = ViewMode::Normal;
    let mut mode = TuiMode::Navigate;
    let mut feature_cursor: usize = 0;
    let tick_rate = Duration::from_millis(TICK_MS);
    let mut last_tick = Instant::now();

    loop {
        // Poll for async results (non-blocking)
        if let Ok(result) = result_rx.try_recv() {
            mode = TuiMode::Result {
                message: extract_summary(&result),
                is_error: !result.success,
                shown_at: Instant::now(),
            };
        }

        // Auto-dismiss Result after N seconds
        if let TuiMode::Result { shown_at, .. } = &mode {
            if shown_at.elapsed() >= Duration::from_secs(RESULT_DISMISS_SECS) {
                mode = TuiMode::Navigate;
            }
        }

        let mut data = dashboard::collect_data(app, selected, view_mode, feature_cursor);
        // Clamp feature cursor
        let feat_max: usize = data.features.iter().map(|f| 1 + f.children.len()).sum();
        if feat_max > 0 && feature_cursor >= feat_max {
            feature_cursor = feat_max - 1;
            data.feature_cursor = feature_cursor;
        }

        // Render dashboard + overlay
        let mode_ref = &mode;
        terminal.draw(|f| {
            dashboard::render(f, &data);
            overlays::render_overlay(f, f.area(), mode_ref, &data);
        })?;

        // Event handling
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if matches!(mode, TuiMode::Navigate) {
                    // Feature-view specific keybinds
                    if view_mode == ViewMode::Features {
                        let feat_handled = match key.code {
                            KeyCode::Char('j') | KeyCode::Down => {
                                if feat_max > 0 && feature_cursor < feat_max - 1 { feature_cursor += 1; }
                                true
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                if feature_cursor > 0 { feature_cursor -= 1; }
                                true
                            }
                            KeyCode::Char('n') => {
                                mode = TuiMode::Input { form: input::create_feature_form() };
                                true
                            }
                            KeyCode::Enter => {
                                if let Some((space, ids, count)) = feature_children_ids(&data.features, feature_cursor) {
                                    if !ids.is_empty() {
                                        mode = TuiMode::Confirm {
                                            action: PendingAction::FeatureToQueue { space, issue_ids: ids },
                                            message: format!("Push {} tasks to queue?", count),
                                        };
                                    }
                                }
                                true
                            }
                            KeyCode::Char('u') => {
                                if let Some((space, id, current_status)) = feature_at_cursor(&data.features, feature_cursor) {
                                    let next = next_status(&current_status).to_string();
                                    let _ = cmd_tx.send(TuiCommand::IssueUpdateStatus {
                                        space, issue_id: id, status: next,
                                    });
                                    mode = TuiMode::Executing {
                                        description: "Updating status...".into(),
                                        started: Instant::now(),
                                    };
                                }
                                true
                            }
                            _ => false,
                        };
                        if feat_handled { continue; }
                    }
                    if let Some(true) = handle_navigate(key, &mut mode, &mut view_mode, &mut selected, app, cmd_tx) {
                        return Ok(());
                    }
                } else if matches!(mode, TuiMode::Command { .. }) {
                    handle_command(key, &mut mode, cmd_tx);
                } else if matches!(mode, TuiMode::Input { .. }) {
                    handle_input(key, &mut mode, cmd_tx);
                } else if matches!(mode, TuiMode::Confirm { .. }) {
                    handle_confirm(key, &mut mode, cmd_tx);
                } else if matches!(mode, TuiMode::Executing { .. }) {
                    if key.code == KeyCode::Esc {
                        mode = TuiMode::Navigate;
                    }
                } else if matches!(mode, TuiMode::Result { .. }) {
                    mode = TuiMode::Navigate;
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
}

// ========== Key Handlers ==========

/// Returns Some(true) to quit, Some(false) for handled, None for unhandled
fn handle_navigate(
    key: crossterm::event::KeyEvent,
    mode: &mut TuiMode,
    view_mode: &mut ViewMode,
    selected: &mut u8,
    app: &App,
    cmd_tx: &mpsc::Sender<TuiCommand>,
) -> Option<bool> {
    match key.code {
        KeyCode::Char('q') => return Some(true),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Some(true),

        // Command mode
        KeyCode::Char(':') => {
            *mode = TuiMode::Command { input: String::new(), cursor: 0 };
        }

        // Spawn form
        KeyCode::Char('s') => {
            *mode = TuiMode::Input { form: input::create_spawn_form(*selected) };
        }

        // Queue add form
        KeyCode::Char('t') => {
            *mode = TuiMode::Input { form: input::create_queue_form() };
        }

        // Auto-cycle
        KeyCode::Char('a') => {
            let _ = cmd_tx.send(TuiCommand::AutoCycle);
            *mode = TuiMode::Executing {
                description: "Running auto-cycle...".into(),
                started: Instant::now(),
            };
        }

        // Kill (with confirm)
        KeyCode::Char('k') => {
            let pane = *selected;
            *mode = TuiMode::Confirm {
                action: PendingAction::Kill { pane },
                message: format!("Kill agent on pane {}?", pane),
            };
        }

        // Complete/done (with confirm)
        KeyCode::Char('d') => {
            let pane = *selected;
            *mode = TuiMode::Confirm {
                action: PendingAction::Complete { pane },
                message: format!("Mark pane {} as done?", pane),
            };
        }

        // View toggles
        KeyCode::Char('f') => {
            *view_mode = if *view_mode == ViewMode::Features { ViewMode::Normal } else { ViewMode::Features };
        }
        KeyCode::Char('b') => {
            *view_mode = if *view_mode == ViewMode::Board { ViewMode::Normal } else { ViewMode::Board };
        }
        KeyCode::Char('c') => {
            *view_mode = if *view_mode == ViewMode::Coord { ViewMode::Normal } else { ViewMode::Coord };
        }
        KeyCode::Char('p') => {
            *view_mode = if *view_mode == ViewMode::Projects { ViewMode::Normal } else { ViewMode::Projects };
        }
        KeyCode::Char('i') => {
            *view_mode = if *view_mode == ViewMode::Infra { ViewMode::Normal } else { ViewMode::Infra };
        }
        KeyCode::Char('g') => {
            *view_mode = if *view_mode == ViewMode::Intel { ViewMode::Normal } else { ViewMode::Intel };
        }

        // Pane selection
        KeyCode::Char(c @ '1'..='9') => {
            let n = c.to_digit(10).unwrap() as u8;
            if n <= crate::config::pane_count() {
                *selected = n;
            }
        }

        // Tab cycles
        KeyCode::Tab => {
            let max = crate::config::pane_count();
            *selected = if *selected >= max { 1 } else { *selected + 1 };
        }
        KeyCode::BackTab => {
            let max = crate::config::pane_count();
            *selected = if *selected <= 1 { max } else { *selected - 1 };
        }

        // Send Ctrl-C to pane
        KeyCode::Char('x') => {
            let mut pty = app.pty_lock();
            if let Some(agent) = pty.agents.get_mut(selected) {
                let _ = agent.send_ctrl_c();
            }
        }

        // Restart (kill via PTY)
        KeyCode::Char('r') => {
            let mut pty = app.pty_lock();
            let _ = pty.kill(*selected);
        }

        _ => {}
    }
    Some(false)
}

fn handle_command(
    key: crossterm::event::KeyEvent,
    mode: &mut TuiMode,
    cmd_tx: &mpsc::Sender<TuiCommand>,
) {
    let (input_str, cursor) = match mode {
        TuiMode::Command { input, cursor } => (input, cursor),
        _ => return,
    };

    match key.code {
        KeyCode::Esc => {
            *mode = TuiMode::Navigate;
        }
        KeyCode::Enter => {
            let trimmed = input_str.trim().to_string();
            if let Some(cmd) = input::parse_command(&trimmed) {
                let desc = format!(":{}", &trimmed);
                let _ = cmd_tx.send(cmd);
                *mode = TuiMode::Executing { description: desc, started: Instant::now() };
            } else if trimmed.is_empty() {
                *mode = TuiMode::Navigate;
            } else {
                *mode = TuiMode::Result {
                    message: format!("Unknown command: {}", trimmed),
                    is_error: true,
                    shown_at: Instant::now(),
                };
            }
        }
        KeyCode::Backspace => {
            if *cursor > 0 {
                input_str.remove(*cursor - 1);
                *cursor -= 1;
            }
        }
        KeyCode::Left => {
            if *cursor > 0 { *cursor -= 1; }
        }
        KeyCode::Right => {
            if *cursor < input_str.len() { *cursor += 1; }
        }
        KeyCode::Home => { *cursor = 0; }
        KeyCode::End => { *cursor = input_str.len(); }
        KeyCode::Char(c) => {
            input_str.insert(*cursor, c);
            *cursor += 1;
        }
        _ => {}
    }
}

fn handle_input(
    key: crossterm::event::KeyEvent,
    mode: &mut TuiMode,
    cmd_tx: &mpsc::Sender<TuiCommand>,
) {
    let form = match mode {
        TuiMode::Input { form } => form,
        _ => return,
    };

    match key.code {
        KeyCode::Esc => {
            *mode = TuiMode::Navigate;
        }
        KeyCode::Tab => {
            form.focused = (form.focused + 1) % form.fields.len();
        }
        KeyCode::BackTab => {
            form.focused = if form.focused == 0 { form.fields.len() - 1 } else { form.focused - 1 };
        }
        KeyCode::Enter => {
            if form.focused < form.fields.len() - 1 {
                form.focused += 1;
            } else if input::form_is_valid(form) {
                if let Some(cmd) = input::form_to_command(form) {
                    let desc = form.title.clone();
                    let _ = cmd_tx.send(cmd);
                    *mode = TuiMode::Executing { description: desc, started: Instant::now() };
                }
            } else {
                *mode = TuiMode::Result {
                    message: "Required fields missing".into(),
                    is_error: true,
                    shown_at: Instant::now(),
                };
            }
        }
        KeyCode::Backspace => {
            let field = &mut form.fields[form.focused];
            if field.cursor > 0 {
                field.value.remove(field.cursor - 1);
                field.cursor -= 1;
            }
        }
        KeyCode::Left => {
            let field = &mut form.fields[form.focused];
            if field.cursor > 0 { field.cursor -= 1; }
        }
        KeyCode::Right => {
            let field = &mut form.fields[form.focused];
            if field.cursor < field.value.len() { field.cursor += 1; }
        }
        KeyCode::Home => {
            form.fields[form.focused].cursor = 0;
        }
        KeyCode::End => {
            let len = form.fields[form.focused].value.len();
            form.fields[form.focused].cursor = len;
        }
        KeyCode::Char(c) => {
            let field = &mut form.fields[form.focused];
            field.value.insert(field.cursor, c);
            field.cursor += 1;
        }
        _ => {}
    }
}

fn handle_confirm(
    key: crossterm::event::KeyEvent,
    mode: &mut TuiMode,
    cmd_tx: &mpsc::Sender<TuiCommand>,
) {
    let action = match mode {
        TuiMode::Confirm { action, .. } => action.clone(),
        _ => return,
    };

    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => {
            let (cmd, desc) = match action {
                PendingAction::Kill { pane } => (
                    TuiCommand::Kill { pane: pane.to_string(), reason: Some("TUI kill".into()) },
                    format!("Killing pane {}...", pane),
                ),
                PendingAction::Complete { pane } => (
                    TuiCommand::Complete { pane: pane.to_string(), summary: None },
                    format!("Completing pane {}...", pane),
                ),
                PendingAction::FeatureToQueue { space, issue_ids } => (
                    TuiCommand::FeatureToQueue { space, issue_ids },
                    "Pushing to queue...".to_string(),
                ),
            };
            let _ = cmd_tx.send(cmd);
            *mode = TuiMode::Executing { description: desc, started: Instant::now() };
        }
        KeyCode::Char('n') | KeyCode::Esc => {
            *mode = TuiMode::Navigate;
        }
        _ => {}
    }
}

// ========== Helpers ==========

/// Get (space, issue_id, current_status) at flat cursor position
fn feature_at_cursor(features: &[dashboard::FeatureSnapshot], cursor: usize) -> Option<(String, String, String)> {
    let mut idx = 0;
    for feat in features {
        if idx == cursor {
            return Some((feat.space.clone(), feat.id.clone(), feat.status.clone()));
        }
        idx += 1;
        for child in &feat.children {
            if idx == cursor {
                return Some((feat.space.clone(), child.id.clone(), child.status.clone()));
            }
            idx += 1;
        }
    }
    None
}

/// Get (space, child_ids, count) for the feature at cursor (for queue push)
fn feature_children_ids(features: &[dashboard::FeatureSnapshot], cursor: usize) -> Option<(String, Vec<String>, usize)> {
    let mut idx = 0;
    for feat in features {
        if idx == cursor {
            // On a feature row — push all children
            let ids: Vec<String> = feat.children.iter().map(|c| c.id.clone()).collect();
            let count = ids.len();
            return Some((feat.space.clone(), ids, count));
        }
        idx += 1;
        for child in &feat.children {
            if idx == cursor {
                // On a child row — push just this child
                return Some((feat.space.clone(), vec![child.id.clone()], 1));
            }
            idx += 1;
        }
    }
    None
}

fn next_status(current: &str) -> &str {
    match current {
        "todo" | "backlog" => "in_progress",
        "in_progress" => "done",
        "done" => "todo",
        _ => "in_progress",
    }
}

fn extract_summary(result: &TuiResult) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&result.message) {
        if let Some(status) = v.get("status").and_then(|s| s.as_str()) {
            let pane = v.get("pane").and_then(|p| p.as_u64()).map(|p| format!(" P{}", p)).unwrap_or_default();
            return format!("{}: {}{}", result.description, status, pane);
        }
        if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
            return format!("{}: {}", result.description, err);
        }
    }
    let msg: String = result.message.chars().take(60).collect();
    format!("{}: {}", result.description, msg)
}

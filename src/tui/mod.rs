pub mod dashboard;
pub mod dispatch;
pub mod input;
pub mod overlays;
pub mod widgets;

use std::collections::VecDeque;
use std::io;
use std::sync::{mpsc, Arc};
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
    Dashboard,
    Features,
    Board,
    Coord,
    Projects,
    Infra,
    Intel,
    Audit,
    Log,
    Pipeline,
}

// ========== TUI Mode (State Machine) ==========

#[derive(Clone)]
pub enum TuiMode {
    Navigate,
    Command {
        input: String,
        cursor: usize,
        completions: Vec<(String, String)>,
        comp_idx: Option<usize>,
    },
    Input {
        form: input::FormState,
    },
    Confirm {
        action: PendingAction,
        message: String,
    },
    Executing {
        description: String,
        _started: Instant,
    },
    Result {
        message: String,
        is_error: bool,
        shown_at: Instant,
    },
    Talk {
        target_pane: u8,
        input: String,
        cursor: usize,
    },
}

#[derive(Clone)]
pub enum PendingAction {
    Kill {
        pane: u8,
    },
    Complete {
        pane: u8,
    },
    FeatureToQueue {
        space: String,
        issue_ids: Vec<String>,
    },
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
    McpDispatch {
        tool: String,
        args: serde_json::Value,
    },
    Orchestrate {
        request: String,
        project: Option<String>,
    },
    FactoryGo {
        project: Option<String>,
        request: String,
        template: Option<String>,
    },
    Talk {
        pane: u8,
        message: String,
    },
    AddScreen {
        name: Option<String>,
        layout: Option<String>,
        panes: Option<u8>,
    },
    RemoveScreen {
        screen_ref: String,
        force: bool,
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

    // Spawn background auto-cycle timer: runs every 30s when queue has work
    let cycle_tx = cmd_tx.clone();
    let cycle_app = Arc::clone(&app);
    rt.spawn(async move {
        auto_cycle_timer(cycle_app, cycle_tx).await;
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

/// Background auto-cycle timer: checks queue every 30s and triggers auto_cycle when there's work
async fn auto_cycle_timer(_app: Arc<App>, cmd_tx: mpsc::Sender<TuiCommand>) {
    use crate::queue;
    let interval = std::time::Duration::from_secs(30);
    // Small initial delay to let TUI initialize
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    loop {
        tokio::time::sleep(interval).await;

        // Only cycle when there's work to do
        let q = queue::load_queue();
        let has_pending = q
            .tasks
            .iter()
            .any(|t| t.status == queue::QueueStatus::Pending);
        let has_running = q
            .tasks
            .iter()
            .any(|t| t.status == queue::QueueStatus::Running);

        if has_pending || has_running {
            if cmd_tx.send(TuiCommand::AutoCycle).is_err() {
                break; // TUI shut down
            }
        }
    }
}

async fn execute_command(app: &App, cmd: TuiCommand) -> TuiResult {
    match cmd {
        TuiCommand::Spawn {
            pane,
            project,
            role,
            task,
        } => {
            let desc = format!("Spawn P{} {}", pane, project);
            let result = tools::spawn(
                app,
                types::SpawnRequest {
                    pane,
                    project,
                    role,
                    provider: None,
                    model: None,
                    task,
                    prompt: None,
                    autonomous: None,
                },
            )
            .await;
            let success = !result.contains("\"error\"") && !result.contains("Error");
            TuiResult {
                description: desc,
                success,
                message: result,
            }
        }
        TuiCommand::Kill { pane, reason } => {
            let desc = format!("Kill P{}", pane);
            let result = tools::kill(app, types::KillRequest { pane, reason }).await;
            TuiResult {
                description: desc,
                success: true,
                message: result,
            }
        }
        TuiCommand::Complete { pane, summary } => {
            let desc = format!("Complete P{}", pane);
            let result = tools::complete(app, types::CompleteRequest { pane, summary }).await;
            TuiResult {
                description: desc,
                success: true,
                message: result,
            }
        }
        TuiCommand::AutoCycle => {
            let result = tools::auto_cycle(app).await;
            TuiResult {
                description: "Auto-cycle".into(),
                success: true,
                message: result,
            }
        }
        TuiCommand::QueueAdd {
            project,
            task,
            role,
            priority,
        } => {
            let desc = format!("Queue: {}", &task);
            let result = tools::queue_add(
                app,
                types::QueueAddRequest {
                    project,
                    task,
                    role,
                    priority,
                    prompt: None,
                    depends_on: None,
                    max_retries: None,
                },
            )
            .await;
            let success = !result.contains("\"error\"");
            TuiResult {
                description: desc,
                success,
                message: result,
            }
        }
        TuiCommand::FeatureCreate {
            space,
            title,
            issue_type,
            priority,
        } => {
            let desc = format!("Create: {}", &title);
            let result = tools::tracker_tools::issue_create(&types::IssueCreateRequest {
                space,
                title,
                issue_type: Some(issue_type),
                priority,
                description: None,
                assignee: None,
                milestone: None,
                labels: None,
                estimated_acu: None,
                role: None,
                parent: None,
                sprint: None,
            });
            let success = !result.contains("\"error\"");
            TuiResult {
                description: desc,
                success,
                message: result,
            }
        }
        TuiCommand::FeatureToQueue { space, issue_ids } => {
            let desc = format!("Queue {} issues", issue_ids.len());
            let result = tools::tracker_tools::feature_to_queue(&types::FeatureToQueueRequest {
                space,
                issue_ids,
                sequential: Some(false),
            });
            let success = !result.contains("\"error\"");
            TuiResult {
                description: desc,
                success,
                message: result,
            }
        }
        TuiCommand::IssueUpdateStatus {
            space,
            issue_id,
            status,
        } => {
            let desc = format!("{} → {}", issue_id, status);
            let result = tools::tracker_tools::issue_update_full(&types::IssueUpdateFullRequest {
                space,
                issue_id,
                status: Some(status),
                priority: None,
                assignee: None,
                title: None,
                description: None,
                milestone: None,
                add_label: None,
                remove_label: None,
                estimated_acu: None,
                actual_acu: None,
                sprint: None,
                role: None,
            });
            TuiResult {
                description: desc,
                success: true,
                message: result,
            }
        }
        TuiCommand::McpDispatch { tool, args } => {
            let desc = format!(":{}", tool);
            let result = dispatch::dispatch_mcp_tool(app, &tool, args).await;
            let success = !result.contains("\"error\"");
            TuiResult {
                description: desc,
                success,
                message: result,
            }
        }
        TuiCommand::Orchestrate { request, project } => {
            let desc = format!(
                "Orchestrate: {}",
                &request.chars().take(30).collect::<String>()
            );
            let result = tools::orchestrate::orchestrate(
                app,
                types::OrchestrateRequest {
                    request,
                    project,
                    concurrent_qa: Some(true),
                    concurrent_security: Some(false),
                    max_panes: None,
                },
            )
            .await;
            let success = !result.contains("\"error\"");
            TuiResult {
                description: desc,
                success,
                message: result,
            }
        }
        TuiCommand::FactoryGo {
            project,
            request,
            template,
        } => {
            let desc = format!("Factory: {}", &request.chars().take(30).collect::<String>());
            let result = tools::factory_tools::factory_run(
                app,
                types::FactoryRequest {
                    request,
                    project,
                    template,
                    priority: None,
                },
            )
            .await;
            let success = !result.contains("\"error\"");
            TuiResult {
                description: desc,
                success,
                message: result,
            }
        }
        TuiCommand::Talk { pane, message } => {
            let desc = format!("Talk P{}", pane);
            // Tmux-first: get target from state
            let pane_data = app.state.blocking_read();
            let tmux_target = pane_data
                .panes
                .get(&pane.to_string())
                .and_then(|p| p.tmux_target.clone());
            drop(pane_data);

            let send_result = if let Some(target) = tmux_target {
                crate::tmux::send_command(&target, &message)
            } else {
                // PTY fallback
                let mut pty = app.pty_lock();
                pty.send_line(pane, &message)
            };
            // Also log to multi_agent messages for audit trail
            let _ = crate::multi_agent::msg_send("tui", &pane.to_string(), &message);
            match send_result {
                Ok(()) => TuiResult {
                    description: desc,
                    success: true,
                    message: format!(
                        "Sent to pane {}: {}",
                        pane,
                        &message.chars().take(50).collect::<String>()
                    ),
                },
                Err(e) => TuiResult {
                    description: desc,
                    success: false,
                    message: format!("Failed to send to pane {}: {}", pane, e),
                },
            }
        }
        TuiCommand::AddScreen {
            name,
            layout,
            panes,
        } => {
            let desc = format!(
                "Add screen{}",
                name.as_deref()
                    .map(|n| format!(" '{}'", n))
                    .unwrap_or_default()
            );
            let result = tools::screen_tools::add_screen(app, name, layout, panes);
            let success = !result.contains("\"error\"");
            TuiResult {
                description: desc,
                success,
                message: result,
            }
        }
        TuiCommand::RemoveScreen { screen_ref, force } => {
            let desc = format!("Remove screen '{}'", screen_ref);
            let result = tools::screen_tools::remove_screen(app, screen_ref, force);
            let success = !result.contains("\"error\"");
            TuiResult {
                description: desc,
                success,
                message: result,
            }
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
    let mut action_log: VecDeque<dashboard::ActionLogEntry> = VecDeque::new();

    loop {
        // Poll for async results (non-blocking)
        if let Ok(result) = result_rx.try_recv() {
            // Add to action log
            let entry = dashboard::ActionLogEntry {
                timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                tool: result.description.clone(),
                success: result.success,
                summary: extract_summary(&result),
            };
            action_log.push_front(entry);
            if action_log.len() > 50 {
                action_log.pop_back();
            }

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
        data.action_log = action_log.iter().cloned().collect();
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
                                if feat_max > 0 && feature_cursor < feat_max - 1 {
                                    feature_cursor += 1;
                                }
                                true
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                if feature_cursor > 0 {
                                    feature_cursor -= 1;
                                }
                                true
                            }
                            KeyCode::Char('n') => {
                                mode = TuiMode::Input {
                                    form: input::create_feature_form(),
                                };
                                true
                            }
                            KeyCode::Enter => {
                                if let Some((space, ids, count)) =
                                    feature_children_ids(&data.features, feature_cursor)
                                {
                                    if !ids.is_empty() {
                                        mode = TuiMode::Confirm {
                                            action: PendingAction::FeatureToQueue {
                                                space,
                                                issue_ids: ids,
                                            },
                                            message: format!("Push {} tasks to queue?", count),
                                        };
                                    }
                                }
                                true
                            }
                            KeyCode::Char('u') => {
                                if let Some((space, id, current_status)) =
                                    feature_at_cursor(&data.features, feature_cursor)
                                {
                                    let next = next_status(&current_status).to_string();
                                    let _ = cmd_tx.send(TuiCommand::IssueUpdateStatus {
                                        space,
                                        issue_id: id,
                                        status: next,
                                    });
                                    mode = TuiMode::Executing {
                                        description: "Updating status...".into(),
                                        _started: Instant::now(),
                                    };
                                }
                                true
                            }
                            _ => false,
                        };
                        if feat_handled {
                            continue;
                        }
                    }
                    if let Some(true) =
                        handle_navigate(key, &mut mode, &mut view_mode, &mut selected, app, cmd_tx)
                    {
                        return Ok(());
                    }
                } else if matches!(mode, TuiMode::Command { .. }) {
                    handle_command(key, &mut mode, cmd_tx);
                } else if matches!(mode, TuiMode::Input { .. }) {
                    handle_input(key, &mut mode, cmd_tx);
                } else if matches!(mode, TuiMode::Confirm { .. }) {
                    handle_confirm(key, &mut mode, cmd_tx);
                } else if matches!(mode, TuiMode::Talk { .. }) {
                    handle_talk(key, &mut mode, cmd_tx);
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
    _app: &App,
    cmd_tx: &mpsc::Sender<TuiCommand>,
) -> Option<bool> {
    match key.code {
        KeyCode::Char('q') => return Some(true),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Some(true),

        // Command mode
        KeyCode::Char(':') => {
            *mode = TuiMode::Command {
                input: String::new(),
                cursor: 0,
                completions: Vec::new(),
                comp_idx: None,
            };
        }

        // Spawn form
        KeyCode::Char('s') => {
            *mode = TuiMode::Input {
                form: input::create_spawn_form(*selected),
            };
        }

        // Talk to agent on selected pane
        KeyCode::Char('t') => {
            *mode = TuiMode::Talk {
                target_pane: *selected,
                input: String::new(),
                cursor: 0,
            };
        }

        // Factory form
        KeyCode::Char('w') => {
            *mode = TuiMode::Input {
                form: input::create_factory_form(),
            };
        }

        // Orchestrate form — natural language → full pipeline
        KeyCode::Char('o') => {
            *mode = TuiMode::Input {
                form: input::create_orchestrate_form(),
            };
        }

        // Auto-cycle
        KeyCode::Char('a') => {
            let _ = cmd_tx.send(TuiCommand::AutoCycle);
            *mode = TuiMode::Executing {
                description: "Running auto-cycle...".into(),
                _started: Instant::now(),
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
            *view_mode = if *view_mode == ViewMode::Features {
                ViewMode::Normal
            } else {
                ViewMode::Features
            };
        }
        KeyCode::Char('b') => {
            *view_mode = if *view_mode == ViewMode::Board {
                ViewMode::Normal
            } else {
                ViewMode::Board
            };
        }
        KeyCode::Char('c') => {
            *view_mode = if *view_mode == ViewMode::Coord {
                ViewMode::Normal
            } else {
                ViewMode::Coord
            };
        }
        KeyCode::Char('p') => {
            *view_mode = if *view_mode == ViewMode::Projects {
                ViewMode::Normal
            } else {
                ViewMode::Projects
            };
        }
        KeyCode::Char('i') => {
            *view_mode = if *view_mode == ViewMode::Infra {
                ViewMode::Normal
            } else {
                ViewMode::Infra
            };
        }
        KeyCode::Char('g') => {
            *view_mode = if *view_mode == ViewMode::Intel {
                ViewMode::Normal
            } else {
                ViewMode::Intel
            };
        }
        KeyCode::Char('h') => {
            *view_mode = if *view_mode == ViewMode::Audit {
                ViewMode::Normal
            } else {
                ViewMode::Audit
            };
        }
        KeyCode::Char('l') => {
            *view_mode = if *view_mode == ViewMode::Log {
                ViewMode::Normal
            } else {
                ViewMode::Log
            };
        }
        KeyCode::Char('y') => {
            *view_mode = if *view_mode == ViewMode::Pipeline {
                ViewMode::Normal
            } else {
                ViewMode::Pipeline
            };
        }
        KeyCode::Char('0') => {
            *view_mode = if *view_mode == ViewMode::Dashboard {
                ViewMode::Normal
            } else {
                ViewMode::Dashboard
            };
        }

        // Screen management
        KeyCode::Char('+') | KeyCode::Char('=') => {
            *mode = TuiMode::Input {
                form: input::create_add_screen_form(),
            };
        }
        KeyCode::Char('-') => {
            *mode = TuiMode::Input {
                form: input::create_remove_screen_form(),
            };
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

        // Kill pane (routes through MCP tools::kill)
        KeyCode::Char('x') => {
            let _ = cmd_tx.send(TuiCommand::Kill {
                pane: selected.to_string(),
                reason: Some("TUI: user pressed x".into()),
            });
        }

        // Restart pane (kill + respawn via command channel)
        KeyCode::Char('r') => {
            let _ = cmd_tx.send(TuiCommand::Kill {
                pane: selected.to_string(),
                reason: Some("TUI: restart via r key".into()),
            });
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
    let (input_str, cursor, completions, comp_idx) = match mode {
        TuiMode::Command {
            input,
            cursor,
            completions,
            comp_idx,
        } => (input, cursor, completions, comp_idx),
        _ => return,
    };

    match key.code {
        KeyCode::Esc => {
            *mode = TuiMode::Navigate;
        }
        KeyCode::Tab => {
            if !completions.is_empty() {
                let idx = match comp_idx {
                    Some(i) => (*i + 1) % completions.len(),
                    None => 0,
                };
                *comp_idx = Some(idx);
                let tool_name = completions[idx].0.clone();
                *input_str = format!("{} ", tool_name);
                *cursor = input_str.len();
            }
        }
        KeyCode::Enter => {
            let trimmed = input_str.trim().to_string();
            if let Some(cmd) = input::parse_command(&trimmed) {
                let desc = format!(":{}", &trimmed);
                let _ = cmd_tx.send(cmd);
                *mode = TuiMode::Executing {
                    description: desc,
                    _started: Instant::now(),
                };
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
            *comp_idx = None;
            let prefix = input_str.split_whitespace().next().unwrap_or("");
            *completions = dispatch::completions_for(prefix)
                .iter()
                .map(|(n, d)| (n.to_string(), d.to_string()))
                .collect();
        }
        KeyCode::Left => {
            if *cursor > 0 {
                *cursor -= 1;
            }
        }
        KeyCode::Right => {
            if *cursor < input_str.len() {
                *cursor += 1;
            }
        }
        KeyCode::Home => {
            *cursor = 0;
        }
        KeyCode::End => {
            *cursor = input_str.len();
        }
        KeyCode::Char(c) => {
            input_str.insert(*cursor, c);
            *cursor += 1;
            *comp_idx = None;
            // Update completions only while typing the first word (tool name)
            if !input_str.contains(' ') {
                *completions = dispatch::completions_for(input_str)
                    .iter()
                    .map(|(n, d)| (n.to_string(), d.to_string()))
                    .collect();
            } else {
                completions.clear();
            }
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
            form.focused = if form.focused == 0 {
                form.fields.len() - 1
            } else {
                form.focused - 1
            };
        }
        KeyCode::Enter => {
            if form.focused < form.fields.len() - 1 {
                form.focused += 1;
            } else if input::form_is_valid(form) {
                if let Some(cmd) = input::form_to_command(form) {
                    let desc = form.title.clone();
                    let _ = cmd_tx.send(cmd);
                    *mode = TuiMode::Executing {
                        description: desc,
                        _started: Instant::now(),
                    };
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
            if field.cursor > 0 {
                field.cursor -= 1;
            }
        }
        KeyCode::Right => {
            let field = &mut form.fields[form.focused];
            if field.cursor < field.value.len() {
                field.cursor += 1;
            }
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
                    TuiCommand::Kill {
                        pane: pane.to_string(),
                        reason: Some("TUI kill".into()),
                    },
                    format!("Killing pane {}...", pane),
                ),
                PendingAction::Complete { pane } => (
                    TuiCommand::Complete {
                        pane: pane.to_string(),
                        summary: None,
                    },
                    format!("Completing pane {}...", pane),
                ),
                PendingAction::FeatureToQueue { space, issue_ids } => (
                    TuiCommand::FeatureToQueue { space, issue_ids },
                    "Pushing to queue...".to_string(),
                ),
            };
            let _ = cmd_tx.send(cmd);
            *mode = TuiMode::Executing {
                description: desc,
                _started: Instant::now(),
            };
        }
        KeyCode::Char('n') | KeyCode::Esc => {
            *mode = TuiMode::Navigate;
        }
        _ => {}
    }
}

fn handle_talk(
    key: crossterm::event::KeyEvent,
    mode: &mut TuiMode,
    cmd_tx: &mpsc::Sender<TuiCommand>,
) {
    let (target_pane, input_str, cursor) = match mode {
        TuiMode::Talk {
            target_pane,
            input,
            cursor,
        } => (*target_pane, input, cursor),
        _ => return,
    };

    match key.code {
        KeyCode::Esc => {
            *mode = TuiMode::Navigate;
        }
        KeyCode::Enter => {
            let msg = input_str.trim().to_string();
            if !msg.is_empty() {
                let _ = cmd_tx.send(TuiCommand::Talk {
                    pane: target_pane,
                    message: msg,
                });
                *mode = TuiMode::Executing {
                    description: format!("Talking to P{}...", target_pane),
                    _started: Instant::now(),
                };
            } else {
                *mode = TuiMode::Navigate;
            }
        }
        KeyCode::Backspace => {
            if *cursor > 0 {
                input_str.remove(*cursor - 1);
                *cursor -= 1;
            }
        }
        KeyCode::Left => {
            if *cursor > 0 {
                *cursor -= 1;
            }
        }
        KeyCode::Right => {
            if *cursor < input_str.len() {
                *cursor += 1;
            }
        }
        KeyCode::Home => {
            *cursor = 0;
        }
        KeyCode::End => {
            *cursor = input_str.len();
        }
        KeyCode::Char(c) => {
            input_str.insert(*cursor, c);
            *cursor += 1;
        }
        _ => {}
    }
}

// ========== Helpers ==========

/// Get (space, issue_id, current_status) at flat cursor position
fn feature_at_cursor(
    features: &[dashboard::FeatureSnapshot],
    cursor: usize,
) -> Option<(String, String, String)> {
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
fn feature_children_ids(
    features: &[dashboard::FeatureSnapshot],
    cursor: usize,
) -> Option<(String, Vec<String>, usize)> {
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
            let pane = v
                .get("pane")
                .and_then(|p| p.as_u64())
                .map(|p| format!(" P{}", p))
                .unwrap_or_default();
            return format!("{}: {}{}", result.description, status, pane);
        }
        if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
            return format!("{}: {}", result.description, err);
        }
    }
    let msg: String = result.message.chars().take(60).collect();
    format!("{}: {}", result.description, msg)
}

use std::io;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;

use crate::hub_client::HubClient;
use crate::app::{Action, AppState, Config};
use crate::monitor::{FactoryCommand, MonitorTask, SystemStatsCollector};
use crate::parsers::ParserRegistry;
use crate::pty::PtyManager;
use crate::tmux::TmuxClient;

use super::components::{
    AgentTreeWidget, AnalyticsWidget, DashboardWidget, FactoryPanelWidget, FooterWidget,
    HeaderWidget, HelpWidget, InputWidget, PanePreviewWidget, QueuePanelWidget,
    SubagentLogWidget,
};
use super::Layout;

/// Runs the main application loop
pub async fn run_app(config: Config) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Initialize state
    let mut state = AppState::new();
    let native_mode = config.native_mode;

    // Create tmux client and parser registry
    let tmux_client = Arc::new(TmuxClient::with_capture_lines(config.capture_lines));
    let parser_registry = Arc::new(ParserRegistry::new());

    // Native mode (default): create PTY manager; legacy tmux mode if --tmux
    let mut pty_manager = if native_mode {
        state.flash("DX Terminal v0.3.0".to_string());
        Some(PtyManager::new())
    } else {
        if !tmux_client.is_available() {
            state.set_error(
                "tmux is not running. Remove --tmux flag for native mode.".to_string(),
            );
        }
        None
    };

    // Create hub client if URL configured
    let hub_client = config
        .api_url
        .as_ref()
        .map(|url| HubClient::new(Some(url.clone())));

    // Create channel for monitor updates
    let (tx, mut rx) = mpsc::channel(32);

    // Create channel for factory commands (TUI → monitor)
    let (factory_tx, factory_rx) = mpsc::channel(8);

    // Start monitor task (watches tmux panes + hub API)
    let monitor = MonitorTask::new(
        tmux_client.clone(),
        parser_registry.clone(),
        hub_client,
        tx,
        factory_rx,
        Duration::from_millis(config.poll_interval_ms),
    );
    let monitor_handle = tokio::spawn(async move {
        monitor.run().await;
    });

    // Create system stats collector
    let mut system_stats = SystemStatsCollector::new();

    // Main loop
    let result = run_loop(
        &mut terminal,
        &mut state,
        &mut rx,
        &tmux_client,
        &mut system_stats,
        &factory_tx,
        &mut pty_manager,
    )
    .await;

    // Cleanup
    monitor_handle.abort();
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
    rx: &mut mpsc::Receiver<crate::monitor::MonitorUpdate>,
    tmux_client: &TmuxClient,
    system_stats: &mut SystemStatsCollector,
    factory_tx: &mpsc::Sender<FactoryCommand>,
    pty_manager: &mut Option<PtyManager>,
) -> Result<()> {
    loop {
        // Advance animation tick
        state.tick();

        // Update system stats
        system_stats.refresh();
        state.system_stats = system_stats.stats().clone();

        // Process PTY events in native mode
        if let Some(ref mut mgr) = pty_manager {
            mgr.process_events().await;
        }

        // Periodically refresh git info (~every 60 ticks = ~5s)
        if state.tick % 60 == 0 {
            state.refresh_git_info();
        }

        // Draw UI
        terminal.draw(|frame| {
            let size = frame.area();
            let main_chunks = Layout::main_layout_all_with_analytics(
                size,
                state.show_queue,
                state.show_dashboard,
                state.show_factory,
                state.show_analytics,
            );

            // Header
            HeaderWidget::render(frame, main_chunks[0], state);

            // Always show input widget at bottom of right column
            let input_height = InputWidget::calculate_height(state.get_input(), 6);

            if state.show_subagent_log {
                // With subagent log: sidebar | summary+preview+input | subagent_log
                let (left, preview, subagent_log) =
                    Layout::content_layout_with_log(main_chunks[1], state.sidebar_width);
                AgentTreeWidget::render(frame, left, state);

                // Split preview area for summary, preview, and input
                let preview_chunks = ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Vertical)
                    .constraints([
                        ratatui::layout::Constraint::Length(15),
                        ratatui::layout::Constraint::Min(5),
                        ratatui::layout::Constraint::Length(input_height + 2),
                    ])
                    .split(preview);
                PanePreviewWidget::render_summary(frame, preview_chunks[0], state);
                PanePreviewWidget::render_detailed(frame, preview_chunks[1], state);
                InputWidget::render(frame, preview_chunks[2], state);
                SubagentLogWidget::render(frame, subagent_log, state);
            } else {
                // Normal: sidebar | summary+preview+input
                let (left, summary, preview, input_area) = Layout::content_layout_with_input(
                    main_chunks[1],
                    state.sidebar_width,
                    input_height,
                    state.show_summary_detail,
                );
                AgentTreeWidget::render(frame, left, state);
                if state.show_summary_detail {
                    PanePreviewWidget::render_summary(frame, summary, state);
                }
                PanePreviewWidget::render_detailed(frame, preview, state);
                InputWidget::render(frame, input_area, state);
            }

            // Analytics panel (only when visible)
            if state.show_analytics {
                AnalyticsWidget::render(
                    frame,
                    main_chunks[2],
                    &state.usage_tracker,
                    &state.git_info_cache,
                );
            }

            // Queue panel (only when visible)
            if state.show_queue {
                QueuePanelWidget::render(frame, main_chunks[3], state);
            }

            // Dashboard panel (only when visible)
            if state.show_dashboard {
                DashboardWidget::render(frame, main_chunks[4], state);
            }

            // Factory panel (only when visible)
            if state.show_factory {
                FactoryPanelWidget::render(frame, main_chunks[5], state);
            }

            // Footer
            FooterWidget::render(frame, main_chunks[6], state);

            // Help overlay
            if state.show_help {
                HelpWidget::render(frame, size);
            }
        })?;

        // Handle events with short timeout for responsive UI (~60fps)
        let timeout = Duration::from_millis(16);

        tokio::select! {
            // Handle monitor updates
            Some(update) = rx.recv() => {
                state.agents = update.agents;
                state.queue_tasks = update.queue_tasks;
                state.hub_connected = update.hub_connected;
                if let Some(msg) = update.flash {
                    state.flash(msg);
                }
                if let Some(d) = update.digest {
                    state.digest = d;
                }
                if let Some(a) = update.alerts {
                    state.alerts = a;
                }
                if let Some(d) = update.dashboard {
                    state.dashboard = d;
                }
                if let Some(f) = update.factory_requests {
                    state.factory_requests = f;
                }
                // Populate agent branches from git cache
                for agent in state.agents.root_agents.iter_mut() {
                    if agent.branch.is_none() || agent.branch.as_deref() == Some("") {
                        if let Some(info) = state.git_info_cache.get(&agent.path) {
                            agent.branch = Some(info.branch.clone());
                        }
                    }
                }

                // Ensure selected index is valid
                if state.selected_index >= state.agents.root_agents.len() {
                    state.selected_index = state.agents.root_agents.len().saturating_sub(1);
                }
                // Clean up invalid selections
                let max_idx = state.agents.root_agents.len();
                state.selected_agents.retain(|&idx| idx < max_idx);
            }

            // Handle keyboard and mouse events
            _ = tokio::time::sleep(timeout) => {
                // Process all pending events to avoid input lag
                while event::poll(Duration::from_millis(0))? {
                    let event = event::read()?;

                    // Handle mouse events
                    if let Event::Mouse(mouse) = event {
                        let size = terminal.size()?;
                        let area = ratatui::layout::Rect::new(0, 0, size.width, size.height);
                        let main_chunks = Layout::main_layout_all_with_analytics(area, state.show_queue, state.show_dashboard, state.show_factory, state.show_analytics);
                        let footer_area = main_chunks[6];
                        let (sidebar, _, _, input_area) = Layout::content_layout_with_input(
                            main_chunks[1], state.sidebar_width, 3, state.show_summary_detail
                        );

                        match mouse.kind {
                            MouseEventKind::Down(MouseButton::Left) => {
                                let x = mouse.column;
                                let y = mouse.row;

                                // Check footer button clicks first
                                if let Some(button) = FooterWidget::hit_test(x, y, footer_area, state) {
                                    use super::components::FooterButton;
                                    match button {
                                        FooterButton::Approve => {
                                            let indices = state.get_operation_indices();
                                            for idx in indices {
                                                if let Some(agent) = state.agents.get_agent(idx) {
                                                    if agent.status.needs_attention() {
                                                        let target = agent.target.clone();
                                                        let _ = tmux_client.send_keys(&target, "y");
                                                        let _ = tmux_client.send_keys(&target, "Enter");
                                                    }
                                                }
                                            }
                                            state.clear_selection();
                                        }
                                        FooterButton::Reject => {
                                            let indices = state.get_operation_indices();
                                            for idx in indices {
                                                if let Some(agent) = state.agents.get_agent(idx) {
                                                    if agent.status.needs_attention() {
                                                        let target = agent.target.clone();
                                                        let _ = tmux_client.send_keys(&target, "n");
                                                        let _ = tmux_client.send_keys(&target, "Enter");
                                                    }
                                                }
                                            }
                                            state.clear_selection();
                                        }
                                        FooterButton::ApproveAll => {
                                            for agent in &state.agents.root_agents {
                                                if agent.status.needs_attention() {
                                                    let _ = tmux_client.send_keys(&agent.target, "y");
                                                    let _ = tmux_client.send_keys(&agent.target, "Enter");
                                                }
                                            }
                                        }
                                        FooterButton::ToggleSelect => {
                                            state.toggle_selection();
                                        }
                                        FooterButton::Focus => {
                                            if let Some(agent) = state.selected_agent() {
                                                let target = agent.target.clone();
                                                let _ = tmux_client.focus_pane(&target);
                                            }
                                        }
                                        FooterButton::Help => {
                                            state.toggle_help();
                                        }
                                        FooterButton::Quit => {
                                            state.should_quit = true;
                                        }
                                    }
                                }
                                // Check if click is in sidebar - try to select agent
                                else if x >= sidebar.x && x < sidebar.x + sidebar.width
                                    && y >= sidebar.y && y < sidebar.y + sidebar.height
                                {
                                    state.focus_sidebar();
                                    // Calculate which agent was clicked based on row
                                    // Each agent takes ~4 lines in the tree view (varies)
                                    // Simple heuristic: use relative row position
                                    let rel_y = (y - sidebar.y).saturating_sub(1) as usize;
                                    let agents_count = state.agents.root_agents.len();
                                    if agents_count > 0 {
                                        // Estimate ~4 lines per agent (header + info + status)
                                        let estimated_idx = rel_y / 4;
                                        if estimated_idx < agents_count {
                                            state.select_agent(estimated_idx);
                                        }
                                    }
                                }
                                // Check if click is in input area
                                else if x >= input_area.x && x < input_area.x + input_area.width
                                    && y >= input_area.y && y < input_area.y + input_area.height
                                {
                                    state.focus_input();
                                }
                            }
                            MouseEventKind::ScrollUp => {
                                state.select_prev();
                            }
                            MouseEventKind::ScrollDown => {
                                state.select_next();
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Handle keyboard events
                    if let Event::Key(key) = event {
                        let action = map_key_to_action(key.code, key.modifiers, state);

                        match action {
                            Action::Quit => {
                                state.should_quit = true;
                            }
                            Action::NextAgent => {
                                state.select_next();
                            }
                            Action::PrevAgent => {
                                state.select_prev();
                            }
                            Action::ToggleSelection => {
                                state.toggle_selection();
                            }
                            Action::SelectAll => {
                                state.select_all();
                            }
                            Action::ClearSelection => {
                                state.clear_selection();
                            }
                            Action::Approve => {
                                let indices = state.get_operation_indices();
                                let mut approved = 0usize;
                                for idx in &indices {
                                    if let Some(agent) = state.agents.get_agent(*idx) {
                                        if agent.status.needs_attention() {
                                            let target = agent.target.clone();
                                            if let Err(e) = tmux_client.send_keys(&target, "y") {
                                                state.set_error(format!("Failed to approve: {}", e));
                                                break;
                                            }
                                            if let Err(e) = tmux_client.send_keys(&target, "Enter") {
                                                state.set_error(format!("Failed to send Enter: {}", e));
                                                break;
                                            }
                                            approved += 1;
                                        }
                                    }
                                }
                                if approved > 0 {
                                    state.flash(format!("Approved {} agent(s)", approved));
                                }
                                state.clear_selection();
                            }
                            Action::Reject => {
                                let indices = state.get_operation_indices();
                                for idx in indices {
                                    if let Some(agent) = state.agents.get_agent(idx) {
                                        if agent.status.needs_attention() {
                                            let target = agent.target.clone();
                                            if let Err(e) = tmux_client.send_keys(&target, "n") {
                                                state.set_error(format!("Failed to reject: {}", e));
                                                break;
                                            }
                                            if let Err(e) = tmux_client.send_keys(&target, "Enter") {
                                                state.set_error(format!("Failed to send Enter: {}", e));
                                                break;
                                            }
                                        }
                                    }
                                }
                                state.clear_selection();
                            }
                            Action::ApproveAll => {
                                for agent in &state.agents.root_agents {
                                    if agent.status.needs_attention() {
                                        if let Err(e) = tmux_client.send_keys(&agent.target, "y") {
                                            state.set_error(format!("Failed to approve {}: {}", agent.target, e));
                                            break;
                                        }
                                        if let Err(e) = tmux_client.send_keys(&agent.target, "Enter") {
                                            state.set_error(format!("Failed to send Enter to {}: {}", agent.target, e));
                                            break;
                                        }
                                    }
                                }
                            }
                            Action::FocusPane => {
                                if let Some(agent) = state.selected_agent() {
                                    let target = agent.target.clone();
                                    if let Err(e) = tmux_client.focus_pane(&target) {
                                        state.set_error(format!("Failed to focus: {}", e));
                                    }
                                }
                            }
                            Action::ToggleSubagentLog => {
                                state.toggle_subagent_log();
                            }
                            Action::ToggleSummaryDetail => {
                                state.toggle_summary_detail();
                            }
                            Action::Refresh => {
                                state.clear_error();
                            }
                            Action::ShowHelp => {
                                state.toggle_help();
                            }
                            Action::HideHelp => {
                                state.show_help = false;
                            }
                            Action::FocusInput => {
                                state.focus_input();
                            }
                            Action::FocusSidebar => {
                                state.focus_sidebar();
                            }
                            Action::ClearInput => {
                                state.take_input();
                            }
                            Action::InputChar(c) => {
                                state.input_char(c);
                            }
                            Action::InputNewline => {
                                state.input_newline();
                            }
                            Action::InputBackspace => {
                                state.input_backspace();
                            }
                            Action::CursorLeft => {
                                state.cursor_left();
                            }
                            Action::CursorRight => {
                                state.cursor_right();
                            }
                            Action::CursorHome => {
                                state.cursor_home();
                            }
                            Action::CursorEnd => {
                                state.cursor_end();
                            }
                            Action::SendInput => {
                                let input = state.take_input();
                                if !input.is_empty() {
                                    if let Some(agent) = state.selected_agent() {
                                        let target = agent.target.clone();
                                        let agent_path = agent.abbreviated_path();
                                        let pane_num = agent.pane as u8 + 1;

                                        // Use PTY manager in native mode, tmux otherwise
                                        if let Some(ref mgr) = pty_manager {
                                            if let Err(e) = mgr.send_input(pane_num, &input) {
                                                state.set_error(format!("Failed to send input: {}", e));
                                            } else {
                                                state.flash(format!("Sent to {}", agent_path));
                                            }
                                        } else {
                                            if let Err(e) = tmux_client.send_keys_literal(&target, &input) {
                                                state.set_error(format!("Failed to send input: {}", e));
                                            } else if let Err(e) = tmux_client.send_keys(&target, "Enter") {
                                                state.set_error(format!("Failed to send Enter: {}", e));
                                            } else {
                                                state.flash(format!("Sent to {}", agent_path));
                                            }
                                        }
                                    }
                                }
                                // Stay in input mode for consecutive inputs
                            }
                            Action::SendInputToAll => {
                                let input = state.take_input();
                                if !input.is_empty() {
                                    let indices = state.get_operation_indices();
                                    let mut sent = 0usize;
                                    for idx in &indices {
                                        if let Some(agent) = state.agents.get_agent(*idx) {
                                            let target = agent.target.clone();
                                            if tmux_client.send_keys_literal(&target, &input).is_ok() {
                                                let _ = tmux_client.send_keys(&target, "Enter");
                                                sent += 1;
                                            }
                                        }
                                    }
                                    if sent > 0 {
                                        state.flash(format!("Sent to {} agent(s)", sent));
                                    }
                                }
                            }
                            Action::SendNumber(num) => {
                                if let Some(agent) = state.selected_agent() {
                                    let target = agent.target.clone();
                                    let num_str = num.to_string();
                                    if let Err(e) = tmux_client.send_keys(&target, &num_str) {
                                        state.set_error(format!("Failed to send number: {}", e));
                                    } else if let Err(e) = tmux_client.send_keys(&target, "Enter") {
                                        state.set_error(format!("Failed to send Enter: {}", e));
                                    }
                                }
                            }
                            Action::SidebarWider => {
                                state.sidebar_width = (state.sidebar_width + 5).min(70);
                            }
                            Action::SidebarNarrower => {
                                state.sidebar_width = state.sidebar_width.saturating_sub(5).max(15);
                            }
                            Action::SelectAgent(idx) => {
                                state.select_agent(idx);
                            }
                            Action::ScrollUp => {
                                state.select_prev();
                            }
                            Action::ScrollDown => {
                                state.select_next();
                            }
                            Action::ToggleQueue => {
                                state.toggle_queue();
                            }
                            Action::ToggleDashboard => {
                                state.toggle_dashboard();
                            }
                            Action::ToggleFactory => {
                                state.toggle_factory();
                            }
                            Action::ToggleAnalytics => {
                                state.toggle_analytics();
                            }
                            Action::EnterCommandBar => {
                                state.take_input(); // Clear any existing input
                                state.focus_command_bar();
                                if !state.show_factory {
                                    state.show_factory = true;
                                }
                            }
                            Action::SubmitFactory => {
                                let input = state.take_input();
                                if !input.is_empty() {
                                    state.flash("Factory: submitting...".to_string());
                                    let _ = factory_tx.try_send(FactoryCommand::Submit {
                                        request: input,
                                    });
                                }
                                state.focus_sidebar();
                            }
                            Action::CancelCommandBar => {
                                state.take_input();
                                state.focus_sidebar();
                            }
                            Action::PreviewScrollUp => {
                                state.preview_scroll_up(5);
                            }
                            Action::PreviewScrollDown => {
                                state.preview_scroll_down(5);
                            }
                            Action::PreviewScrollBottom => {
                                state.preview_scroll_reset();
                            }
                            Action::None => {}
                        }
                    }
                }
            }
        }

        if state.should_quit {
            break;
        }
    }

    Ok(())
}

fn map_key_to_action(code: KeyCode, modifiers: KeyModifiers, state: &AppState) -> Action {
    // If help is shown, any key closes it
    if state.show_help {
        return Action::HideHelp;
    }

    // If command bar is focused, handle factory command input
    if state.is_command_bar_focused() {
        return match code {
            KeyCode::Esc => Action::CancelCommandBar,
            KeyCode::Enter => Action::SubmitFactory,
            KeyCode::Backspace => Action::InputBackspace,
            KeyCode::Left => Action::CursorLeft,
            KeyCode::Right => Action::CursorRight,
            KeyCode::Home => Action::CursorHome,
            KeyCode::End => Action::CursorEnd,
            KeyCode::Char(c) => Action::InputChar(c),
            _ => Action::None,
        };
    }

    // If input panel is focused, handle input-specific keys
    if state.is_input_focused() {
        return match code {
            // Esc moves focus back to sidebar
            KeyCode::Esc => Action::FocusSidebar,
            // Shift+Enter or Alt+Enter inserts newline
            KeyCode::Enter if modifiers.contains(KeyModifiers::SHIFT) => Action::InputNewline,
            KeyCode::Enter if modifiers.contains(KeyModifiers::ALT) => Action::InputNewline,
            // Ctrl+Enter sends to all selected agents
            KeyCode::Enter if modifiers.contains(KeyModifiers::CONTROL) => Action::SendInputToAll,
            KeyCode::Enter => Action::SendInput,
            KeyCode::Backspace => Action::InputBackspace,
            // Cursor movement
            KeyCode::Left => Action::CursorLeft,
            KeyCode::Right => Action::CursorRight,
            KeyCode::Home => Action::CursorHome,
            KeyCode::End => Action::CursorEnd,
            KeyCode::Char(c) => Action::InputChar(c),
            _ => Action::None,
        };
    }

    // Sidebar focused
    match code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => Action::Quit,

        KeyCode::Char('j') | KeyCode::Down => Action::NextAgent,
        KeyCode::Char('k') | KeyCode::Up => Action::PrevAgent,
        KeyCode::Tab => Action::NextAgent,

        // Left/Right arrows for focus navigation
        KeyCode::Right => Action::FocusInput,
        KeyCode::Left => Action::None, // Already on sidebar

        // Multi-selection
        KeyCode::Char(' ') => Action::ToggleSelection,
        KeyCode::Char('a') if modifiers.contains(KeyModifiers::CONTROL) => Action::SelectAll,

        // Approval
        KeyCode::Char('y') | KeyCode::Char('Y') => Action::Approve,
        KeyCode::Char('n') | KeyCode::Char('N') => Action::Reject,
        KeyCode::Char('a') | KeyCode::Char('A') => Action::ApproveAll,

        // Number keys for quick choice selection (1-9)
        KeyCode::Char(c @ '1'..='9') => {
            let num = c.to_digit(10).unwrap() as u8;
            Action::SendNumber(num)
        }

        // Focus pane with 'f'
        KeyCode::Char('f') | KeyCode::Char('F') => Action::FocusPane,

        KeyCode::Char('s') | KeyCode::Char('S') => Action::ToggleSubagentLog,
        KeyCode::Char('t') | KeyCode::Char('T') => Action::ToggleSummaryDetail,
        KeyCode::Char('r') => Action::Refresh,

        // Sidebar resize (only < and >)
        KeyCode::Char('<') => Action::SidebarNarrower,
        KeyCode::Char('>') => Action::SidebarWider,

        KeyCode::Char('Q') => Action::ToggleQueue,
        KeyCode::Char('D') => Action::ToggleDashboard,
        KeyCode::Char('P') => Action::ToggleFactory,
        KeyCode::Char('X') => Action::ToggleAnalytics,
        KeyCode::Char(':') => Action::EnterCommandBar,
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => Action::PreviewScrollUp,
        KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
            Action::PreviewScrollDown
        }
        KeyCode::Char('g') => Action::PreviewScrollBottom,
        KeyCode::PageUp => Action::PreviewScrollUp,
        KeyCode::PageDown => Action::PreviewScrollDown,
        KeyCode::Char('h') | KeyCode::Char('?') => Action::ShowHelp,

        KeyCode::Esc => {
            if !state.selected_agents.is_empty() {
                Action::ClearSelection
            } else if state.show_subagent_log {
                Action::ToggleSubagentLog
            } else {
                Action::None
            }
        }

        _ => Action::None,
    }
}

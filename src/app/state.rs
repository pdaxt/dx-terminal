use crate::hub_client::{QueueTask, AlertsResponse, AnalyticsDigest, FactoryRequest};
use crate::agents::MonitoredAgent;
use crate::analytics::UsageTracker;
use crate::github::{GitHubTracker, GitInfo};
use crate::monitor::SystemStats;
use crate::state_reader::DashboardData;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// Which panel is currently focused
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusedPanel {
    /// Agent list sidebar is focused
    #[default]
    Sidebar,
    /// Input area is focused
    Input,
    /// Factory command bar is focused
    CommandBar,
}

/// Tree structure containing all monitored agents
#[derive(Debug, Clone, Default)]
pub struct AgentTree {
    /// Root agents (directly in tmux panes)
    pub root_agents: Vec<MonitoredAgent>,
}

impl AgentTree {
    /// Creates an empty agent tree
    pub fn new() -> Self {
        Self {
            root_agents: Vec::new(),
        }
    }

    /// Returns the total number of agents (including subagents)
    pub fn total_count(&self) -> usize {
        self.root_agents.iter().map(|a| 1 + a.subagents.len()).sum()
    }

    /// Returns the number of active agents (those needing attention)
    pub fn active_count(&self) -> usize {
        self.root_agents
            .iter()
            .filter(|a| a.status.needs_attention())
            .count()
    }

    /// Returns the total number of running subagents
    pub fn running_subagent_count(&self) -> usize {
        use crate::agents::SubagentStatus;
        self.root_agents
            .iter()
            .flat_map(|a| &a.subagents)
            .filter(|s| matches!(s.status, SubagentStatus::Running))
            .count()
    }

    /// Returns the number of processing agents
    pub fn processing_count(&self) -> usize {
        use crate::agents::AgentStatus;
        self.root_agents
            .iter()
            .filter(|a| matches!(a.status, AgentStatus::Processing { .. }))
            .count()
    }

    /// Gets an agent by index (for selection)
    pub fn get_agent(&self, index: usize) -> Option<&MonitoredAgent> {
        self.root_agents.get(index)
    }

    /// Gets a mutable agent by index
    pub fn get_agent_mut(&mut self, index: usize) -> Option<&mut MonitoredAgent> {
        self.root_agents.get_mut(index)
    }
}

/// Spinner frames for animation
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Main application state
#[derive(Debug)]
pub struct AppState {
    /// Tree of monitored agents
    pub agents: AgentTree,
    /// Currently selected agent index (cursor position)
    pub selected_index: usize,
    /// Multi-selected agent indices
    pub selected_agents: HashSet<usize>,
    /// Which panel is focused
    pub focused_panel: FocusedPanel,
    /// Input buffer (always available)
    pub input_buffer: String,
    /// Cursor position within input buffer (byte offset)
    pub cursor_position: usize,
    /// Whether help is being shown
    pub show_help: bool,
    /// Whether subagent log is shown
    pub show_subagent_log: bool,
    /// Whether summary detail (TODOs and Tools) is shown
    pub show_summary_detail: bool,
    /// Preview scroll offset (0 = bottom/latest, positive = scrolled up)
    pub preview_scroll: usize,
    /// Whether the application should quit
    pub should_quit: bool,
    /// Last error message (if any)
    pub last_error: Option<String>,
    /// Flash notification (auto-clears after N ticks)
    pub flash_message: Option<(String, usize)>,
    /// Sidebar width in percentage (15-70)
    pub sidebar_width: u16,
    /// Animation tick counter
    pub tick: usize,
    /// Last tick time for animation throttling
    last_tick: Instant,
    /// System resource statistics
    pub system_stats: SystemStats,
    /// Queue tasks
    pub queue_tasks: Vec<QueueTask>,
    /// Whether hub API is connected
    pub hub_connected: bool,
    /// Whether queue panel is shown
    pub show_queue: bool,
    /// Dashboard data (capacity, sprint, board, MCPs, activity)
    pub dashboard: DashboardData,
    /// Whether dashboard panel is shown
    pub show_dashboard: bool,
    /// Whether factory panel is shown
    pub show_factory: bool,
    /// Factory pipeline requests
    pub factory_requests: Vec<FactoryRequest>,
    /// 24h analytics digest from hub API
    pub digest: AnalyticsDigest,
    /// Active alerts from hub API
    pub alerts: AlertsResponse,
    /// Token usage tracker (persists to SQLite)
    pub usage_tracker: UsageTracker,
    /// GitHub/git info per project path
    pub github_tracker: GitHubTracker,
    /// Cached git info for UI rendering (refreshed periodically)
    pub git_info_cache: HashMap<String, GitInfo>,
    /// Whether analytics panel is shown
    pub show_analytics: bool,
}

impl AppState {
    /// Creates a new AppState with default settings
    pub fn new() -> Self {
        Self {
            agents: AgentTree::new(),
            selected_index: 0,
            selected_agents: HashSet::new(),
            focused_panel: FocusedPanel::Sidebar,
            input_buffer: String::new(),
            cursor_position: 0,
            show_help: false,
            show_subagent_log: false,
            show_summary_detail: true,
            preview_scroll: 0,
            should_quit: false,
            last_error: None,
            flash_message: None,
            sidebar_width: 35,
            tick: 0,
            last_tick: Instant::now(),
            system_stats: SystemStats::new(),
            queue_tasks: Vec::new(),
            hub_connected: false,
            show_queue: true,
            dashboard: DashboardData::default(),
            show_dashboard: true,
            show_factory: false,
            factory_requests: Vec::new(),
            digest: AnalyticsDigest::default(),
            alerts: AlertsResponse::default(),
            usage_tracker: UsageTracker::new(),
            github_tracker: GitHubTracker::new(),
            git_info_cache: HashMap::new(),
            show_analytics: true,
        }
    }

    /// Advance the animation tick (throttled to ~10fps for spinner)
    pub fn tick(&mut self) {
        const TICK_INTERVAL_MS: u128 = 80; // ~12fps for smooth spinner
        if self.last_tick.elapsed().as_millis() >= TICK_INTERVAL_MS {
            self.tick = self.tick.wrapping_add(1);
            self.last_tick = Instant::now();
            self.clear_expired_flash();
        }
    }

    /// Get the current spinner frame
    pub fn spinner_frame(&self) -> &'static str {
        SPINNER_FRAMES[self.tick % SPINNER_FRAMES.len()]
    }

    /// Check if input panel is focused
    pub fn is_input_focused(&self) -> bool {
        self.focused_panel == FocusedPanel::Input
    }

    /// Check if command bar is focused
    pub fn is_command_bar_focused(&self) -> bool {
        self.focused_panel == FocusedPanel::CommandBar
    }

    /// Focus on the input panel
    pub fn focus_input(&mut self) {
        self.focused_panel = FocusedPanel::Input;
    }

    /// Focus on the sidebar
    pub fn focus_sidebar(&mut self) {
        self.focused_panel = FocusedPanel::Sidebar;
    }

    /// Focus on the command bar
    pub fn focus_command_bar(&mut self) {
        self.focused_panel = FocusedPanel::CommandBar;
    }

    /// Toggle focus between panels
    pub fn toggle_focus(&mut self) {
        self.focused_panel = match self.focused_panel {
            FocusedPanel::Sidebar => FocusedPanel::Input,
            FocusedPanel::Input => FocusedPanel::Sidebar,
            FocusedPanel::CommandBar => FocusedPanel::Sidebar,
        };
    }

    /// Add a character to the input buffer at cursor position
    pub fn input_char(&mut self, c: char) {
        self.input_buffer.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
    }

    /// Add a newline to the input buffer at cursor position
    pub fn input_newline(&mut self) {
        self.input_buffer.insert(self.cursor_position, '\n');
        self.cursor_position += 1;
    }

    /// Delete the character before the cursor
    pub fn input_backspace(&mut self) {
        if self.cursor_position > 0 {
            // Find the previous character boundary
            let prev_boundary = self.input_buffer[..self.cursor_position]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.input_buffer.remove(prev_boundary);
            self.cursor_position = prev_boundary;
        }
    }

    /// Get the current input buffer
    pub fn get_input(&self) -> &str {
        &self.input_buffer
    }

    /// Get the current cursor position
    pub fn get_cursor_position(&self) -> usize {
        self.cursor_position
    }

    /// Take and clear the input buffer
    pub fn take_input(&mut self) -> String {
        self.cursor_position = 0;
        std::mem::take(&mut self.input_buffer)
    }

    /// Move cursor left by one character
    pub fn cursor_left(&mut self) {
        if self.cursor_position > 0 {
            // Find the previous character boundary
            self.cursor_position = self.input_buffer[..self.cursor_position]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor right by one character
    pub fn cursor_right(&mut self) {
        if self.cursor_position < self.input_buffer.len() {
            // Find the next character boundary
            if let Some(c) = self.input_buffer[self.cursor_position..].chars().next() {
                self.cursor_position += c.len_utf8();
            }
        }
    }

    /// Move cursor to the beginning of the input
    pub fn cursor_home(&mut self) {
        self.cursor_position = 0;
    }

    /// Move cursor to the end of the input
    pub fn cursor_end(&mut self) {
        self.cursor_position = self.input_buffer.len();
    }

    /// Returns the currently selected agent
    pub fn selected_agent(&self) -> Option<&MonitoredAgent> {
        self.agents.get_agent(self.selected_index)
    }

    /// Returns the currently selected agent mutably
    pub fn selected_agent_mut(&mut self) -> Option<&mut MonitoredAgent> {
        self.agents.get_agent_mut(self.selected_index)
    }

    /// Selects the next agent
    pub fn select_next(&mut self) {
        if !self.agents.root_agents.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.agents.root_agents.len();
            self.preview_scroll = 0;
        }
    }

    /// Selects the previous agent
    pub fn select_prev(&mut self) {
        if !self.agents.root_agents.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.agents.root_agents.len() - 1;
            } else {
                self.selected_index -= 1;
            }
            self.preview_scroll = 0;
        }
    }

    /// Selects an agent by index
    pub fn select_agent(&mut self, index: usize) {
        if index < self.agents.root_agents.len() {
            self.selected_index = index;
            self.preview_scroll = 0;
        }
    }

    /// Toggles selection of the current agent
    pub fn toggle_selection(&mut self) {
        if self.selected_agents.contains(&self.selected_index) {
            self.selected_agents.remove(&self.selected_index);
        } else {
            self.selected_agents.insert(self.selected_index);
        }
    }

    /// Selects all agents
    pub fn select_all(&mut self) {
        for i in 0..self.agents.root_agents.len() {
            self.selected_agents.insert(i);
        }
    }

    /// Clears all selections
    pub fn clear_selection(&mut self) {
        self.selected_agents.clear();
    }

    /// Returns indices to operate on (selected agents, or current if none selected)
    pub fn get_operation_indices(&self) -> Vec<usize> {
        if self.selected_agents.is_empty() {
            vec![self.selected_index]
        } else {
            let mut indices: Vec<usize> = self.selected_agents.iter().copied().collect();
            indices.sort();
            indices
        }
    }

    /// Check if an agent is in multi-selection
    pub fn is_multi_selected(&self, index: usize) -> bool {
        self.selected_agents.contains(&index)
    }

    /// Toggles help display
    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    /// Toggles subagent log display
    pub fn toggle_subagent_log(&mut self) {
        self.show_subagent_log = !self.show_subagent_log;
    }

    /// Toggles summary detail (TODOs and Tools) display
    pub fn toggle_summary_detail(&mut self) {
        self.show_summary_detail = !self.show_summary_detail;
    }

    /// Scroll preview up by N lines (clamped to content length)
    pub fn preview_scroll_up(&mut self, lines: usize) {
        let max_scroll = self.max_preview_scroll();
        self.preview_scroll = self.preview_scroll.saturating_add(lines).min(max_scroll);
    }

    /// Maximum scroll offset based on selected agent's content
    fn max_preview_scroll(&self) -> usize {
        self.selected_agent()
            .map(|a| a.last_content.lines().count().saturating_sub(1))
            .unwrap_or(0)
    }

    /// Scroll preview down by N lines (toward bottom)
    pub fn preview_scroll_down(&mut self, lines: usize) {
        self.preview_scroll = self.preview_scroll.saturating_sub(lines);
    }

    /// Reset preview scroll to bottom (latest output)
    pub fn preview_scroll_reset(&mut self) {
        self.preview_scroll = 0;
    }

    /// Toggles queue panel visibility
    pub fn toggle_queue(&mut self) {
        self.show_queue = !self.show_queue;
    }

    /// Toggles dashboard panel visibility
    pub fn toggle_dashboard(&mut self) {
        self.show_dashboard = !self.show_dashboard;
    }

    /// Toggles factory panel visibility
    pub fn toggle_factory(&mut self) {
        self.show_factory = !self.show_factory;
    }

    /// Toggles analytics panel visibility
    pub fn toggle_analytics(&mut self) {
        self.show_analytics = !self.show_analytics;
    }

    /// Refresh git info for all tracked agent paths
    pub fn refresh_git_info(&mut self) {
        let paths: Vec<String> = self.agents.root_agents.iter()
            .map(|a| a.path.clone())
            .filter(|p| !p.is_empty())
            .collect();
        for path in paths {
            let info = self.github_tracker.get_info(&path).clone();
            self.git_info_cache.insert(path, info);
        }
    }

    /// Sets an error message
    pub fn set_error(&mut self, message: String) {
        self.last_error = Some(message);
    }

    /// Clears the error message
    pub fn clear_error(&mut self) {
        self.last_error = None;
    }

    /// Show a flash notification that auto-clears after ~3 seconds
    pub fn flash(&mut self, message: String) {
        self.flash_message = Some((message, self.tick + 36)); // ~3s at 12fps
    }

    /// Check and clear expired flash messages (call in tick)
    pub fn clear_expired_flash(&mut self) {
        if let Some((_, expires)) = &self.flash_message {
            if self.tick >= *expires {
                self.flash_message = None;
            }
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::AgentType;

    #[test]
    fn test_app_state_navigation() {
        let mut state = AppState::new();

        // Add some agents
        state.agents.root_agents.push(MonitoredAgent::new(
            "1".to_string(),
            "main:0.0".to_string(),
            "main".to_string(),
            0,
            "code".to_string(),
            0,
            "/home/user/project1".to_string(),
            AgentType::ClaudeCode,
            1000,
        ));
        state.agents.root_agents.push(MonitoredAgent::new(
            "2".to_string(),
            "main:0.1".to_string(),
            "main".to_string(),
            0,
            "code".to_string(),
            1,
            "/home/user/project2".to_string(),
            AgentType::OpenCode,
            1001,
        ));

        assert_eq!(state.selected_index, 0);
        state.select_next();
        assert_eq!(state.selected_index, 1);
        state.select_next();
        assert_eq!(state.selected_index, 0); // Wraps around
        state.select_prev();
        assert_eq!(state.selected_index, 1); // Wraps around
    }
}

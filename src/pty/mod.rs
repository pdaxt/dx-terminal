pub mod agent;
pub mod health;
pub mod output;

use self::agent::AgentHandle;
use self::health::PaneHealth;
use std::collections::HashMap;

/// Manages all PTY-based agent processes
#[derive(Default)]
pub struct PtyManager {
    pub agents: HashMap<u8, AgentHandle>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawn a new agent in a PTY
    pub fn spawn(
        &mut self,
        pane_num: u8,
        command: &str,
        args: &[&str],
        cwd: &str,
        env_vars: Vec<(String, String)>,
    ) -> anyhow::Result<()> {
        // Kill existing agent on this pane first
        if self.agents.contains_key(&pane_num) {
            self.kill(pane_num)?;
        }

        let handle = AgentHandle::spawn(command, args, cwd, env_vars, 50, 120)?;
        self.agents.insert(pane_num, handle);
        Ok(())
    }

    /// Kill an agent on a pane
    pub fn kill(&mut self, pane_num: u8) -> anyhow::Result<()> {
        if let Some(mut handle) = self.agents.remove(&pane_num) {
            handle.kill()?;
        }
        Ok(())
    }

    /// Send a line of text to an agent's PTY
    pub fn send_line(&mut self, pane_num: u8, text: &str) -> anyhow::Result<()> {
        if let Some(handle) = self.agents.get_mut(&pane_num) {
            handle.send_line(text)?;
        } else {
            anyhow::bail!("No agent on pane {}", pane_num);
        }
        Ok(())
    }

    /// Get last N lines of output from an agent
    pub fn last_output(&self, pane_num: u8, lines: usize) -> Option<String> {
        self.agents.get(&pane_num).map(|h| h.last_output(lines))
    }

    /// Get vt100 screen text from an agent
    pub fn screen_text(&self, pane_num: u8) -> Option<String> {
        self.agents.get(&pane_num).map(|h| h.screen_text())
    }

    /// Check if a pane has a running agent
    pub fn is_running(&self, pane_num: u8) -> bool {
        self.agents.get(&pane_num).is_some_and(|h| h.is_running())
    }

    /// Check if a pane has any agent (running or not)
    pub fn has_agent(&self, pane_num: u8) -> bool {
        self.agents.contains_key(&pane_num)
    }

    /// Health check a pane
    pub fn check_health(&self, pane_num: u8, markers: &[String]) -> PaneHealth {
        health::check_pane(self, pane_num, markers)
    }

    /// Get exit code for a pane's agent
    pub fn exit_code(&self, pane_num: u8) -> Option<i32> {
        self.agents.get(&pane_num).and_then(|h| h.exit_code())
    }

    /// Get line count for a pane
    pub fn line_count(&self, pane_num: u8) -> usize {
        self.agents.get(&pane_num).map_or(0, |h| h.line_count())
    }

    /// Kill all agents (shutdown)
    pub fn kill_all(&mut self) {
        let panes: Vec<u8> = self.agents.keys().copied().collect();
        for pane in panes {
            let _ = self.kill(pane);
        }
    }
}

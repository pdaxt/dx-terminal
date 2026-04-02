//! Background health monitor: polls agent panes every 2s, classifies health,
//! and updates state. Enables real-time stuck/dead/rate-limited detection.
//!
//! Issue #6: Real-time agent health monitoring in TUI dashboard.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tokio::time::interval;

use crate::app::App;
use crate::config;
use crate::state::types::PaneHealthStatus;
use crate::tmux;

/// How often to poll pane output for health changes.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Seconds of unchanged output before marking an active agent as "stuck".
/// AI agents legitimately pause for 30-60s during API calls, so we use
/// a generous threshold. Tune this based on observed agent behavior.
const STUCK_THRESHOLD_SECS: i64 = 120;

/// Handle returned by [`start`] to stop the monitor.
pub struct HealthMonitorHandle {
    stop_tx: watch::Sender<bool>,
    handle: tokio::task::JoinHandle<()>,
}

impl HealthMonitorHandle {
    /// Signal the monitor to stop and wait for it to finish.
    pub async fn stop(self) {
        let _ = self.stop_tx.send(true);
        let _ = self.handle.await;
    }
}

/// Start the background health monitor. Polls all panes every 2s.
pub fn start(app: Arc<App>) -> HealthMonitorHandle {
    let (stop_tx, stop_rx) = watch::channel(false);

    let handle = tokio::spawn(async move {
        let mut ticker = interval(POLL_INTERVAL);
        loop {
            ticker.tick().await;
            if *stop_rx.borrow() {
                break;
            }
            poll_all_panes(&app).await;
        }
    });

    HealthMonitorHandle { stop_tx, handle }
}

/// Poll all configured panes and update their health.
async fn poll_all_panes(app: &App) {
    let pane_count = config::pane_count();

    for pane in 1..=pane_count {
        let ps = app.state.get_pane(pane).await;

        // Capture output: tmux-first, then PTY fallback
        let output = if let Some(ref target) = ps.tmux_target {
            if tmux::pane_exists(target) {
                tmux::capture_output_extended(target, 40)
            } else {
                String::new()
            }
        } else {
            let pty = app.pty_lock();
            pty.last_output(pane, 40).unwrap_or_default()
        };

        let output_hash = hash_output(&output);
        let health = classify(pane, &ps, &output, app).await;

        app.state.update_pane_health(pane, health, output_hash).await;
    }
}

/// Classify the health of a single pane based on its output and state.
async fn classify(
    pane: u8,
    ps: &crate::state::types::PaneState,
    output: &str,
    app: &App,
) -> PaneHealthStatus {
    let trimmed = output.trim();

    // No output and no target = empty pane
    if trimmed.is_empty() && ps.tmux_target.is_none() {
        let pty = app.pty_lock();
        if !pty.is_running(pane) {
            return PaneHealthStatus::Empty;
        }
    }

    // Check for rate limiting
    if is_rate_limited(trimmed) {
        return PaneHealthStatus::RateLimited;
    }

    // Check for approval prompts
    if is_awaiting_approval(trimmed) {
        return PaneHealthStatus::AwaitingApproval;
    }

    // Check for errors / crashes
    if is_dead(trimmed, pane, &ps.tmux_target, app) {
        return PaneHealthStatus::Dead;
    }

    // Check for completion (shell prompt after work — agent exited)
    if is_finished(trimmed, &ps.tmux_target) {
        return PaneHealthStatus::Finished;
    }

    // Check if agent is at idle prompt (Claude Code ❯, Codex ›, etc.)
    if is_idle_prompt(trimmed) {
        return PaneHealthStatus::Idle;
    }

    // Check for stuck (output unchanged for STUCK_THRESHOLD_SECS)
    if let Some(ref last_changed) = ps.last_output_changed_at {
        if is_stuck(last_changed) {
            return PaneHealthStatus::Stuck;
        }
    }

    // If output is flowing and we didn't match any terminal state, it's working
    if ps.tmux_target.is_some() {
        return PaneHealthStatus::Working;
    }

    // Default: idle
    PaneHealthStatus::Idle
}

/// Check if the agent is at an idle prompt waiting for input.
/// Detects Claude Code (❯), Codex CLI (›), shell prompts, and common idle indicators.
fn is_idle_prompt(output: &str) -> bool {
    // Take the last ~400 chars, but snap to a char boundary
    let tail: &str = if output.len() > 400 {
        let start = output.len() - 400;
        // Find the next valid char boundary
        let safe_start = output.ceil_char_boundary(start);
        &output[safe_start..]
    } else {
        output
    };
    let lower = tail.to_lowercase();

    // Claude Code idle indicators
    let idle_patterns = [
        "? for shortcuts",
        "esc to interrupt",
        "/help for commands",
        "for shortcuts",
        "left \u{00b7}",  // "left ·" — Codex context remaining
    ];

    // Must have an idle pattern AND no active work indicators
    let has_idle_pattern = idle_patterns.iter().any(|p| lower.contains(p));
    let has_work_indicator = lower.contains("thinking")
        || lower.contains("analyzing")
        || lower.contains("reading")
        || lower.contains("writing")
        || lower.contains("running");

    has_idle_pattern && !has_work_indicator
}

/// Check if output indicates rate limiting.
fn is_rate_limited(output: &str) -> bool {
    let patterns = [
        "rate limit",
        "hit your limit",
        "too many requests",
        "429",
        "quota exceeded",
        "Usage limit reached",
    ];
    let lower = output.to_lowercase();
    patterns.iter().any(|p| lower.contains(&p.to_lowercase()))
}

/// Check if output shows an approval/permission prompt.
fn is_awaiting_approval(output: &str) -> bool {
    let tail: &str = if output.len() > 600 {
        let start = output.len() - 600;
        let safe_start = output.ceil_char_boundary(start);
        &output[safe_start..]
    } else {
        output
    };
    let lower = tail.to_lowercase();
    lower.contains("allow ")
        || lower.contains("(y/n)")
        || lower.contains("approve?")
        || lower.contains("yes, allow")
        || lower.contains("do you want to")
        || lower.contains("permission required")
}

/// Check if the pane's process is dead.
fn is_dead(output: &str, pane: u8, tmux_target: &Option<String>, app: &App) -> bool {
    // If tmux target exists but pane is gone
    if let Some(ref target) = tmux_target {
        if !tmux::pane_exists(target) {
            return true;
        }
    }

    // If PTY and process exited with error
    let pty = app.pty_lock();
    if let Some(code) = pty.exit_code(pane) {
        if code != 0 {
            return true;
        }
    }

    // Check for fatal patterns
    let fatal = ["panic:", "SIGTERM", "SIGKILL", "Killed", "Segmentation fault"];
    fatal.iter().any(|p| output.contains(p))
}

/// Check if the agent finished (shell prompt after work).
fn is_finished(output: &str, tmux_target: &Option<String>) -> bool {
    if let Some(ref target) = tmux_target {
        return tmux::check_done(target);
    }

    // Check last line for shell prompt
    output
        .lines()
        .last()
        .map(|line| {
            let l = line.trim();
            l.ends_with('$') || l.ends_with("$ ") || l.ends_with('%') || l.ends_with("% ")
        })
        .unwrap_or(false)
}

/// Check if output hasn't changed for longer than the stuck threshold.
fn is_stuck(last_changed_at: &str) -> bool {
    let now = chrono::Local::now().naive_local();
    if let Ok(changed) = chrono::NaiveDateTime::parse_from_str(last_changed_at, "%Y-%m-%dT%H:%M:%S")
    {
        let elapsed = now.signed_duration_since(changed);
        return elapsed.num_seconds() > STUCK_THRESHOLD_SECS;
    }
    false
}

/// Hash output for change detection.
fn hash_output(output: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    output.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_detection() {
        assert!(is_rate_limited("Error: rate limit exceeded, try again"));
        assert!(is_rate_limited("HTTP 429 Too Many Requests"));
        assert!(is_rate_limited("Usage limit reached for this billing period"));
        assert!(!is_rate_limited("Running cargo test..."));
    }

    #[test]
    fn approval_detection() {
        assert!(is_awaiting_approval("Allow Bash: rm -rf /tmp (y/n)?"));
        assert!(is_awaiting_approval("Do you want to make this edit?"));
        assert!(is_awaiting_approval("Permission required:\n> 1. Yes, allow once"));
        assert!(!is_awaiting_approval("Building project..."));
    }

    #[test]
    fn finished_detection() {
        assert!(is_finished("done\npran@mac ~/Projects $", &None));
        assert!(is_finished("complete\n% ", &None));
        assert!(!is_finished("Running tests...\ntest 1 passed", &None));
    }

    #[test]
    fn stuck_detection_old_timestamp() {
        // 5 minutes ago should be stuck (threshold is 120s)
        let five_min_ago = (chrono::Local::now() - chrono::Duration::minutes(5))
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();
        assert!(is_stuck(&five_min_ago));
    }

    #[test]
    fn stuck_detection_recent_timestamp() {
        // 10 seconds ago should NOT be stuck
        let ten_sec_ago = (chrono::Local::now() - chrono::Duration::seconds(10))
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();
        assert!(!is_stuck(&ten_sec_ago));
    }

    #[test]
    fn hash_output_deterministic() {
        let h1 = hash_output("hello world");
        let h2 = hash_output("hello world");
        let h3 = hash_output("different");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn fatal_pattern_in_output() {
        // Test the pattern matching part of is_dead without needing an App
        let fatal = ["panic:", "SIGTERM", "SIGKILL", "Killed", "Segmentation fault"];
        assert!(fatal.iter().any(|p| "thread 'main' panic: index out of bounds".contains(p)));
        assert!(!fatal.iter().any(|p| "Running cargo test...".contains(p)));
        assert!(fatal.iter().any(|p| "process received SIGTERM".contains(p)));
        assert!(fatal.iter().any(|p| "Segmentation fault (core dumped)".contains(p)));
    }
}

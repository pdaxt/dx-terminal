//! Auto-approval for Claude permission prompts in managed tmux panes.
//!
//! Polls each pane every 200ms via `tmux capture-pane`, matches the last
//! visible lines against known Claude Code permission patterns, and sends
//! the correct keystrokes to approve.  Uses only string operations — no
//! grep, no pipes, no external pattern tools.

use std::collections::HashMap;
use std::process::Command;
use std::time::{Duration, Instant};
use tokio::sync::watch;

/// Minimum time between approvals on the same pane to avoid double-sends.
const COOLDOWN: Duration = Duration::from_millis(600);

/// How often we poll each pane.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// A recognized permission prompt and the keys to send.
struct Pattern {
    /// Substring to look for (case-insensitive match against last N lines).
    needle: &'static str,
    /// tmux send-keys arguments to approve this prompt.
    keys: &'static [&'static str],
}

/// All patterns we recognize. Order matters — first match wins.
const PATTERNS: &[Pattern] = &[
    // Claude Code: "Do you want to proceed?" / "Do you want to make this edit?"
    Pattern {
        needle: "do you want to",
        keys: &["Enter"],
    },
    // Claude Code: numbered menu "❯ 1. Yes, allow once" (must be before generic "allow")
    Pattern {
        needle: "yes, allow",
        keys: &["Enter"],
    },
    // Claude Code: "Allow <tool>?" with Enter for yes
    Pattern {
        needle: "allow ",
        keys: &["y", "Enter"],
    },
    // Claude Code: "Press Enter to continue"
    Pattern {
        needle: "press enter to continue",
        keys: &["Enter"],
    },
    // Claude Code: numbered choice list with cursor on "Yes"
    Pattern {
        needle: "\u{276f} 1.",
        keys: &["Enter"],
    },
    // Codex CLI: "Approve? (y/n)"
    Pattern {
        needle: "approve? (y",
        keys: &["y", "Enter"],
    },
    // Generic (y/N) or (Y/n) confirmation
    Pattern {
        needle: "(y/n)",
        keys: &["y", "Enter"],
    },
];

/// Capture the last ~20 lines of a tmux pane. Returns empty string on failure.
fn capture_pane(target: &str) -> String {
    Command::new("tmux")
        .args(["capture-pane", "-t", target, "-p", "-S", "-20"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

/// Send a sequence of keys to a tmux pane.
fn send_keys(target: &str, keys: &[&str]) {
    for key in keys {
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", target, key])
            .status();
    }
}

/// Check a pane's output against all patterns. Returns the matched pattern
/// index (for logging) and the keys to send, or `None`.
fn check_pane(output: &str) -> Option<(usize, &'static [&'static str])> {
    // Build a lowercase view of the non-empty trailing lines.
    let lower = output.to_lowercase();
    // Only scan the last portion — avoids matching stale approvals higher up.
    let tail: &str = {
        let bytes = lower.as_bytes();
        let start = bytes.len().saturating_sub(1200);
        &lower[start..]
    };

    for (idx, pat) in PATTERNS.iter().enumerate() {
        if tail.contains(pat.needle) {
            return Some((idx, pat.keys));
        }
    }
    None
}

/// Handle returned by [`start`] that can stop the approval loop.
pub struct ApprovalHandle {
    stop_tx: watch::Sender<bool>,
    handle: tokio::task::JoinHandle<ApprovalStats>,
}

/// Cumulative statistics from an approval loop run.
#[derive(Debug, Clone, Default)]
pub struct ApprovalStats {
    pub approvals: u64,
    pub polls: u64,
}

impl ApprovalHandle {
    /// Signal the loop to stop and wait for it to finish.
    pub async fn stop(self) -> ApprovalStats {
        let _ = self.stop_tx.send(true);
        self.handle.await.unwrap_or_default()
    }
}

/// Start the approval loop for the given tmux pane targets.
/// Returns a handle that can stop the loop.
pub fn start(pane_targets: Vec<String>) -> ApprovalHandle {
    let (stop_tx, stop_rx) = watch::channel(false);
    let handle = tokio::spawn(approval_loop(pane_targets, stop_rx));
    ApprovalHandle { stop_tx, handle }
}

async fn approval_loop(
    pane_targets: Vec<String>,
    mut stop_rx: watch::Receiver<bool>,
) -> ApprovalStats {
    let mut stats = ApprovalStats::default();
    let mut cooldowns: HashMap<String, Instant> = HashMap::new();
    let mut interval = tokio::time::interval(POLL_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = interval.tick() => {}
            _ = stop_rx.changed() => {
                if *stop_rx.borrow() {
                    break;
                }
            }
        }

        stats.polls += 1;

        for target in &pane_targets {
            // Respect cooldown
            if let Some(last) = cooldowns.get(target) {
                if last.elapsed() < COOLDOWN {
                    continue;
                }
            }

            let output = capture_pane(target);
            if output.trim().is_empty() {
                continue;
            }

            if let Some((idx, keys)) = check_pane(&output) {
                tracing::info!(
                    target = %target,
                    pattern = idx,
                    "auto-approving permission prompt"
                );
                send_keys(target, keys);
                cooldowns.insert(target.clone(), Instant::now());
                stats.approvals += 1;
            }
        }
    }

    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_do_you_want() {
        let output = "Some output\n  Do you want to make this edit? (y/n)\n";
        let result = check_pane(output);
        assert!(result.is_some());
        let (idx, keys) = result.unwrap();
        assert_eq!(idx, 0); // "do you want to"
        assert_eq!(keys, &["Enter"]);
    }

    #[test]
    fn matches_allow_bash() {
        let output = "Working...\n  Allow Bash: rm -rf /tmp/test (y/n)? Enter for yes\n";
        let result = check_pane(output);
        assert!(result.is_some());
        let (_, keys) = result.unwrap();
        assert_eq!(keys, &["y", "Enter"]);
    }

    #[test]
    fn matches_numbered_menu() {
        let output = "Permission required:\n❯ 1. Yes, allow once\n  2. No\n";
        let result = check_pane(output);
        assert!(result.is_some());
        let (_, keys) = result.unwrap();
        assert_eq!(keys, &["Enter"]);
    }

    #[test]
    fn matches_approve_yn() {
        let output = "Codex wants to run tests.\nApprove? (y/n) ";
        let result = check_pane(output);
        assert!(result.is_some());
        let (_, keys) = result.unwrap();
        assert_eq!(keys, &["y", "Enter"]);
    }

    #[test]
    fn no_match_on_normal_output() {
        let output = "Running cargo test...\ntest result: ok. 16 passed\n$ ";
        assert!(check_pane(output).is_none());
    }

    #[test]
    fn matches_only_tail_not_stale() {
        // Pattern appears far above the 1200-byte tail window
        let mut output = "Allow Bash: old command\n".to_string();
        output.push_str(&"x".repeat(1400));
        output.push_str("\nRunning tests...\n");
        assert!(check_pane(&output).is_none());
    }
}

//! RuntimeReplicator — single server-side task that owns all live polling.
//!
//! Instead of each WebSocket connection spawning its own tmux poller and JSONL tailer,
//! one replicator task discovers panes, captures output, tails sessions, and publishes
//! typed deltas through the EventBus. WebSocket handlers just forward these events.
//!
//! This fixes:
//! - Per-client polling duplication (cost scales with clients × panes)
//! - Unstable pane identity (pane number used as both display slot and entity ID)
//! - Lossy session streaming (re-reads last N events each cycle instead of cursor-based)
//! - Missing events from mutation paths (set_pane without broadcast)

use std::collections::HashMap;
use std::sync::Arc;
use serde_json::json;

use crate::app::App;
use crate::session_stream::SessionTailer;
use crate::state::events::StateEvent;
use crate::tmux;

/// Start the runtime replicator as a background tokio task.
/// Call once at server startup. All clients receive events through the EventBus.
pub fn start(app: Arc<App>) {
    tokio::spawn(run_replicator(app));
}

async fn run_replicator(app: Arc<App>) {
    let interval = tokio::time::Duration::from_secs(1);
    let mut prev_outputs: HashMap<String, String> = HashMap::new();
    let mut session_tailer = SessionTailer::new();

    // Track pane→tmux_target mapping for stable identity
    let mut pane_targets: HashMap<u8, String> = HashMap::new();

    tracing::info!("RuntimeReplicator started — polling tmux + JSONL every 1s");

    loop {
        tokio::time::sleep(interval).await;

        let state = app.state.get_state_snapshot().await;
        let max_panes = crate::config::pane_count();

        // --- Phase 1: Discover live panes (once, shared across all clients) ---
        let live_panes = match tokio::task::spawn_blocking(|| {
            tmux::discover_live_panes()
        }).await {
            Ok(panes) => panes,
            Err(_) => continue,
        };

        // Build authoritative target list: state panes first, then discovered
        let mut targets: Vec<(u8, String, Option<usize>)> = Vec::new(); // (pane_num, target, live_idx)
        let mut used_targets: std::collections::HashSet<String> = std::collections::HashSet::new();

        // 1) State-managed panes with tmux targets
        for i in 1..=max_panes {
            if let Some(p) = state.panes.get(&i.to_string()) {
                if let Some(ref target) = p.tmux_target {
                    targets.push((i, target.clone(), None));
                    used_targets.insert(target.clone());
                }
            }
        }

        // 2) Auto-discovered panes that aren't already in state
        let mut next_pane = max_panes + 1;
        for (idx, lp) in live_panes.iter().enumerate() {
            if !used_targets.contains(&lp.target) {
                // Try to assign to an empty state slot first
                let pane_num = if (idx + 1) as u8 <= max_panes
                    && !targets.iter().any(|(p, _, _)| *p == (idx + 1) as u8)
                {
                    (idx + 1) as u8
                } else {
                    let n = next_pane;
                    next_pane += 1;
                    n
                };
                targets.push((pane_num, lp.target.clone(), Some(idx)));
                used_targets.insert(lp.target.clone());
            }
        }

        // Update stable identity map
        let new_targets: HashMap<u8, String> = targets.iter()
            .map(|(p, t, _)| (*p, t.clone()))
            .collect();

        // Detect panes that disappeared since last cycle
        for (pane, old_target) in &pane_targets {
            if !new_targets.contains_key(pane) {
                // Pane disappeared — but don't override reconciler's judgment
                tracing::debug!("Replicator: pane {} (target {}) no longer discovered", pane, old_target);
            }
        }
        pane_targets = new_targets;

        if targets.is_empty() {
            continue;
        }

        // --- Phase 2: Capture terminal output diffs (once for all clients) ---
        let capture_targets: Vec<(u8, String)> = targets.iter()
            .map(|(p, t, _)| (*p, t.clone()))
            .collect();

        let captures: Vec<(u8, String, String)> = match tokio::task::spawn_blocking(move || {
            capture_targets.iter().map(|(i, target)| {
                (*i, target.clone(), tmux::capture_output(target))
            }).collect::<Vec<_>>()
        }).await {
            Ok(c) => c,
            Err(_) => continue,
        };

        for (pane_num, target, output) in captures {
            let key = format!("{}:{}", pane_num, target);
            let prev = prev_outputs.get(&key).map(|s| s.as_str()).unwrap_or("");

            if output != prev {
                let new_lines = if output.len() > prev.len() && output.starts_with(prev) {
                    output[prev.len()..].to_string()
                } else {
                    let lines: Vec<&str> = output.lines().collect();
                    let tail_start = lines.len().saturating_sub(30);
                    lines[tail_start..].join("\n")
                };

                if !new_lines.trim().is_empty() {
                    app.state.event_bus.send(StateEvent::OutputChunk {
                        pane: pane_num,
                        output: new_lines,
                        full_lines: output.lines().count(),
                        tmux_target: Some(target.clone()),
                    });
                }

                prev_outputs.insert(key, output);
            }
        }

        // --- Phase 3: Cursor-based JSONL tailing (once for all clients) ---
        let jsonl_polls: Vec<(u8, String)> = live_panes.iter().enumerate()
            .filter_map(|(idx, lp)| {
                lp.jsonl_path.as_ref().map(|jp| {
                    let pane_num = if idx < max_panes as usize {
                        (idx + 1) as u8
                    } else {
                        max_panes + 1 + idx as u8
                    };
                    (pane_num, jp.clone())
                })
            })
            .collect();

        if !jsonl_polls.is_empty() {
            // Use cursor-based tailing — no duplicate events, no missed events
            let tailer = &mut session_tailer;
            let session_updates: Vec<(u8, Vec<crate::session_stream::SessionEvent>)> =
                jsonl_polls.iter().filter_map(|(pane_num, jp)| {
                    let events = tailer.poll_new_events(jp, 20);
                    if events.is_empty() { None } else { Some((*pane_num, events)) }
                }).collect();

            for (pane_num, events) in session_updates {
                app.state.event_bus.send(StateEvent::SessionEventChunk {
                    pane: pane_num,
                    events: json!(events),
                });
            }
        }

        // --- Phase 4: Forward sync status periodically ---
        // (SyncManager already broadcasts SyncEvents — we just ensure they're in the bus)
        // This is handled by forward_sync_events in ws.rs, but we could consolidate later.
    }
}

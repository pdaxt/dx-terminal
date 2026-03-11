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

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use serde_json::json;

use crate::app::App;
use crate::session_stream::SessionTailer;
use crate::state::events::StateEvent;
use crate::state::types::DxTerminalState;
use crate::tmux;

#[derive(Clone, Debug, PartialEq)]
struct FeatureSnapshot {
    title: String,
    phase: String,
    state: String,
    readiness: serde_json::Value,
}

/// Start the runtime replicator as a background tokio task.
/// Call once at server startup. All clients receive events through the EventBus.
pub fn start(app: Arc<App>) {
    tokio::spawn(run_replicator(app));
}

async fn run_replicator(app: Arc<App>) {
    let interval = tokio::time::Duration::from_secs(1);
    let mut prev_outputs: HashMap<String, String> = HashMap::new();
    let mut session_tailer = SessionTailer::new();
    let mut vision_fingerprints: HashMap<String, u64> = HashMap::new();
    let mut vision_features: HashMap<String, HashMap<String, FeatureSnapshot>> = HashMap::new();

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

        // --- Phase 0: Watch VDD state files for active projects ---
        // This covers hook-driven or external vision mutations, not just in-process API calls.
        let watched_visions = collect_watched_visions(&state, &live_panes);
        let active_vision_paths: HashSet<String> = watched_visions.iter().cloned().collect();
        for project_path in &watched_visions {
            let Some(fingerprint) = vision_fingerprint(project_path) else {
                continue;
            };
            let current_features = vision_feature_snapshots(project_path);

            match vision_fingerprints.get(project_path) {
                Some(previous) if *previous != fingerprint => {
                    vision_fingerprints.insert(project_path.clone(), fingerprint);
                    if let Some((project_name, current)) = current_features {
                        let previous = vision_features.get(project_path);
                        let events = diff_vision_features(&project_name, previous, &current);
                        if events.is_empty() {
                            app.state.event_bus.send(StateEvent::VisionChanged {
                                project: project_name,
                                summary: vision_change_summary(project_path),
                                feature_id: None,
                                feature_title: None,
                                phase: None,
                                state: None,
                                readiness: None,
                            });
                        } else {
                            for event in events {
                                app.state.event_bus.send(event);
                            }
                        }
                        vision_features.insert(project_path.clone(), current);
                    } else {
                        app.state.event_bus.send(StateEvent::VisionChanged {
                            project: vision_project_name(project_path),
                            summary: vision_change_summary(project_path),
                            feature_id: None,
                            feature_title: None,
                            phase: None,
                            state: None,
                            readiness: None,
                        });
                        vision_features.remove(project_path);
                    }
                }
                None => {
                    // Baseline the current file without emitting a startup event.
                    vision_fingerprints.insert(project_path.clone(), fingerprint);
                    if let Some((_, current)) = current_features {
                        vision_features.insert(project_path.clone(), current);
                    }
                }
                _ => {
                    if !vision_features.contains_key(project_path) {
                        if let Some((_, current)) = current_features {
                            vision_features.insert(project_path.clone(), current);
                        }
                    }
                }
            }
        }
        vision_fingerprints.retain(|path, _| active_vision_paths.contains(path));
        vision_features.retain(|path, _| active_vision_paths.contains(path));

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

fn collect_watched_visions(state: &DxTerminalState, live_panes: &[tmux::LivePane]) -> Vec<String> {
    let mut project_paths = HashSet::new();

    for pane in state.panes.values() {
        if let Some(project_path) = resolve_vision_project_path(&pane.project_path) {
            project_paths.insert(project_path);
        }
        if let Some(workspace_path) = pane.workspace_path.as_deref() {
            if let Some(project_path) = resolve_vision_project_path(workspace_path) {
                project_paths.insert(project_path);
            }
        }
    }

    for pane in live_panes {
        if let Some(project_path) = resolve_vision_project_path(&pane.cwd) {
            project_paths.insert(project_path);
        }
    }

    let mut paths: Vec<String> = project_paths.into_iter().collect();
    paths.sort();
    paths
}

fn resolve_vision_project_path(candidate: &str) -> Option<String> {
    if candidate.trim().is_empty() || candidate == "--" {
        return None;
    }

    let candidate_path = Path::new(candidate);
    let start = if candidate_path.is_file() {
        candidate_path.parent()?
    } else {
        candidate_path
    };

    find_vision_root(start).map(|path| path.to_string_lossy().to_string())
}

fn find_vision_root(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        if dir.join(".vision/vision.json").exists() {
            return Some(dir.to_path_buf());
        }
    }
    None
}

fn vision_fingerprint(project_path: &str) -> Option<u64> {
    let vision_path = Path::new(project_path).join(".vision/vision.json");
    let content = std::fs::read_to_string(vision_path).ok()?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    Some(hasher.finish())
}

fn vision_project_name(project_path: &str) -> String {
    let summary = crate::vision::vision_summary(project_path);
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&summary) {
        if let Some(project) = value.get("project").and_then(|v| v.as_str()) {
            return project.to_string();
        }
    }

    Path::new(project_path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "--".to_string())
}

fn vision_change_summary(project_path: &str) -> String {
    let summary = crate::vision::vision_summary(project_path);
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&summary) {
        if let Some(change) = value.get("recent_changes").and_then(|v| v.as_array()).and_then(|changes| changes.first()) {
            let field = change.get("field").and_then(|v| v.as_str()).unwrap_or("");
            let reason = change.get("reason").and_then(|v| v.as_str()).unwrap_or("Vision updated");
            return if field.is_empty() {
                reason.to_string()
            } else {
                format!("{}: {}", field, reason)
            };
        }
    }

    "Vision updated".to_string()
}

fn vision_feature_snapshots(project_path: &str) -> Option<(String, HashMap<String, FeatureSnapshot>)> {
    let tree = crate::vision::vision_tree(project_path);
    let value: serde_json::Value = serde_json::from_str(&tree).ok()?;
    if value.get("error").is_some() {
        return None;
    }

    let project = value.get("project").and_then(|v| v.as_str()).unwrap_or("--").to_string();
    let mut features = HashMap::new();

    for goal in value.get("goals").and_then(|v| v.as_array()).into_iter().flatten() {
        for feature in goal.get("features").and_then(|v| v.as_array()).into_iter().flatten() {
            let feature_id = match feature.get("id").and_then(|v| v.as_str()) {
                Some(id) => id.to_string(),
                None => continue,
            };
            features.insert(feature_id, FeatureSnapshot {
                title: feature.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                phase: feature.get("phase").or_else(|| feature.get("status")).and_then(|v| v.as_str()).unwrap_or("planned").to_string(),
                state: feature.get("state").and_then(|v| v.as_str()).unwrap_or("planned").to_string(),
                readiness: feature.get("readiness").cloned().unwrap_or_else(|| json!({})),
            });
        }
    }

    Some((project, features))
}

fn diff_vision_features(
    project: &str,
    previous: Option<&HashMap<String, FeatureSnapshot>>,
    current: &HashMap<String, FeatureSnapshot>,
) -> Vec<StateEvent> {
    let mut feature_ids: Vec<String> = current.keys().cloned().collect();
    if let Some(prev) = previous {
        for feature_id in prev.keys() {
            if !current.contains_key(feature_id) {
                feature_ids.push(feature_id.clone());
            }
        }
    }
    feature_ids.sort();
    feature_ids.dedup();

    let mut events = Vec::new();
    for feature_id in feature_ids {
        let prev = previous.and_then(|features| features.get(&feature_id));
        let next = current.get(&feature_id);
        if prev == next {
            continue;
        }

        let (feature_title, phase, state, readiness) = match next {
            Some(snapshot) => (
                Some(snapshot.title.clone()),
                Some(snapshot.phase.clone()),
                Some(snapshot.state.clone()),
                Some(snapshot.readiness.clone()),
            ),
            None => (prev.map(|snapshot| snapshot.title.clone()), None, None, None),
        };

        events.push(StateEvent::VisionChanged {
            project: project.to_string(),
            summary: feature_change_summary(&feature_id, prev, next),
            feature_id: Some(feature_id),
            feature_title,
            phase,
            state,
            readiness,
        });
    }

    events
}

fn feature_change_summary(
    feature_id: &str,
    previous: Option<&FeatureSnapshot>,
    current: Option<&FeatureSnapshot>,
) -> String {
    match (previous, current) {
        (None, Some(snapshot)) => format!("{} added in {}", feature_id, snapshot.phase),
        (Some(_), None) => format!("{} removed from vision", feature_id),
        (Some(prev), Some(next)) if prev.phase != next.phase => {
            format!("{} phase {} -> {}", feature_id, prev.phase, next.phase)
        }
        (Some(prev), Some(next)) if prev.state != next.state => {
            format!("{} state {} -> {}", feature_id, prev.state, next.state)
        }
        (Some(prev), Some(next)) if prev.readiness != next.readiness => {
            format!("{} readiness updated", feature_id)
        }
        (Some(_), Some(_)) => format!("{} updated", feature_id),
        (None, None) => "Vision updated".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::types::{DxTerminalState, PaneState};

    #[test]
    fn resolves_vision_root_from_nested_workspace_path() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("demo");
        std::fs::create_dir_all(project.join(".vision")).unwrap();
        std::fs::create_dir_all(project.join("src/nested")).unwrap();
        std::fs::write(project.join(".vision/vision.json"), r#"{"project":"demo"}"#).unwrap();

        let resolved = resolve_vision_project_path(&project.join("src/nested").to_string_lossy());
        assert_eq!(resolved.as_deref(), Some(project.to_string_lossy().as_ref()));
    }

    #[test]
    fn collects_and_dedupes_project_paths_from_state_and_live_panes() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("demo");
        std::fs::create_dir_all(project.join(".vision")).unwrap();
        std::fs::create_dir_all(project.join("app")).unwrap();
        std::fs::write(project.join(".vision/vision.json"), r#"{"project":"demo"}"#).unwrap();

        let mut state = DxTerminalState::default();
        let mut pane = PaneState::default();
        pane.project_path = project.to_string_lossy().to_string();
        pane.workspace_path = Some(project.join("app").to_string_lossy().to_string());
        state.panes.insert("1".into(), pane);

        let live = vec![tmux::LivePane {
            target: "dx:1.1".into(),
            session: "dx".into(),
            window: 1,
            pane_idx: 1,
            window_name: "build".into(),
            command: "claude".into(),
            cwd: project.join("app").to_string_lossy().to_string(),
            pid: 1,
            jsonl_path: None,
            session_id: None,
        }];

        let watched = collect_watched_visions(&state, &live);
        assert_eq!(watched, vec![project.to_string_lossy().to_string()]);
    }

    #[test]
    fn vision_fingerprint_changes_when_file_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("demo");
        std::fs::create_dir_all(project.join(".vision")).unwrap();
        let vision_file = project.join(".vision/vision.json");
        std::fs::write(&vision_file, r#"{"project":"demo","updated_at":"1"}"#).unwrap();
        let first = vision_fingerprint(project.to_string_lossy().as_ref()).unwrap();

        std::fs::write(&vision_file, r#"{"project":"demo","updated_at":"2"}"#).unwrap();
        let second = vision_fingerprint(project.to_string_lossy().as_ref()).unwrap();

        assert_ne!(first, second);
    }

    #[test]
    fn diff_vision_features_emits_phase_and_readiness_deltas() {
        let previous = HashMap::from([(
            "F1.1".to_string(),
            FeatureSnapshot {
                title: "Ship login".into(),
                phase: "discovery".into(),
                state: "active".into(),
                readiness: json!({"ready_for_build": false}),
            },
        )]);
        let current = HashMap::from([(
            "F1.1".to_string(),
            FeatureSnapshot {
                title: "Ship login".into(),
                phase: "build".into(),
                state: "active".into(),
                readiness: json!({"ready_for_build": true}),
            },
        )]);

        let events = diff_vision_features("demo", Some(&previous), &current);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StateEvent::VisionChanged { project, summary, feature_id, phase, readiness, .. } => {
                assert_eq!(project, "demo");
                assert_eq!(feature_id.as_deref(), Some("F1.1"));
                assert_eq!(phase.as_deref(), Some("build"));
                assert_eq!(summary, "F1.1 phase discovery -> build");
                assert_eq!(readiness.as_ref().and_then(|r| r.get("ready_for_build")).and_then(|v| v.as_bool()), Some(true));
            }
            other => panic!("unexpected event: {:?}", other),
        }
    }
}

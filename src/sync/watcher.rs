use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;

use super::SyncConfig;

/// Run the file watcher in a blocking thread.
/// Watches configured directories and sends batches of changed paths via channel.
pub fn run_watcher(config: SyncConfig, tx: mpsc::Sender<Vec<PathBuf>>) -> anyhow::Result<()> {
    let (event_tx, event_rx) = std::sync::mpsc::channel::<Result<Event, notify::Error>>();

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = event_tx.send(res);
        },
        Config::default().with_poll_interval(Duration::from_secs(2)),
    )?;

    // Watch configured directories
    for dir in &config.watch_dirs {
        let watch_path = config.root.join(dir);
        if watch_path.exists() {
            let mode = if watch_path.is_dir() {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };
            if let Err(e) = watcher.watch(&watch_path, mode) {
                tracing::warn!("Failed to watch {:?}: {}", watch_path, e);
            } else {
                tracing::debug!("Watching {:?}", watch_path);
            }
        }
    }

    // Also watch root for top-level files (Cargo.toml, CLAUDE.md, etc.)
    let _ = watcher.watch(&config.root, RecursiveMode::NonRecursive);

    tracing::info!("File watcher started for {:?}", config.root);

    // Debounce loop — collect events over debounce_ms, then send batch
    let debounce = Duration::from_millis(config.debounce_ms);
    let mut pending: Vec<PathBuf> = Vec::new();
    let mut last_event = std::time::Instant::now();

    loop {
        match event_rx.recv_timeout(debounce) {
            Ok(Ok(event)) => {
                for path in event.paths {
                    if should_ignore(&path, &config.ignore_patterns) {
                        continue;
                    }
                    if !pending.contains(&path) {
                        pending.push(path);
                    }
                }
                last_event = std::time::Instant::now();
            }
            Ok(Err(e)) => {
                tracing::warn!("Watch error: {}", e);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Debounce expired — send batch if we have pending changes
                if !pending.is_empty() && last_event.elapsed() >= debounce {
                    let batch = std::mem::take(&mut pending);
                    if tx.blocking_send(batch).is_err() {
                        break; // Receiver dropped
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}

/// Check if a path should be ignored based on patterns
fn should_ignore(path: &Path, patterns: &[String]) -> bool {
    let path_str = path.display().to_string();
    for pattern in patterns {
        if pattern.ends_with('/') {
            // Directory pattern
            if path_str.contains(pattern) || path_str.contains(&pattern[..pattern.len() - 1]) {
                return true;
            }
        } else if pattern.starts_with("*.") {
            // Extension pattern
            let ext = &pattern[1..];
            if path_str.ends_with(ext) {
                return true;
            }
        } else if path_str.contains(pattern) {
            return true;
        }
    }
    false
}

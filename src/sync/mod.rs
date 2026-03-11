pub mod watcher;
pub mod git;

use std::path::PathBuf;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Events emitted by the sync system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SyncEvent {
    /// File changed on disk
    FileChanged {
        path: String,
        kind: String, // "create", "modify", "remove"
        project: String,
    },
    /// Git commit created
    GitCommit {
        project: String,
        message: String,
        sha: String,
        files_changed: usize,
    },
    /// Git push completed
    GitPush {
        project: String,
        branch: String,
        success: bool,
        error: Option<String>,
    },
    /// Git status changed
    GitStatus {
        project: String,
        dirty_files: usize,
        branch: String,
        ahead: usize,
        behind: usize,
    },
    /// Sync conflict detected
    SyncConflict {
        project: String,
        files: Vec<String>,
    },
}

/// Sync configuration per project
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Root directory to watch
    pub root: PathBuf,
    /// Project name (for event tagging)
    pub project: String,
    /// Directories to watch (relative to root)
    pub watch_dirs: Vec<String>,
    /// Patterns to ignore
    pub ignore_patterns: Vec<String>,
    /// Auto-commit enabled
    pub auto_commit: bool,
    /// Auto-push enabled
    pub auto_push: bool,
    /// Debounce interval in milliseconds
    pub debounce_ms: u64,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            project: String::new(),
            watch_dirs: vec![
                ".vision".into(),
                "src".into(),
                "assets".into(),
                "CLAUDE.md".into(),
                "Cargo.toml".into(),
                "package.json".into(),
            ],
            ignore_patterns: vec![
                "target/".into(),
                "node_modules/".into(),
                ".git/".into(),
                "*.swp".into(),
                "*.swo".into(),
                ".DS_Store".into(),
            ],
            auto_commit: true,
            auto_push: true,
            debounce_ms: 500,
        }
    }
}

/// The sync manager — coordinates file watching, git sync, and event broadcasting
pub struct SyncManager {
    pub config: SyncConfig,
    pub event_tx: broadcast::Sender<SyncEvent>,
}

impl SyncManager {
    pub fn new(config: SyncConfig) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self { config, event_tx }
    }

    /// Start the sync system — file watcher + git sync loop
    pub async fn start(self: Arc<Self>) -> anyhow::Result<()> {
        tracing::info!(
            "Sync started for project '{}' at {:?}",
            self.config.project,
            self.config.root
        );

        // Channel for file watcher → git sync
        let (file_tx, mut file_rx) = tokio::sync::mpsc::channel::<Vec<PathBuf>>(64);

        // Start file watcher in background thread
        let watcher_config = self.config.clone();
        let watcher_handle = std::thread::spawn(move || {
            if let Err(e) = watcher::run_watcher(watcher_config, file_tx) {
                tracing::error!("File watcher error: {}", e);
            }
        });

        // Git sync loop — processes file change batches
        let sync_self = Arc::clone(&self);
        tokio::spawn(async move {
            let mut pending_changes: Vec<PathBuf> = Vec::new();
            let mut last_commit = std::time::Instant::now();
            let commit_interval = std::time::Duration::from_secs(30); // Max 30s between commits

            loop {
                // Wait for file changes (with timeout for periodic commits)
                let timeout = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    file_rx.recv(),
                );

                match timeout.await {
                    Ok(Some(changed_files)) => {
                        // Broadcast file change events
                        for path in &changed_files {
                            let path_str = path.display().to_string();
                            let kind = if path.exists() { "modify" } else { "remove" };
                            let _ = sync_self.event_tx.send(SyncEvent::FileChanged {
                                path: path_str,
                                kind: kind.to_string(),
                                project: sync_self.config.project.clone(),
                            });
                        }
                        pending_changes.extend(changed_files);
                    }
                    Ok(None) => break, // Channel closed
                    Err(_) => {} // Timeout — check if we should commit
                }

                // Auto-commit when:
                // 1. We have pending changes AND
                // 2. Either enough time passed OR enough files changed
                let should_commit = !pending_changes.is_empty()
                    && (last_commit.elapsed() >= commit_interval || pending_changes.len() >= 10);

                if should_commit && sync_self.config.auto_commit {
                    match git::auto_commit(&sync_self.config.root, &pending_changes) {
                        Ok(Some((sha, message, count))) => {
                            tracing::info!("Auto-commit: {} ({} files)", message, count);
                            let _ = sync_self.event_tx.send(SyncEvent::GitCommit {
                                project: sync_self.config.project.clone(),
                                message,
                                sha: sha.clone(),
                                files_changed: count,
                            });

                            // Auto-push if enabled
                            if sync_self.config.auto_push {
                                let root = sync_self.config.root.clone();
                                let project = sync_self.config.project.clone();
                                let tx = sync_self.event_tx.clone();
                                tokio::task::spawn_blocking(move || {
                                    let result = git::push(&root);
                                    let branch = git::current_branch(&root)
                                        .unwrap_or_else(|| "unknown".into());
                                    let _ = tx.send(SyncEvent::GitPush {
                                        project,
                                        branch,
                                        success: result.is_ok(),
                                        error: result.err().map(|e| e.to_string()),
                                    });
                                });
                            }
                        }
                        Ok(None) => {} // Nothing to commit
                        Err(e) => tracing::warn!("Auto-commit failed: {}", e),
                    }
                    pending_changes.clear();
                    last_commit = std::time::Instant::now();
                }

                // Periodically broadcast git status
                if last_commit.elapsed() >= std::time::Duration::from_secs(10) {
                    let root = sync_self.config.root.clone();
                    let project = sync_self.config.project.clone();
                    let tx = sync_self.event_tx.clone();
                    tokio::task::spawn_blocking(move || {
                        if let Ok(status) = git::status(&root) {
                            let _ = tx.send(SyncEvent::GitStatus {
                                project,
                                dirty_files: status.dirty_count,
                                branch: status.branch,
                                ahead: status.ahead,
                                behind: status.behind,
                            });
                        }
                    });
                }
            }
        });

        // Keep the watcher thread handle (it runs until dropped)
        drop(watcher_handle);

        Ok(())
    }
}

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use crate::app::App;
use crate::config;

const VISION_SOCKET_PREFIX: &str = "vision-events-";
const VISION_SOCKET_SUFFIX: &str = ".sock";
const VISION_REPLAY_FILE: &str = "vision-events.jsonl";
const VISION_REPLAY_MAX_AGE_MS: u64 = 30_000;
const VISION_REPLAY_MAX_COUNT: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReplayEnvelope {
    ts_ms: u64,
    payload: Value,
}

pub fn vision_socket_dir() -> PathBuf {
    config::dx_root().join("ipc")
}

pub fn vision_socket_path_for_pid(pid: u32) -> PathBuf {
    vision_socket_dir().join(format!("{}{}{}", VISION_SOCKET_PREFIX, pid, VISION_SOCKET_SUFFIX))
}

pub fn vision_socket_path() -> PathBuf {
    vision_socket_path_for_pid(std::process::id())
}

pub fn discover_vision_socket_paths() -> Vec<PathBuf> {
    let mut sockets = Vec::new();
    if let Ok(entries) = std::fs::read_dir(vision_socket_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_vision_socket_path(&path) {
                sockets.push(path);
            }
        }
    }
    sockets.sort();
    sockets
}

pub fn vision_replay_log_path() -> PathBuf {
    vision_socket_dir().join(VISION_REPLAY_FILE)
}

pub fn append_replay_event(raw_payload: &str) {
    let Ok(payload) = serde_json::from_str::<Value>(raw_payload) else {
        return;
    };

    let path = vision_replay_log_path();
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }

    let mut entries = load_replay_entries(&path);
    entries.push(ReplayEnvelope {
        ts_ms: now_ms(),
        payload,
    });
    retain_recent_entries(&mut entries, now_ms(), VISION_REPLAY_MAX_AGE_MS, VISION_REPLAY_MAX_COUNT);
    let _ = write_replay_entries(&path, &entries);
}

fn is_vision_socket_path(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            name.starts_with(VISION_SOCKET_PREFIX)
                && name.ends_with(VISION_SOCKET_SUFFIX)
                && name[VISION_SOCKET_PREFIX.len()..name.len() - VISION_SOCKET_SUFFIX.len()]
                    .chars()
                    .all(|c| c.is_ascii_digit())
        })
        .unwrap_or(false)
}

pub fn start_local_ipc(app: Arc<App>) {
    tokio::spawn(async move {
        if let Err(err) = run_local_ipc(app).await {
            tracing::warn!("local IPC listener unavailable: {}", err);
        }
    });
}

async fn run_local_ipc(app: Arc<App>) -> anyhow::Result<()> {
    let socket_path = vision_socket_path();
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent).context("create ipc parent dir")?;
    }

    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }

    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("bind ipc socket {}", socket_path.display()))?;
    tracing::info!("local IPC listener active at {}", socket_path.display());
    replay_recent_events(app.as_ref());

    loop {
        let (stream, _) = listener.accept().await?;
        let app = Arc::clone(&app);
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream, app).await {
                tracing::debug!("ipc connection failed: {}", err);
            }
        });
    }
}

async fn handle_connection(mut stream: UnixStream, app: Arc<App>) -> anyhow::Result<()> {
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;
    if buf.is_empty() {
        return Ok(());
    }

    let payload: Value = serde_json::from_slice(&buf)?;
    let project_path = payload
        .get("project_path")
        .or_else(|| payload.get("path"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let result = payload.get("result").and_then(|v| v.as_str()).unwrap_or("");
    let feature_id = payload.get("feature_id").and_then(|v| v.as_str());

    if !project_path.is_empty() && !result.is_empty() {
        crate::vision_events::emit_from_result(app.as_ref(), project_path, result, feature_id);
    }

    let _ = stream.write_all(b"{\"status\":\"ok\"}").await;
    Ok(())
}

fn replay_recent_events(app: &App) {
    let path = vision_replay_log_path();
    let mut entries = load_replay_entries(&path);
    if entries.is_empty() {
        return;
    }

    retain_recent_entries(&mut entries, now_ms(), VISION_REPLAY_MAX_AGE_MS, VISION_REPLAY_MAX_COUNT);
    let _ = write_replay_entries(&path, &entries);

    for entry in entries {
        let project_path = entry
            .payload
            .get("project_path")
            .or_else(|| entry.payload.get("path"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let result = entry.payload.get("result").and_then(|v| v.as_str()).unwrap_or("");
        let feature_id = entry.payload.get("feature_id").and_then(|v| v.as_str());
        if !project_path.is_empty() && !result.is_empty() {
            crate::vision_events::emit_from_result(app, project_path, result, feature_id);
        }
    }
}

fn load_replay_entries(path: &std::path::Path) -> Vec<ReplayEnvelope> {
    std::fs::read_to_string(path)
        .ok()
        .map(|content| {
            content
                .lines()
                .filter_map(|line| serde_json::from_str::<ReplayEnvelope>(line).ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn write_replay_entries(path: &std::path::Path, entries: &[ReplayEnvelope]) -> std::io::Result<()> {
    let content = entries
        .iter()
        .filter_map(|entry| serde_json::to_string(entry).ok())
        .collect::<Vec<_>>()
        .join("\n");
    let content = if content.is_empty() { content } else { format!("{}\n", content) };
    std::fs::write(path, content)
}

fn retain_recent_entries(entries: &mut Vec<ReplayEnvelope>, now_ms: u64, max_age_ms: u64, max_count: usize) {
    entries.retain(|entry| now_ms.saturating_sub(entry.ts_ms) <= max_age_ms);
    if entries.len() > max_count {
        let keep_from = entries.len() - max_count;
        entries.drain(0..keep_from);
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_path_lives_under_dx_root() {
        let path = vision_socket_path();
        assert!(path.starts_with(vision_socket_dir()));
        assert!(is_vision_socket_path(&path));
        assert!(path.starts_with(config::dx_root()));
    }

    #[test]
    fn socket_path_is_namespaced_by_pid() {
        let path = vision_socket_path_for_pid(4242);
        assert!(path.ends_with("vision-events-4242.sock"));
        assert!(is_vision_socket_path(&path));
    }

    #[test]
    fn retain_recent_entries_filters_old_and_caps_count() {
        let now = 10_000;
        let mut entries = vec![
            ReplayEnvelope { ts_ms: 1_000, payload: serde_json::json!({"i":1}) },
            ReplayEnvelope { ts_ms: 8_000, payload: serde_json::json!({"i":2}) },
            ReplayEnvelope { ts_ms: 9_000, payload: serde_json::json!({"i":3}) },
            ReplayEnvelope { ts_ms: 9_500, payload: serde_json::json!({"i":4}) },
        ];

        retain_recent_entries(&mut entries, now, 2_500, 2);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].payload["i"], 3);
        assert_eq!(entries[1].payload["i"], 4);
    }
}

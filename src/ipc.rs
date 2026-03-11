use std::fs::OpenOptions;
use std::io;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
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
const VISION_CURSOR_PREFIX: &str = "vision-cursor-";
const VISION_CURSOR_SUFFIX: &str = ".json";
const LOCK_SUFFIX: &str = ".lock";
const VISION_REPLAY_MAX_AGE_MS: u64 = 30_000;
const VISION_REPLAY_MAX_COUNT: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReplayEnvelope {
    seq: u64,
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

pub fn prepare_outbound_event(payload: Value) -> Option<String> {
    let path = vision_replay_log_path();
    with_exclusive_lock(&lock_path_for(&path), || {
        let now = now_ms();
        let mut entries = load_replay_entries(&path);
        let next_seq = entries.last().map(|entry| entry.seq + 1).unwrap_or(1);
        let mut payload = payload;
        payload["replay_seq"] = Value::from(next_seq);
        payload["replay_ts_ms"] = Value::from(now);
        entries.push(ReplayEnvelope {
            seq: next_seq,
            ts_ms: now,
            payload: payload.clone(),
        });
        retain_recent_entries(&mut entries, now, VISION_REPLAY_MAX_AGE_MS, VISION_REPLAY_MAX_COUNT);
        write_replay_entries(&path, &entries)?;
        serde_json::to_string(&payload)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    })
    .ok()
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

pub fn start_local_ipc(app: Arc<App>, runtime_id: String) {
    tokio::spawn(async move {
        if let Err(err) = run_local_ipc(app, runtime_id).await {
            tracing::warn!("local IPC listener unavailable: {}", err);
        }
    });
}

async fn run_local_ipc(app: Arc<App>, runtime_id: String) -> anyhow::Result<()> {
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
    replay_recent_events(app.as_ref(), &runtime_id);

    loop {
        let (stream, _) = listener.accept().await?;
        let app = Arc::clone(&app);
        let runtime_id = runtime_id.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream, app, runtime_id).await {
                tracing::debug!("ipc connection failed: {}", err);
            }
        });
    }
}

async fn handle_connection(mut stream: UnixStream, app: Arc<App>, runtime_id: String) -> anyhow::Result<()> {
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
    let replay_seq = payload.get("replay_seq").and_then(|v| v.as_u64());

    if !project_path.is_empty() && !result.is_empty() {
        crate::vision_events::emit_from_result(app.as_ref(), project_path, result, feature_id);
        if let Some(seq) = replay_seq {
            advance_cursor(&runtime_id, seq);
        }
    }

    let _ = stream.write_all(b"{\"status\":\"ok\"}").await;
    Ok(())
}

fn replay_recent_events(app: &App, runtime_id: &str) {
    let path = vision_replay_log_path();
    let entries = with_exclusive_lock(&lock_path_for(&path), || {
        let mut entries = load_replay_entries(&path);
        retain_recent_entries(&mut entries, now_ms(), VISION_REPLAY_MAX_AGE_MS, VISION_REPLAY_MAX_COUNT);
        write_replay_entries(&path, &entries)?;
        Ok(entries)
    })
    .unwrap_or_default();
    if entries.is_empty() {
        return;
    }

    let last_seq = read_cursor(runtime_id);
    let mut max_seq = last_seq;

    for entry in entries {
        if entry.seq <= last_seq {
            continue;
        }
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
            max_seq = max_seq.max(entry.seq);
        }
    }
    if max_seq > last_seq {
        advance_cursor(runtime_id, max_seq);
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
    atomic_write(path, &content)
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

fn lock_path_for(path: &Path) -> PathBuf {
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("ipc");
    path.with_file_name(format!("{}{}", name, LOCK_SUFFIX))
}

fn with_exclusive_lock<T, F>(lock_path: &Path, f: F) -> io::Result<T>
where
    F: FnOnce() -> io::Result<T>,
{
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lock_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(lock_path)?;
    let fd = lock_file.as_raw_fd();
    let lock_result = unsafe { libc::flock(fd, libc::LOCK_EX) };
    if lock_result != 0 {
        return Err(io::Error::last_os_error());
    }
    f()
}

fn atomic_write(path: &Path, content: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("ipc");
    let tmp = path.with_file_name(format!("{}.tmp-{}", file_name, std::process::id()));
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)
}

fn runtime_cursor_path(runtime_id: &str) -> PathBuf {
    let safe: String = runtime_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    vision_socket_dir().join(format!("{}{}{}", VISION_CURSOR_PREFIX, safe, VISION_CURSOR_SUFFIX))
}

fn read_cursor(runtime_id: &str) -> u64 {
    std::fs::read_to_string(runtime_cursor_path(runtime_id))
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .and_then(|value| value.get("last_seq").and_then(|v| v.as_u64()))
        .unwrap_or(0)
}

fn advance_cursor(runtime_id: &str, seq: u64) {
    let path = runtime_cursor_path(runtime_id);
    let lock_path = lock_path_for(&path);
    let _ = with_exclusive_lock(&lock_path, || {
        let current = read_cursor(runtime_id);
        let next = current.max(seq);
        atomic_write(&path, &serde_json::json!({ "last_seq": next }).to_string())
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn with_temp_dx_root<T>(f: impl FnOnce() -> T) -> T {
        let _guard = TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let original = std::env::var("DX_ROOT").ok();
        std::env::set_var("DX_ROOT", tmp.path());
        std::fs::create_dir_all(vision_socket_dir()).unwrap();

        let result = f();

        match original {
            Some(value) => std::env::set_var("DX_ROOT", value),
            None => std::env::remove_var("DX_ROOT"),
        }
        result
    }

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
            ReplayEnvelope { seq: 1, ts_ms: 1_000, payload: serde_json::json!({"i":1}) },
            ReplayEnvelope { seq: 2, ts_ms: 8_000, payload: serde_json::json!({"i":2}) },
            ReplayEnvelope { seq: 3, ts_ms: 9_000, payload: serde_json::json!({"i":3}) },
            ReplayEnvelope { seq: 4, ts_ms: 9_500, payload: serde_json::json!({"i":4}) },
        ];

        retain_recent_entries(&mut entries, now, 2_500, 2);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].payload["i"], 3);
        assert_eq!(entries[1].payload["i"], 4);
    }

    #[test]
    fn cursor_path_is_sanitized() {
        let path = runtime_cursor_path("web:3100/demo");
        assert!(path.ends_with("vision-cursor-web-3100-demo.json"));
    }

    #[test]
    fn prepare_outbound_event_assigns_unique_sequences_under_contention() {
        with_temp_dx_root(|| {
            let mut workers = Vec::new();
            for i in 0..12 {
                workers.push(std::thread::spawn(move || {
                    let body = prepare_outbound_event(serde_json::json!({
                        "project_path": format!("/tmp/project-{i}"),
                        "result": r#"{"status":"ok"}"#,
                    }))
                    .unwrap();
                    serde_json::from_str::<Value>(&body)
                        .unwrap()["replay_seq"]
                        .as_u64()
                        .unwrap()
                }));
            }

            let mut seqs = workers
                .into_iter()
                .map(|worker| worker.join().unwrap())
                .collect::<Vec<_>>();
            seqs.sort_unstable();
            assert_eq!(seqs, (1..=12).collect::<Vec<_>>());

            let entries = load_replay_entries(&vision_replay_log_path());
            assert_eq!(entries.len(), 12);
            assert_eq!(entries.last().map(|entry| entry.seq), Some(12));
        });
    }

    #[test]
    fn advance_cursor_is_monotonic() {
        with_temp_dx_root(|| {
            advance_cursor("web-3100-demo", 8);
            advance_cursor("web-3100-demo", 3);
            assert_eq!(read_cursor("web-3100-demo"), 8);
        });
    }
}

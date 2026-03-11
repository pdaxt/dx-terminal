use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use crate::app::App;
use crate::config;

pub fn vision_socket_path() -> PathBuf {
    config::dx_root().join("vision-events.sock")
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
        match UnixStream::connect(&socket_path).await {
            Ok(_) => {
                tracing::info!("local IPC listener already active at {}", socket_path.display());
                return Ok(());
            }
            Err(_) => {
                let _ = std::fs::remove_file(&socket_path);
            }
        }
    }

    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("bind ipc socket {}", socket_path.display()))?;
    tracing::info!("local IPC listener active at {}", socket_path.display());

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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_path_lives_under_dx_root() {
        let path = vision_socket_path();
        assert!(path.ends_with("vision-events.sock"));
        assert!(path.starts_with(config::dx_root()));
    }
}

use std::path::Path;
use anyhow::Result;
use crate::state::types::AgentOSState;

pub fn load_state(path: &Path) -> AgentOSState {
    if path.exists() {
        match std::fs::read_to_string(path) {
            Ok(contents) => {
                match serde_json::from_str(&contents) {
                    Ok(state) => return state,
                    Err(e) => tracing::warn!("Failed to parse state.json: {}", e),
                }
            }
            Err(e) => tracing::warn!("Failed to read state.json: {}", e),
        }
    }
    let state = AgentOSState::default();
    let _ = save_state(path, &state);
    state
}

pub fn save_state(path: &Path, state: &AgentOSState) -> Result<()> {
    let json = serde_json::to_string_pretty(state)?;
    atomic_write(path, json.as_bytes())
}

pub fn read_json(path: &Path) -> serde_json::Value {
    if path.exists() {
        if let Ok(contents) = std::fs::read_to_string(path) {
            if let Ok(v) = serde_json::from_str(&contents) {
                return v;
            }
        }
    }
    serde_json::Value::Object(serde_json::Map::new())
}

pub fn write_json(path: &Path, value: &serde_json::Value) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    atomic_write(path, json.as_bytes())
}

/// Atomic write: write to temp file, then rename — prevents corruption on crash
fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

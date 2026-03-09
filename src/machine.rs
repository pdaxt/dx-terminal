use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config;

const IP_OFFSET: u8 = 100;

/// Machine identity for a pane — unique IP, hostname, MAC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineIdentity {
    pub pane: u8,
    pub ip: String,
    pub hostname: String,
    pub mac: String,
    pub theme: String,
    pub registered_at: String,
}

fn registry_path() -> PathBuf {
    config::dx_root().join("machines.json")
}

/// Generate IP for a pane: 127.0.0.{100+N}
pub fn pane_ip(pane: u8) -> String {
    format!("127.0.0.{}", IP_OFFSET.saturating_add(pane))
}

/// Generate hostname: agent-{theme}-{N}
pub fn pane_hostname(pane: u8) -> String {
    let theme = config::theme_name(pane).to_lowercase();
    format!("agent-{}-{}", theme, pane)
}

/// Generate deterministic virtual MAC (locally-administered bit 02:xx)
pub fn pane_mac(pane: u8) -> String {
    format!("02:00:00:05:{:02x}:{:02x}", pane, pane)
}

/// Register a pane as a machine — called during spawn
pub fn register(pane: u8) -> MachineIdentity {
    let identity = MachineIdentity {
        pane,
        ip: pane_ip(pane),
        hostname: pane_hostname(pane),
        mac: pane_mac(pane),
        theme: config::theme_name(pane).to_string(),
        registered_at: crate::state::now(),
    };
    let mut reg = load_registry();
    reg.insert(pane.to_string(), identity.clone());
    let _ = save_registry(&reg);
    identity
}

/// Deregister a pane's machine — called during kill
pub fn deregister(pane: u8) {
    let mut reg = load_registry();
    reg.remove(&pane.to_string());
    let _ = save_registry(&reg);
}

/// Get machine identity for a pane
pub fn get(pane: u8) -> Option<MachineIdentity> {
    load_registry().remove(&pane.to_string())
}

/// List all registered machines sorted by pane
pub fn list_all() -> Vec<MachineIdentity> {
    let reg = load_registry();
    let mut machines: Vec<MachineIdentity> = reg.into_values().collect();
    machines.sort_by_key(|m| m.pane);
    machines
}

/// Deregister all machines (called on shutdown)
pub fn deregister_all() {
    let _ = save_registry(&HashMap::new());
}

/// MCP tool: machine info for one or all panes
pub fn machine_info(pane: Option<u8>) -> Value {
    if let Some(p) = pane {
        match get(p) {
            Some(m) => json!({
                "pane": m.pane, "ip": m.ip, "hostname": m.hostname,
                "mac": m.mac, "theme": m.theme, "registered_at": m.registered_at,
            }),
            None => {
                // Return identity even if not registered (deterministic)
                json!({
                    "pane": p, "ip": pane_ip(p), "hostname": pane_hostname(p),
                    "mac": pane_mac(p), "theme": config::theme_name(p),
                    "registered": false,
                })
            }
        }
    } else {
        let machines = list_all();
        let items: Vec<Value> = machines.iter().map(|m| json!({
            "pane": m.pane, "ip": m.ip, "hostname": m.hostname,
            "mac": m.mac, "theme": m.theme,
        })).collect();
        json!({"count": items.len(), "machines": items})
    }
}

/// MCP tool: list all registered machines with network info
pub fn machine_list() -> Value {
    let machines = list_all();
    let count = config::pane_count();
    let items: Vec<Value> = machines.iter().map(|m| json!({
        "pane": m.pane, "ip": m.ip, "hostname": m.hostname,
        "mac": m.mac, "theme": m.theme, "registered_at": m.registered_at,
    })).collect();
    json!({
        "count": items.len(),
        "machines": items,
        "subnet": "127.0.0.0/24",
        "ip_range": format!("127.0.0.{}-127.0.0.{}", IP_OFFSET + 1, IP_OFFSET + count),
        "total_slots": count,
    })
}

// --- Persistence ---

fn load_registry() -> HashMap<String, MachineIdentity> {
    let path = registry_path();
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(reg) = serde_json::from_str(&content) {
                return reg;
            }
        }
    }
    HashMap::new()
}

fn save_registry(registry: &HashMap<String, MachineIdentity>) -> anyhow::Result<()> {
    let path = registry_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(registry)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pane_ip() {
        assert_eq!(pane_ip(1), "127.0.0.101");
        assert_eq!(pane_ip(9), "127.0.0.109");
        assert_eq!(pane_ip(20), "127.0.0.120");
    }

    #[test]
    fn test_pane_mac() {
        assert_eq!(pane_mac(1), "02:00:00:05:01:01");
        assert_eq!(pane_mac(15), "02:00:00:05:0f:0f");
    }
}

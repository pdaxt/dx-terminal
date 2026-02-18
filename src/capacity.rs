use crate::config;
use crate::state::persistence::{read_json, write_json};
use chrono::Local;

pub struct CapacityData {
    pub acu_used: f64,
    pub acu_total: f64,
    pub reviews_used: usize,
    pub reviews_total: usize,
}

pub fn load_capacity() -> CapacityData {
    let cap_config = read_json(&config::capacity_root().join("config.json"));
    let pane_count = cap_config.get("pane_count").and_then(|v| v.as_f64()).unwrap_or(9.0);
    let hours = cap_config.get("hours_per_day").and_then(|v| v.as_f64()).unwrap_or(8.0);
    let factor = cap_config.get("availability_factor").and_then(|v| v.as_f64()).unwrap_or(0.8);
    let review_bw = cap_config.get("review_bandwidth").and_then(|v| v.as_u64()).unwrap_or(12) as usize;

    let daily_acu = pane_count * hours * factor;
    let today = Local::now().format("%Y-%m-%d").to_string();

    let work_log = read_json(&config::capacity_root().join("work_log.json"));
    let entries = work_log.get("entries").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let today_entries: Vec<_> = entries.iter().filter(|e| {
        e.get("logged_at").and_then(|v| v.as_str()).map_or(false, |s| s.starts_with(&today))
    }).collect();

    let acu_used: f64 = today_entries.iter()
        .filter_map(|e| e.get("acu_spent").and_then(|v| v.as_f64()))
        .sum();
    let reviews_used = today_entries.iter()
        .filter(|e| e.get("review_needed").and_then(|v| v.as_bool()).unwrap_or(false))
        .count();

    CapacityData {
        acu_used: (acu_used * 10.0).round() / 10.0,
        acu_total: (daily_acu * 10.0).round() / 10.0,
        reviews_used,
        reviews_total: review_bw,
    }
}

pub fn log_work_entry(entry: serde_json::Value) -> anyhow::Result<()> {
    let path = config::capacity_root().join("work_log.json");
    let mut log = read_json(&path);
    let entries = log.as_object_mut()
        .unwrap()
        .entry("entries")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .unwrap();
    entries.push(entry);
    write_json(&path, &log)?;
    Ok(())
}

use std::path::PathBuf;

pub const SESSION_NAME: &str = "claude6";

pub struct ThemeConfig {
    pub fg: &'static str,
    pub name: &'static str,
}

pub const THEMES: [(u8, ThemeConfig); 9] = [
    (1, ThemeConfig { fg: "#00d4ff", name: "CYAN" }),
    (2, ThemeConfig { fg: "#00ff41", name: "GREEN" }),
    (3, ThemeConfig { fg: "#bf00ff", name: "PURPLE" }),
    (4, ThemeConfig { fg: "#ff9500", name: "ORANGE" }),
    (5, ThemeConfig { fg: "#ff3366", name: "RED" }),
    (6, ThemeConfig { fg: "#ffcc00", name: "YELLOW" }),
    (7, ThemeConfig { fg: "#c0c0c0", name: "SILVER" }),
    (8, ThemeConfig { fg: "#00cec9", name: "TEAL" }),
    (9, ThemeConfig { fg: "#fd79a8", name: "PINK" }),
];

pub fn theme_name(pane: u8) -> &'static str {
    THEMES
        .iter()
        .find(|(n, _)| *n == pane)
        .map(|(_, t)| t.name)
        .unwrap_or("UNKNOWN")
}

pub fn theme_fg(pane: u8) -> &'static str {
    THEMES
        .iter()
        .find(|(n, _)| *n == pane)
        .map(|(_, t)| t.fg)
        .unwrap_or("#ffffff")
}

pub fn resolve_pane(pane_ref: &str) -> Option<u8> {
    // Try numeric first
    if let Ok(n) = pane_ref.parse::<u8>() {
        if (1..=9).contains(&n) {
            return Some(n);
        }
    }
    // Theme name or shortcut — O(1) match, no heap allocation
    match pane_ref.to_lowercase().as_str() {
        "cyan" | "c" => Some(1),
        "green" | "g" => Some(2),
        "purple" | "p" => Some(3),
        "orange" | "o" => Some(4),
        "red" | "r" => Some(5),
        "yellow" | "y" => Some(6),
        "silver" | "s" => Some(7),
        "teal" | "t" => Some(8),
        "pink" | "k" => Some(9),
        _ => None,
    }
}

pub fn role_short(role: &str) -> &'static str {
    match role {
        "pm" => "PM",
        "architect" => "ARCH",
        "frontend" => "FE",
        "backend" => "BE",
        "qa" => "QA",
        "security" => "SEC",
        "code_reviewer" => "CR",
        "devops" => "OPS",
        "developer" => "DEV",
        _ => "--",
    }
}

pub fn agentos_root() -> PathBuf {
    dirs_path("agentos")
}

pub fn capacity_root() -> PathBuf {
    dirs_path("capacity")
}

pub fn collab_root() -> PathBuf {
    dirs_path("collab")
}

pub fn claude_json_path() -> PathBuf {
    home_dir().join(".claude.json")
}

pub fn multi_agent_root() -> PathBuf {
    home_dir().join(".claude").join("multi_agent")
}

pub fn preamble_dir() -> PathBuf {
    agentos_root().join("preambles")
}

pub fn state_file() -> PathBuf {
    agentos_root().join("state.json")
}

fn dirs_path(name: &str) -> PathBuf {
    home_dir().join(".config").join(name)
}

pub fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

pub fn projects_dir() -> PathBuf {
    home_dir().join("Projects")
}

pub fn resolve_project_path(project: &str) -> String {
    if project.starts_with('/') {
        return project.to_string();
    }
    let p = projects_dir().join(project);
    if p.exists() {
        return p.to_string_lossy().to_string();
    }
    // Fuzzy: try case-insensitive match
    if let Ok(entries) = std::fs::read_dir(projects_dir()) {
        let lower = project.to_lowercase();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name == lower || name.contains(&lower) {
                return entry.path().to_string_lossy().to_string();
            }
        }
    }
    p.to_string_lossy().to_string()
}

use std::path::PathBuf;
use std::sync::OnceLock;
use serde::{Deserialize, Serialize};

/// Runtime configuration — loaded once at startup from ~/.config/agentos/config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Number of agent panes (1-20, default 9)
    #[serde(default = "default_pane_count")]
    pub pane_count: u8,
    /// Tmux session name prefix
    #[serde(default = "default_session_name")]
    pub session_name: String,
    /// Web dashboard port
    #[serde(default = "default_web_port")]
    pub web_port: u16,
    /// Theme definitions per pane (auto-generated if missing)
    #[serde(default)]
    pub themes: Vec<ThemeEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeEntry {
    pub fg: String,
    pub name: String,
}

fn default_pane_count() -> u8 { 9 }
fn default_session_name() -> String { "agentos".into() }
fn default_web_port() -> u16 { 3100 }

/// Default color palette (cycles if pane_count > len)
const DEFAULT_THEMES: &[(& str, &str)] = &[
    ("#00d4ff", "CYAN"),
    ("#00ff41", "GREEN"),
    ("#bf00ff", "PURPLE"),
    ("#ff9500", "ORANGE"),
    ("#ff3366", "RED"),
    ("#ffcc00", "YELLOW"),
    ("#c0c0c0", "SILVER"),
    ("#00cec9", "TEAL"),
    ("#fd79a8", "PINK"),
    ("#6c5ce7", "INDIGO"),
    ("#e17055", "CORAL"),
    ("#00b894", "MINT"),
    ("#fdcb6e", "GOLD"),
    ("#e84393", "MAGENTA"),
    ("#74b9ff", "SKY"),
    ("#55efc4", "AQUA"),
    ("#fab1a0", "PEACH"),
    ("#81ecec", "ICE"),
    ("#a29bfe", "LAVENDER"),
    ("#ffeaa7", "CREAM"),
];

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            pane_count: 9,
            session_name: "agentos".into(),
            web_port: 3100,
            themes: Vec::new(),
        }
    }
}

impl RuntimeConfig {
    /// Load from disk or create default
    pub fn load() -> Self {
        let path = agentos_root().join("config.json");
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(mut cfg) = serde_json::from_str::<RuntimeConfig>(&content) {
                    cfg.ensure_themes();
                    return cfg;
                }
            }
        }
        let mut cfg = Self::default();
        cfg.ensure_themes();
        let _ = cfg.save();
        cfg
    }

    fn ensure_themes(&mut self) {
        while self.themes.len() < self.pane_count as usize {
            let idx = self.themes.len() % DEFAULT_THEMES.len();
            self.themes.push(ThemeEntry {
                fg: DEFAULT_THEMES[idx].0.to_string(),
                name: DEFAULT_THEMES[idx].1.to_string(),
            });
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = agentos_root().join("config.json");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Get theme for pane (1-indexed)
    pub fn theme_name(&self, pane: u8) -> &str {
        self.themes.get((pane as usize).wrapping_sub(1))
            .map(|t| t.name.as_str())
            .unwrap_or("UNKNOWN")
    }

    pub fn theme_fg(&self, pane: u8) -> &str {
        self.themes.get((pane as usize).wrapping_sub(1))
            .map(|t| t.fg.as_str())
            .unwrap_or("#ffffff")
    }
}

/// Global config singleton — initialized once at startup
static RUNTIME_CONFIG: OnceLock<RuntimeConfig> = OnceLock::new();

/// Initialize the global config (call once at startup)
pub fn init() -> &'static RuntimeConfig {
    RUNTIME_CONFIG.get_or_init(RuntimeConfig::load)
}

/// Get the global config (panics if init() wasn't called)
pub fn get() -> &'static RuntimeConfig {
    RUNTIME_CONFIG.get().expect("config::init() must be called before config::get()")
}

// --- Pane resolution (uses global config) ---

pub fn theme_name(pane: u8) -> &'static str {
    get().theme_name(pane)
}

pub fn theme_fg(pane: u8) -> &'static str {
    get().theme_fg(pane)
}

pub fn pane_count() -> u8 {
    get().pane_count
}

pub fn session_name() -> &'static str {
    &get().session_name
}

pub fn resolve_pane(pane_ref: &str) -> Option<u8> {
    let max = pane_count();

    // Try numeric first
    if let Ok(n) = pane_ref.parse::<u8>() {
        if n >= 1 && n <= max {
            return Some(n);
        }
    }

    // Theme name or shortcut — search configured themes
    let lower = pane_ref.to_lowercase();
    let cfg = get();
    for (idx, theme) in cfg.themes.iter().enumerate() {
        let pane_num = (idx + 1) as u8;
        if pane_num > max { break; }

        if theme.name.to_lowercase() == lower {
            return Some(pane_num);
        }
        // First-char shortcut (unique per theme)
        if lower.len() == 1 {
            let first = theme.name.to_lowercase().chars().next().unwrap_or(' ');
            if lower.starts_with(first) {
                return Some(pane_num);
            }
        }
    }
    None
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

// --- Path helpers ---

pub fn agentos_root() -> PathBuf {
    home_dir().join(".config").join("agentos")
}

pub fn capacity_root() -> PathBuf {
    home_dir().join(".config").join("capacity")
}

pub fn collab_root() -> PathBuf {
    home_dir().join(".config").join("collab")
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

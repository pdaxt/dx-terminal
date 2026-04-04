use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn spawns_dir() -> PathBuf {
    let dir = dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".dx/spawns");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(60)
        .collect()
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Idle,
    Observing,
    Observed,
    Building,
    Built,
    Selling,
    Sold,
    Learning,
    Iterating,
    Failed { phase: String, error: String },
}

impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Phase::Idle => write!(f, "idle"),
            Phase::Observing => write!(f, "observing"),
            Phase::Observed => write!(f, "observed"),
            Phase::Building => write!(f, "building"),
            Phase::Built => write!(f, "built"),
            Phase::Selling => write!(f, "selling"),
            Phase::Sold => write!(f, "sold"),
            Phase::Learning => write!(f, "learning"),
            Phase::Iterating => write!(f, "iterating"),
            Phase::Failed { phase, .. } => write!(f, "failed:{}", phase),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub ts: String,
    pub phase: String,
    pub action: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnState {
    pub name: String,
    pub slug: String,
    pub niche: String,
    pub phase: Phase,
    pub cycle: u32,
    pub created_at: String,
    pub updated_at: String,
    // Phase output paths (relative to spawn dir)
    pub market_research: Option<String>,
    pub prospects: Option<String>,
    pub competitors: Option<String>,
    pub product_spec: Option<String>,
    pub deployed_url: Option<String>,
    pub outreach_stats: Option<String>,
    pub learnings: Option<String>,
    pub log: Vec<LogEntry>,
}

impl SpawnState {
    pub fn new(niche: &str) -> Self {
        let slug = slugify(niche);
        let ts = now();
        Self {
            name: niche.to_string(),
            slug,
            niche: niche.to_string(),
            phase: Phase::Idle,
            cycle: 0,
            created_at: ts.clone(),
            updated_at: ts,
            market_research: None,
            prospects: None,
            competitors: None,
            product_spec: None,
            deployed_url: None,
            outreach_stats: None,
            learnings: None,
            log: Vec::new(),
        }
    }

    pub fn dir(&self) -> PathBuf {
        spawns_dir().join(&self.slug)
    }

    pub fn ensure_dirs(&self) {
        for sub in ["research", "build", "build/content", "build/project", "sell", "sell/emails", "sell/social", "learn"] {
            let _ = std::fs::create_dir_all(self.dir().join(sub));
        }
    }

    pub fn save(&mut self) -> Result<(), String> {
        self.updated_at = now();
        self.ensure_dirs();
        let path = self.dir().join("state.json");
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("serialize: {}", e))?;
        // Atomic write: tmp file + rename
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, &json).map_err(|e| format!("write: {}", e))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("rename: {}", e))?;
        Ok(())
    }

    pub fn load(slug: &str) -> Result<Self, String> {
        let path = spawns_dir().join(slug).join("state.json");
        let json = std::fs::read_to_string(&path)
            .map_err(|e| format!("Spawn '{}' not found: {}", slug, e))?;
        serde_json::from_str(&json).map_err(|e| format!("parse: {}", e))
    }

    pub fn load_latest() -> Result<Self, String> {
        let mut all = Self::list_all();
        all.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        all.into_iter().next().ok_or_else(|| "No spawns found. Run: dx spawn run \"your niche\"".into())
    }

    pub fn list_all() -> Vec<Self> {
        let dir = spawns_dir();
        let mut spawns = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let state_path = entry.path().join("state.json");
                if state_path.exists() {
                    if let Ok(json) = std::fs::read_to_string(&state_path) {
                        if let Ok(state) = serde_json::from_str::<Self>(&json) {
                            spawns.push(state);
                        }
                    }
                }
            }
        }
        spawns.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        spawns
    }

    pub fn transition(&mut self, to: Phase) -> Result<(), String> {
        let valid = match (&self.phase, &to) {
            (Phase::Idle, Phase::Observing) => true,
            (Phase::Observing, Phase::Observed) => true,
            (Phase::Observing, Phase::Failed { .. }) => true,
            (Phase::Observed, Phase::Building) => true,
            (Phase::Building, Phase::Built) => true,
            (Phase::Building, Phase::Failed { .. }) => true,
            (Phase::Built, Phase::Selling) => true,
            (Phase::Selling, Phase::Sold) => true,
            (Phase::Selling, Phase::Failed { .. }) => true,
            (Phase::Sold, Phase::Learning) => true,
            (Phase::Learning, Phase::Iterating) => true,
            (Phase::Learning, Phase::Failed { .. }) => true,
            (Phase::Iterating, Phase::Observing) => true, // next cycle
            (Phase::Failed { .. }, _) => true, // can retry from failed
            _ => false,
        };
        if !valid {
            return Err(format!("Invalid transition: {} → {}", self.phase, to));
        }
        self.phase = to;
        Ok(())
    }

    pub fn log(&mut self, phase: &str, action: &str, ok: bool, detail: &str) {
        self.log.push(LogEntry {
            ts: now(),
            phase: phase.into(),
            action: action.into(),
            ok,
            detail: detail.into(),
        });
    }

    pub fn or_latest(slug: Option<String>) -> Result<Self, String> {
        match slug {
            Some(s) => Self::load(&s),
            None => Self::load_latest(),
        }
    }
}

//! Screen Management — dynamic screen/pane orchestration for DX Terminal.
//!
//! Screens are groups of tmux panes that can be added/removed at runtime.
//! Each screen maps to a tmux window with N panes inside.
//! The first 3 screens (9 panes) are the default "claude-6" layout.
//!
//! This module enables:
//! - Adding new screens on the fly (dx_add_screen MCP tool)
//! - Removing screens (dx_remove_screen)
//! - Listing screens with their agents (dx_list_screens)
//! - Moving panes between screens (dx_move_pane)
//! - Dynamic pane count beyond the original 9

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::sync::RwLock;

/// Screen layout types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScreenLayout {
    /// 3 panes side by side (default)
    #[default]
    EvenHorizontal,
    /// 3 panes stacked vertically
    EvenVertical,
    /// 4 panes in a 2x2 grid
    Grid2x2,
    /// Single pane full screen
    Single,
    /// 2 panes side by side
    Split2,
}

impl ScreenLayout {
    #[allow(clippy::should_implement_trait)]
    pub fn pane_count(&self) -> u8 {
        match self {
            ScreenLayout::Single => 1,
            ScreenLayout::Split2 => 2,
            ScreenLayout::EvenHorizontal | ScreenLayout::EvenVertical => 3,
            ScreenLayout::Grid2x2 => 4,
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "single" | "1" => Self::Single,
            "split" | "split2" | "2" => Self::Split2,
            "horizontal" | "even_horizontal" | "3" | "h" => Self::EvenHorizontal,
            "vertical" | "even_vertical" | "v" => Self::EvenVertical,
            "grid" | "grid2x2" | "4" | "2x2" => Self::Grid2x2,
            _ => Self::EvenHorizontal,
        }
    }
}

/// A screen is a group of panes in a tmux window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screen {
    /// Screen ID (1-based)
    pub id: u8,
    /// Display name (e.g., "Screen 1" or custom)
    pub name: String,
    /// Pane numbers in this screen (1-indexed, global)
    pub panes: Vec<u8>,
    /// Layout type
    pub layout: ScreenLayout,
    /// Tmux window index
    pub tmux_window: Option<u32>,
    /// When this screen was created
    pub created_at: String,
}

/// Persistent screen configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScreenConfig {
    pub screens: Vec<Screen>,
    /// Next pane number to allocate
    pub next_pane: u8,
    /// Tmux session name
    pub session: String,
}

/// Screen Manager — owns the screen lifecycle
pub struct ScreenManager {
    config: RwLock<ScreenConfig>,
    config_path: PathBuf,
}

impl ScreenManager {
    pub fn new(config_dir: PathBuf) -> Self {
        let config_path = config_dir.join("screens.json");
        let config = if config_path.exists() {
            std::fs::read_to_string(&config_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            ScreenConfig::default()
        };

        Self {
            config: RwLock::new(config),
            config_path,
        }
    }

    /// Initialize with default 3 screens × 3 panes = 9 panes
    pub fn init_default(&self, session: &str) {
        let mut cfg = self.config.write().unwrap();
        if !cfg.screens.is_empty() {
            return; // Already initialized
        }

        cfg.session = session.to_string();
        let now = chrono::Local::now().to_rfc3339();

        for screen_idx in 0..3u8 {
            let pane_start = screen_idx * 3 + 1;
            let panes: Vec<u8> = (pane_start..pane_start + 3).collect();
            cfg.screens.push(Screen {
                id: screen_idx + 1,
                name: format!("Screen {}", screen_idx + 1),
                panes,
                layout: ScreenLayout::EvenHorizontal,
                tmux_window: Some((screen_idx + 1) as u32),
                created_at: now.clone(),
            });
        }
        cfg.next_pane = 10; // Next available pane number
        drop(cfg);
        let _ = self.save();
    }

    /// Add a new screen with specified layout
    pub fn add_screen(
        &self,
        name: Option<String>,
        layout: Option<String>,
        panes_override: Option<u8>,
    ) -> Result<Screen, String> {
        let mut cfg = self.config.write().map_err(|e| e.to_string())?;

        let layout = layout
            .map(|s| ScreenLayout::from_str(&s))
            .unwrap_or_default();

        let pane_count = panes_override.unwrap_or_else(|| layout.pane_count());
        let screen_id = cfg.screens.len() as u8 + 1;
        let name = name.unwrap_or_else(|| format!("Screen {}", screen_id));

        // Allocate pane numbers
        let pane_start = cfg.next_pane;
        let panes: Vec<u8> = (pane_start..pane_start + pane_count).collect();
        cfg.next_pane = pane_start + pane_count;

        let now = chrono::Local::now().to_rfc3339();

        // Create tmux window
        let tmux_window = self.create_tmux_screen(&cfg.session, &name, &layout, pane_count);

        let screen = Screen {
            id: screen_id,
            name,
            panes,
            layout,
            tmux_window,
            created_at: now,
        };

        cfg.screens.push(screen.clone());
        drop(cfg);
        let _ = self.save();

        Ok(screen)
    }

    /// Remove a screen by ID or name
    pub fn remove_screen(&self, screen_ref: &str) -> Result<Screen, String> {
        let mut cfg = self.config.write().map_err(|e| e.to_string())?;

        let idx = cfg
            .screens
            .iter()
            .position(|s| {
                s.id.to_string() == screen_ref || s.name.to_lowercase() == screen_ref.to_lowercase()
            })
            .ok_or_else(|| format!("Screen '{}' not found", screen_ref))?;

        // Don't allow removing the last screen
        if cfg.screens.len() <= 1 {
            return Err("Cannot remove the last screen".to_string());
        }

        let screen = cfg.screens.remove(idx);

        // Kill tmux window if it exists
        if let Some(window) = screen.tmux_window {
            let target = format!("{}:{}", cfg.session, window);
            let _ = Command::new("tmux")
                .args(["kill-window", "-t", &target])
                .output();
        }

        drop(cfg);
        let _ = self.save();

        Ok(screen)
    }

    /// List all screens
    pub fn list_screens(&self) -> Vec<Screen> {
        self.config.read().unwrap().screens.clone()
    }

    /// Get total pane count across all screens
    pub fn total_panes(&self) -> u8 {
        self.config
            .read()
            .unwrap()
            .screens
            .iter()
            .map(|s| s.panes.len() as u8)
            .sum()
    }

    /// Find which screen a pane belongs to
    pub fn screen_for_pane(&self, pane: u8) -> Option<Screen> {
        self.config
            .read()
            .unwrap()
            .screens
            .iter()
            .find(|s| s.panes.contains(&pane))
            .cloned()
    }

    /// Get a specific screen by ID
    pub fn get_screen(&self, id: u8) -> Option<Screen> {
        self.config
            .read()
            .unwrap()
            .screens
            .iter()
            .find(|s| s.id == id)
            .cloned()
    }

    /// Get all pane numbers
    pub fn all_panes(&self) -> Vec<u8> {
        self.config
            .read()
            .unwrap()
            .screens
            .iter()
            .flat_map(|s| s.panes.iter().copied())
            .collect()
    }

    /// Save config to disk
    fn save(&self) -> Result<(), String> {
        let cfg = self.config.read().map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(&*cfg).map_err(|e| e.to_string())?;
        if let Some(parent) = self.config_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&self.config_path, json).map_err(|e| e.to_string())
    }

    /// Create a tmux window with the right layout
    fn create_tmux_screen(
        &self,
        session: &str,
        name: &str,
        layout: &ScreenLayout,
        pane_count: u8,
    ) -> Option<u32> {
        // Create the window
        let output = Command::new("tmux")
            .args([
                "new-window",
                "-t",
                session,
                "-n",
                name,
                "-P",
                "-F",
                "#{window_index}",
            ])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let window_idx: u32 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .ok()?;

        // Split panes according to layout
        let target_base = format!("{}:{}", session, window_idx);
        for _ in 1..pane_count {
            let split_dir = match layout {
                ScreenLayout::EvenVertical => "-v",
                _ => "-h",
            };
            let _ = Command::new("tmux")
                .args(["split-window", split_dir, "-t", &target_base])
                .output();
        }

        // Apply even layout
        let tmux_layout = match layout {
            ScreenLayout::EvenHorizontal | ScreenLayout::Split2 => "even-horizontal",
            ScreenLayout::EvenVertical => "even-vertical",
            ScreenLayout::Grid2x2 => "tiled",
            ScreenLayout::Single => "even-horizontal",
        };
        let _ = Command::new("tmux")
            .args(["select-layout", "-t", &target_base, tmux_layout])
            .output();

        Some(window_idx)
    }

    /// Get screen summary as JSON
    pub fn summary(&self) -> serde_json::Value {
        let cfg = self.config.read().unwrap();
        let screens: Vec<serde_json::Value> = cfg
            .screens
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "name": s.name,
                    "panes": s.panes,
                    "layout": s.layout,
                    "pane_count": s.panes.len(),
                    "tmux_window": s.tmux_window,
                })
            })
            .collect();

        serde_json::json!({
            "screens": screens,
            "total_screens": cfg.screens.len(),
            "total_panes": cfg.screens.iter().map(|s| s.panes.len()).sum::<usize>(),
            "next_pane": cfg.next_pane,
            "session": cfg.session,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_init_default() {
        let dir = TempDir::new().unwrap();
        let mgr = ScreenManager::new(dir.path().to_path_buf());
        mgr.init_default("test-session");

        let screens = mgr.list_screens();
        assert_eq!(screens.len(), 3);
        assert_eq!(screens[0].panes, vec![1, 2, 3]);
        assert_eq!(screens[1].panes, vec![4, 5, 6]);
        assert_eq!(screens[2].panes, vec![7, 8, 9]);
        assert_eq!(mgr.total_panes(), 9);
    }

    #[test]
    fn test_add_screen() {
        let dir = TempDir::new().unwrap();
        let mgr = ScreenManager::new(dir.path().to_path_buf());
        mgr.init_default("test");

        let screen = mgr
            .add_screen(
                Some("Dev Screen".to_string()),
                Some("grid2x2".to_string()),
                None,
            )
            .unwrap();

        assert_eq!(screen.id, 4);
        assert_eq!(screen.panes, vec![10, 11, 12, 13]);
        assert_eq!(screen.layout, ScreenLayout::Grid2x2);
        assert_eq!(mgr.total_panes(), 13);
    }

    #[test]
    fn test_remove_screen() {
        let dir = TempDir::new().unwrap();
        let mgr = ScreenManager::new(dir.path().to_path_buf());
        mgr.init_default("test");

        let removed = mgr.remove_screen("3").unwrap();
        assert_eq!(removed.id, 3);
        assert_eq!(mgr.list_screens().len(), 2);
        assert_eq!(mgr.total_panes(), 6);
    }

    #[test]
    fn test_cannot_remove_last_screen() {
        let dir = TempDir::new().unwrap();
        let mgr = ScreenManager::new(dir.path().to_path_buf());
        mgr.init_default("test");

        mgr.remove_screen("3").unwrap();
        mgr.remove_screen("2").unwrap();
        let err = mgr.remove_screen("1").unwrap_err();
        assert!(err.contains("last screen"));
    }

    #[test]
    fn test_screen_for_pane() {
        let dir = TempDir::new().unwrap();
        let mgr = ScreenManager::new(dir.path().to_path_buf());
        mgr.init_default("test");

        let screen = mgr.screen_for_pane(5).unwrap();
        assert_eq!(screen.id, 2);
        assert!(mgr.screen_for_pane(99).is_none());
    }

    #[test]
    fn test_layout_from_str() {
        assert_eq!(ScreenLayout::from_str("single"), ScreenLayout::Single);
        assert_eq!(ScreenLayout::from_str("grid"), ScreenLayout::Grid2x2);
        assert_eq!(ScreenLayout::from_str("2x2"), ScreenLayout::Grid2x2);
        assert_eq!(ScreenLayout::from_str("v"), ScreenLayout::EvenVertical);
        assert_eq!(
            ScreenLayout::from_str("unknown"),
            ScreenLayout::EvenHorizontal
        );
    }

    #[test]
    fn test_layout_pane_count() {
        assert_eq!(ScreenLayout::Single.pane_count(), 1);
        assert_eq!(ScreenLayout::Split2.pane_count(), 2);
        assert_eq!(ScreenLayout::EvenHorizontal.pane_count(), 3);
        assert_eq!(ScreenLayout::Grid2x2.pane_count(), 4);
    }

    #[test]
    fn test_summary() {
        let dir = TempDir::new().unwrap();
        let mgr = ScreenManager::new(dir.path().to_path_buf());
        mgr.init_default("test");

        let summary = mgr.summary();
        assert_eq!(summary["total_screens"], 3);
        assert_eq!(summary["total_panes"], 9);
        assert_eq!(summary["next_pane"], 10);
    }

    #[test]
    fn test_persistence() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();

        // Create and init
        {
            let mgr = ScreenManager::new(path.clone());
            mgr.init_default("test");
            mgr.add_screen(Some("Extra".to_string()), None, None)
                .unwrap();
        }

        // Reload
        {
            let mgr = ScreenManager::new(path);
            let screens = mgr.list_screens();
            assert_eq!(screens.len(), 4);
            assert_eq!(screens[3].name, "Extra");
        }
    }
}

//! PTY Pool — multi-PTY manager with input routing and per-pane terminal state.
//!
//! This is the F1 foundation for G10 (Kill tmux). It replaces both the old
//! `PtyManager` (mod.rs) and the unfinished manager.rs/session.rs with a single,
//! clean implementation that owns all PTY sessions.
//!
//! Design:
//! - Each pane gets a `portable_pty` PTY + `vt100::Parser` for full screen state
//! - Input routing: one "focused" pane receives keyboard input (broadcast optional)
//! - Output: background reader thread per pane feeds vt100 parser + scrollback
//! - Events flow out via `tokio::sync::mpsc` channel
//! - All pane state behind `Arc<Mutex<>>` for thread-safe access from TUI + web

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, PtySize};
use tokio::sync::mpsc;

/// Unique identifier for a pane (u8 allows 0-255, more than enough)
pub type PaneId = u8;

/// Events emitted by the pool
#[derive(Debug, Clone)]
pub enum PoolEvent {
    /// New output on a pane (raw bytes for streaming to dashboard)
    Output { pane: PaneId, data: Vec<u8> },
    /// A pane's process exited
    Exited { pane: PaneId },
    /// Focus changed
    FocusChanged { from: Option<PaneId>, to: PaneId },
}

/// Configuration for spawning a pane
#[derive(Debug, Clone)]
pub struct PaneConfig {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    pub rows: u16,
    pub cols: u16,
}

impl Default for PaneConfig {
    fn default() -> Self {
        Self {
            command: "zsh".into(),
            args: vec![],
            cwd: PathBuf::from("."),
            env: vec![],
            rows: 24,
            cols: 80,
        }
    }
}

/// Per-pane state: PTY handles + terminal parser + scrollback
struct Pane {
    /// vt100 parser — full terminal screen state (colors, cursor, content)
    parser: Arc<Mutex<vt100::Parser>>,
    /// Writer to send input to the PTY
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    /// Child process handle
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    /// Reader thread join handle
    reader_handle: Option<std::thread::JoinHandle<()>>,
    /// Current terminal size
    rows: u16,
    cols: u16,
    /// Config used to spawn
    config: PaneConfig,
}

/// The PTY Pool — owns all terminal sessions with input routing
pub struct PtyPool {
    panes: HashMap<PaneId, Pane>,
    /// Currently focused pane (receives keyboard input)
    focused: Option<PaneId>,
    /// When true, input goes to ALL panes (like tmux synchronize-panes)
    broadcast: bool,
    /// Event channel sender (clone per reader thread)
    event_tx: mpsc::Sender<PoolEvent>,
    /// Max scrollback lines per pane's vt100 parser
    scrollback_lines: u32,
}

impl PtyPool {
    /// Create a new pool. Returns the pool and a receiver for events.
    pub fn new(scrollback_lines: u32) -> (Self, mpsc::Receiver<PoolEvent>) {
        let (tx, rx) = mpsc::channel(512);
        (
            Self {
                panes: HashMap::new(),
                focused: None,
                broadcast: false,
                event_tx: tx,
                scrollback_lines,
            },
            rx,
        )
    }

    /// Spawn a new pane. Returns the pane ID.
    pub fn spawn(&mut self, id: PaneId, config: PaneConfig) -> Result<()> {
        // Kill existing pane if present
        if self.panes.contains_key(&id) {
            self.kill(id)?;
        }

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: config.rows,
                cols: config.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("Failed to open PTY")?;

        let mut cmd = CommandBuilder::new(&config.command);
        for arg in &config.args {
            cmd.arg(arg);
        }
        cmd.cwd(&config.cwd);
        for (key, val) in &config.env {
            cmd.env(key, val);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env_remove("CLAUDECODE"); // Prevent nested detection

        let child = pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn process")?;
        drop(pair.slave); // Not needed after spawn

        let writer = pair.master.take_writer()?;
        let mut reader = pair.master.try_clone_reader()?;

        // Keep master alive — dropping it closes the PTY
        // We leak it intentionally; the child/reader will detect EOF when killed
        let _master = pair.master;

        let parser = Arc::new(Mutex::new(vt100::Parser::new(
            config.rows,
            config.cols,
            self.scrollback_lines as usize,
        )));

        // Spawn reader thread
        let parser_clone = parser.clone();
        let event_tx = self.event_tx.clone();
        let pane_id = id;
        let reader_handle = std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = &buf[..n];
                        // Feed into vt100 parser
                        if let Ok(mut p) = parser_clone.lock() {
                            p.process(data);
                        }
                        // Emit output event
                        let _ = event_tx.blocking_send(PoolEvent::Output {
                            pane: pane_id,
                            data: data.to_vec(),
                        });
                    }
                    Err(_) => break,
                }
            }
            let _ = event_tx.blocking_send(PoolEvent::Exited { pane: pane_id });
        });

        let pane = Pane {
            parser,
            writer: Arc::new(Mutex::new(writer)),
            child: Arc::new(Mutex::new(child)),
            reader_handle: Some(reader_handle),
            rows: config.rows,
            cols: config.cols,
            config,
        };

        self.panes.insert(id, pane);

        // Auto-focus first pane
        if self.focused.is_none() {
            self.focused = Some(id);
        }

        Ok(())
    }

    /// Send raw bytes to the focused pane (keyboard input routing)
    pub fn send_input(&self, data: &[u8]) -> Result<()> {
        if self.broadcast {
            for pane in self.panes.values() {
                let mut w = pane.writer.lock().map_err(|e| anyhow::anyhow!("{}", e))?;
                w.write_all(data)?;
                w.flush()?;
            }
            Ok(())
        } else if let Some(id) = self.focused {
            self.send_to(id, data)
        } else {
            bail!("No focused pane")
        }
    }

    /// Send raw bytes to a specific pane
    pub fn send_to(&self, id: PaneId, data: &[u8]) -> Result<()> {
        let pane = self.panes.get(&id).context("Pane not found")?;
        let mut w = pane.writer.lock().map_err(|e| anyhow::anyhow!("{}", e))?;
        w.write_all(data)?;
        w.flush()?;
        Ok(())
    }

    /// Send a line of text (appends \r) to the focused pane
    pub fn send_line(&self, text: &str) -> Result<()> {
        let mut data = text.as_bytes().to_vec();
        data.push(b'\r');
        self.send_input(&data)
    }

    /// Send a line to a specific pane
    pub fn send_line_to(&self, id: PaneId, text: &str) -> Result<()> {
        let mut data = text.as_bytes().to_vec();
        data.push(b'\r');
        self.send_to(id, &data)
    }

    /// Set the focused pane
    pub fn focus(&mut self, id: PaneId) -> Result<()> {
        if !self.panes.contains_key(&id) {
            bail!("Pane {} does not exist", id);
        }
        let old = self.focused;
        self.focused = Some(id);
        let _ = self.event_tx.try_send(PoolEvent::FocusChanged {
            from: old,
            to: id,
        });
        Ok(())
    }

    /// Get the currently focused pane
    pub fn focused(&self) -> Option<PaneId> {
        self.focused
    }

    /// Toggle broadcast mode (input goes to all panes)
    pub fn set_broadcast(&mut self, enabled: bool) {
        self.broadcast = enabled;
    }

    /// Get the full terminal screen contents for a pane (what you'd see on screen)
    pub fn screen_contents(&self, id: PaneId) -> Option<String> {
        self.panes.get(&id).and_then(|p| {
            p.parser.lock().ok().map(|parser| {
                parser.screen().contents()
            })
        })
    }

    /// Get the terminal screen as rows of cells (for TUI rendering with colors)
    pub fn screen_rows(&self, id: PaneId) -> Option<Vec<String>> {
        self.panes.get(&id).and_then(|p| {
            p.parser.lock().ok().map(|parser| {
                let screen = parser.screen();
                (0..screen.size().0)
                    .map(|row| screen.rows_formatted(row, row + 1))
                    .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
                    .collect()
            })
        })
    }

    /// Get cursor position for a pane
    pub fn cursor_position(&self, id: PaneId) -> Option<(u16, u16)> {
        self.panes.get(&id).and_then(|p| {
            p.parser.lock().ok().map(|parser| parser.screen().cursor_position())
        })
    }

    /// Get the terminal size for a pane
    pub fn pane_size(&self, id: PaneId) -> Option<(u16, u16)> {
        self.panes.get(&id).map(|p| (p.rows, p.cols))
    }

    /// Resize a pane's terminal
    pub fn resize(&mut self, id: PaneId, rows: u16, cols: u16) -> Result<()> {
        let pane = self.panes.get_mut(&id).context("Pane not found")?;
        if let Ok(mut parser) = pane.parser.lock() {
            parser.set_size(rows, cols);
        }
        pane.rows = rows;
        pane.cols = cols;
        // Note: portable-pty resize requires the master handle, which we
        // currently don't retain. This will be addressed when we add the
        // layout engine (F2). For now, vt100 parser tracks the logical size.
        Ok(())
    }

    /// Kill a pane's process
    pub fn kill(&mut self, id: PaneId) -> Result<()> {
        if let Some(mut pane) = self.panes.remove(&id) {
            // Kill the child process
            if let Ok(mut child) = pane.child.lock() {
                let _ = child.kill();
            }
            // Join reader thread
            if let Some(handle) = pane.reader_handle.take() {
                let _ = handle.join();
            }
            // Update focus if we killed the focused pane
            if self.focused == Some(id) {
                self.focused = self.panes.keys().copied().min();
            }
        }
        Ok(())
    }

    /// Kill all panes (shutdown)
    pub fn kill_all(&mut self) {
        let ids: Vec<PaneId> = self.panes.keys().copied().collect();
        for id in ids {
            let _ = self.kill(id);
        }
    }

    /// Get all active pane IDs (sorted)
    pub fn pane_ids(&self) -> Vec<PaneId> {
        let mut ids: Vec<PaneId> = self.panes.keys().copied().collect();
        ids.sort();
        ids
    }

    /// Get pane count
    pub fn len(&self) -> usize {
        self.panes.len()
    }

    /// Check if pool is empty
    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }

    /// Check if a pane's process is still running
    pub fn is_running(&self, id: PaneId) -> bool {
        self.panes.get(&id).is_some_and(|p| {
            p.child
                .lock()
                .ok()
                .and_then(|mut c| c.try_wait().ok())
                .map_or(true, |status| status.is_none())
        })
    }

    /// Check if a specific pane exists
    pub fn has_pane(&self, id: PaneId) -> bool {
        self.panes.contains_key(&id)
    }
}

impl Drop for PtyPool {
    fn drop(&mut self) {
        self.kill_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn test_config(cmd: &str) -> PaneConfig {
        PaneConfig {
            command: cmd.into(),
            args: vec!["-c".into(), "echo hello && sleep 0.1".into()],
            cwd: PathBuf::from("/tmp"),
            env: vec![],
            rows: 24,
            cols: 80,
        }
    }

    #[test]
    fn test_pool_creation() {
        let (pool, _rx) = PtyPool::new(1000);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert!(pool.focused().is_none());
        assert!(pool.pane_ids().is_empty());
    }

    #[test]
    fn test_spawn_and_auto_focus() {
        let (mut pool, _rx) = PtyPool::new(1000);
        let config = test_config("/bin/sh");

        pool.spawn(1, config).expect("spawn should succeed");

        assert_eq!(pool.len(), 1);
        assert!(!pool.is_empty());
        assert_eq!(pool.focused(), Some(1));
        assert!(pool.has_pane(1));
        assert_eq!(pool.pane_ids(), vec![1]);
    }

    #[test]
    fn test_multiple_panes() {
        let (mut pool, _rx) = PtyPool::new(1000);

        pool.spawn(1, test_config("/bin/sh")).unwrap();
        pool.spawn(2, test_config("/bin/sh")).unwrap();
        pool.spawn(3, test_config("/bin/sh")).unwrap();

        assert_eq!(pool.len(), 3);
        // First pane stays focused
        assert_eq!(pool.focused(), Some(1));
        assert_eq!(pool.pane_ids(), vec![1, 2, 3]);
    }

    #[test]
    fn test_focus_switching() {
        let (mut pool, _rx) = PtyPool::new(1000);

        pool.spawn(1, test_config("/bin/sh")).unwrap();
        pool.spawn(2, test_config("/bin/sh")).unwrap();

        assert_eq!(pool.focused(), Some(1));

        pool.focus(2).unwrap();
        assert_eq!(pool.focused(), Some(2));

        // Focus non-existent pane should error
        assert!(pool.focus(99).is_err());
    }

    #[test]
    fn test_kill_focused_pane_moves_focus() {
        let (mut pool, _rx) = PtyPool::new(1000);

        pool.spawn(1, test_config("/bin/sh")).unwrap();
        pool.spawn(2, test_config("/bin/sh")).unwrap();
        pool.spawn(3, test_config("/bin/sh")).unwrap();

        pool.focus(2).unwrap();
        pool.kill(2).unwrap();

        // Focus should move to lowest remaining pane
        assert_eq!(pool.focused(), Some(1));
        assert_eq!(pool.len(), 2);
        assert!(!pool.has_pane(2));
    }

    #[test]
    fn test_kill_all() {
        let (mut pool, _rx) = PtyPool::new(1000);

        pool.spawn(1, test_config("/bin/sh")).unwrap();
        pool.spawn(2, test_config("/bin/sh")).unwrap();

        pool.kill_all();

        assert!(pool.is_empty());
        assert!(pool.focused().is_none());
    }

    #[test]
    fn test_send_input_to_focused() {
        let (mut pool, _rx) = PtyPool::new(1000);

        // Spawn a shell that stays alive
        let config = PaneConfig {
            command: "/bin/sh".into(),
            args: vec![],
            cwd: PathBuf::from("/tmp"),
            env: vec![],
            rows: 24,
            cols: 80,
        };
        pool.spawn(1, config).unwrap();

        // Should succeed — we have a focused pane
        assert!(pool.send_input(b"echo test\r").is_ok());
    }

    #[test]
    fn test_send_input_no_focus() {
        let (pool, _rx) = PtyPool::new(1000);
        // No panes, no focus — should error
        assert!(pool.send_input(b"hello").is_err());
    }

    #[test]
    fn test_send_to_specific_pane() {
        let (mut pool, _rx) = PtyPool::new(1000);

        let config = PaneConfig {
            command: "/bin/sh".into(),
            args: vec![],
            cwd: PathBuf::from("/tmp"),
            env: vec![],
            rows: 24,
            cols: 80,
        };
        pool.spawn(1, config.clone()).unwrap();
        pool.spawn(2, config).unwrap();

        // Send to pane 2 (not focused)
        assert!(pool.send_to(2, b"echo pane2\r").is_ok());
        // Non-existent pane
        assert!(pool.send_to(99, b"nope").is_err());
    }

    #[test]
    fn test_screen_contents() {
        let (mut pool, _rx) = PtyPool::new(1000);

        let config = PaneConfig {
            command: "/bin/sh".into(),
            args: vec!["-c".into(), "echo 'HELLO_PTY_POOL'".into()],
            cwd: PathBuf::from("/tmp"),
            env: vec![],
            rows: 24,
            cols: 80,
        };
        pool.spawn(1, config).unwrap();

        // Give the process time to produce output
        std::thread::sleep(Duration::from_millis(200));

        let contents = pool.screen_contents(1);
        assert!(contents.is_some());
        let text = contents.unwrap();
        assert!(
            text.contains("HELLO_PTY_POOL"),
            "Expected screen to contain 'HELLO_PTY_POOL', got: {:?}",
            text.lines().take(5).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_pane_size() {
        let (mut pool, _rx) = PtyPool::new(1000);

        let config = PaneConfig {
            rows: 30,
            cols: 100,
            ..test_config("/bin/sh")
        };
        pool.spawn(1, config).unwrap();

        assert_eq!(pool.pane_size(1), Some((30, 100)));
        assert_eq!(pool.pane_size(99), None);
    }

    #[test]
    fn test_resize() {
        let (mut pool, _rx) = PtyPool::new(1000);
        pool.spawn(1, test_config("/bin/sh")).unwrap();

        pool.resize(1, 40, 120).unwrap();
        assert_eq!(pool.pane_size(1), Some((40, 120)));

        // Resize non-existent pane should error
        assert!(pool.resize(99, 40, 120).is_err());
    }

    #[test]
    fn test_broadcast_mode() {
        let (mut pool, _rx) = PtyPool::new(1000);

        let config = PaneConfig {
            command: "/bin/sh".into(),
            args: vec![],
            cwd: PathBuf::from("/tmp"),
            env: vec![],
            rows: 24,
            cols: 80,
        };
        pool.spawn(1, config.clone()).unwrap();
        pool.spawn(2, config).unwrap();

        pool.set_broadcast(true);
        // Should send to both panes without error
        assert!(pool.send_input(b"echo broadcast\r").is_ok());
    }

    #[test]
    fn test_respawn_same_id() {
        let (mut pool, _rx) = PtyPool::new(1000);

        pool.spawn(1, test_config("/bin/sh")).unwrap();
        assert_eq!(pool.len(), 1);

        // Spawning same ID should kill old and replace
        pool.spawn(1, test_config("/bin/sh")).unwrap();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn test_cursor_position() {
        let (mut pool, _rx) = PtyPool::new(1000);
        pool.spawn(1, test_config("/bin/sh")).unwrap();

        // Cursor position should be available
        let pos = pool.cursor_position(1);
        assert!(pos.is_some());

        // Non-existent pane
        assert!(pool.cursor_position(99).is_none());
    }

    #[test]
    fn test_is_running() {
        let (mut pool, _rx) = PtyPool::new(1000);

        // Spawn a long-running process
        let config = PaneConfig {
            command: "/bin/sh".into(),
            args: vec!["-c".into(), "sleep 10".into()],
            cwd: PathBuf::from("/tmp"),
            env: vec![],
            rows: 24,
            cols: 80,
        };
        pool.spawn(1, config).unwrap();

        assert!(pool.is_running(1));
        assert!(!pool.is_running(99)); // non-existent

        // Spawn a short-lived process
        let short = PaneConfig {
            command: "/bin/sh".into(),
            args: vec!["-c".into(), "true".into()],
            cwd: PathBuf::from("/tmp"),
            env: vec![],
            rows: 24,
            cols: 80,
        };
        pool.spawn(2, short).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert!(!pool.is_running(2));
    }

    #[test]
    fn test_send_line() {
        let (mut pool, _rx) = PtyPool::new(1000);

        let config = PaneConfig {
            command: "/bin/sh".into(),
            args: vec![],
            cwd: PathBuf::from("/tmp"),
            env: vec![],
            rows: 24,
            cols: 80,
        };
        pool.spawn(1, config).unwrap();

        assert!(pool.send_line("echo hello").is_ok());
        assert!(pool.send_line_to(1, "echo world").is_ok());
        assert!(pool.send_line_to(99, "nope").is_err());
    }
}

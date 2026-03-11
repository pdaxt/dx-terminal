use portable_pty::{Child, CommandBuilder, MasterPty, PtySize};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

/// Represents a running agent in a PTY
pub struct AgentHandle {
    pub child: Box<dyn Child + Send + Sync>,
    #[allow(dead_code)] // Needed to keep PTY alive
    master: Box<dyn MasterPty + Send>,
    pub writer: Box<dyn Write + Send>,
    pub parser: Arc<Mutex<vt100::Parser>>,
    pub output_lines: Arc<Mutex<VecDeque<String>>>,
    pub reader_handle: Option<std::thread::JoinHandle<()>>,
    pub exit_status: Arc<Mutex<Option<i32>>>,
}

impl AgentHandle {
    /// Spawn a new agent in a PTY
    pub fn spawn(
        command: &str,
        args: &[&str],
        cwd: &str,
        env_vars: Vec<(String, String)>,
        rows: u16,
        cols: u16,
    ) -> anyhow::Result<Self> {
        let pty_system = portable_pty::native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(command);
        for arg in args {
            cmd.arg(*arg);
        }
        cmd.cwd(cwd);
        for (k, v) in &env_vars {
            cmd.env(k, v);
        }
        // Prevent Claude Code from detecting nested session
        cmd.env_remove("CLAUDECODE");

        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave); // Not needed after spawn

        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, 0)));
        let output_lines = Arc::new(Mutex::new(VecDeque::with_capacity(1000)));

        // Spawn blocking reader thread
        let parser_clone = parser.clone();
        let lines_clone = output_lines.clone();
        let reader_handle = std::thread::spawn(move || {
            Self::read_loop(reader, parser_clone, lines_clone);
        });

        Ok(Self {
            child,
            master: pair.master,
            writer,
            parser,
            output_lines,
            reader_handle: Some(reader_handle),
            exit_status: Arc::new(Mutex::new(None)),
        })
    }

    /// Background reader: feeds PTY output into vt100 parser and line buffer
    fn read_loop(
        mut reader: Box<dyn Read + Send>,
        parser: Arc<Mutex<vt100::Parser>>,
        lines: Arc<Mutex<VecDeque<String>>>,
    ) {
        let mut buf = [0u8; 4096];
        let mut line_buf = String::new();
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let bytes = &buf[..n];
                    // Feed into vt100 parser
                    if let Ok(mut p) = parser.lock() {
                        p.process(bytes);
                    }
                    // Also track raw lines for simple text access
                    let text = String::from_utf8_lossy(bytes);
                    line_buf.push_str(&text);
                    while let Some(pos) = line_buf.find('\n') {
                        let line = line_buf[..pos].trim_end_matches('\r').to_string();
                        if let Ok(mut l) = lines.lock() {
                            l.push_back(line);
                            while l.len() > 1000 {
                                l.pop_front();
                            }
                        }
                        line_buf = line_buf[pos + 1..].to_string();
                    }
                }
                Err(_) => break,
            }
        }
    }

    /// Send text followed by Enter key
    pub fn send_line(&mut self, text: &str) -> anyhow::Result<()> {
        write!(self.writer, "{}\r", text)?;
        self.writer.flush()?;
        Ok(())
    }

    /// Send Ctrl-C to the PTY
    pub fn send_ctrl_c(&mut self) -> anyhow::Result<()> {
        self.writer.write_all(&[0x03])?; // ETX = Ctrl-C
        self.writer.flush()?;
        Ok(())
    }

    /// Get the current terminal screen content from vt100
    pub fn screen_text(&self) -> String {
        if let Ok(p) = self.parser.lock() {
            p.screen().contents()
        } else {
            String::new()
        }
    }

    /// Get the last N lines from the output buffer
    pub fn last_lines(&self, n: usize) -> Vec<String> {
        if let Ok(lines) = self.output_lines.lock() {
            lines.iter().rev().take(n).rev().cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Get the last N lines as a single string
    pub fn last_output(&self, n: usize) -> String {
        self.last_lines(n).join("\n")
    }

    /// Check if the child process is still running.
    /// Sets exit_status to -1 on first detection of reader EOF (compare-and-set to avoid races).
    pub fn is_running(&self) -> bool {
        let reader_done = self
            .reader_handle
            .as_ref()
            .map_or(true, |h| h.is_finished());
        if reader_done {
            // Reader finished = process exited — set sentinel only if not already set
            if let Ok(mut status) = self.exit_status.lock() {
                if status.is_none() {
                    *status = Some(-1);
                }
            }
            return false;
        }
        true
    }

    /// Get the exit code if the process has exited
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_status.lock().ok().and_then(|s| *s)
    }

    /// Kill the child process and join the reader thread.
    pub fn kill(&mut self) -> anyhow::Result<()> {
        // Try graceful signals first (non-blocking)
        let _ = self.send_line("/exit");
        let _ = self.send_ctrl_c();

        // Force kill immediately
        self.child.kill()?;

        // Join reader thread so it doesn't outlive the killed process
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }

        Ok(())
    }

    /// Total line count captured
    pub fn line_count(&self) -> usize {
        self.output_lines.lock().ok().map(|l| l.len()).unwrap_or(0)
    }
}

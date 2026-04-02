use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use dx_agent_core::Result;

#[derive(Debug, Clone, Deserialize)]
pub struct BashInput {
    pub command: String,
    pub timeout: Option<u64>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BashOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
}

pub fn execute_bash(input: BashInput, cwd: &Path) -> Result<BashOutput> {
    let timeout_ms = input.timeout.unwrap_or(120_000);
    let timeout_dur = Duration::from_millis(timeout_ms);

    let mut child = Command::new("sh")
        .arg("-lc")
        .arg(&input.command)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let start = Instant::now();

    // Poll for completion with timeout
    loop {
        match child.try_wait()? {
            Some(status) => {
                let output = child.wait_with_output()?;
                return Ok(BashOutput {
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    exit_code: status.code(),
                    timed_out: false,
                });
            }
            None => {
                if start.elapsed() > timeout_dur {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(BashOutput {
                        stdout: String::new(),
                        stderr: format!("command timed out after {timeout_ms}ms"),
                        exit_code: None,
                        timed_out: true,
                    });
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}
